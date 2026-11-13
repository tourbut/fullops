# 설계 문서와 실제 계약의 차이 (묶음 0)

- 대상: `docs/design.md`의 16절(미정-M1~M5)과 7.2·8.5·8.6·9.2·10.4절의 가정.
- 근거: `claude-code.md`, `codex.md`, `orca.md`. 확인한 버전은 Claude Code 2.1.323, Codex 0.163.0, Orca 1.4.256이다.
- 판정 표기: **확인됨**은 가정이 맞다는 뜻이다. **고침**은 설계 문장을 바꿔야 한다는 뜻이다. **결정 필요**는 어느 쪽으로 고칠지 사람이나 A 담당이 골라야 한다는 뜻이다. **미확인**은 이 묶음이 확인하지 못해 가정이 그대로 남는다는 뜻이다.
- 이 묶음은 세션을 띄우거나 Orca의 상태를 조회하지 못했다. 문서·도움말·설치본의 코드로 확인한 것이므로, 표의 "고침"도 실환경 점검(설계 12.4절)에서 한 번 더 봐야 한다.

## 0. 뼈대 동결에 영향을 주는 것 (먼저 읽을 것)

설계 16절은 "미정 항목이 가정과 달라도 5~8절의 타입·상태 형식·규칙 표·CLI 표면은 바뀌지 않는다"고 했다. 대부분은 그렇다. 아래 다섯은 그 경계를 넘는다.

| # | 내용 | 닿는 곳 |
|---|---|---|
| Z1 | Orca에 `ack` 명령이 없고, 소비형 `check`는 읽는 순간 처리됨으로 표시한다. `--peek`는 표시하지 않고 전달 ID도 주지 않는다. `wait`의 "ack하지 않고 반환 → 나중에 `--ack <delivery>`" 모델이 현재 Orca와 맞지 않는다 | 8.6절 전체, `Orca::check`·`Orca::ack` 시그니처(10.2절), `state.json`의 `messages[].delivery`(6.2.1절), X-05의 탈출구 문구 |
| Z2 | `run-create`는 `--objective`가 필수다 | `Orca::run_create()` 시그니처(10.2절) |
| Z3 | `worker-start`에서 `--task`와 `--spec`은 함께 쓸 수 없고, `--agent`와 `--terminal`도 함께 쓸 수 없다. `--model`·`--effort`는 `--terminal`과 함께 쓸 수 없다 | 8.5절의 명령 형태, `StartRequest`(10.2절) |
| Z4 | Stop 입력에 `reason` 필드가 없다(두 호스트 모두). StopFailure의 `error`는 본문이 아니라 종류 이름이다 | `HookInput.stop_reason`·`HookInput.error`(7.2절), Stop 공통 전제(7.5절), F-01의 본문 |
| Z5 | Codex의 PreToolUse 도구 이름은 `Bash`·`apply_patch`·`spawn_agent`다 | 7.2절 표(`Host::tool_kind`의 입력) |

## 1. M1 — hook 명령을 실행하는 셸

