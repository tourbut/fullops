# Claude Code hook 계약 (미정-M1·M2·M5 가운데 Claude Code 부분)

- 확인한 날: 2026-11-13. 이 PC의 Claude Code는 `2.1.323`이다(`claude --version`).
- 확인 방법: 공식 문서를 읽었다. 새 세션을 띄워 hook을 실제로 돌려 보지는 않았다(이 묶음에 금지된 일이다). 그래서 아래의 "사실"은 "문서가 그렇게 말한다"는 뜻이고, 실행으로 확인한 것은 따로 "관측"이라고 적었다.
- 표기: **사실**은 출처가 있는 것, **추정**은 출처에서 미루어 본 것, **미확인**은 확인하지 못한 것이다.

출처 약어는 다음과 같다.

| 약어 | 출처 |
|---|---|
| H | https://code.claude.com/docs/en/hooks.md (hooks reference) |
| PR | https://code.claude.com/docs/en/plugins-reference.md |
| PL | https://code.claude.com/docs/en/plugins/loading.md |
| PC | https://code.claude.com/docs/en/plugins/components.md |
| TR | https://code.claude.com/docs/en/tools-reference.md |
| BIN | 이 PC의 `~/.local/bin/claude.exe` 2.1.323 안의 문자열(문서에 없는 것을 확인할 때만 썼다) |
| P124 | 이 PC에 설치된 1.24.0의 `~/.claude/plugins/cache/fullops-squad/fullops-squad/1.24.0/hooks/hooks.json` |

## 1. hook 명령을 실행하는 셸 (M1)

### 1.1 사실

1. command hook은 두 가지 형태가 있다(H "Exec form and shell form").
   - **shell form**: `args`가 없을 때다. `command` 문자열을 셸에 넘긴다. macOS·Linux는 `sh -c`, Windows는 Git Bash, Git Bash가 설치돼 있지 않으면 PowerShell이다. `shell` 필드(`"bash"` 또는 `"powershell"`)로 직접 고를 수 있다.
   - **exec form**: `args`가 있을 때다. `command`를 PATH에서 실행 파일로 찾아 셸 없이 직접 띄운다. `args`의 각 원소가 그대로 인자 하나가 된다. `$`·작은따옴표·백틱은 해석되지 않고 글자 그대로 넘어간다. `shell` 필드는 무시된다.
2. 경로 자리표시자 `${CLAUDE_PROJECT_DIR}`, `${CLAUDE_PLUGIN_ROOT}`, `${CLAUDE_PLUGIN_DATA}`는 두 형태 모두에서 `command`(exec form은 `args`의 각 원소도)에 문자열로 치환된다. 같은 이름의 환경 변수도 hook 프로세스에 주어진다(H, PR "Environment variables").
3. `$HOME`은 Claude Code가 치환하지 않는다. shell form에서 셸이 풀 뿐이다. 따라서 exec form에서는 풀리지 않는다(H: "`$` … pass through verbatim because there is no shell").
4. PowerShell shell form에서는 `${CLAUDE_PLUGIN_ROOT}` 표기를 Claude Code가 `${env:CLAUDE_PLUGIN_ROOT}`로 바꿔 준다. 따옴표로만 이루어진 명령(`"C:\\Tools\\guard.exe"`)은 PowerShell이 실행하지 않고 출력만 한다. 실행하려면 앞에 `&`를 붙여야 한다(H "PowerShell started no program").
5. Windows의 exec form은 `command`가 `.exe` 같은 진짜 실행 파일로 풀려야 한다. `.cmd`·`.bat`는 셸 없이 띄울 수 없다(H).
6. **Git Bash가 없는 Windows에서는 실패가 차단이 될 수 있다.** shell form의 PreToolUse·PermissionRequest hook이 `onFailure` 없이 실패하거나 시간이 넘으면 도구 호출을 막는다. 이때 종료 코드 0과 2 밖의 값은 JSON을 출력했어도 실패로 친다(H "Failed hooks on Windows without Git Bash"). `"onFailure": "continue"`를 주면 실패해도 진행한다. `onFailure`는 v2.1.295 이상에서 쓴다.
7. hook은 현재 디렉터리에서, Claude Code의 환경을 물려받아 실행된다. 현재 디렉터리가 없어졌으면 세션 시작 디렉터리 → 프로젝트 루트 → 홈 → 임시 디렉터리 순으로 대신 쓴다(H "Hook handler fields").
8. 한 이벤트에 걸린 hook들은 병렬로 실행된다. 같은 handler를 여러 설정 파일에 적으면 한 번만 돈다. 플러그인의 사본은 따로 돈다(H).
9. 플러그인이 갱신되거나 제거되면 이전 버전 폴더에 `.orphaned_at` 표식이 생기고 14일 뒤 지워진다. 그동안 이전 버전을 이미 읽은 세션은 계속 돈다. 떠 있는 세션의 hook·MCP는 `/reload-plugins`를 할 때까지 이전 버전의 경로를 쓴다(PL "Cleanup of previous versions").
10. 플러그인 루트의 `bin/` 폴더는 플러그인이 켜져 있는 동안 Bash 도구(샌드박스 밖 PowerShell 도구 포함)의 PATH에 들어간다. 사용자의 PATH 뒤에 붙는다. claude.ai와 Cowork는 이 폴더가 있는 플러그인을 설치하지 않는다(PR, PC).

