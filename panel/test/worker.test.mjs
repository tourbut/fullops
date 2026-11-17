// worker가 status.json을 읽는 경우들. `node --test`로 돌린다(의존 없음).
import assert from 'node:assert/strict'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'

import activate, { readStatus, snapshot } from '../src/worker.mjs'

const FIXTURE = readFileSync(new URL('./fixtures/status.design.json', import.meta.url), 'utf8')
const DESIGN = new URL('../../docs/design.md', import.meta.url)

/** 임시 레포. status는 status.json에 쓸 글이고, 주지 않으면 파일을 만들지 않는다. */
function repo(t, { status, active = true } = {}) {
  const root = mkdtempSync(path.join(tmpdir(), 'fullops-panel-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  mkdirSync(path.join(root, '.git', 'fullops'), { recursive: true })
  if (active) {
    mkdirSync(path.join(root, '.fullops'))
    writeFileSync(path.join(root, '.fullops', 'fullops.toml'), 'schema = 2\n')
  }
  if (status !== undefined) writeFileSync(path.join(root, '.git', 'fullops', 'status.json'), status)
  return root
}

/** main 레포의 git worktree 하나(.git이 파일이다). */
function worktree(t, main, name) {
  const root = mkdtempSync(path.join(tmpdir(), 'fullops-panel-wt-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const gitDir = path.join(main, '.git', 'worktrees', name)
  mkdirSync(gitDir, { recursive: true })
  writeFileSync(path.join(gitDir, 'commondir'), '../..\n')
  writeFileSync(path.join(root, '.git'), `gitdir: ${gitDir}\n`)
  return root
}

const workspace = (dir, name = 'main', branch = 'main') => ({ id: name, kind: 'worktree', name, path: dir, branch, hostId: null })

test('설계 6.2.3절의 예시 status.json을 읽는다', (t) => {
  const root = repo(t, { status: FIXTURE })
  const answer = snapshot([workspace(root)], null)

  assert.equal(answer.repos.length, 1)
  const [only] = answer.repos
  assert.equal(only.state, 'ok')
  assert.equal(only.name, path.basename(root))
  assert.equal(only.status.mode, 'coor')
  assert.equal(only.status.version, '2.0.0')
  assert.equal(only.status.tasks[0].stage, 'reported')
  assert.equal(only.status.dispatches[0].settled, true)
  // pending[]의 id가 건을 가리키는 키다.
  assert.deepEqual(only.status.pending, [{ id: 'msg_3', key: 'PAY-12', sha: '4f0c1a2', landing: 'merge_left', hold_reason: null }])
  assert.equal(only.status.decisions[0].id, 'D001')
  assert.deepEqual(only.status.ready, ['PAY-13'])
  assert.deepEqual(only.status.sync_pending, { art: '9c1e0aa' })
  assert.deepEqual(
    [only.status.metrics.subagent_calls, only.status.metrics.workflow_runs, only.status.metrics.sub_dispatches],
    [31, 2, 3]
  )
  // 패널에 가는 답은 JSON이다.
  assert.deepEqual(JSON.parse(JSON.stringify(answer)), answer)
})

test('표본은 설계 문서의 예시와 같다', { skip: !existsSync(DESIGN) && '레포 밖(docs/design.md 없음)' }, () => {
  const text = readFileSync(DESIGN, 'utf8')
  const block = /```json\r?\n([\s\S]*?)\r?\n```/.exec(text.slice(text.indexOf('#### 6.2.3')))
  assert.deepEqual(JSON.parse(FIXTURE), JSON.parse(block[1]))
})

test('워크트리들은 레포 한 건으로 모이고 초점이 있는 레포를 찾는다', (t) => {
  const main = repo(t, { status: FIXTURE })
  const other = repo(t, { status: FIXTURE })
  const dev = worktree(t, main, 'dev')
  mkdirSync(path.join(dev, 'src', 'deep'), { recursive: true })

  const answer = snapshot(
    [workspace(other, 'other', 'main'), workspace(main, 'app', 'main'), workspace(path.join(dev, 'src', 'deep'), 'app-dev', 'fullops/dev')],
    { displayName: 'app-dev', branch: 'fullops/dev', terminals: [] }
  )

  assert.equal(answer.repos.length, 2)
  const app = answer.repos[1]
  assert.deepEqual(app.workspaces, [
    { name: 'app', branch: 'main' },
    { name: 'app-dev', branch: 'fullops/dev' }
  ])
  assert.equal(app.name, path.basename(main))
  assert.equal(answer.focus, app.id)
  assert.equal(snapshot([workspace(main)], { displayName: '없는 이름', branch: 'main' }).focus, null)
})

test('status.json이 없으면 활성 레포는 none이고, 활성 표식도 없는 레포는 뺀다', (t) => {
  const active = repo(t)
  const unrelated = repo(t, { active: false })
  const answer = snapshot([workspace(active, 'a'), workspace(unrelated, 'b')], null)

  assert.equal(answer.repos.length, 1)
  assert.equal(answer.repos[0].state, 'none')
  assert.equal(answer.repos[0].status, undefined)
})

test('깨진 status.json은 broken, 모르는 새 형식은 newer다', (t) => {
  const root = repo(t)
  const file = path.join(root, '.git', 'fullops', 'status.json')
  for (const [text, state] of [
    [FIXTURE.slice(0, 200), 'broken'],
    ['', 'broken'],
    ['[]', 'broken'],
    ['null', 'broken'],
    ['{"generated_at":1}', 'broken'],
    ['{"v":"1"}', 'broken'],
    ['{"v":2,"tasks":[]}', 'newer']
  ]) {
    writeFileSync(file, text)
    assert.equal(readStatus(file).state, state, text.slice(0, 20))
    assert.equal(snapshot([workspace(root)], null).repos[0].state, state)
  }
  // 파일 자리에 폴더가 있어도 죽지 않는다.
  assert.equal(readStatus(path.join(root, '.git', 'fullops')).state, 'broken')
})

test('목록·객체 자리에 다른 값이 와도 모양을 맞춘다', (t) => {
  const root = repo(t, {
    status: JSON.stringify({
      v: 1,
      generated_at: 'x',
      mode: null,
      tasks: 'x',
      dispatches: null,
      pending: [null, 3, { id: 'msg_1' }],
      decisions: {},
      ready: [1, 'A-1', null],
      sync_pending: [],
      metrics: 5,
      token: '패널에 넘기지 않는 필드'
    })
  })
  assert.deepEqual(snapshot([workspace(root)], null).repos[0].status, {
    generated_at: null,
    mode: null,
    version: null,
    tasks: [],
    dispatches: [],
    pending: [{ id: 'msg_1' }],
    decisions: [],
    ready: ['A-1'],
    sync_pending: {},
    metrics: null
  })
})

test('레포가 아니거나 읽을 수 없는 워크스페이스는 건너뛴다', (t) => {
  const plain = mkdtempSync(path.join(tmpdir(), 'fullops-panel-plain-'))
  t.after(() => rmSync(plain, { recursive: true, force: true }))
  const broken = mkdtempSync(path.join(tmpdir(), 'fullops-panel-plain-'))
  t.after(() => rmSync(broken, { recursive: true, force: true }))
  writeFileSync(path.join(broken, '.git'), 'gitdir가 아닌 글\n')

  const list = [workspace(plain), workspace(broken), workspace(path.join(plain, 'missing')), { path: 42 }, null, 'x']
  assert.deepEqual(snapshot(list, null), { repos: [], focus: null })
  assert.deepEqual(snapshot(undefined, undefined), { repos: [], focus: null })
})

test('activate가 status.read를 등록하고, 초점을 못 읽어도 답한다', async (t) => {
  const root = repo(t, { status: FIXTURE })
  const handlers = {}
  activate({
    panels: { handle: (name, handler) => void (handlers[name] = handler) },
    host: {
      call: async (method) => {
        if (method === 'workspace.list') return { workspaces: [workspace(root)], truncated: false }
        throw Object.assign(new Error('없다'), { code: 'unavailable' })
      }
    }
  })

  assert.deepEqual(Object.keys(handlers), ['status.read'])
  const answer = await handlers['status.read']()
  assert.equal(answer.repos[0].state, 'ok')
  assert.equal(answer.focus, null)
})
