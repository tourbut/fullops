# Codex hook 계약 (미정-M1·M2 가운데 Codex 부분)

- 확인한 날: 2026-11-13. 이 PC의 Codex는 `codex-cli 0.163.0`이다(`codex --version`).
- 확인 방법: 공식 문서와, 같은 버전 태그(`rust-v0.163.0`)의 공개 소스를 읽었다. 세션을 띄워 hook을 실제로 돌려 보지는 않았다.
- 표기: **사실**은 출처가 있는 것, **추정**은 미루어 본 것, **미확인**은 확인하지 못한 것이다.

| 약어 | 출처 |
|---|---|
| D | https://learn.chatgpt.com/docs/hooks.md (developers.openai.com/codex/hooks 가 이곳으로 넘어간다) |
| S | https://github.com/openai/codex 의 `rust-v0.163.0` 태그. 파일 경로는 `codex-rs/` 아래다 |
| CFG | 이 PC의 `~/.codex/config.toml`·`~/.codex/hooks.json`(읽기만 했다) |

## 1. hook 명령을 실행하는 셸 (M1)

### 1.1 사실

1. **Windows에서는 `cmd.exe`로 실행한다.** `%COMSPEC%`(없으면 `cmd.exe`)에 `/C "<명령 문자열>"`을 준다. 그 밖의 OS에서는 `$SHELL`(없으면 `/bin/sh`)에 `-lc <명령 문자열>`을 준다(S `hooks/src/engine/command_runner.rs`의 `build_command`·`default_shell_program`). 문서(D)는 셸을 밝히지 않는다.
2. Windows 전용 문자열을 따로 줄 수 있다. handler의 `commandWindows`(TOML은 `command_windows`도 된다)가 있으면 Windows에서는 그것을 `command` 대신 쓴다(D "Config shape", S `config/src/hook_config.rs`, `hooks/src/engine/discovery.rs`).
3. 플러그인의 hook에는 환경 변수 `PLUGIN_ROOT`, `PLUGIN_DATA`와 호환용 `CLAUDE_PLUGIN_ROOT`, `CLAUDE_PLUGIN_DATA`가 주어진다(D "Plugin-bundled hooks"). 같은 네 이름의 `${이름}` 표기는 명령 문자열 안에서 값으로 치환된다(S `discovery.rs`: `command.replace("${KEY}", value)`).
4. 작업 디렉터리는 세션의 `cwd`다. 환경은 세션이 잡아 둔 환경에 hook용 값을 덮은 것이다(S `command_runner.rs`).
5. 플러그인은 hook을 기본으로 `<플러그인 루트>/hooks/hooks.json`에서 읽는다. `.codex-plugin/plugin.json`의 `hooks` 항목(`./`로 시작하는 경로, 경로 배열, 객체, 객체 배열)이 있으면 그것이 기본 파일을 대신한다(D).
6. **관리 대상이 아닌 hook은 사용자가 검토해 신뢰한 뒤에만 실행된다.** 신뢰는 hook 정의의 해시에 묶인다. 새 hook과 바뀐 hook은 검토 전까지 건너뛴다. CLI에서는 `/hooks`로 검토한다. 플러그인 갱신으로 묶여 있는 스크립트가 바뀌면 정의가 같아도 다시 검토해야 한다(D "Review and trust hooks").
7. hook은 기본으로 켜져 있다. `[features] hooks = false`로 끈다(D).

**관측**(CFG): 이 PC의 `config.toml`에는 `[hooks.state."fullops-squad@fullops-squad:hooks/hooks.json:pre_tool_use:0:0"]` 꼴의 표가 hook마다 있고 값은 `trusted_hash = "sha256:…"`다. 키는 `<플러그인 ID>:<hook 파일 상대 경로>:<이벤트>:<묶음 번호>:<handler 번호>`다. 1.24.0은 Codex에서도 `hooks/hooks.json`을 쓰고 있다.

### 1.2 설계 10.4절의 문자열에 대한 판단

설계의 Codex 문자열 `exec "$HOME/.fullops/bin/fullops" hook codex`는 **Windows에서 동작하지 않는다.** `cmd.exe`에는 `exec`가 없고 `$HOME`을 풀지 않는다. macOS·Linux에서는 `$SHELL -lc`로 실행되므로 동작한다.

고친 형태는 다음과 같다. 플러그인 루트 변수가 있으므로 설계가 미뤄 둔 "마켓플레이스판과 같은 폴백"도 넣을 수 있다.