### 1.2 설계 10.4절의 문자열에 대한 판단

| 판 | 설계의 문자열 | 판단 |
|---|---|---|
| Claude 자체 마켓플레이스판 | `B="$HOME/.fullops/bin/fullops"; [ -x "$B" ] \|\| B="${CLAUDE_PLUGIN_ROOT}/bin/fullops"; exec "$B" hook claude --plugin-root "${CLAUDE_PLUGIN_ROOT}"` | macOS·Linux와 **Git Bash가 있는 Windows**에서는 문서상 동작한다. **Git Bash가 없는 Windows**에서는 PowerShell로 실행돼 실패하고, PreToolUse에서는 그 실패가 도구 호출을 막는다(1.1의 6). 설계의 G2(hook 오류가 작업을 막지 않는다)에 어긋난다 |
| Claude 디렉터리판 | `exec "${CLAUDE_PLUGIN_ROOT}/bin/fullops" hook claude --in-place` | 위와 같다. PowerShell에는 `exec`가 없다 |

고치는 길은 두 가지다. 둘 다 바이너리 쪽 인터페이스(`fullops hook claude [--plugin-root] [--in-place]`)는 그대로다.

**길 A (shell form 유지, 최소 수정).** 문자열은 그대로 두고 handler마다 `"shell": "bash"`와 `"onFailure": "continue"`를 더한다. Git Bash가 없는 Windows에서는 hook이 돌지 않지만 작업을 막지도 않는다. 그 환경에서는 fullops가 조용히 꺼진 상태가 되므로 `doctor`가 알려야 한다.

```json
{ "type": "command", "shell": "bash", "onFailure": "continue", "timeout": 8,
  "command": "B=\"$HOME/.fullops/bin/fullops\"; [ -x \"$B\" ] || B=\"${CLAUDE_PLUGIN_ROOT}/bin/fullops\"; exec \"$B\" hook claude --plugin-root \"${CLAUDE_PLUGIN_ROOT}\"" }
```

**길 B (exec form, 셸에 기대지 않음).** 문서가 경로 자리표시자를 쓰는 hook에 권하는 형태다.

```json
{ "type": "command", "onFailure": "continue", "timeout": 8,
  "command": "${CLAUDE_PLUGIN_ROOT}/bin/fullops",
  "args": ["hook", "claude", "--plugin-root", "${CLAUDE_PLUGIN_ROOT}"] }
```

길 B에는 걸리는 점이 둘 있다.