| 설계의 자리와 문장 | 확인 결과 | 고칠 내용 |
|---|---|---|
| 16절 M1 "지금의 가정: POSIX 셸(10.4절)" | **고침.** Claude Code는 shell form을 macOS·Linux에서 `sh -c`, Windows에서 Git Bash로 실행하고, Git Bash가 없으면 PowerShell로 실행한다. Codex는 Windows에서 `cmd.exe /C`, 그 밖에서 `$SHELL -lc`로 실행한다 | "Claude Code: Git Bash가 있으면 POSIX 셸, 없으면 PowerShell. `args`를 주면 셸 없이 직접 실행(exec form). Codex: Windows는 cmd.exe, 그 밖은 로그인 셸. Windows용 문자열은 `commandWindows`로 따로 준다" |
| 16절 M1 "`$HOME`·`${CLAUDE_PLUGIN_ROOT}`가 풀리는가" | `${CLAUDE_PLUGIN_ROOT}`는 Claude Code가 `command`와 `args`에 문자열로 치환하고 환경 변수로도 준다(**확인됨**). `$HOME`은 호스트가 풀지 않고 POSIX 셸만 푼다. exec form·PowerShell·cmd에서는 풀리지 않는다(**고침**) | 10.4절에 "`$HOME`은 POSIX 셸 문자열에서만 쓴다"를 적는다 |
| 16절 M1 "Codex에 플러그인 루트 변수가 있는가" | **있다.** `PLUGIN_ROOT`, `PLUGIN_DATA`와 호환용 `CLAUDE_PLUGIN_ROOT`, `CLAUDE_PLUGIN_DATA`가 환경 변수로 주어지고, `${이름}` 표기는 명령 문자열에서 치환된다 | 10.4절 Codex 행의 "플러그인 루트 변수가 확인되면 … 그 전까지 setup 스킬이 `install`을 한 번 실행한다"를 지우고 폴백을 넣은 문자열로 바꾼다 |
| 10.4절 표, Claude 자체 마켓플레이스판 문자열 | Git Bash가 있는 환경에서는 문서상 동작한다. **Git Bash가 없는 Windows에서는 PowerShell로 실행돼 실패하고, PreToolUse에서는 그 실패가 도구 호출을 막는다**(G2 위반) | **결정 필요.** (가) 문자열을 두고 `"shell": "bash"`, `"onFailure": "continue"`를 더한다. Git Bash가 없으면 fullops가 꺼진 상태가 되고 `doctor`가 알린다. (나) exec form으로 바꾼다: `"command": "${CLAUDE_PLUGIN_ROOT}/bin/fullops"`, `"args": ["hook","claude","--plugin-root","${CLAUDE_PLUGIN_ROOT}"]`, `"onFailure": "continue"`. 이 경우 고정 위치 우선 판단은 문자열에 담을 수 없다. 자세한 것은 `claude-code.md` 1.2절 |
| 10.4절 표, Claude 디렉터리판 문자열 | 위와 같은 문제가 있다 | 위와 같은 선택. exec form이면 `"command": "${CLAUDE_PLUGIN_ROOT}/bin/fullops"`, `"args": ["hook","claude","--in-place"]` |
| 10.4절 표, Codex 패키지 문자열 `exec "$HOME/.fullops/bin/fullops" hook codex` | **고침.** Windows의 cmd.exe에서 동작하지 않는다 | `command`는 POSIX용으로 두고 `commandWindows`를 더한다. 문자열은 `codex.md` 1.2절 |
| 10.4절 "hooks/codex.json — Codex 패키지에서만" | **고침.** Codex는 기본으로 `hooks/hooks.json`만 읽는다 | "Codex 패키지는 `hooks/hooks.json`을 쓰거나, `.codex-plugin/plugin.json`에 `\"hooks\": \"./hooks/codex.json\"`을 적는다" |
| 10.4절 "`bin/fullops`를 확장자 없이 두는 이유: cmd·PowerShell은 `fullops`를 `fullops.exe`로 풀고, POSIX 셸은 런처 스크립트를 실행한다" | **미확인.** exec form에서 Windows가 확장자 없는 경로를 `.exe`로 푸는지 문서에 없다. 같은 폴더에 확장자 없는 런처가 함께 있을 때의 동작도 모른다 | 실환경 점검 항목에 넣는다. 확인 전에는 Windows 문자열에 `fullops.exe`를 적는 쪽이 안전하다(Codex의 `commandWindows`는 그렇게 적었다) |
| 10.3절 "고정 위치 … 호스트가 버전별 캐시를 지워도 떠 있는 세션의 hook이 깨지지 않는다" | Claude Code는 이전 버전 폴더를 14일 동안 남겨 두고, 떠 있는 세션은 `/reload-plugins` 전까지 이전 경로를 쓴다. Claude Code에서는 호스트가 이 문제를 이미 막고 있다. Codex는 확인하지 못했다 | 문장에 "Claude Code는 이전 버전 폴더를 14일 유지한다"를 덧붙인다. 고정 위치를 Claude 판에서도 유지할지는 위의 (가)·(나) 선택에 딸린다 |
| 10.3절·7.5절 S-04 자동 설치 | Codex는 관리 대상이 아닌 hook을 사용자가 `/hooks`에서 신뢰하기 전에는 실행하지 않는다. 설치 직후와 등록 문자열이 바뀐 갱신 직후에는 SessionStart hook도 돌지 않는다 | 10.4절에 "Codex는 설치 뒤 `/hooks`에서 한 번 신뢰해야 hook이 돈다. setup 스킬과 `doctor`가 안내한다"를 더한다 |
| 7.1절 표의 "등록 타임아웃" | **확인됨.** 두 호스트 모두 `timeout` 필드이고 단위는 초다 | 없음 |
| 6.4절 "홈은 Windows에서 `USERPROFILE`" | Git Bash의 `$HOME`은 보통 `USERPROFILE`과 같지만 사용자가 `HOME`을 따로 정하면 다르다(추정). hook 문자열이 `$HOME`을 쓰면 바이너리가 쓰는 홈과 어긋날 수 있다 | 10.4절에 주의 한 문장. exec form을 고르면 이 문제는 없다 |

