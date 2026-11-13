# Orca 표본

Orca CLI의 `--json` 응답과, Orca가 세션에 넣는 글의 형식 표본이다. 근거는 `docs/contracts/orca.md`다. 기준 버전은 Orca 1.4.256이다.

- 값은 전부 가짜다.
- **실제 응답을 받아 만든 것이 아니다.** 상태를 조회하거나 바꾸는 Orca 명령은 실행하지 않았다. 필드 이름은 설치본의 CLI가 응답을 출력하는 코드에서 읽었다.
- 코드에서 이름을 확인한 필드만 넣었다. 값의 종류를 확인하지 못한 필드(`worker-start`의 `mode`, `send`의 `lifecycle`, `worker-show`의 `activity` 가운데 `idle` 밖의 값)는 넣지 않았다.
- `<…>`로 적은 값은 Orca가 채우는 문장이나 ID이고 내용을 알 수 없다는 표시다.
- `fake-orca`의 각본을 쓸 때는 이 형태를 따르되, 실환경 점검에서 실제 응답과 한 번 견줘야 한다.

## 응답

`orchestration` 명령의 `--json` 출력은 한 줄 `{"ok":…,"result":{…}}`다. `terminal` 명령은 `id`와 `_meta`가 붙은 여러 줄 형태다.

| 파일 | 명령 | 확인하지 못한 것 |
|---|---|---|
| `run_create.json` | `orchestration run-create --objective … --json` | - |
| `run_use.json` | `orchestration run-use --id … --json` | - |
| `worker_start.ready.json` | `orchestration worker-start … --json`, 성공 | `workspace` 값의 정확한 모양 |
| `worker_start.unobserved.json` | 같은 명령. 턴 시작이 아직 관측되지 않음(`ok: true`, 종료 코드 0). `next`의 문장은 CLI 코드의 것이다 | - |
| `worker_start.failed.json` | 같은 명령. 실패(`ok: false`, 종료 코드 1) | `failedStage`의 값은 코드에 있는 단계 이름 하나를 골랐다. `lastError`와 `next`의 문장 |
| `check.empty.json` | `orchestration check … --json`, 메일 없음 | - |
| `check.timed_out.json` | `--wait`가 시간이 넘음 | - |
| `check.worker_done.json`, `check.worker_done_failed.json` | `worker_done` 한 건. 결과는 `payload.outcome`이다 | `from`의 정확한 형태(보낸 쪽 주소에서 `dispatch:`를 뗀 것이라는 것만 안다) |
| `check.mixed.json` | `heartbeat`·`status`·`question`·`escalation`·`worker_done`이 한 묶음에 든 경우 | 위와 같다 |
| `check.legacy_delivery.json` | 읽을 때 자동으로 처리됨 표시를 하지 않는 옛 호스트. `deliveryId`가 온다 | 현재 버전에서는 이 필드가 오지 않는다 |
| `check.keepalive.stderr.txt` | `--wait` 동안 15초마다 **stderr**에 나오는 줄 | - |
| `worker_show.live.json` | `orchestration worker-show --dispatch … --json`, 일하는 중 | `state`·`stage`의 조합 |
| `worker_show.settled.json` | 같은 명령. 보고를 마침 | 정산된 worker의 `state`·`stage` 조합은 추정이다 |
| `worker_show.exited.json` | 같은 명령. 프로세스가 끝남 | 위와 같다 |
| `worker_list.json` | `orchestration worker-list --run … --json` | `host` 값 |
| `send.escalation.json` | `orchestration send --type escalation … --json` | - |
| `task_create.json` | `orchestration task-create --spec … --json` | 막 만든 Task의 `status` 값 |
| `task_list.json` | `orchestration task-list --run … --json` | - |
| `terminal_list.json` | `terminal list --worktree path:… --json` | 원소의 다른 필드, `id`와 `_meta.runtimeId`의 값 |
| `terminal_send.json` | `terminal send --terminal … --text … --json` | 위와 같다 |
| `error.json` | 오류일 때의 한 줄 | `code`의 값 목록 |

ID의 모양은 `run_`, `task_`, `ctx_`(Dispatch), `msg_`, `delivery_` 뒤에 16진수 12자다. 터미널 핸들은 `term_<UUID>`다.

## Orca가 세션에 넣는 글

| 파일 | 내용 | 확인 수준 |
|---|---|---|
| `preamble.terminal.txt` | 터미널 방식 worker가 받는 preamble과 `=== TASK ===` 뒤의 spec. spec은 설계 8.5절의 문자열 형식이다 | preamble의 글은 Orca 서버 코드의 조립 함수에서 읽었다. 조건부 절(`BASE DRIFT`, `SUB-DISPATCH`, 안 읽은 메일 안내)은 뺐다 |
| `dispatch_prompt.claude.txt` | 위 preamble이 Claude Code의 UserPromptSubmit에 오는 모양(안내 줄 + 붙여 넣기 표식) | 조립한 것이다 |
| `wake.terminal.txt` | 메일이 왔을 때 터미널 세션에 넣는 글. 줄바꿈으로 시작하고 끝난다 | 문구는 코드에서 읽었다 |
| `wake.terminal_unacked.txt` | 확인하지 않은 전달 뒤에 새 메일이 왔을 때 | 위와 같다 |
| `wake.chat_message.txt` | 채팅 세션에 통째로 넣는 메시지 한 건 | 형식 함수는 코드에서 읽었다. 실제로 이 글이 들어가는지는 확인하지 못했다 |