- exec form에는 `$HOME`도 조건문도 없으므로 "고정 위치 실행본이 있으면 그것을 쓴다"를 문자열에 담을 수 없다. 플러그인 폴더의 실행본이 항상 진입점이 된다. 고정 위치를 우선해야 하면 그 판단을 바이너리가 해야 한다(프로세스가 하나 더 뜬다). 다만 1.1의 9 때문에 Claude Code에서는 고정 위치가 없어도 떠 있는 세션의 hook이 14일 동안 깨지지 않는다. 설계 10.3절이 고정 위치를 두는 이유("호스트가 버전별 캐시를 지워도 떠 있는 세션의 hook이 깨지지 않는다")는 Claude Code에서는 호스트가 이미 해결하고 있다.
- **미확인**: Windows에서 확장자 없는 `…/bin/fullops`가 `fullops.exe`로 풀리는가. 문서는 "`.exe` 같은 진짜 실행 파일로 풀려야 한다"고만 말한다. 같은 폴더에 확장자 없는 POSIX 런처(`bin/fullops`)와 `bin/fullops.exe`가 함께 있을 때 어느 쪽이 잡히는지도 모른다. 실환경 점검에서 한 번 돌려 봐야 한다.

**추정**: 길 A의 Git Bash에서 `[ -x "$HOME/.fullops/bin/fullops" ]`는 `fullops.exe`만 있어도 참이다(MSYS가 `.exe`를 붙여 찾는다). Git Bash의 `$HOME`은 보통 `USERPROFILE`과 같지만 사용자가 `HOME`을 따로 정했으면 다르다. 설계 6.4절은 Windows의 홈을 `USERPROFILE`로 정했으므로 그런 PC에서는 hook 문자열이 보는 폴더와 `fullops install`이 쓰는 폴더가 달라진다.

### 1.3 1.24.0이 실제로 쓰는 문자열 (관측)

P124의 모든 hook은 `python3 -I -X utf8 -c "…Path.home()/'.fullops-squad/runtime/…'…" <인자>` 꼴이다. 셸 문법도 `$HOME`도 쓰지 않고 실행 파일 하나와 인자만으로 이루어져 있다. 그래서 Git Bash·PowerShell·cmd 어디서든 같은 문자열이 돈다. 홈 폴더는 프로세스 안에서 찾는다. 같은 파일이 Codex 쪽 캐시(`~/.codex/plugins/cache/fullops-squad/fullops-squad/1.24.0/hooks/hooks.json`)에도 그대로 들어 있다.

## 2. 설정 형식과 제한 시간 (M2)

**사실**(H "Configuration", "Hook handler fields"):

- 플러그인의 hook은 `hooks/hooks.json`에 둔다. 최상위에 `description`(선택)과 `hooks`가 있다. `hooks.<이벤트>[]`의 원소는 `{ "matcher"?, "hooks": [handler…] }`다.
- handler 공통 필드: `type`(`command`·`http`·`mcp_tool`·`prompt`·`agent`), `if`, `timeout`, `statusMessage`, `once`. command 전용: `command`, `args`, `async`, `asyncRewake`, `shell`, `onFailure`.
- `timeout`의 단위는 **초**다. 기본값은 command가 600초이고, UserPromptSubmit에서는 30초다. SessionEnd hook들은 1.5초를 나눠 쓴다.
- 시간이 넘으면 hook을 취소하고 출력을 버린다. PreToolUse에서 시간이 넘은 hook은 도구 호출을 막지 않는다(`onFailure: "block"`이 아닐 때).
- matcher: 글자·숫자·`_`·`-`·공백·`,`·`|`만 있으면 정확한 이름(또는 `|`·`,`로 나눈 이름 목록)이다. 그 밖의 글자가 있으면 JavaScript 정규식(앵커 없음)이다. UserPromptSubmit과 Stop은 matcher가 없다. SessionStart의 matcher는 `source`, StopFailure의 matcher는 `error`에 걸린다.
- `Bash` matcher는 Monitor 도구가 띄우는 셸 명령에도 걸린다(v2.1.314 이상). 이때 hook은 Bash 호출 형태의 입력을 받는다.