덧붙이는 사실: Claude Code는 플러그인 루트의 `bin/`을 Bash 도구의 PATH에 넣는다. Claude Code 세션에서는 설치 없이 `fullops`라는 이름으로 CLI를 부를 수 있다. claude.ai와 Cowork는 `bin/`이 있는 플러그인을 설치하지 않는다(미정-M8과 닿는다).

## 2. M2 — hook 계약

### 2.1 Codex

| 설계의 자리와 문장 | 확인 결과 | 고칠 내용 |
|---|---|---|
| 16절 M2 "지금의 가정: Claude Code와 같은 형태" | 입력 필드 이름과 출력 형태는 같다(**확인됨**). 도구 이름은 다르다(아래) | - |
| 7.2절 표 "등록 이벤트: SessionStart, UserPromptSubmit, PreToolUse, Stop" | **확인됨.** 네 이벤트가 모두 있고 `StopFailure`는 없다 | 없음 |
| 7.2절 표 "PreToolUse 매처: `shell\|exec_command\|local_shell\|apply_patch\|spawn_agent`" | **고침.** hook이 받는 이름은 셸 계열이 전부 `Bash`다 | `Bash\|apply_patch\|spawn_agent` |
| 7.2절 표 "셸 도구: `shell`·`exec_command`·`local_shell`" | **고침** | "`Bash`. 명령은 `tool_input.command`" |
| 7.2절 표 "방언은 OS로 정한다(Windows는 PowerShell)" | **미확인** | 그대로 두고 실환경 점검 항목에 넣는다 |
| 7.2절 표 "편집 도구: `apply_patch`(패치 본문이 `tool_input.command`에 온다)" | **확인됨** | 없음 |
| 7.2절 표 "하위 에이전트 도구: `spawn_agent`(글은 `prompt`·`message`)" | 이름은 **확인됨.** 인자에 `message`가 있다. `prompt`는 0.163.0에서 찾지 못했다 | 둘 다 읽는 지금 문장을 둬도 해가 없다 |
| 7.2절 "판정 출력은 두 호스트에 같은 형태로 낸다 … Codex가 각 필드를 실제로 처리하는지는 미정-M2" | **확인됨.** 다섯 형태를 모두 처리한다 | "미정-M2" 문구를 지운다. "Codex의 출력 스키마는 모르는 최상위 필드를 허용하지 않으므로 표의 필드만 낸다"를 더한다 |
| 7.4절 `apply_patch` 문법과 "패치 문법이 이와 다르면 대상 목록이 비고 통과다(미정-M2에서 실제 형식으로 확정)" | **확인됨.** 다섯 표식이 모두 맞다 | 문법에 바깥 봉투 `*** Begin Patch`·`*** End Patch`, 선택 줄 `*** Environment ID:`, `*** End of File`, 글이 붙는 `@@ <문맥>` 머리줄을 더한다. "경로는 `cwd` 기준 상대 경로일 수 있다"를 더한다 |
| 7.6절 "Codex가 그 필드(`stop_hook_active`)를 주는지 확인되지 않았고" | Codex는 준다(필수 필드) | 근거 문장을 "두 호스트 모두 주지만 턴 기준이 두 호스트에서 같게 동작하므로 쓰지 않는다"로 바꾼다 |
| 7.6절 "UserPromptSubmit이 오지 않는 호스트에서는 턴이 늘지 않아 …" | Codex는 UserPromptSubmit을 보낸다. Stop을 막아 이어 가는 동안에는 UserPromptSubmit이 오지 않으므로(소스 확인) 턴 기준 한 번 막기가 뜻대로 동작한다 | 문장은 일반론으로 남겨도 된다 |
| 7.2절 `caps`: Codex `background_wake = false` | **미확인** | 그대로 둔다 |

### 2.2 Claude Code

