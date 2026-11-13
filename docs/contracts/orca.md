# Orca 계약 (미정-M3·M4·M5)

- 확인한 날: 2026-11-13. 이 PC의 Orca는 `1.4.256`이다(`orca --version`). 설계의 최소 버전은 1.4.246이다.
- 확인 방법: `orca … --help`를 읽었고, 설치 폴더의 문서와 CLI 코드를 읽었다. **상태를 조회하거나 바꾸는 명령은 하나도 실행하지 않았다.** 그래서 응답 JSON은 실제 응답을 받아 본 것이 아니라 CLI가 응답을 출력하는 코드에서 읽은 형태다.
- 표기: **사실**은 출처가 있는 것, **추정**은 미루어 본 것, **미확인**은 확인하지 못한 것이다.

| 약어 | 출처 |
|---|---|
| HELP | `orca <명령> --help` |
| GUIDE | 설치 폴더 `resources/app.asar.unpacked/out/cli/bundled-skill-guides.js`에 든 안내서. `orca skills get orchestration`이 출력하는 글과 같은 것이다(명령은 실행하지 않고 파일을 읽었다) |
| CLI | 설치 폴더 `resources/app.asar.unpacked/out/cli/**`의 JavaScript(읽을 수 있는 형태) |
| SRV | 설치 폴더 `resources/app.asar.unpacked/out/main/chunks/orcad-retained-source-*.js`와 `resources/orcad-template/orcad-server.js`(압축된 번들. 문자열과 짧은 함수만 읽었다) |
| ENV | 이 작업을 한 셸의 환경 변수 **이름**(값은 기록하지 않았다) |

설치 폴더는 이 PC에서 `%LOCALAPPDATA%\Programs\orca\resources`다.

## 1. 실행 파일과 출력 봉투

**사실**:

- 실행 파일을 고르는 순서는 `ORCA_CLI_COMMAND` → `orca-dev`(세션에 `ORCA_DEV_REPO_ROOT`가 있을 때) → `orca-ide`(Orca 터미널 밖의 Linux) → `orca`다. 고른 것이 실행되지 않으면 다른 것으로 넘어가지 않는다(`~/.agents/skills/orchestration/SKILL.md`). 설계 9.2절과 같다.
- Windows 설치본의 `bin/`에는 `orca.exe`와 `orca.cmd`가 있다. `orca.cmd`는 `orchestration send`와 `orchestration reply`를 **거절한다**(종료 코드 2, "orca.cmd cannot safely forward orchestration message bodies. Use orca.exe"). 이유는 cmd.exe가 본문의 줄바꿈을 다시 해석하기 때문이다. Rust의 `Command::new("orca")`는 Windows에서 `orca.exe`를 찾으므로 영향이 없지만, 셸을 거치거나 `.cmd`를 직접 가리키면 걸린다.
- `--json`의 출력 형태는 둘이다(CLI `handlers/orchestration/agent-view.js`, `format.js`, `cli-error.js`).

| 경우 | 형태 |
|---|---|
| `orchestration` 명령, `--json`만 | **한 줄**: `{"ok":true,"result":{…}}`. `result`는 CLI가 고른 평면 필드다. `next`가 있으면 마지막에 온다. 값이 없는 필드는 빠진다 |
| `orchestration` 명령, `--json --full` | 여러 줄(들여쓰기 2): 런타임 응답 전체 `{"id":…,"ok":true,"result":{…중첩된 원본…},"_meta":{"runtimeId":…}}` |
| `terminal` 등 그 밖의 명령, `--json` | 위의 여러 줄 전체 응답 |
| 오류, `orchestration` 명령 | 한 줄: `{"ok":false,"error":{"code":…,"message":…,"data"?:…}}` |
| 결과는 있는데 실패로 치는 경우(예: `worker-start`가 `ready`가 아님) | `{"ok":false,"result":{…}}`와 0이 아닌 종료 코드 |

설계 9.2절의 "`{"ok":true,"result":…}` 형태를 읽는다"는 맞다. 읽을 것은 `--full` 없는 평면 형태다.

- ID의 형태는 `<머리말>_<16진수 12자>`다(SRV의 ID 함수). Run은 `run_…`, Task는 `task_…`, **Dispatch는 `ctx_…`**, 메시지는 `msg_…`, 전달은 `delivery_…`다. 터미널 핸들은 `term_<UUID>`다(ENV의 값 모양). 채팅 방식 worker의 핸들은 `structworker_…`로 시작한다(CLI `shared/orchestration/workers/structured-worker-handle.js`).
- 변경 명령에는 `--retry-request <id>`가 있다. 결과를 모르게 된 요청을 같은 ID로 다시 보내면 다시 실행하지 않고 기록된 결과를 돌려준다. `orchestration request-show --request <id>`는 그 요청이 반영됐는지를 읽기만 한다(HELP).