설계 7.1절 표의 "등록 타임아웃"(SessionStart 10, UserPromptSubmit 5, PreToolUse 8, Stop 20, StopFailure 15)은 초 단위 `timeout`으로 그대로 적으면 된다.

## 3. 입력 (M2)

### 3.1 공통 필드

**사실**(H "Common input fields"): 입력은 stdin의 JSON 객체 하나이고 필드는 snake_case다.

| 필드 | 설명 |
|---|---|
| `session_id` | 세션 ID |
| `prompt_id` | 처리 중인 사용자 프롬프트의 UUID. 첫 입력 전에는 없다 |
| `transcript_path` | 대화 기록 파일 경로 |
| `cwd` | hook이 불린 시점의 작업 디렉터리. 워크트리에 들어가거나 `cd` 하면 따라 바뀐다 |
| `scratchpad_dir` | 세션 임시 폴더(v2.1.257 이상). 없을 수 있다 |
| `permission_mode` | `default`·`plan`·`acceptEdits`·`auto`·`dontAsk`·`bypassPermissions` |
| `effort` | `{ "level": … }` |
| `hook_event_name` | 이벤트 이름 |
| `agent_id`, `agent_type` | hook이 하위 에이전트 안에서 불렸거나 `--agent`로 띄웠을 때만 있다 |

하위 에이전트의 도구 호출도 부모와 같은 `session_id`로 PreToolUse를 탄다. 구분하려면 `agent_id`를 봐야 한다(표본 `pre_tool_use.bash_in_subagent.json`).

### 3.2 이벤트별 필드

| 이벤트 | 더해지는 필드 | 비고 |
|---|---|---|
| SessionStart | `source`(`startup`·`resume`·`clear`·`compact`·**`fork`**), `model`(없을 수 있다), `agent_type`, `session_title`. `resume`·`fork`에는 `seconds_since_last_response` 등 네 필드가 더 온다 | `fork`는 기존 세션에서 갈라진 새 세션이다 |
| UserPromptSubmit | `prompt`, `session_title` | 붙여 넣은 글은 `<pasted_content id="…">` 줄과 `</pasted_content id="…">` 줄 사이에 펼쳐져 온다. 사람이 친 입력만이 아니라 예약 작업 실행, 백그라운드 하위 에이전트의 보고, 다른 세션이 보낸 메시지에도 불린다 |
| PreToolUse | `tool_name`, `tool_input`, `tool_use_id`, MCP 도구면 `mcp_server` | 아래 3.3 |
| Stop | `stop_hook_active`, `last_assistant_message`, `background_tasks[]`, `session_crons[]` | **`reason` 필드는 없다.** 사용자가 끊은 경우에는 Stop이 돌지 않는다. API 오류로 끝나면 Stop 대신 StopFailure가 돈다 |
| StopFailure | `error`, `error_details`(선택), `last_assistant_message`(선택) | `error`는 오류 **종류 이름**이다: `rate_limit`·`overloaded`·`authentication_failed`·`oauth_org_not_allowed`·`account_on_hold`·`billing_error`·`invalid_request`·`model_not_found`·`server_error`·`max_output_tokens`·`cloud_credential_error`·`unknown`. 자세한 글은 `error_details`와 `last_assistant_message`에 있다 |

`background_tasks[]`의 원소는 `id`, `type`(`shell`·`subagent`·`monitor`·`workflow`·`teammate` 등), `status`, `description`, `command`(shell일 때) 등을 가진다. 실행 중인 백그라운드 작업이 있는지 Stop hook이 알 수 있다.

### 3.3 PreToolUse의 `tool_input`

**사실**(H "PreToolUse input", TR):

| 도구 | 필드 |
|---|---|
| `Bash` | `command`, `description`, `timeout`(ms), `run_in_background` |
| `PowerShell` | Bash와 같다. 명령은 `command` |
| `Write` | `file_path`, `content` |
| `Edit` | `file_path`, `old_string`, `new_string`, `replace_all` |
| `Agent` | `prompt`, `description`, `subagent_type`, `model` |
| `AskUserQuestion` | **추정**(2.2.2. 문서로 확인하지 못했다): `questions[]`. 원소는 `question`, `header`, `multiSelect`, `options[]`(`label`, `description`) |