| 설계의 자리와 문장 | 확인 결과 | 고칠 내용 |
|---|---|---|
| 7.2절 `HookInput.source` 주석 "startup\|clear\|resume\|compact" | **고침.** `fork`가 더 있다(두 호스트 모두) | 주석에 `fork`를 더한다. S-01이 `fork`에도 안내를 낼지는 **결정 필요**(갈라진 새 세션이므로 `startup`처럼 다루는 쪽을 권한다) |
| 7.2절 `HookInput.stop_reason` "Stop: 필드 `reason`", 7.5절 Stop 공통 전제 "입력에 `reason`이 있고 `end_turn`이 아니면 전부 통과한다" | **고침.** Stop 입력에 `reason`이 없다. 사용자가 끊으면 Stop이 돌지 않고, API 오류면 StopFailure가 돈다 | 필드와 전제를 지운다. 남겨도 해는 없지만 걸리는 일이 없다 |
| 7.2절 `HookInput.error` "StopFailure: 필드 `error`의 앞 300자", F-01의 본문 "`<오류 앞 300자>`" | **고침.** `error`는 `rate_limit` 같은 종류 이름이다. 글은 `error_details`와 `last_assistant_message`에 있다 | "본문은 `error`와 `error_details`(없으면 `last_assistant_message`)를 이어 앞 300자"로 바꾼다. 읽는 입력 필드 목록에 `error_details`를 더한다 |
| 7.2절 표 "PreToolUse 매처: `Bash\|PowerShell\|Write\|Edit\|MultiEdit\|Agent\|Task`" | 현재 도구 목록에 `MultiEdit`와 `Task`(하위 에이전트)는 없다. matcher는 이름 목록이라 남겨도 해가 없다 | **확인됨**(바꾸지 않아도 된다) |
| 7.2절 표 "편집 도구: `Write`(`file_path`, `content`), `Edit`(…)" | **확인됨.** 다만 `file_path`는 항상 절대 경로이고 **Windows에서는 역슬래시**다 | 7.4절 편집 대상 계산에 "경로는 구분자를 `/`로 맞추고 레포 루트 기준 상대 경로로 바꾼 뒤 비교한다"를 더한다 |
| 7.2절 표 "하위 에이전트 도구: `Agent`, `Task`(글은 `prompt`·`description`)" | `Agent`의 인자에 `prompt`, `description`, `subagent_type`, `model`이 있다 | **확인됨** |
| 7.2절 출력 표 `Warn` → `{"systemMessage": …}` | 형태는 **확인됨.** 다만 `systemMessage`는 **사용자에게만** 보이고 모델에게는 가지 않는다(두 호스트 모두) | 7.6절 "`Warn`으로 통과시킨다"와 P-03에 "모델은 이 글을 보지 못한다"를 적는다. 모델에게 알려야 하는 것은 SessionStart·UserPromptSubmit의 `Context`로 낸다. Stop에서 `additionalContext`를 쓰면 대화가 이어지므로(Claude Code) Stop의 `Warn`에는 쓰지 않는다 |
| 7.2절 출력 표 `Context`·`Deny`·`Block` | **확인됨** | 없음 |
| 7.6절 "`BlockAlways` … 지문과 무관하게 막는다" | Claude Code는 Stop hook이 턴을 여덟 번 연달아 이어 가게 하면 그다음 차단을 무시한다. 도구를 한 번 부르면 횟수가 돌아간다 | "Claude Code는 도구 호출 없이 여덟 번 연달아 막으면 호스트가 턴을 끝낸다"를 적는다 |
| 7.7절 S-01 안내의 분량 | `additionalContext`는 Claude Code에서 10,000자, Codex에서 약 2,500 토큰이 한도다. 넘으면 파일로 빠지고 앞부분만 모델에게 간다 | 7.7절에 한도를 적고 `messages`가 안내를 그 안에 맞춘다 |
| 7.1절 "이벤트 종류는 stdin의 `hook_event_name`으로 가른다", 7.2절 "snake_case만 읽는다" | **확인됨** | 없음 |
| 1절·7.1절 "hook은 … 종료 코드는 항상 0이며 판정은 stdout JSON으로만 낸다" | **확인됨.** stdout에는 JSON 객체 하나만 있어야 한다 | 없음 |
| 1절 "도구 호출당 프로세스는 최대 1개" | 하위 에이전트의 도구 호출도 부모의 `session_id`로 PreToolUse를 탄다(`agent_id`가 붙는다) | 7.2절에 "하위 에이전트의 호출은 `agent_id`가 있다. v1은 구분하지 않는다"를 적어 둔다 |

## 3. M3 — Orca CLI

