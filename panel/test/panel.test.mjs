// 매니페스트가 Orca 계약(docs/contracts/orca.md 5절)과 맞는지, 패널 화면이 표본과 깨진 입력을 그리는지 본다.
// 화면은 브라우저 없이 본다: index.html의 인라인 스크립트를 가짜 document로 돌려 그려진 HTML 글을 확인한다.
import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import test from 'node:test'
import vm from 'node:vm'

const file = (name) => new URL(`../${name}`, import.meta.url)
const manifest = JSON.parse(readFileSync(file('orca-plugin.json'), 'utf8'))
const page = readFileSync(file('src/index.html'), 'utf8')
const preview = readFileSync(file('src/preview.js'), 'utf8')

test('매니페스트는 worker가 딸린 패널의 계약을 지킨다', async () => {
  const known = ['manifestVersion', 'execution', 'id', 'name', 'version', 'description', 'engines', 'pluginApi', 'main', 'contributes', 'capabilities']
  assert.deepEqual(Object.keys(manifest).filter((key) => !known.includes(key)), [], 'Orca는 모르는 키를 거절한다')
  assert.equal(manifest.manifestVersion, 2)
  // 패널만으로는 파일을 읽지 못한다. Node worker(main)가 있어야 하고 그러려면 trusted-worker다.
  assert.equal(manifest.execution, 'trusted-worker')
  assert.match(manifest.id, /^(?!orca-)[a-z0-9]+(?:-[a-z0-9]+)*$/)
  assert.equal(manifest.publisher, undefined)
  assert.match(manifest.version, /^\d+\.\d+\.\d+$/)
  assert.match(manifest.engines.orca, /^>=1\.4\.(24[6-9]|2[5-9]\d|[3-9]\d\d)$/, 'publisher를 뺀 형식 2는 1.4.246 이상이다')
  assert.equal(manifest.pluginApi, 1)

  assert.match(manifest.main, /^[^.][^\\]*\.mjs$/)
  assert.equal(typeof (await import(file(manifest.main).href)).default, 'function', 'main은 activate를 기본 내보내기로 낸다')

  assert.deepEqual(Object.keys(manifest.contributes), ['panels'])
  const [panel] = manifest.contributes.panels
  assert.deepEqual(Object.keys(panel).sort(), ['entry', 'icon', 'id', 'title'])
  assert.match(panel.entry, /\.html$/)
  assert.ok(existsSync(file(panel.entry)))
  assert.ok(['activity', 'bar-chart-3', 'gauge', 'layers', 'flag'].includes(panel.icon))

  // worker가 부르는 workspace.list·workspace.readContext의 권한이다. 그 밖의 권한은 청하지 않는다.
  assert.deepEqual(manifest.capabilities, [{ kind: 'workspace:read' }])
})