- `file_path`는 항상 절대 경로다. **Windows에서는 역슬래시로 온다**(Git Bash에서 hook이 돌아도 그렇다). 슬래시로 쓴 비교는 맞지 않으므로 구분자를 맞춘 뒤 비교해야 한다.
- Windows에서 PowerShell 도구가 켜져 있으면 Claude는 PowerShell을 주 셸로 쓴다. Git Bash가 없는 Windows에서는 Bash 도구가 아예 등록되지 않는다. 셸 명령을 보는 hook은 `Bash|PowerShell`을 함께 걸어야 한다.
- 현재 도구 목록(TR)에 `MultiEdit`와 `Task`(하위 에이전트용)는 없다. 하위 에이전트 도구는 `Agent`다. matcher는 이름 목록이라 없는 이름을 넣어 둬도 해가 없다.

## 4. 출력 (M2)

### 4.1 종료 코드와 stdout

**사실**(H "Exit code output"):

- 종료 코드 0: 성공. stdout이 JSON 객체 하나면 그 필드를 적용한다. `{`로 시작하지 않으면 평문으로 본다. `{`로 시작해 `}`로 끝나는데 JSON이 아니면 해석 실패(막지 않는 오류)다.
- 종료 코드 2: 막는 오류. stderr가 사유다.
- 그 밖: 막지 않는 오류. 다만 stdout이 검증을 통과하는 JSON이면 종료 코드를 무시하고 그 필드를 적용한다.
- SessionStart와 UserPromptSubmit에서는 평문 stdout이 모델의 문맥에 더해진다. 그 밖의 이벤트에서는 디버그 기록에만 남는다.
- stdout에는 JSON 객체 하나만 있어야 한다. 앞뒤에 다른 글이 있으면 JSON으로 읽지 않는다.

설계의 "항상 종료 코드 0, 판정은 stdout JSON으로만"은 이 규칙과 맞는다. `{}`는 아무 필드도 없는 유효한 JSON이라 통과다.

### 4.2 필드

**사실**(H "JSON output", "Decision control"):

| 필드 | 뜻 |
|---|---|
| `continue`, `stopReason` | `continue: false`면 Claude가 완전히 멈춘다 |
| `systemMessage` | **사용자에게** 보이는 경고다. 모델에게 가는 글이 아니다 |
| `suppressOutput` | 받기는 하지만 효과가 없다 |
| `decision: "block"` + `reason` | UserPromptSubmit, Stop 등. Stop에서는 Claude가 멈추지 못하고 `reason`을 받아 계속한다 |
| `hookSpecificOutput.hookEventName` | `hookSpecificOutput`을 쓰면 반드시 넣는다 |
| `hookSpecificOutput.additionalContext` | SessionStart, UserPromptSubmit, PreToolUse, Stop 등. 모델에게 system reminder로 들어간다. **Stop에서는 대화를 계속 이어 가게 만든다**(막는 것과 같은 효과) |
| `hookSpecificOutput.permissionDecision` | PreToolUse. `allow`·`deny`·`ask`·`defer`. 여러 hook이 내면 `deny` > `defer` > `ask` > `allow` |
| `hookSpecificOutput.permissionDecisionReason` | 거부 사유. `deny`일 때 모델에게 전해진다 |

`additionalContext`, `systemMessage`, 평문 stdout은 각각 **10,000자**가 한도다. 넘으면 파일로 저장되고 모델은 파일 경로와 앞 2,000자만 받는다.

설계 7.2절의 출력 표 다섯 줄은 Claude Code에서 그대로 유효하다. 표본은 `scenarios/fixtures/host/output/`에 있다.

### 4.3 Stop을 거듭 막을 때의 상한