| 설계의 자리와 문장 | 확인 결과 | 고칠 내용 |
|---|---|---|
| 9.2절 "실행 파일은 `ORCA_CLI_COMMAND` → `orca-dev` → `orca-ide` → `orca` 순 … 실패해도 다른 실행 파일로 바꾸지 않는다" | **확인됨** | "Windows의 `orca.cmd`는 `orchestration send`·`reply`를 거절하므로 `orca.exe`를 직접 실행한다"를 더한다 |
| 9.2절 "모든 호출에 `--json`을 붙이고 `{"ok":true,"result":…}` 형태를 읽는다" | **확인됨.** `orchestration` 명령은 한 줄 평면 형태다. `terminal list`는 `id`·`_meta`가 붙은 여러 줄 형태다. 오류는 `{"ok":false,"error":{code,message}}`다 | 세 형태를 적는다 |
| 9.2절 "최소 버전 1.4.246" | 확인한 버전은 1.4.256이다. 1.4.246에서 같은지는 **미확인** | 최소 버전을 확인한 버전으로 올릴지 **결정 필요** |
| 9.2절 표 `wait` 행 "`orchestration check --run R`, ack, …" / 10.2절 `Orca::ack` | **고침(Z1).** `ack` 명령이 없다. `check --ack <delivery_id>`가 있지만 옛 호스트용이고, 현재 버전의 소비형 `check`는 스스로 처리됨으로 표시한다 | 아래 3.1 |
| 9.2절 "worker 응답에서 읽는 값은 `dispatchId`, `taskId`, `runId`, `outcome`(또는 `state`가 `succeeded`·`failed`), `stage == "settled"`, `liveness`, `terminal`" | 필드 이름은 **확인됨.** `outcome`은 `worker-list`에만 있고 `worker-show`에는 없다. `worker-show`의 `liveness`는 `"unverifiable (<사유>)"`처럼 괄호가 붙을 수 있다. `stage`에는 `settled` 말고 `released`·`assignment_settled`도 있다 | "정산은 `state`가 `succeeded`·`failed`이거나 `stage`가 `settled`·`released`일 때로 본다. `liveness`는 앞 단어로 비교한다"로 바꾼다. 정확한 조합은 **미확인** |
| 9.2절 "메시지에서 읽는 값은 `id`, `type`, `body`, `payload.taskId`, `payload.dispatchId`, `run_id`, 전달 ID" | **고침.** 평면 응답의 메시지에는 `run_id`가 없다. 전달 ID는 메시지가 아니라 결과의 `deliveryId`이고 옛 호스트에서만 온다. `subject`, `from`, `payload.outcome`이 더 있다 | "`id`, `type`, `from`, `subject`, `body`, `payload.taskId`, `payload.dispatchId`, `payload.outcome`. Run은 호출한 `--run`으로 안다" |
| 8.6절 completion 기록 2번 "`outcome`은 Orca 메시지의 값이고 없으면 `succeeded`다" | 값의 자리는 `payload.outcome`이다. `worker_done`은 `--outcome`이 필수라 보통 있다 | 자리를 적는다 |
| 8.5절 8단계 `worker-start --run <R> --spec "<spec>" --worktree <경로> --agent <CLI> [--model] [--effort] [--terminal <핸들>] [--task <id>] --timeout-ms … --json` | **고침(Z3).** 인자 이름은 모두 있다. 조합이 다르다 | "`--spec-file -`(stdin)로 spec을 준다. `--worktree path:<경로>`. 새 터미널이면 `--agent <CLI> [--model <모델> [--effort <effort>]]`, 있는 터미널이면 `--terminal <핸들>`만 준다. `--task <id>`를 주면 `--spec`은 주지 않는다(그 Task의 spec이 쓰인다)" |
| 8.5절 `dispatch` 인자 `[--task <Orca task id>]` | `--task`와 spec은 함께 줄 수 없으므로, `--task`를 받으면 fullops의 spec 문자열이 worker에게 가지 않는다 | **결정 필요.** `--task`를 빼거나, `--task`일 때는 spec을 `task-create` 단계에서 넣게 한다 |
| 8.5절 9단계 "Orca 결과에 dispatch ID가 없으면 `id`를 `"pending:<run>:<started_at>"`으로 쓰고 …" | 영수증에는 `dispatchId`·`taskId`·`runId`가 항상 있다(코드상). `terminal`은 채팅 방식 worker면 없을 수 있다 | 대체 경로는 남겨도 되지만 주 경로가 아니다. "`terminal`이 없을 수 있다"를 적는다 |
| 8.5절 8단계 "실패하면 종료 코드 4이고 상태를 쓰지 않는다" / "다시 실행해도 안전하다" | 실패한 시작도 Task·Dispatch·터미널을 남길 수 있다(`residualResources`). Orca 안내서는 "실패하면 다시 실행하지 말라"고 한다. `state`가 `outcome_unknown`이고 종료 코드 0인 경우(턴 시작 미관측)는 **성공으로 다뤄야 한다**(다시 보내면 일이 두 번 간다) | "종료 코드 0이면 `state`와 무관하게 상태를 쓴다. 실패면 영수증의 `dispatchId`·`failedStage`·`next`를 그대로 보여 주고 종료 코드 4. 결과를 모르게 된 호출은 `--retry-request <id>`로 같은 요청을 다시 보낸다" |
| 8.5절 1단계 "없으면 Orca `run-create`로 만든다" / 10.2절 `fn run_create(&self)` | **고침(Z2).** `--objective <text>`가 필수다 | `run_create(objective: &str)`. 값은 과제 키나 과제 목표로 한다 |
| 8.5절 `--terminal <핸들>`로 있는 터미널에 다시 배정, T-03의 탈출구 "`fullops dispatch <KEY> --terminal <핸들>`" | 끝난 worker의 터미널은 `worker_done` 약 2분 뒤 자동으로 놓인다. 그 뒤의 `--terminal` 재사용은 거절된다. Orca가 권하는 후속 지시는 `orchestration send --to dispatch:<id>`다(끝난 worker는 후속 Task로 받는다) | 8.5절에 재사용 창(약 2분)을 적는다. 필요하면 `worker-start --keep-open`을 쓴다. 후속 지시를 `send --to dispatch:<id>`로 바꿀지는 **결정 필요** |
| 8.5절 D-06 "`--terminal`이 구현 dispatch의 터미널과 같다", 9.2절 `review cleanup`의 `terminal list --worktree path:<경로>`, 7.3절 2번 | worker가 채팅 방식으로 뜨면 터미널이 없다(핸들이 `structworker_…`). 뜨는 방식은 사용자 설정을 따르고 플래그가 없다 | 세 곳에 "터미널이 없는 worker에는 적용하지 않는다(모름 = 통과)"를 적는다 |
| 8.6절 4단계 "`orchestration check --run <R> --json`을 기다림 방식으로 부른다(인자는 미정-M3)" | 기다림은 `--wait --timeout-ms <n>`이다. 15초마다 stderr에 keepalive 줄이 나온다 | 아래 3.1 |
| 8.6절 4단계 "'no longer bound' 오류면 `orchestration run-use --id <R>` 뒤 한 번 다시" | `run-use --id <run_id>`는 **확인됨.** 오류 문구는 **미확인** | 문구 대신 `error.code`로 가르게 한다(코드 값은 실환경에서 확인) |
| 8.6절 7단계 "`worker-list --run <R>`로 생존을 확인 … liveness가 `exited`" | **확인됨.** 다만 Orca 안내서는 침묵·시간 초과·`unverifiable`만으로 종료를 단정하지 말라고 한다 | `exited`일 때만 `lost`로 쓴다는 지금 문장을 유지한다 |
| 9.2절 표 X-03 `orchestration worker-show --dispatch <ID>` | **확인됨** | 없음 |
| 9.2절 표 T-11 `orchestration check [--run R] --peek` | **확인됨.** lead가 소비형 check를 치기 전에 hook이 미리 보는 구조는 현재 Orca의 모델과 맞는다 | 없음 |
| 9.2절 표 F-01 `orchestration send --type escalation …`, 7.5절 F-01의 명령 | 인자는 **확인됨.** Orca도 worker가 `worker_done` 없이 멈추면 스스로 `stopReason`이 든 escalation을 보낸다 | F-01에 "Orca의 자체 escalation과 겹칠 수 있다. `[fullops:limit]` 제목으로 구분한다"를 적는다 |
| 7.4절 8번 Orca 하위 명령 분류(`worker-start`, `task-create --spec`, `dispatch --inject`, `terminal send --text`, `check --peek --run`) | 인자 이름은 모두 **확인됨.** `task-create`와 `worker-start`에는 `--spec-file <path\|->`도 있다 | T-04에 "`--spec-file`이면 spec을 알 수 없으므로 `Unknown`(통과)"을 적는다 |
| 16절 M3 "preamble에는 `--dispatch-id <값>`이 있다" | **확인됨.** `--task-id <값> --dispatch-id <값>`이 escalation 줄과 worker_done 줄에 있다. 값은 `task_<16진수 12자>`, `ctx_<16진수 12자>`다 | 6.2.1절의 예시 ID(`dsp_01`)는 그대로 둬도 된다(형식에 기대지 않는다) |
| 7.5절 P-01 "프롬프트에 `--dispatch-id` 뒤 공백 또는 `=`와 …" | **확인됨.** 같은 세션에 새 dispatch의 preamble이 다시 올 수 있다는 점도 P-01의 "다르면 다시 묶는다"와 맞는다 | 없음 |
| 16절 M3 "worker 셸에 `ORCA_TERMINAL_HANDLE`이 있는가" | 터미널 방식의 Claude Code 세션에는 있다(관측). 채팅 방식 세션과 Codex(환경 정책에 따라)에서는 없을 수 있다 | 7.3절 2번에 "없으면 이 단계를 건너뛴다"가 이미 함축돼 있다. 문장으로 적는다 |
| 8.5절 D-04·1단계 "dispatch 받은 세션은 항상 자기 소유 Run을 쓴다" | **확인됨.** Orca도 worker가 자기 Run이나 조상 Run을 묶지 못하게 한다. 하위 배정 깊이에는 상한이 있다 | 없음 |
| 8.6절 5단계 메시지 종류 `heartbeat`·`status`·`worker_done`·`question`·`escalation` | 종류가 더 있다: `dispatch`, `merge_ready`, `handoff`, `decision_gate` | 5단계에 "그 밖의 종류는 처리할 메시지로 모은다"를 더한다 |