test('패널은 파일 하나이고 바깥 자원을 부르지 않는다', () => {
  // 브라우저 클라이언트의 프레임은 인라인 스크립트·스타일만 허용한다. 하나뿐인 src는 Orca 밖에서만 타는 미리보기다.
  assert.deepEqual(page.match(/\bsrc\s*[:=]\s*['"][^'"]*['"]/g), ["src: 'preview.js'"])
  assert.doesNotMatch(page, /<link\b|@import|url\(|https?:\/\/|\bfetch\(|XMLHttpRequest|WebSocket/)
  assert.equal(page.match(/<script\b/g).length, 1)
})

/** index.html의 스크립트를 가짜 document에서 돌린다. app.innerHTML이 그려진 화면이다. */
function mount(search = '') {
  const app = { innerHTML: '', addEventListener() {} }
  const node = () => ({ classList: { add() {} }, style: { setProperty() {} }, append() {} })
  const context = vm.createContext({
    document: { getElementById: () => app, documentElement: node(), head: node(), createElement: () => ({}), hidden: false, activeElement: null },
    location: { search },
    URLSearchParams,
    setInterval() {}
  })
  context.window = context
  vm.runInContext(/<script>([\s\S]*?)<\/script>/.exec(page)[1], context)
  return { app, context }
}

async function render(answer) {
  const { app, context } = mount()
  context.start({ on() {}, plugin: { request: async () => (answer instanceof Error ? Promise.reject(answer) : answer) } })
  await context.refresh()
  return app.innerHTML
}

const okRepo = (status) => ({
  repos: [
    {
      id: 'r1',
      name: 'app',
      workspaces: [],
      active: true,
      state: 'ok',
      status: { generated_at: 1, mode: 'coor', version: '2.0.0', tasks: [], dispatches: [], pending: [], decisions: [], ready: [], sync_pending: {}, metrics: null, ...status }
    }
  ],
  focus: 'r1'
})

test('미리보기 표본이 그려진다: 대기 건, 준비된 과제, 병렬 지표', async () => {
  const { app, context } = mount('?theme=dark')
  vm.runInContext(preview, context)
  await context.refresh()
  const html = app.innerHTML

  for (const text of [
    'data-id="msg_3"', '병합 남음', '푸시 남음', '보류 사유: 운영 배포 승인을 기다린다', '키 없음', '미계산',
    '준비된 과제', 'PAY-13', 'PAY-15',
    '병렬 진행', '서브에이전트 호출', '<dd>31</dd>', '워크플로 실행', '<dd>2</dd>', '하위 배정', '<dd>3</dd>',
    'D001', '진행', '생략', '리뷰 중', '재작업 1/2', '9c1e0aa', 'coor 모드', 'docs-site'
  ]) {
    assert.ok(html.includes(text), `화면에 "${text}"가 없다`)
  }
  // 정산된 배정은 '진행 중인 배정'에 넣지 않는다(표본 4건 가운데 3건).
  assert.match(html, /진행 중인 배정 <span>3<\/span>/)
})

test('파일에서 온 글은 모두 이스케이프한다', async () => {
  const bad = '<img src=x onerror="alert(1)">'
  const html = await render({
    repos: [
      {
        ...okRepo({
          tasks: [{ key: bad, role: bad, stage: bad, sha: bad }],
          dispatches: [{ key: bad, role: bad, purpose: bad, agent: bad, model: bad }],
          pending: [{ id: `"${bad}`, key: bad, sha: bad, landing: bad, hold_reason: bad }],
          decisions: [{ id: bad, key: bad, question: bad, options: [bad] }],
          ready: [bad],
          sync_pending: { [bad]: bad },
          version: bad,
          mode: bad
        }).repos[0],
        name: bad
      },
      { id: `"${bad}`, name: bad, state: 'broken', detail: bad }
    ],
    focus: 'r1'
  })
  assert.ok(html.includes('&lt;img src=x onerror=&quot;alert(1)&quot;&gt;'))
  assert.doesNotMatch(html, /<img|onerror="/)
})

test('없는 파일, 깨진 파일, 새 형식, 레포 없음, worker 오류를 글로 알린다', async () => {
  const one = (fields) => ({ repos: [{ id: 'r1', name: 'app', ...fields }], focus: null })
  assert.match(await render(one({ state: 'none' })), /아직 기록된 상태가 없다/)
  assert.match(await render(one({ state: 'broken', detail: 'Unexpected end of JSON input' })), /읽지 못했다[\s\S]*Unexpected end of JSON input/)
  assert.match(await render(one({ state: 'newer', detail: 'v2' })), /새 형식이다[\s\S]*v2/)
  assert.match(await render(one({ state: 'ok' })), /읽지 못했다/)
  assert.match(await render({ repos: [], focus: null }), /fullops를 쓰는 레포가 없다/)
  assert.match(await render(Object.assign(new Error('worker가 없다'), { code: 'unavailable' })), /worker에게 묻지 못했다[\s\S]*unavailable worker가 없다/)
})

test('빈 상태와 계산하지 않은 지표, 모르는 어휘에서도 죽지 않는다', async () => {
  const quiet = await render(okRepo({ metrics: { at: 0 } }))
  assert.match(quiet, /열린 과제가 없다/)
  assert.match(quiet, /아직 계산하지 않았다/)

  const odd = await render(
    okRepo({
      tasks: [{ key: 'A-1', stage: 'constructor' }],
      pending: [{ id: 'msg_1', landing: '__proto__' }],
      dispatches: [{ key: 'A-1', purpose: 'toString', started_at: 'x' }],
      decisions: [{ id: 'D002', options: 'x', expires_at: 1 }],
      metrics: { at: 5, turns_per_task: null }
    })
  )
  for (const text of ['constructor', 'data-id="msg_1"', '미계산', 'toString', '시각 모름', '만료됨', '<dd>-</dd>']) {
    assert.ok(odd.includes(text), `화면에 "${text}"가 없다`)
  }
})

test('결정의 1번 선택지는 추천 배지로, 과제 키는 버튼으로 그린다', async () => {
  const html = await render(
    okRepo({
      decisions: [
        { id: 'D001', key: 'A-1', question: '재시도 횟수는?', options: ['3번 (추천)', '5번 (추천)'], asked_at: 1 },
        { id: 'D002', key: null, question: '리뷰를 진행할까?', options: ['Go (Recommended)', '생략'], asked_at: 1 },
        { id: 'D003', question: '기계가 연 결정', options: ['진행', '생략'], asked_at: 1 }
      ]
    })
  )
  // 표시는 배지로 옮기고, 1번이 아닌 선택지의 글은 그대로 둔다
  assert.ok(html.includes('<span class="chip">3번 <span class="badge ok">추천</span></span>'), html)
  assert.ok(html.includes('<span class="chip">5번 (추천)</span>'))
  assert.ok(html.includes('<span class="chip">Go <span class="badge ok">추천</span></span>'))
  // 표시가 없는 1번 선택지에는 배지를 붙이지 않는다
  assert.ok(html.includes('<span class="chip">진행</span>'))
  assert.equal(html.split('class="badge ok">추천').length - 1, 2)
  assert.match(html, /<span class="key">D001<\/span><span class="grow"><button type="button" class="open key " data-key="A-1" data-role=""/)
  assert.equal(html.split('data-key=').length - 1, 1)
})

test('과제 키를 누르면 그 과제의 글을 열고 목록으로 돌아온다', async () => {
  const { app, context } = mount()
  const asked = []
  const bad = '<script>alert(1)</script>'
  const task = {
    state: 'ok',
    key: 'A-1',
    role: 'dev',
    route: { class: 'direct', role: 'dev', reason: null, decided_by: 'manual' },
    inbox: { text: `# 지시서\n- [x] 진행 ${bad}`, truncated: false, from: 'app-dev', name: 'to_dev.md' },
    record: { text: '첫 차수 보고', truncated: true, from: 'app', name: 'dev.r0.md' },
    rounds: ['dev.r1.md', 'dev.r0.md'],
    request: null
  }
  const status = okRepo({ dispatches: [{ key: 'A-1', role: 'dev', purpose: 'build', agent: 'claude', model: 'm', started_at: 1, settled: false }] })
  context.start({
    on() {},
    plugin: {
      request: async (name, input) => {
        asked.push([name, input])
        return name === 'task.read' ? task : status
      }
    }
  })
  await context.refresh()
  assert.match(app.innerHTML, /<button type="button" class="open key " data-key="A-1" data-role="dev"/)

  await context.openTask('A-1', 'dev')
  assert.deepEqual(JSON.parse(JSON.stringify(asked.at(-1))), ['task.read', { repo: 'r1', key: 'A-1', role: 'dev' }])
  const html = app.innerHTML
  for (const text of ['← 목록', '바로 배정 → dev', '지시서(진행 중)', 'to_dev.md · app-dev', '진행 &lt;script&gt;', '앞부분만 보여 준다', '차수 2개 가운데']) {
    assert.ok(html.includes(text), `화면에 "${text}"가 없다`)
  }
  assert.doesNotMatch(html, /<script>alert/)
  // 폴링은 연 과제도 다시 읽는다
  await context.refresh()
  assert.equal(asked.filter(([name]) => name === 'task.read').length, 2)

  // 못 읽은 과제는 이유를 보인다
  task.state = 'bad'
  task.detail = '과제 키가 아니다'
  await context.refresh()
  assert.match(app.innerHTML, /과제 키가 아니다/)
})