## 2. 명령별 인자와 응답 (M3)

응답 필드는 CLI의 출력 코드에서 읽은 것이다. 표본은 `scenarios/fixtures/orca/`에 있다.

### 2.1 `orchestration run-create`

```
orca orchestration run-create --objective <text> [--from <handle>] [--retry-request <id>] [--json] [--full]
```

- **`--objective`가 필수다.** Run은 이름 공간이자 우편함이다. `worker-start --spec`은 묶인 Run이 없으면 스스로 하나 만든다.
- 응답: `{ runId, objective, legacy?, next? }`.
- Run은 부르는 터미널(조율자)에 묶인다. 나중에 만든 Run이 `--run` 없는 명령의 기본이 된다. 이전 Run의 메일도 계속 조율자를 깨운다(GUIDE `coordinator-loop`).

### 2.2 `orchestration run-use`

```
orca orchestration run-use --id <run_id> [--from <handle>] [--takeover-legacy] [--json]
```

- 응답: `{ runId, objective, legacy? }`. worker는 자기 Run이나 조상 Run을 묶을 수 없다(HELP).

### 2.3 `orchestration worker-start`

```
orca orchestration worker-start (--task <task_id> | --spec <text> | --spec-file <path|->)
     [--keep-open] [--on <server>] [--worktree <current|selector|new-child|new-top-level>]
     (--agent <agent> | --terminal <handle>) [--task-title <text>] [--deps <json_array>] [--parent <task_id>]
     [--model <id>] [--effort <level>] [--retry-of <dispatch_id>] [--timeout-ms <n>] [--run <run_id>]
     [--from <handle>] [--retry-request <id>] [--json] [--full]
```

**사실**(HELP):

- `--task`, `--spec`, `--spec-file`은 **셋 중 하나**다. `--spec-file -`은 stdin에서 읽는다(따옴표·백틱을 셸에서 감쌀 필요가 없다).
- `--agent`와 `--terminal`은 **둘 중 하나**다. `--model`·`--effort`는 `--terminal`과 함께 쓸 수 없다. `--effort`는 `--model`이 있어야 한다.
- `--worktree`의 값은 선택자다: `current`, `path:<경로>`, `branch:<브랜치>`, `id:<repo-id>::<경로>`, `name:<이름>`, `new-child`, `new-top-level` 등. 경로를 그대로 주는 형태는 도움말에 없다. `--terminal`을 다시 쓸 때는 그 터미널의 워크트리를 `--worktree`로 준다.
- `--agent`는 Orca의 에이전트 ID다: `claude`, `codex`, `cursor`, `antigravity`, `muse`, `zcode`, `opencode` 등.
- worker가 터미널로 뜰지 채팅으로 뜰지는 사용자의 설정을 따른다. 플래그가 없다. **터미널이 없는 worker가 있을 수 있다.**
- 끝난 worker는 스스로 닫힌다(`--keep-open`이 아니면). `worker_done`이 받아들여지면 약 2분 뒤 터미널을 놓는다(`autoReleaseAt`). 그 전에 `--terminal`로 다시 쓰면 놓기가 취소되고, 지난 뒤에는 시작이 거절된다.
- 종료 코드: `ready`면 0. "턴 시작이 아직 관측되지 않음"도 평면 출력에서는 0이다. 실패나 결과 불명은 1이다.
- 실패한 시작도 Task·Dispatch·터미널을 남길 수 있다(`residualResources`). 안내서는 "실패하면 다시 실행하지 말고 `failedStage`를 읽으라"고 한다. 같은 Task에 다시 시도하려면 `--task <task_id> --retry-of <dispatch_id>`를 쓴다. `--retry-of` 사슬에서 시작이 세 번 실패하면 그 Dispatch는 차단된다(GUIDE `recovery-and-cleanup`).

응답(CLI `worker-start-agent-view.js`):

| 필드 | 뜻 |
|---|---|
| `dispatchId`, `taskId`, `runId` | 항상 온다 |
| `runCreated` | Run을 새로 만들었으면 `true` |
| `state` | `ready`, `outcome_unknown`, `failed` 등 |
| `agent`, `model`, `effort` | 실제로 쓴 값(없으면 요청한 값) |
| `mode`, `modeDetail` | 뜬 방식. 값의 종류는 미확인 |
| `terminal` | 에이전트 터미널의 핸들. 채팅 방식이면 없을 수 있다 |
| `workspace` | 워크트리 ID |
| `failedStage`, `lastError`, `warning`, `residualResources[]`, `nextCommands`, `keptOpen` | 실패·경고일 때 |
| `next` | 다음에 할 일을 적은 문장 |

