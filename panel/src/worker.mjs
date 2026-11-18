// fullops 패널의 worker. 패널(샌드박스 프레임)은 파일을 읽지 못하므로 여기서 status.json을 읽어 준다.
// 읽기만 한다. 어떤 파일도 쓰지 않고 프로세스도 띄우지 않는다.
import { lstatSync, readdirSync, readFileSync, statSync } from 'node:fs'
import path from 'node:path'

const STATUS_V = 1

const isObj = (x) => x !== null && typeof x === 'object' && !Array.isArray(x)
const rows = (x) => (Array.isArray(x) ? x.filter(isObj) : [])

function stat(file) {
  try {
    return statSync(file)
  } catch {
    return null
  }
}

function readText(file) {
  try {
    return readFileSync(file, 'utf8')
  } catch {
    return null
  }
}

/** 폴더에서 위로 올라가며 작업 트리 루트와 Git 공용 디렉터리를 찾는다. 레포가 아니면 null. */
export function findRepo(start) {
  let dir = path.resolve(start)
  for (;;) {
    const dotGit = path.join(dir, '.git')
    const st = stat(dotGit)
    if (st?.isDirectory()) return { root: dir, common: dotGit }
    if (st?.isFile()) {
      // git worktree: .git 파일의 gitdir가 가리키는 폴더의 commondir가 공용 디렉터리다.
      const found = /^gitdir:\s*(.+)$/m.exec(readText(dotGit) ?? '')
      if (!found) return null
      const gitDir = path.resolve(dir, found[1].trim())
      const rel = readText(path.join(gitDir, 'commondir'))?.trim()
      return { root: dir, common: rel ? path.resolve(gitDir, rel) : gitDir }
    }
    const up = path.dirname(dir)
    if (up === dir) return null
    dir = up
  }
}

/** status.json 한 파일을 읽는다. state는 ok(읽었다), none(파일이 없다), broken(읽지 못했다), newer(모르는 새 형식)다. */
export function readStatus(file) {
  let text
  try {
    text = readFileSync(file, 'utf8')
  } catch (error) {
    return error?.code === 'ENOENT' ? { state: 'none' } : { state: 'broken', detail: String(error?.code ?? error) }
  }
  let raw
  try {
    raw = JSON.parse(text)
  } catch (error) {
    return { state: 'broken', detail: error.message }
  }
  if (!isObj(raw) || typeof raw.v !== 'number') return { state: 'broken', detail: 'v가 없다' }
  if (raw.v > STATUS_V) return { state: 'newer', detail: `v${raw.v}` }
  return { state: 'ok', status: normalize(raw) }
}

/** 패널이 그리는 필드만 고르고, 목록·객체 자리에 다른 것이 와도 패널이 죽지 않게 모양을 맞춘다. */
export function normalize(raw) {
  return {
    generated_at: typeof raw.generated_at === 'number' ? raw.generated_at : null,
    mode: typeof raw.mode === 'string' ? raw.mode : null,
    version: typeof raw.version === 'string' ? raw.version : null,
    tasks: rows(raw.tasks),
    dispatches: rows(raw.dispatches),
    pending: rows(raw.pending),
    decisions: rows(raw.decisions),
    ready: Array.isArray(raw.ready) ? raw.ready.filter((key) => typeof key === 'string') : [],
    sync_pending: isObj(raw.sync_pending) ? raw.sync_pending : {},
    metrics: isObj(raw.metrics) ? raw.metrics : null
  }
}

/**
 * Orca 워크스페이스 목록에서 fullops를 쓰는 레포를 모은다. 한 레포의 워크트리들은 공용 디렉터리가 같아 한 건이 된다.
 * focus는 초점이 있는 워크스페이스(workspace.readContext의 답)이고, 그 레포의 id를 focus로 돌려준다.
 */
export function snapshot(workspaces, focus) {
  const repos = new Map()
  for (const ws of Array.isArray(workspaces) ? workspaces : []) {
    const found = typeof ws?.path === 'string' ? findRepo(ws.path) : null
    if (!found) continue
    // 설계의 same_path와 같다: Windows는 대소문자를 가리지 않고 심볼릭 링크는 풀지 않는다.
    const id = repoId(found.common)
    let repo = repos.get(id)
    if (!repo) {
      const name = path.basename(found.common) === '.git' ? path.basename(path.dirname(found.common)) : path.basename(found.common)
      repo = { id, name, workspaces: [], active: false, ...readStatus(path.join(found.common, 'fullops', 'status.json')) }
      repos.set(id, repo)
    }
    repo.workspaces.push({ name: ws.name ?? null, branch: ws.branch ?? null })
    repo.active ||= stat(path.join(found.root, '.fullops', 'fullops.toml'))?.isFile() === true
  }
  // 활성 표식도 status.json도 없는 레포는 fullops와 무관하다.
  const list = [...repos.values()].filter((repo) => repo.active || repo.state !== 'none')
  const focused = isObj(focus)
    ? list.find((repo) => repo.workspaces.some((ws) => ws.name === focus.displayName && ws.branch === focus.branch))
    : null
  return { repos: list, focus: focused?.id ?? null }
}