### 3.1 `wait`와 ack (Z1)의 고칠 방향

현재 Orca의 사실은 넷이다. (1) 소비형 `check`는 묶음을 돌려주는 순간 처리됨으로 표시한다. (2) `--peek`는 표시하지 않는다. (3) 돌려주는 묶음은 종류로 나뉘지 않는다. `--types`는 `--wait`를 깨우는 조건일 뿐이다. (4) 조율자가 턴을 끝내면 Orca가 터미널에 "You have N orchestration messages. Run `orca orchestration check --run R`"을 넣어 깨운다. 스크립트가 소비형 `check`를 하면 그 알림이 사라진다.

설계 8.6절의 문장별로 보면 다음과 같다.

| 설계 문장 | 맞지 않는 점 | 가장 가까운 대안 |
|---|---|---|
| 2단계 "`--ack` 값마다 … Orca에 ack한다" | 현재 버전에는 ack할 전달 ID가 오지 않는다 | `wait --ack`의 값은 **메시지 ID**로 한다. `state.json`의 `acked_at`을 쓴 뒤, Orca 쪽은 소비형 `orchestration check --run R`을 한 번 불러 처리됨으로 표시한다(옛 호스트가 `deliveryId`를 주면 `--ack`로 넘긴다) |
| 4단계 "`check --run <R> --json`을 기다림 방식으로" | 소비형이면 반환과 동시에 처리됨이 된다. "ack하지 않고 반환"이 성립하지 않는다 | `check --run R --peek --wait --types "worker_done,question,escalation" --timeout-ms <n> --json`. heartbeat·status로는 깨지 않는다 |
| 5단계 "`heartbeat`·`status`: 세고 ack한다" | 종류별로 골라 ack할 수 없다 | 세기만 한다. 처리됨 표시는 2단계의 소비형 check 때 묶음째로 된다 |
| 6단계 "처리할 메시지가 하나라도 있으면 ack하지 않고 즉시 반환" | `--peek`면 그대로 성립한다 | 유지 |
| 반환 JSON의 `"delivery": "dlv_8"`, `state.json`의 `messages[].delivery` | 현재 버전에는 값이 없다 | 필드를 선택 값으로 둔다(없으면 `null`). `next` 문구는 `--ack <메시지 ID>`로 쓴다 |
| X-05의 탈출구 "`fullops wait --run <R> --ack <delivery>`" | 위와 같다 | `--ack <메시지 ID>` |