`state`가 `outcome_unknown`이고 턴 시작이 관측되지 않은 경우는 `ok: true`, 종료 코드 0이며 `next`가 "다시 보내지 말라"고 말한다.

### 2.4 `orchestration check`

```
orca orchestration check [--terminal <handle>] [--run <run_id>] [--ack <delivery_id>]
     [--unread | --peek | --all] [--types <type,...>] [--wait] [--timeout-ms <n>] [--retry-request <id>] [--json]
```

**사실**(HELP):

- **`ack`라는 하위 명령은 없다.** `orca orchestration ack --help`는 "Unknown command"다. 확인 처리는 `check --ack <delivery_id>`다.
- 기본 동작(소비형): 묶인 Run의 가장 오래된 안 읽은 묶음을 돌려주고 **그 자리에서 처리됨으로 표시한다.** `--ack`는 필요 없다. `--ack`는 "묶음을 스스로 표시하지 않는 옛 Orca"를 위한 것이고, 이미 처리된 전달에 대한 `--ack`는 아무 일도 하지 않는다.
- `--peek`: 안 읽은 메시지만 돌려주고 표시하지 않는다. `--all`: 최근 메시지를 돌려주고 표시하지 않는다. `--unread`·`--peek`·`--all`은 하나만 쓴다.
- `--wait`: 맞는 메시지가 오거나 `--timeout-ms`가 지날 때까지 막는다. 15초마다 stderr에 `{"_keepalive":true,"_heartbeat":true,"elapsedMs":…,"deadlineMs":…}` 한 줄을 쓴다. `--peek --wait`를 함께 쓸 수 있다.
- `--types`는 `--wait`를 깨우는 조건이다. 돌려주는 전달은 언제나 묶음 전체라서 종류로 걸러지지 않는다. `--peek`와 `--all`만 줄을 거른다. PowerShell에서는 쉼표 목록을 따옴표로 감싼다.
- **"받은 편지함을 지켜보기만 하는 스크립트는 `--peek`를 써야 하고 `--ack`를 해서는 안 된다. 그렇지 않으면 에이전트가 볼 메일을 가져간다."** 소비형 check는 에이전트 자신의 것이다.
- 문맥을 잃었을 때는 `orchestration inbox --terminal run:<run_id> --limit <n> --full`로 이미 처리한 메일을 본다.

응답(CLI `message-agent-views.js`의 `checkView`):

| 필드 | 뜻 |
|---|---|
| `messages[]` | 아래 표 |
| `deliveryId` | **옛 호스트일 때만** 온다(읽을 때 자동으로 표시하지 않는 호스트). 현재 버전에서는 빠진다 |
| `replayed`, `acknowledged`, `timedOut`, `cancelled`, `connectionLost` | 해당할 때만 `true`. 기다리다 시간이 넘으면 `messages: []`와 `timedOut: true`다. `cancelled`·`connectionLost`면 실패로 친다 |
| `legacyCompatibility`, `next` | 해당할 때만 |

메시지 한 건:

| 필드 | 뜻 |
|---|---|
| `id` | `msg_…` |
| `type` | `status`, `dispatch`, `worker_done`, `merge_ready`, `escalation`, `handoff`, `decision_gate`, `question`, `heartbeat`(HELP `send`). 없으면 `status`로 본다 |
| `from` | 보낸 쪽 주소에서 앞의 `dispatch:`를 뗀 것 |
| `subject`, `body`, `priority` | 있을 때만. `ask`로 보낸 질문은 `subject` 없이 `body`가 질문이다 |
| `payload` | 보낸 쪽의 payload가 JSON 객체면 객체로, 아니면 문자열로 온다. `send`의 구조화 플래그가 만드는 키는 `taskId`, `dispatchId`, `outcome`(`succeeded`·`failed`), `filesModified[]`, `reportPath`, `phase`다(CLI `message-payload.js`) |
| `followUp`, `readOnly` | 해당할 때만 |

평면 응답의 메시지에는 `run_id`가 없다. Run은 호출할 때 준 `--run`으로 안다.

### 2.5 `orchestration worker-show`

```
orca orchestration worker-show --dispatch <dispatch_id> [--json] [--full]
```