**사실**(H "Stop input"): Stop hook이 턴을 여덟 번 연달아 이어 가게 하면, Claude Code가 그다음 차단을 무시하고 턴을 끝낸다. Claude가 도구를 한 번 부르면 횟수가 0으로 돌아간다. 상한은 환경 변수 `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`으로 올릴 수 있다. `stop_hook_active`는 Stop hook 때문에 이미 이어 가는 중이면 `true`다.

### 4.4 StopFailure

**사실**: 출력과 종료 코드를 무시한다(`terminalSequence`만 예외). 알림과 기록 용도다. 설계의 F-01은 효과만 내므로 맞는다.

## 5. 세션을 깨우는 입력 (M5)

| 항목 | 내용 | 구분 |
|---|---|---|
| UserPromptSubmit이 불리는 경우 | 사람이 친 입력, 예약 작업(`/loop` 포함) 실행, 백그라운드 하위 에이전트의 보고, 다른 세션이 보낸 메시지 | 사실(H "UserPromptSubmit") |
| 백그라운드 **셸** 작업이 끝났을 때 UserPromptSubmit이 불리는가 | 문서의 목록에 없다 | 미확인 |
| `<task-notification>` | 공식 문서(hooks, sub-agents, tools-reference, interactive-mode, scheduled-tasks, cross-session-messaging)에 이 문자열이 없다. 2.1.323 실행 파일에는 `task-notification` 태그와 그 안의 `task-id`·`tool-use-id`·`task-type`·`status` 태그 이름이 있다 | 태그 이름은 관측(BIN). 형식과 hook 입력에 실리는지는 미확인 |
| Stop hook이 막아 이어 갈 때 UserPromptSubmit이 불리는가 | 문서의 목록에 없다 | 추정: 불리지 않는다 |
| Orca가 넣는 입력 | `orca.md` 6절 | - |

Orca가 worker에게 넣는 배정 프롬프트는 "첫 줄 안내 + 붙여 넣은 preamble" 꼴이다. Claude Code는 붙여 넣은 글을 `<pasted_content id="…">` 줄 사이에 펼쳐 hook에 준다. 표본은 `scenarios/fixtures/host/claude/user_prompt_submit.orca_dispatch.json`이다.

## 6. 확인하지 못한 것

| 항목 | 이유 | 확인하는 방법 |
|---|---|---|
| exec form에서 Windows가 확장자 없는 `bin/fullops`를 `fullops.exe`로 푸는가 | 세션을 띄울 수 없었다 | 실환경 점검에서 길 B의 hook을 한 번 등록해 본다 |
| Git Bash가 없는 Windows에서 길 A·B가 실제로 어떻게 끝나는가 | 위와 같다 | 위와 같다 |
| `<task-notification>`이 UserPromptSubmit의 `prompt`에 실리는가, 그 글의 형식 | 문서에 없고 실제 세션의 본문은 복사하지 않기로 했다 | `FULLOPS_TRACE=1`로 hook을 돌려 `turns.jsonl`의 분류를 본다(본문은 저장하지 않는다) |
| Stop 차단으로 이어 갈 때 UserPromptSubmit이 정말 안 불리는가 | 문서에 명시가 없다 | X-07 시나리오를 실환경에서 한 번 돌린다 |
| `onFailure`를 모르는 옛 버전(v2.1.295 미만)이 이 필드를 무시하는가 | 문서에 없다 | 최소 지원 버전을 2.1.295로 정하면 확인할 필요가 없다 |
| `AskUserQuestion`의 `tool_input` 모양, 그 도구에 PreToolUse가 불리는가, 카드에 답했을 때 UserPromptSubmit이 불리지 않는가(추정: 도구 결과로 돌아온다) | 2.2.2에서 네트워크와 세션 없이 넣었다 | `FULLOPS_TRACE=1`로 lead 세션에서 카드를 한 번 내고 `fullops decision list --open`과 `turns.jsonl`을 본다. 모양이 다르면 hook은 적지도 막지도 않는다 |