`fullops wait`를 백그라운드로 걸어 두는 설계(X-05 (b))와 Orca의 자체 알림은 함께 온다. lead는 `wait`의 완료 알림과 Orca의 "You have …" 입력을 둘 다 받을 수 있다. 둘 가운데 하나만 쓸지(예: Orca의 알림에 맡기고 `wait`는 Codex처럼 깨워 주지 않는 경우에만 쓴다)는 **결정 필요**다. 어느 쪽이든 T-11이 소비형 check 직전에 미리 보고 기록하므로 completion은 빠지지 않는다.

## 4. M4 — Orca 패널

| 설계의 자리와 문장 | 확인 결과 | 고칠 내용 |
|---|---|---|
| 16절 M4 "지금의 가정: `status.json` 파일을 읽는다. 안 되면 `fullops status --json`을 부른다" | 패널(`sandboxed-ui`)만으로는 파일도 프로세스도 쓸 수 없다. Node worker가 딸린 플러그인(`trusted-worker`, `main`)이어야 하고, 패널은 `plugin.request`로 worker에게 묻는다. worker가 `fs`·`child_process`를 쓸 수 있다는 것은 문서의 "Node"에서 미루어 본 것이다(**추정**) | "패널은 `trusted-worker` 플러그인으로 만든다. worker가 `status.json`을 읽어 패널에 준다. 워크스페이스 경로는 `host.call('workspace.list')`로 얻는다. 설치할 때 사용자의 코드 실행 승인이 필요하다" |
| 18.2절 묶음 H "`orca plugin validate`·`check` 통과" | 두 명령이 있다(**확인됨**) | 없음 |