응답(CLI `worker-agent-views.js`): `dispatchId`, `taskId`, `runId`, `followUpOf?`, `state`, `stage`, `liveness`, `activity?`, `lastActivity?`, `terminal?`, `terminalState?`, `workspace?`, `attention?`, `waitingOnHuman?`, `lastError?`, `autoReleaseAt?`, `next?`.

- `liveness`는 `live`, `exited`, `unverifiable` 가운데 하나이고, 자세한 사유가 있으면 `"unverifiable (<사유>)"`처럼 **괄호가 붙은 문자열**로 온다. 앞 단어로 비교해야 한다.
- `terminalState`는 `active`, `reclaimable`, `retained`, `release_pending`, `release_unknown`, `released`다(HELP `worker-list`).
- `outcome` 필드는 `worker-show`의 평면 응답에 **없다.** 결과는 `state`로 본다.
- **부분 확인**: `state`에는 `ready`, `succeeded`, `failed`, `outcome_unknown`, `stopping`, `stopped`, `abandoned`, `accepted`, `unsupervised`가, `stage`에는 `input_accepted`, `settled`, `assignment_settled`, `released`, `stop_requested`, `process_stopped`, `abandoned`, `terminal_readying` 등이 코드에 문자열로 있다(SRV). 어떤 조합이 "정산됨"인지는 실제 응답으로 확인하지 못했다. `stage == "settled"`만 보면 터미널을 놓은 뒤(`released`)를 놓칠 수 있다.
- 침묵·시간 초과·`unverifiable`만으로는 종료를 단정하지 말라고 안내서가 거듭 말한다(GUIDE "Safety").

### 2.6 `orchestration worker-list`

```
orca orchestration worker-list [--run <run_id>] [--terminal-state <state>] [--include-remote] [--cursor <cursor>] [--limit <1-100>] [--json] [--full]
```

응답: `{ workers[], scope, nextCursor?, warnings? }`. `workers[]`의 원소는 `dispatchId`, `taskId`, `state`, `outcome?`(`state`와 다를 때만), `liveness`(괄호 없이 `live`·`exited`·`unverifiable`), `agent?`(`<id>/<model>`), `terminal?`, `terminalState?`, `workspace?`, `host?`, `attention?`, `autoReleaseAt?`, `next?`다.

- 기본으로 최신 100건까지다. 더 있으면 `nextCursor`가 온다.
- `--run`이 없으면 부르는 터미널에 묶인 Run이 범위다. 묶인 것이 없으면 모든 Run이다. 어느 쪽이었는지는 `scope.source`(`flag`·`bound`·`all`)가 말한다.
- `outcome`의 값은 `succeeded`, `failed`, `in_progress`, `outcome_unknown`이다(CLI `shared/orchestration/fleet/orchestration-fleet-outcome-resolution.js`).

### 2.7 `orchestration send`

```
orca orchestration send --subject <text> [--to <run:id|dispatch:id|legacy_handle>] [--run <run_id>] [--from <handle>]
     [--body <text>] [--type <type>] [--priority <level>] [--payload <json>] [--task-id <id>] [--dispatch-id <id>]
     [--outcome <succeeded|failed>] [--files-modified <csv>] [--report-path <path>] [--phase <text>] [--retry-request <id>] [--json]
```

- `--subject`가 필수다. `worker_done`은 `--outcome`이 필수다. `--payload`와 구조화 플래그(`--task-id` 등)는 함께 쓸 수 없다.
- worker가 받는 쪽을 생략하면 자기 조율자에게 간다. `worker_done`과 `heartbeat`는 정확한 Dispatch의 신호라 그룹 주소로 보낼 수 없다.
- 응답: `{ messageId, lifecycle?, autoReleaseAt?, warnings?, next? }`. 중계되면 `{ messageId, dispatchId, queuedFor }`다.
- 설계의 F-01이 쓰는 `--type escalation --subject … --body … --dispatch-id <id> [--task-id <task>]`는 모두 있는 인자다.
- Orca 스스로도 worker가 `worker_done` 없이 멈추면 조율자에게 `stopReason`이 든 `escalation`을 보낸다(GUIDE "When Orca wakes you").

### 2.8 `orchestration task-create`, `task-list`

```
orca orchestration task-create (--spec <text> | --spec-file <path|->) [--task-title <text>] [--display-name <text>] [--deps <json_array>] [--parent <task_id>] [--run <run_id>] [--from <handle>] [--json]
orca orchestration task-list [--status <status>] [--ready] [--brief] [--run <run_id>] [--from <handle>] [--json] [--full]
```