// 과제 키(TaskKey)와 역할 ID(RoleId)의 문법. 패널이 준 값으로 경로를 만들므로 이 꼴만 받는다(`..`·구분자가 못 들어온다).
const KEY = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/
const ROLE = /^[a-z][a-z0-9_-]{0,63}$/
// 응답은 1MiB까지다. 파일마다 앞부분만 보낸다.
const MAX_TEXT = 64 * 1024

const repoId = (common) => (process.platform === 'win32' ? common.toLowerCase() : common)

/** 일반 파일이면 앞 MAX_TEXT자와 바뀐 시각. 없거나 폴더거나 링크면 null. */
function readCapped(file) {
  const st = (() => {
    try {
      return lstatSync(file)
    } catch {
      return null
    }
  })()
  if (!st?.isFile()) return null
  const text = readText(file)
  if (text == null) return null
  return { text: text.slice(0, MAX_TEXT), truncated: text.length > MAX_TEXT, mtime: st.mtimeMs }
}

function list(dir) {
  try {
    return readdirSync(dir)
  } catch {
    return []
  }
}

/** 인박스 front matter의 key. */
function frontKey(text) {
  const front = /^---\r?\n([\s\S]*?)\r?\n---/.exec(text)
  return front ? /^key:\s*["']?([^"'\r\n]+?)["']?\s*$/m.exec(front[1])?.[1] ?? null : null
}

/**
 * 과제 하나의 글을 모은다(읽기만 한다). 그 레포의 체크아웃마다(열린 워크스페이스와 원본 체크아웃) 본다.
 * 지시서는 그 키의 인박스 가운데 가장 늦게 바뀐 것(일하는 worker의 워크트리가 먼저다), 끝난 차수의 기록은 `tasks/<KEY>/<역할>.r<n>.md`다.
 */
export function readTask(workspaces, input) {
  const key = input?.key
  const role = input?.role ?? null
  if (typeof key !== 'string' || !KEY.test(key)) return { state: 'bad', detail: '과제 키가 아니다' }
  if (role !== null && (typeof role !== 'string' || !ROLE.test(role))) return { state: 'bad', detail: '역할 ID가 아니다' }
  const roots = new Map()
  for (const ws of Array.isArray(workspaces) ? workspaces : []) {
    const found = typeof ws?.path === 'string' ? findRepo(ws.path) : null
    if (!found || repoId(found.common) !== input?.repo) continue
    roots.set(found.root, ws.name ?? path.basename(found.root))
    // 원본 체크아웃은 Orca에 열려 있지 않아도 본다.
    const main = path.basename(found.common) === '.git' ? path.dirname(found.common) : null
    if (main && !roots.has(main)) roots.set(main, path.basename(main))
  }
  if (roots.size === 0) return { state: 'none', detail: '그 레포의 워크스페이스가 열려 있지 않다' }

  let inbox = null
  const records = new Map()
  let request = null
  let route = null
  for (const [root, from] of roots) {
    const dir = path.join(root, '.fullops')
    const names = role ? [`to_${role}.md`] : list(path.join(dir, 'inbox')).filter((n) => /^to_[a-z][a-z0-9_-]*\.md$/.test(n))
    for (const name of names) {
      const file = readCapped(path.join(dir, 'inbox', name))
      if (file && frontKey(file.text) === key && (!inbox || file.mtime > inbox.mtime)) inbox = { ...file, name, from }
    }
    const task = path.join(dir, 'tasks', key)
    for (const name of list(task)) {
      const round = /^([a-z][a-z0-9_-]*)\.r(\d+)\.md$/.exec(name)
      if (!round || (role && round[1] !== role) || records.has(name)) continue
      const file = readCapped(path.join(task, name))
      if (file) records.set(name, { ...file, name, role: round[1], round: Number(round[2]), from })
    }
    if (!request) {
      const file = readCapped(path.join(task, 'request.md'))
      if (file) request = { ...file, from }
    }
    if (!route) {
      try {
        const raw = JSON.parse(readCapped(path.join(task, 'route.json'))?.text ?? 'null')
        if (isObj(raw)) {
          const text = (x) => (typeof x === 'string' ? x : null)
          route = { class: text(raw.class), role: text(raw.role), reason: text(raw.reason), decided_by: text(raw.decided_by) }
        }
      } catch {
        // 깨진 분류 기록은 보여 주지 않는다.
      }
    }
  }
  const done = [...records.values()].sort((a, b) => b.round - a.round || a.name.localeCompare(b.name))
  const strip = (file) => file && { text: file.text, truncated: file.truncated, from: file.from, ...(file.name && { name: file.name }) }
  return {
    state: 'ok',
    key,
    role,
    route,
    inbox: strip(inbox),
    // 가장 늦은 차수의 글만 보내고 나머지는 이름만 보낸다.
    record: strip(done[0] ?? null),
    rounds: done.map((r) => r.name),
    request: strip(request)
  }
}

export default function activate(orca) {
  orca.panels.handle('status.read', async () => {
    const [listed, focus] = await Promise.all([
      orca.host.call('workspace.list', {}),
      // 초점을 모르면 패널이 첫 레포를 보여 준다.
      orca.host.call('workspace.readContext', {}).catch(() => null)
    ])
    return snapshot(listed?.workspaces, focus)
  })
  orca.panels.handle('task.read', async (input) => {
    const listed = await orca.host.call('workspace.list', {})
    return readTask(listed?.workspaces, input)
  })
}
