// 미리보기 전용. index.html을 Orca 밖에서 파일로 열었을 때만 불려, 표본 데이터로 패널을 그린다.
// 주소의 ?theme=dark 와 ?case=none|broken|newer|empty|quiet 로 테마와 상태를 고른다.
;(() => {
  const query = new URLSearchParams(location.search)
  const dark = query.get('theme') === 'dark'
  const root = document.documentElement
  root.classList.add(dark ? 'dark' : 'light')
  // Orca가 프레임에 주는 --orca-* 변수의 대역이다(값은 어림한 것이다).
  const tokens = dark
    ? { background: '#161616', foreground: '#f2f2f2', card: '#1e1e1e', 'muted-foreground': '#a1a1a1', border: '#303030', input: '#3a3a3a', accent: '#2a2a2a', ring: '#8a8a8a' }
    : { background: '#ffffff', foreground: '#111111', card: '#fafafa', 'muted-foreground': '#6b6b6b', border: '#e2e2e2', input: '#d0d0d0', accent: '#f0f0f0', ring: '#8a8a8a' }
  for (const [name, value] of Object.entries(tokens)) root.style.setProperty(`--orca-${name}`, value)
  root.style.setProperty('--orca-radius', '6px')

  const now = Math.floor(Date.now() / 1000)
  const status = {
    generated_at: now - 40,
    mode: 'coor',
    version: '2.0.0',
    tasks: [
      { key: 'PAY-12', role: 'dev', stage: 'reported', rounds: 0, budget: 2, sha: '4f0c1a2', held: false },
      { key: 'PAY-14', role: 'dev', stage: 'assigned', rounds: 1, budget: 2, sha: 'a81d3f0', held: false },
      { key: 'UI-7', role: 'art', stage: 'reviewing', rounds: 0, budget: 2, sha: '0be77c4', held: false },
      { key: 'OPS-3', role: 'dev', stage: 'reported', rounds: 0, budget: 2, sha: 'c2d9e51', held: true },
      { key: 'PAY-9', role: 'dev', stage: 'merged', rounds: 0, budget: 2, sha: '77aa01b', held: false }
    ],
    dispatches: [
      { key: 'PAY-12', role: 'dev', purpose: 'build', agent: 'claude', model: 'sonnet', started_at: now - 5400, settled: true, sub: false },
      { key: 'PAY-14', role: 'dev', purpose: 'fix', agent: 'claude', model: 'sonnet', started_at: now - 780, settled: false, sub: false },
      { key: 'PAY-14', role: 'dev', purpose: 'build', agent: 'codex', model: 'gpt-codex', started_at: now - 300, settled: false, sub: true },
      { key: 'UI-7', role: 'reviewer', purpose: 'review', agent: 'codex', model: 'gpt-codex', started_at: now - 120, settled: false, sub: false }
    ],
    pending: [
      { id: 'msg_3', key: 'PAY-12', sha: '4f0c1a2', landing: 'merge_left', hold_reason: null },
      { id: 'msg_7', key: 'UI-7', sha: '0be77c4', landing: 'push_left', hold_reason: null },
      { id: 'msg_9', key: 'OPS-3', sha: 'c2d9e51', landing: null, hold_reason: '운영 배포 승인을 기다린다' },
      { id: 'msg_11', key: null, sha: null, landing: null, hold_reason: null }
    ],
    decisions: [
      { id: 'D001', key: 'PAY-12', question: '변경이 620줄이라 리뷰 기준을 넘는다. 리뷰를 진행할까?', options: ['진행', '생략'], asked_at: now - 1500, expires_at: now + 84900 },
      { id: 'D002', key: 'PAY-13', question: 'PAY-13의 재시도 횟수를 몇 번으로 할까?', options: ['3번 (추천)', '5번', '재시도 없음'], asked_at: now - 300, expires_at: null }
    ],
    ready: ['PAY-13', 'PAY-15'],
    sync_pending: { art: '9c1e0aa' },
    metrics: { at: now - 200, since: now - 6 * 86400, human_turns: 12, done_tasks: 5, turns_per_task: 2.4, unattended_dispatches: 7, unattended_tasks: 4, subagent_calls: 31, workflow_runs: 2, sub_dispatches: 3 }
  }
  const quiet = { generated_at: now - 4000, mode: 'dev', version: '2.0.0', tasks: [], dispatches: [], pending: [], decisions: [], ready: [], sync_pending: {}, metrics: { at: 0 } }
  const repo = (fields) => ({ id: 'c:/work/shop-api/.git', name: 'shop-api', workspaces: [], active: true, ...fields })
  const cases = {
    none: [repo({ state: 'none' })],
    broken: [repo({ state: 'broken', detail: 'Unexpected end of JSON input' })],
    newer: [repo({ state: 'newer', detail: 'v2' })],
    quiet: [repo({ state: 'ok', status: quiet })],
    empty: []
  }
  const repos = cases[query.get('case')] ?? [
    repo({ state: 'ok', status }),
    { id: 'c:/work/docs-site/.git', name: 'docs-site', workspaces: [], active: true, state: 'none' }
  ]
  // 과제 키를 누르면 여는 글의 표본이다(worker의 task.read 답).
  const task = (input) => ({
    state: 'ok',
    key: input.key,
    role: input.role,
    route: { class: 'direct', role: input.role ?? 'dev', reason: null, decided_by: 'manual' },
    inbox: {
      name: `to_${input.role ?? 'dev'}.md`,
      from: 'shop-api-dev',
      truncated: false,
      text: `---\nkey: ${input.key}\nrole: ${input.role ?? 'dev'}\ngoal: 결제 실패 시 재시도 버튼\n---\n\n## 해야 할 일과 파일 소유권\n\n- [x] src/pay/retry.ts에 재시도 흐름\n- [ ] 실패 사유 문구\n\n## 완료 기준과 검증\n\n- npm test -- pay\n`
    },
    record: null,
    rounds: [],
    request: { from: 'shop-api', truncated: false, text: '결제가 실패하면 재시도 버튼을 보여 주고 실패 사유를 알려 준다.' }
  })
  const request = async (name, input) => (name === 'task.read' ? task(input) : { repos, focus: repos[0]?.id ?? null })
  start({ on() {}, plugin: { request } })
})()