- Task에는 Run이 있어야 한다. 먼저 `run-create`를 한다(GUIDE `coordinator-loop`).
- `task-create` 응답: `{ taskId, status }`. Task 상태는 `pending`, `ready`, `dispatched`, `completed`, `failed`, `blocked`다(HELP `task-update`).
- `task-list` 응답: `{ tasks[], runId, legacyReadOnly? }`. 원소는 `id`, `status`, `title`, `deps?`, `parentId?`, `dispatchId?`, `assignee?`다. spec과 결과 본문은 `--full`에서만 온다.

### 2.9 `orchestration dispatch`

```
orca orchestration dispatch --task <task_id> --to <handle> [--from <handle>] [--run <run_id>] [--inject] [--dry-run] [--return-preamble] [--json]
```

`--inject`가 있다. 설계의 T-02가 보는 형태와 맞는다.

### 2.10 `terminal list`, `terminal send`

```
orca terminal list [--worktree <selector>] [--limit <n>] [--include-visual-layouts] [--json]
orca terminal send [--terminal <handle>] [--text <text>] [--enter] [--interrupt] [--wait-submit <seconds>] [--retry-request <id>] [--json]
```

- `terminal list`의 `--json`은 전체 응답 형태(여러 줄)다. `result.terminals[]`의 원소에 `handle`, `title`, `paneTitle?`, `connected`, `orphaned?`, `executionHostId?`, `worktreePath`, `preview`가 있다(CLI `terminal-format.js`). `result.truncated`와 `result.totalCount`도 있다. 그 밖의 필드는 확인하지 못했다.
- `terminal send`의 응답은 `result.send`에 `handle`, `accepted`, `refusedReason?`, `bytesWritten`, `prompt?`가 있다.
- 설계 9.2절의 `terminal list --worktree path:<경로>`는 유효한 선택자다.

## 3. worker에게 주어지는 preamble (M3)

**사실**(SRV의 preamble 조립 함수, CLI `shared/orca-dispatch-status-prompt.js`): worker가 받는 프롬프트는 아래 순서로 조립된다. 전체 표본은 `scenarios/fixtures/orca/preamble.terminal.txt`다(값은 가짜다).

1. 첫 줄 `Please carry out this task from my Orca coordinator by following the brief I pasted below.`(붙여 넣기 앞에 타이핑하는 안내 줄).
2. preamble. 첫 문장은 `You are working inside Orca, a multi-agent IDE. You are a dispatched worker.`다.
3. `Your coordinator's terminal handle is: <핸들>`(채팅 방식 조율자면 `Your coordinator's Orca session ID is: <ID>`), `Your task ID is: <taskId>`, 채팅 방식 worker면 `Your Orca session ID is: <핸들>`.
4. `=== CLI COMMANDS ===` 절. 그 안에 `ask`, `escalation`, `worker_done` 명령이 들어 있다. dispatch ID와 task ID는 다음 꼴로 들어간다.

```
orca orchestration send --from <worker 핸들> --type escalation --subject "Blocked: <reason>" --body "<details>" --task-id <taskId> --dispatch-id <dispatchId>
orca orchestration send --from <worker 핸들> --type worker_done --subject "<short status>" --body "<…>" --task-id <taskId> --dispatch-id <dispatchId> --outcome succeeded
```

5. `=== AFTER YOU SEND worker_done ===` 절. 터미널 worker에게는 "셸을 끝내지 말라. 다음 일이 새 preamble과 TASK 블록으로 이 터미널에 온다"고 말한다.
6. 조건에 따라 `--- BASE DRIFT ---` 절과 `=== SUB-DISPATCH ===` 절.
7. `=== TASK ===` 줄 다음에 spec 본문.

설계가 기대는 사실에 대한 확인은 다음과 같다.

- preamble에는 `--dispatch-id <값>`과 `--task-id <값>`이 공백으로 이어져 들어 있다. 설계 P-01의 패턴(`--dispatch-id` 뒤 공백 또는 `=`와 `[A-Za-z0-9_.:-]+`)이 맞는다. 값은 `ctx_<16진수 12자>`와 `task_<16진수 12자>`다.
- 실행 파일 이름은 `orca`가 아닐 수 있다(`orca-dev` 또는 `ORCA_CLI_COMMAND`의 값).
- 같은 세션이 나중에 **새 Dispatch ID가 든 새 preamble**을 받을 수 있다(후속 일). 설계 P-01이 "그 값이 세션의 dispatch와 다르면 다시 묶는다"로 정한 것과 맞는다.
- preamble은 "이 일만, 스스로 하라. task가 요구하지 않으면 하위 에이전트나 helper worker를 쓰지 말라"고 지시한다. 하위 배정이 허용될 때만 `=== SUB-DISPATCH ===` 절이 붙는다.