```json
{ "type": "command", "timeout": 8,
  "command": "B=\"$HOME/.fullops/bin/fullops\"; [ -x \"$B\" ] || B=\"${PLUGIN_ROOT}/bin/fullops\"; exec \"$B\" hook codex --plugin-root \"${PLUGIN_ROOT}\"",
  "commandWindows": "if exist \"%USERPROFILE%\\.fullops\\bin\\fullops.exe\" (\"%USERPROFILE%\\.fullops\\bin\\fullops.exe\" hook codex --plugin-root \"${PLUGIN_ROOT}\") else (\"${PLUGIN_ROOT}\\bin\\fullops.exe\" hook codex --plugin-root \"${PLUGIN_ROOT}\")" }
```

- **추정**: `cmd.exe /C "<줄>"`은 바깥 따옴표 한 쌍을 벗기고 나머지를 실행하므로 위 `commandWindows`가 그대로 돈다. 소스에서 읽은 것이고 실행해 보지는 않았다.
- **추정**: 다른 OS에서는 사용자의 로그인 셸(`$SHELL -lc`)로 실행되므로 `B=…; [ -x … ] || …` 같은 POSIX 문법은 fish 같은 셸에서 실패할 수 있다. `exec "$HOME/.fullops/bin/fullops" hook codex` 한 줄은 bash·zsh·fish에서 모두 유효하다. 로그인 셸이 시작할 때 stdout에 글을 내면 hook 출력이 JSON으로 읽히지 않는다.
- Codex 등록 파일의 이름을 설계대로 `hooks/codex.json`으로 두려면 `.codex-plugin/plugin.json`에 `"hooks": "./hooks/codex.json"`을 적어야 한다. 적지 않으면 Codex는 `hooks/hooks.json`만 읽는다.
- hook 신뢰 절차(1.1의 6) 때문에 설치 직후에는 hook이 돌지 않는다. 사용자가 `/hooks`에서 한 번 신뢰해야 한다. 등록 문자열이 바뀌는 갱신 뒤에도 그렇다.

### 1.3 제한 시간

**사실**(D): handler의 `timeout`은 **초** 단위이고 기본값은 600초다(SessionEnd·Interrupt는 1초, 최대 3초). 시간이 넘으면 프로세스 트리를 끝내고 그 hook 실행을 `timeout`으로 기록한다(S). PreToolUse의 오류·시간 초과·잘못된 응답은 도구를 막지 않는다(D "Managed hooks").

## 2. 이벤트와 matcher (M2)

**사실**(D):

- 이벤트: `SessionStart`, `SessionEnd`, `SubagentStart`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, `PreModelRequest`, `PostModelRequest`, `PreCompact`, `PostCompact`, `UserPromptSubmit`, `SubagentStop`, `Stop`, `Interrupt`.
- **`UserPromptSubmit`이 있다.** `StopFailure`는 없다.
- matcher는 정규식 문자열이다. `"*"`·`""`·생략은 전부에 걸린다. `UserPromptSubmit`과 `Stop`은 matcher를 무시한다. `SessionStart`의 matcher는 `source`에 걸린다.
- handler 종류는 `command`와 `mcp_tool`만 실행된다. `prompt`·`agent`는 읽기만 하고 건너뛴다.

PreToolUse에서 hook이 받는 **도구 이름**은 다음과 같다(D "Tool coverage", S `core/src/tools/hook_names.rs`).

| 실제 도구 | 입력의 `tool_name` | matcher에서 더 통하는 이름 |
|---|---|---|
| 셸 명령, unified exec(`exec_command`) | **`Bash`** | - |
| `apply_patch` | `apply_patch` | `Write`, `Edit` |
| `spawn_agent` | `spawn_agent` | `Agent` |
| MCP 도구 | `mcp__<서버>__<도구>` | - |
| 그 밖의 로컬 함수 도구 | 그 함수 이름(예: `update_plan`) | - |
| 호스팅 도구(`WebSearch` 등) | hook을 타지 않는다 | - |

`shell`, `exec_command`, `local_shell`이라는 이름은 hook 입력에 오지 않는다. 설계 7.2절의 Codex 매처는 `Bash|apply_patch|spawn_agent`여야 한다. `write_stdin`은 이미 통과한 명령의 통로라 PreToolUse를 다시 타지 않는다.

## 3. 입력 (M2)

**사실**(D "Common input fields", S `hooks/schema/generated/*.command.input.schema.json`):

