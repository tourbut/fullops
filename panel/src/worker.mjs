// fullops 패널의 worker. 패널(샌드박스 프레임)은 파일을 읽지 못하므로 여기서 status.json을 읽어 준다.
// 읽기만 한다. 어떤 파일도 쓰지 않고 프로세스도 띄우지 않는다.
import { readFileSync, statSync } from 'node:fs'
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
    const id = process.platform === 'win32' ? found.common.toLowerCase() : found.common
    let repo = repos.get(id)
    if (!repo) {
      const name = path.basename(found.common) === '.git' ? path.basename(path.dirname(found.common)) : path.basename(found.common)
      repo = { id, name, workspaces: [], active: false, ...readStatus(path.join(found.common, 'fullops', 'status.json')) }
      repos.set(id, repo)
    }
    repo.workspaces.push({ name: ws.name ?? null, branch: ws.branch ?? null })
    repo.active ||= stat(path.join(found.root, '.fullops-squad', 'fullops.toml'))?.isFile() === true
  }
  // 활성 표식도 status.json도 없는 레포는 fullops와 무관하다.
  const list = [...repos.values()].filter((repo) => repo.active || repo.state !== 'none')
  const focused = isObj(focus)
    ? list.find((repo) => repo.workspaces.some((ws) => ws.name === focus.displayName && ws.branch === focus.branch))
    : null
  return { repos: list, focus: focused?.id ?? null }
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
}