## 5. M5 — 세션을 깨우는 입력

| 설계의 자리와 문장 | 확인 결과 | 고칠 내용 |
|---|---|---|
| 7.5절 P-02 "프롬프트가 `You have`로 시작하고 `orchestration message`를 담으면 `machine:orca-message`" | 문구는 **확인됨**: `You have <N> orchestration message(s). Run …`과 `You have <N> new orchestration message(s) after delivery …`. 다만 Orca가 넣는 글은 **줄바꿈으로 시작한다** | "앞 공백을 뗀 글이 `You have`로 시작하고"로 바꾼다 |
| 같은 문장 | 채팅 방식 세션에는 메시지가 통째로 온다. 첫 줄이 `──── From: … (<type>) ────`이고 뒤에 `Subject:`·본문·`[Payload: …]`·`[Reply: … orchestration reply --id …]`가 온다(코드에서 읽음). 지금 패턴에 맞지 않는다 | 패턴 하나를 더한다: "앞 공백을 뗀 글이 `──── From:`으로 시작한다". 실제 글은 **미확인** |
| 7.5절 P-02 "`<task-notification>`을 담으면 `machine:task-notification`" | **미확인.** 공식 문서에 이 문자열이 없다. 2.1.323 실행 파일에는 태그 이름이 있다. hook의 `prompt`에 실리는지 모른다 | 패턴을 유지하고 실환경 점검에서 분류 결과를 본다 |
| 7.5절 P-00 "항상: 세션의 `turn += 1`" / P-02 "그 밖은 `human`" | Claude Code의 UserPromptSubmit은 예약 작업 실행, 백그라운드 하위 에이전트의 보고, 다른 세션이 보낸 메시지에도 불린다(사실). 이것들은 지금 `human`으로 세어진다 | 6.7절 지표에 "사람 턴이 부풀 수 있는 경우"로 적는다. 패턴을 더할지는 실제 글을 본 뒤 정한다 |
| 16절 M5 "묶음 0이 실제 세션에서 채집" | **하지 못했다.** 이 묶음은 세션을 띄울 수 없고 실제 프롬프트 본문을 복사하지 않기로 했다. 표본은 Orca 코드의 문구로 만들었다 | 실환경 점검(12.4절)에 "`turns.jsonl`의 `kind` 분포를 본다"를 넣는다 |
| 7.5절 P-01, worker가 받는 배정 프롬프트 | 프롬프트는 안내 줄 `Please carry out this task from my Orca coordinator by following the brief I pasted below.`로 시작하고, Claude Code에서는 preamble이 `<pasted_content id="…">` 줄 사이에 온다 | P-01은 글 안을 찾으므로 그대로 맞는다. P-02는 lead에게만 적용되므로 영향이 없다 |

## 6. 확인된 것의 요약

다음 가정은 고칠 것 없이 맞았다.

- hook 입력은 stdin의 JSON 객체 하나이고 필드는 snake_case이며 `hook_event_name`으로 이벤트를 가른다(두 호스트).
- 읽는 입력 필드 `session_id`, `cwd`, `source`, `prompt`, `tool_name`, `tool_input`은 두 호스트에 같은 이름으로 있다.
- 판정 출력 다섯 형태(`{}`, `additionalContext`, `systemMessage`, `permissionDecision: "deny"`, `decision: "block"`)는 두 호스트가 모두 처리한다.
- 등록 타임아웃은 초 단위 `timeout`이다.
- Codex에는 StopFailure가 없고 UserPromptSubmit이 있다.
- `apply_patch`의 본문은 `tool_input.command`에 오고 문법의 다섯 표식이 맞다.
- Orca의 실행 파일 선택 순서, `--json`의 `{ok, result}` 봉투, `worker-show --dispatch`, `worker-list --run`, `check --peek --run`, `run-use --id`, `send --type escalation --dispatch-id --task-id`, `terminal list --worktree path:<경로>`, `terminal send --text`, `dispatch --inject`.
- preamble의 `--dispatch-id <값>`·`--task-id <값>`.
- 터미널 방식 Claude Code 세션의 `ORCA_TERMINAL_HANDLE`.