| 필드 | 이벤트 | 설명 |
|---|---|---|
| `session_id` | 전부 | 세션 ID. 하위 에이전트의 hook은 부모 세션 ID를 쓴다 |
| `transcript_path` | 전부 | 문자열 또는 `null` |
| `cwd` | 전부 | 세션의 작업 디렉터리 |
| `hook_event_name` | 전부 | 이벤트 이름 |
| `model` | 전부 | 현재 모델 이름 |
| `permission_mode` | SessionStart, UserPromptSubmit, PreToolUse, Stop 등 | `default`·`acceptEdits`·`plan`·`dontAsk`·`bypassPermissions` |
| `turn_id` | 턴 범위 이벤트(UserPromptSubmit, PreToolUse, Stop) | Codex 확장 |
| `source` | SessionStart | `startup`·`resume`·`clear`·`compact`·`fork` |
| `prompt` | UserPromptSubmit | 보내려는 사용자 프롬프트 |
| `tool_name`, `tool_use_id`, `tool_input` | PreToolUse | `Bash`와 `apply_patch`는 `tool_input.command`(문자열)를 쓴다. 그 밖의 도구는 인자 객체를 그대로 준다 |
| `stop_hook_active`, `last_assistant_message` | Stop | `stop_hook_active`는 이 턴이 이미 Stop hook 때문에 이어졌는지다. `last_assistant_message`는 문자열 또는 `null` |
| `agent_id`, `agent_type` | UserPromptSubmit·PreToolUse(선택) | 하위 에이전트 안에서 불렸을 때 |

Stop 입력에 `reason` 필드는 없다. 필드는 snake_case다.

### 3.1 `apply_patch`의 본문

**사실**(S `apply-patch/src/parser.rs`의 문법 주석과 상수): 본문은 `tool_input.command`에 문자열로 온다. 문법은 다음과 같다.

```
start: begin_patch environment_id? hunk+ end_patch
begin_patch: "*** Begin Patch" LF
environment_id: "*** Environment ID: " filename LF
end_patch: "*** End Patch" LF?
hunk: add_hunk | delete_hunk | update_hunk
add_hunk: "*** Add File: " filename LF add_line+
delete_hunk: "*** Delete File: " filename LF
update_hunk: "*** Update File: " filename LF change_move? change?
add_line: "+" /(.+)/ LF
change_move: "*** Move to: " filename LF
change: (change_context | change_line)+ eof_line?
change_context: ("@@" | "@@ " /(.+)/) LF
change_line: ("+" | "-" | " ") /(.+)/ LF
eof_line: "*** End of File" LF
```

- 파서는 표식 앞뒤의 공백을 봐주고(lenient), 엄격 모드는 꺼져 있다.
- 설계 7.4절이 가정한 `*** Add File:`·`*** Delete File:`·`*** Update File:`·`*** Move to:`·`@@`는 모두 맞다. 설계에 없는 것은 바깥 봉투(`*** Begin Patch`·`*** End Patch`), `*** Environment ID:` 줄, `*** End of File` 줄, 글이 붙은 `@@ <문맥>` 머리줄이다.
- **추정**: 경로는 상대 경로로 올 수 있다(문법이 `filename`이라고만 한다). 세션 `cwd` 기준으로 풀어야 한다.

표본은 `scenarios/fixtures/host/codex/pre_tool_use.apply_patch_add.json`과 `…apply_patch_update.json`이다.

### 3.2 `spawn_agent`의 인자

**사실**(S `core/src/tools/handlers/multi_agents_spec.rs`): 인자에 `message`가 있다. `task_name`, `agent_type`, `items`도 있다. 설계가 읽는 `prompt`는 이 버전의 스키마에서 찾지 못했다(**미확인**).

## 4. 출력 (M2)

**사실**(D, S `hooks/schema/generated/*.command.output.schema.json`, `hooks/src/engine/output_parser.rs`):

| 설계의 Verdict | 설계의 출력 | Codex의 처리 |
|---|---|---|
| `Pass` | `{}` | 성공으로 보고 계속한다. Stop은 종료 코드 0일 때 JSON을 기대하므로 빈 출력보다 `{}`가 안전하다(평문은 Stop에서 잘못된 출력이다) |
| `Context` | `{"hookSpecificOutput":{"hookEventName":"SessionStart"\|"UserPromptSubmit","additionalContext":t}}` | 개발자 문맥으로 더한다 |
| `Warn` | `{"systemMessage":t}` | UI나 이벤트 흐름에 경고로 보인다. SessionStart·UserPromptSubmit·Stop·PreToolUse 모두 받는다 |
| `Deny` | `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":r}}` | 도구 호출을 막는다. 옛 형태 `{"decision":"block","reason":r}`도 받는다 |
| `Block` | `{"decision":"block","reason":r}` | 턴을 끝내지 않고, `reason`을 새 사용자 프롬프트처럼 써서 이어 간다. `reason`이 비면 차단을 무시한다 |