## 4. worker 셸의 환경 변수 (M3)

**관측**(ENV): Orca 터미널에서 뜬 Claude Code 세션의 도구 셸에는 `ORCA_TERMINAL_HANDLE`이 있다. 값은 `term_<UUID>` 꼴이다. 함께 보이는 이름은 `ORCA_PANE_KEY`, `ORCA_TAB_ID`, `ORCA_WORKSPACE_ID`, `ORCA_WORKTREE_ID`(`<UUID>::<경로>`), `ORCA_APP_VERSION`, `ORCA_USER_DATA_PATH`, `ORCA_AGENT_HOOK_*` 등이다. `ORCA_CLI_COMMAND`는 이 셸에 없었다(스킬 문서는 "관리되는 WSL 세션에서 내보낸다"고 한다).

**사실**(CLI `handlers/orchestration/terminal-identity.js`, `shared/agent-session/lifecycle/agent-session-caller-env.js`):

- CLI는 부르는 쪽을 `ORCA_AGENT_SESSION_ID`(채팅 방식 세션에 주입되는 ID) → 명시한 `--from`·`--terminal` → `ORCA_TERMINAL_HANDLE` 순으로 정한다.
- 채팅 방식 세션에는 `ORCA_TERMINAL_HANDLE`이 없을 수 있다. 채팅 방식 worker의 핸들은 `structworker_…`다.
- Codex는 `shell_environment_policy` 때문에 도구 셸에서 `ORCA_TERMINAL_HANDLE`이 지워질 수 있다. Orca의 오류 문구가 이 경우를 직접 설명한다("Codex ran this orchestration command without ORCA_TERMINAL_HANDLE…").

설계 7.3절 2번(터미널 핸들로 dispatch를 세션에 묶기)은 터미널 방식의 Claude Code worker에서는 성립한다. 채팅 방식 worker와 환경이 걸러진 Codex에서는 성립하지 않을 수 있고, 그때는 P-01(preamble로 묶기)만 남는다.

## 5. Orca 패널 플러그인 (M4)

**사실**(GUIDE `orca-plugin-authoring`과 그 `manifest`·`host-api`·`recipes` 참고서, HELP `orca plugin …`):

- 플러그인의 권한 단계(`execution`)는 낮은 것부터 `data`, `declarative`, `sandboxed-ui`, `trusted-worker`다.
- **패널만으로는 파일도 프로세스도 쓸 수 없다.** 패널(`sandboxed-ui`)은 `orca`·`orca/*`와 자기 폴더의 파일만 불러올 수 있고, 플러그인 파일 밖으로 나가는 요청은 실패한다("No network"). 패널이 쓸 수 있는 호스트 API는 `workspace.readContext`, `agents.list`, `storage.*`, `secrets.*`, `settings.*`, `notifications.show`, `plugin.request` 등이고 파일 읽기와 프로세스 실행은 없다.
- 패널이 Node 코드를 쓰려면 **worker가 딸린 패널**로 만든다. `execution`이 `trusted-worker`이고 `main`이 있어야 한다. `main`은 "프로세스 밖 플러그인 worker에서 도는 Node 진입점"이다. 패널은 `plugin.request(name, input)`으로 worker의 `orca.panels.handle(name, …)`을 부르고, worker는 `orca.panels.post(event, payload)`로 패널에 밀어 넣는다. 응답은 JSON 1MiB까지, post는 256KiB·10초에 200건까지다. 요청 제한 시간은 기본 30초, 최대 120초다.
- worker는 `host.call('workspace.list')`(권한 `workspace:read`)로 워크스페이스의 ID·종류·경로·브랜치를 얻는다.
- 쉬는 worker는 멈췄다가 다음 명령·이벤트·패널 요청 때 다시 뜬다.
- `orca plugin validate`와 `orca plugin check`가 있다(설계 묶음 H의 완료 확인에 쓰는 명령). `check`와 `test`는 플러그인 코드를 이 PC에서 샌드박스 없이 돌린다.
- 플러그인 하나는 파일 2,000개·50MiB까지다.

**매니페스트**(묶음 H가 2026-11-14에 Orca 1.4.256 설치 폴더에서 읽었다. 출처는 GUIDE의 `orca-plugin-authoring` `manifest`·`host-api`·`recipes` 참고서, 스키마 코드, `worker-panel` 템플릿이다. `orca plugin validate`는 실행하지 않았다):

| 항목 | 값 | `panel/orca-plugin.json`이 쓴 값 |
|---|---|---|
| 파일 이름 | 플러그인 폴더의 `orca-plugin.json` | - |
| `manifestVersion` | `2` | `2` |
| `execution` | `data` \| `declarative` \| `sandboxed-ui` \| `trusted-worker` | `trusted-worker` |
| `id`, `name`, `version`, `description` | 글. 설치 ID는 `<publisher>.<id>`다. `publisher`는 생략할 수 있다 | `fullops-panel`, `fullops`, `2.0.0`(`publisher`는 넣지 않았다) |
| `engines.orca` | 버전 범위 | `>=1.4.256` |
| `pluginApi` | 정수 | `1` |
| `main` | worker의 진입점(`.mjs`를 그대로 읽는다). `trusted-worker`에 필수다 | `src/worker.mjs` |
| `contributes.panels[]` | `{ id, title, icon, entry }`. `entry`는 `.html` 파일이고 `icon`은 Orca의 아이콘 어휘에서 고른다 | `{ "id": "status", "title": "fullops", "icon": "activity", "entry": "src/index.html" }` |
| `capabilities[]` | `{ kind }`. worker가 부르는 호스트 API의 권한 | `[{ "kind": "workspace:read" }]` |
| 모르는 키 | 받지 않는다 | 없음 |

- worker의 진입점은 `export default function activate(orca)`다. 그 안에서 `orca.panels.handle(name, fn)`으로 패널의 `plugin.request(name, input)`에 답한다.
- 패널 메시지 한도는 10초에 120건이다(`panel/`은 5초마다 한 번 묻는다).

**패널 프레임의 두 가지 CSP**(설치 폴더 `resources/app.asar.unpacked/out/shared/plugins/frames/plugin-panel-shell.js`의 두 상수. 2026-11-14에 다시 읽어 같은 값임을 확인했다):

| 프레임 | 쓰는 곳 | CSP |
|---|---|---|
| 격리 게스트(`PLUGIN_GUEST_CSP`) | 데스크톱 앱 | `default-src 'self'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; media-src 'self' data: blob:; connect-src 'self' data: blob:; worker-src 'self' blob:; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'` |
| srcdoc iframe(`PLUGIN_PANEL_CSP`) | 브라우저 클라이언트(serve/web) | `default-src 'none'; connect-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; base-uri 'none'; form-action 'none'` |

- 데스크톱의 격리 게스트는 자기 폴더의 파일(`'self'`)을 불러올 수 있지만, 브라우저 클라이언트의 srcdoc iframe은 인라인 스크립트와 인라인 스타일만 돈다(별도 `.js`·`.css` 파일과 `data:`가 아닌 이미지·글꼴은 불리지 않는다). 그래서 두 프레임에서 모두 도는 패널은 HTML·CSS·JS를 한 파일에 넣어야 한다(`panel/src/index.html`이 그렇게 했다). 네트워크는 두 프레임 모두 막혀 있다(격리 게스트는 CSP가 아니라 세션이 막는다).
- `orca plugin check`는 `ui`가 `components`인 패널만 한 번 그려 본다(`resources/app.asar.unpacked/out/cli/plugin-authoring/plugin-panel-check.js`). HTML 패널은 그리지 않으므로 HTML 패널에서 `check`가 보는 것은 `validate`와 같다. 화면은 `orca plugin shot`이나 미리보기로 본다.
- worker의 실행(묶음 H가 `app.asar`와 `out/main/plugin-host-entry.js`에서 읽었다): worker는 `fork(entry, [], { env: 허용 목록, execArgv: [] })`로 뜨는 Node 프로세스이고 Node의 권한 모델 인자가 없다. 환경 변수는 PATH·HOME·USERPROFILE 등 허용 목록만 넘어간다.

**추정**: worker는 Node 프로세스이므로 `node:fs`로 Git 공용 디렉터리의 `status.json`을 읽고 `node:child_process`로 `fullops status --json`을 띄울 수 있다. 문서는 "패널에 Node가 필요하면 worker를 쓰라"고만 하고 `fs`·`child_process`를 직접 말하지는 않는다. 제한이 있는지는 **미확인**이다(위의 `fork` 인자로 보면 `node:fs`에는 제한이 없다고 본다. 실행으로는 확인하지 않았다. `panel/`은 `node:fs`로 읽고 `child_process`는 쓰지 않는다. 확인 명령은 `orca plugin dev request panel status.read --json`이다. 설계 16절 M4). `workspace.readContext`의 `displayName`·`branch`가 `workspace.list`의 `name`·`branch`와 같은 값인지도 **미확인**이다(다르면 패널이 초점 레포를 찾지 못해 첫 레포를 보여 준다).