설계가 내는 다섯 형태는 모두 Codex가 지원한다. 주의할 점은 다음과 같다.

- 출력 스키마는 `additionalProperties: false`다. 스키마에 없는 최상위 필드를 넣으면 출력 전체가 잘못된 것으로 처리될 수 있다(**추정**: 스키마에서 읽었다).
- PreToolUse에서 `continue`, `stopReason`, `suppressOutput`, `permissionDecision: "ask"`, 옛 `decision: "approve"`를 내면 그 hook 실행을 실패로 표시하고 도구 호출을 계속한다. 설계는 이 값들을 내지 않는다.
- 종료 코드 2도 막는 신호다(PreToolUse·UserPromptSubmit·Stop). 설계는 쓰지 않는다.
- 평문 stdout은 SessionStart와 UserPromptSubmit에서 개발자 문맥으로 더해지고, PreToolUse에서는 무시되며, Stop에서는 잘못된 출력이다.
- hook이 모델에 보내는 글은 기본 약 2,500 토큰이 한도다. 넘으면 파일로 저장하고 미리 보기만 보낸다. handler의 `additionalContextLimit`으로 바꾼다.

### 4.1 Stop을 막은 뒤의 흐름

**사실**(S `core/src/session/turn.rs`): Stop hook이 막으면 `reason`으로 만든 메시지를 기록하고 **같은 턴 안에서** 다음 모델 요청으로 넘어간다. 이때 `stop_hook_active`가 `true`가 된다. 이 이어 가기는 사용자 입력 경로(`inspect_pending_input`, 곧 UserPromptSubmit hook)를 거치지 않는다. 그래서 설계 7.6절의 "턴 기준 한 번 막기"는 Codex에서 뜻대로 동작한다: 막은 뒤 다시 오는 Stop은 같은 턴이다.

## 5. 설계의 다른 가정

| 설계의 가정 | 확인 결과 |
|---|---|
| 7.6절 "Codex가 `stop_hook_active`를 주는지 확인되지 않았다" | 준다(필수 필드). 설계의 턴 기준 방식은 그대로 둬도 된다 |
| 7.6절 "UserPromptSubmit이 오지 않는 호스트" | Codex는 UserPromptSubmit을 보낸다 |
| 7.2절 "방언은 OS로 정한다(Windows는 PowerShell)" | **미확인**. `tool_input.command`가 Windows에서 어떤 셸 문법의 문자열인지 문서와 읽은 소스에서 확인하지 못했다 |
| 7.2절 `caps`: Codex는 `stop_failure = false`, `background_wake = false` | StopFailure는 없다(사실). 백그라운드 작업이 끝났을 때 세션을 깨우는 기능은 확인하지 못했다(미확인) |

## 6. 확인하지 못한 것

| 항목 | 이유 | 확인하는 방법 |
|---|---|---|
| `commandWindows` 문자열이 `cmd.exe`에서 실제로 도는가 | 세션을 띄울 수 없었다 | 실환경 점검에서 한 번 등록해 본다 |
| Windows에서 `Bash` 도구의 `command`가 PowerShell 문법인가 | 문서에 없다 | `FULLOPS_TRACE=1`로 Intent 분류 결과를 본다 |
| hook 프로세스에 `ORCA_TERMINAL_HANDLE`이 보이는가 | Orca의 오류 문구는 Codex의 `shell_environment_policy`가 도구 셸에서 `ORCA_*`를 지울 수 있다고 말한다(`orca.md` 5절). hook 프로세스의 환경도 그런지는 모른다 | 실환경에서 세션 파일의 `terminal` 값을 본다 |
| 바이너리만 바뀐 플러그인 갱신이 hook 재검토를 요구하는가 | 문서는 "묶인 스크립트가 바뀌면"이라고만 한다 | 갱신을 한 번 해 본다 |
| `spawn_agent` 인자에 `prompt`가 오는 버전이 있는가 | 0.163.0의 스키마에서 찾지 못했다 | 해당 없음(`message`를 읽으면 된다) |