설계의 가정("`status.json` 파일을 읽는다. 안 되면 `fullops status --json`을 부른다")은 패널 혼자서는 성립하지 않고, `trusted-worker`로 만들어야 성립한다. 그러면 사용자가 설치할 때 코드 실행을 승인해야 한다(`plugin install … --approve`).

## 6. 세션을 깨우는 입력 (M5)

**사실**(SRV의 알림 문구 함수, GUIDE "When Orca wakes you"):

현재 Orca의 기본 모델은 **조율자가 턴을 끝내면 Orca가 깨운다**는 것이다. 안내서는 "폴링하거나 지켜보는 스크립트를 돌리지 말라. 스크립트의 소비형 `check`는 네 메일을 가져가서 Orca가 너를 깨울 것이 없어진다"고 말한다.

| 경우 | 넣는 글 |
|---|---|
| 터미널 세션에 메일이 왔을 때 | `\nYou have <N> orchestration message(s). Run \`<orca> orchestration check[ --run <run_id>]\`.\n` (한 건이면 `message`, 여러 건이면 `messages`) |
| 확인하지 않은 전달 뒤에 새 메일이 왔을 때(옛 호스트 호환) | `\nYou have <N> new orchestration message(s) after delivery <delivery_id>, which you have not acknowledged. If you no longer have it, run \`… check …\` to see it again; once it is handled, run \`… check … --ack <delivery_id>\`, then \`… check …\` for the new mail.\n` |
| 채팅 세션 | 메시지가 통째로 한 건씩 온다(GUIDE). 형식을 만드는 함수는 `──── From: <주소 대문자> (<주소>) (<type>) ────`, `Subject: <제목>`, 본문, `[Payload: <JSON>]`, `[Reply: <orca> orchestration reply --id <msg_id> --body "..."]`, `─` 60개 줄을 줄바꿈으로 잇는다(SRV) |
| worker에게 일을 줄 때 | 3절의 배정 프롬프트 |

- 터미널 알림은 **줄바꿈으로 시작한다.** hook이 받는 `prompt`에서 앞의 줄바꿈이 남는지는 **미확인**이다. 설계 P-02의 "`You have`로 시작하고 `orchestration message`를 담으면"은 앞 공백을 뗀 뒤 비교해야 안전하다.
- 채팅 형식은 P-02의 두 패턴 어디에도 맞지 않는다. 채팅 세션이 lead일 때 Orca 메일이 `human`으로 세어진다.
- 표본은 `scenarios/fixtures/orca/wake.terminal.txt`, `wake.terminal_unacked.txt`, `wake.chat_message.txt`다. 문구는 코드에서 읽은 것이고, 터미널에 실제로 찍힌 것을 채집한 것은 아니다.

Claude Code 쪽의 `<task-notification>`은 `claude-code.md` 5절에 적었다.

## 7. 확인하지 못한 것

| 항목 | 이유 | 확인하는 방법 |
|---|---|---|
| 모든 응답 JSON의 실제 값 | 상태를 조회하는 명령을 실행하지 않았다 | 실환경 점검에서 `worker-start`·`check`·`worker-show`·`worker-list`의 `--json` 출력을 한 번씩 받아 표본과 견준다 |
| 정산된 worker의 `state`·`stage` 조합 | 위와 같다 | `worker_done` 뒤와 터미널을 놓은 뒤에 `worker-show --json`을 받아 본다 |
| `--worktree`에 날 경로를 줘도 되는가 | 도움말에 없다 | `path:<경로>`를 쓰면 확인할 필요가 없다 |
| `check --wait`의 `--timeout-ms` 상한 | 도움말에 없다(`ask`는 최대 1,800,000ms라고 적혀 있다) | 긴 값을 주고 한 번 돌려 본다 |
| `--peek`로 본 메시지를 나중에 "처리됨"으로 표시하는 다른 방법 | 도움말은 소비형 check만 말한다 | - |
| `mode`의 값, `lifecycle`의 값, 메시지 `from`의 정확한 형태 | 코드에서 값의 종류까지 읽지 못했다 | 실제 응답 |
| 설계 8.6절의 "no longer bound" 오류 문구 | 실행하지 않았다 | 실제 오류의 `error.code`를 본다 |
| 터미널 알림의 앞 줄바꿈이 hook의 `prompt`에 남는가, 채팅 세션에 넣는 글의 정확한 형태 | 세션을 띄우지 않았다 | `FULLOPS_TRACE=1`로 분류 결과를 본다 |
| 플러그인 worker의 `fs`·`child_process` 제한 | 문서에 없다 | 묶음 H가 `worker-panel` 템플릿으로 한 번 돌려 본다 |
