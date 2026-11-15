# 네 호스트 지원을 위한 설계 차이 (묶음 0b)

- 대상: `docs/design.md`의 1절(빼는 것), 7.1·7.2·7.4·7.5·7.6·7.7절(hook), 10.3·10.4·10.5절(설치·패키지), 17절 H6(grok·agy를 버린다는 결정).
- 근거: `grok.md`(Grok Build 1.0.70), `agy.md`(agy 1.4.8), 그리고 묶음 0의 `claude-code.md`·`codex.md`·`orca.md`·`design-delta.md`. Claude Code와 Codex에 대한 판단은 `design-delta.md`가 정본이고 이 문서는 되풀이하지 않는다.
- 판정 표기: **고침**은 설계 문장을 바꿔야 한다는 뜻이다. **결정 필요**는 어느 쪽으로 고칠지 사람이나 A 담당이 골라야 한다는 뜻이다. **미확인**은 이 묶음이 확인하지 못해 실환경에서 봐야 한다는 뜻이다.
- 이 묶음은 Grok과 agy의 세션을 띄우지 않았다. 문서·도움말·설치본으로 확인한 것이다.

## 0. 먼저 읽을 것

### 0.1 한 줄 결론

- **Grok과 agy는 둘 다 hook을 지원한다.** "hook을 못 쓰는 호스트"는 없다. 다만 Grok은 세션 시작과 프롬프트 제출 때 모델에게 글을 줄 수 없고, agy는 이벤트 이름·입력·출력이 전혀 다른 계약이다.
- 원본 1.24.0은 Grok에 hook을 전부 등록하지만 안내 문맥이 모델에게 닿지 않고, agy에는 스킬과 MCP만 들어간다(hook 파일을 agy가 읽지 않는다).
- Claude Code용 패키지를 Grok과 agy에 그대로 쓸 수는 없다. 호스트마다 등록 파일이 따로 있어야 한다.

### 0.2 뼈대(타입·trait·명령 표면)에 닿는 것

| # | 내용 | 닿는 곳 |
|---|---|---|
| N1 | **agy의 hook 입력에는 이벤트 이름이 없다.** 어느 이벤트인지는 등록한 명령이 인자로 알려 줘야 한다 | 7.1절 "이벤트 종류는 stdin의 `hook_event_name`으로 가른다". 명령이 `fullops hook <claude\|codex\|grok\|agy> [--event <이름>] …`이 된다. `Host::decode`가 이벤트를 인자로도 받아야 한다 |
| N2 | **통과 출력이 늘 `{}`는 아니다.** agy의 PreToolUse와 Stop은 `decision`이 필수다 | 7.1절의 감시 스레드·`catch_unwind`·조기 종료가 쓰는 `"{}"`. 호스트와 이벤트에 따라 통과 글이 달라야 한다(예: `Host::pass(event)`) |
| N3 | Grok은 SessionStart와 UserPromptSubmit에서 `Context`를 모델에게 전하지 못한다. agy에는 UserPromptSubmit이 없다 | 7.2절 출력 표, `Caps`(지금은 `stop_failure`, `background_wake` 둘). S-00·S-01·S-05·P-01의 `Context`, P-03의 `Warn` |
| N4 | Stop의 종료 이유가 Grok(`reason`)과 agy(`terminationReason`)에는 있다. Grok은 세션이 끝날 때 Stop을 한 번 더 부른다 | `HookInput.stop_reason`. `design-delta.md`의 Z4는 이 필드를 지우자고 했다. **지우지 않는다** |
| N5 | agy는 `cwd`가 아니라 `workspacePaths` 배열을 주고, hook 프로세스의 작업 디렉터리는 플러그인 폴더다 | `HookInput.cwd`, 7.1절 3단계(위로 올라가며 표식 찾기) |
| N6 | agy는 사용자의 마지막 입력(`lastUserInput`)을 모든 이벤트에 준다. 프롬프트 이벤트는 없다 | `HookInput.prompt`가 프롬프트 이벤트가 아닌 곳에도 올 수 있다. P-00·P-01·P-02, 7.6절의 "턴" |
| N7 | 도구 이름과 인자 이름이 호스트마다 다르다. agy는 인자가 PascalCase다 | `Host::tool_kind`, 7.4절의 편집 대상 계산과 T-08의 글. 어댑터가 `ToolCall`을 Claude의 키 이름으로 맞춰 넘기면 7.4·7.5절은 바뀌지 않는다 |
| N8 | 패키지와 등록 파일이 둘 더 생긴다 | 10.4절 배치, 10.5절 `xtask dist`, 1절 7번과 17절 H6 |

## 1. 네 호스트 한눈에

| 항목 | Claude Code 2.1.323 | Codex 0.163.0 | Grok Build 1.0.70 | agy 1.4.8 |
|---|---|---|---|---|
| 플러그인 매니페스트 | `.claude-plugin/plugin.json` | `.codex-plugin/plugin.json` | `plugin.json`(선택). `.claude-plugin/`도 읽는다 | `plugin.json` 또는 `.antigravity-plugin/plugin.json` |
| hook 등록 파일(플러그인) | `hooks/hooks.json` | `hooks/hooks.json`(또는 매니페스트의 `hooks`) | `hooks/hooks.json` | **루트의 `hooks.json`**. `hooks/hooks.json`은 읽지 않는다 |
| hook 등록 파일(전역) | `~/.claude/settings.json` | `~/.codex/hooks.json` | `~/.grok/hooks/*.json` | `~/.gemini/config/hooks.json` |
| 등록 형식 | `hooks.<이벤트>[].hooks[]` | 같다 | 같다 | `<묶음 이름>.<이벤트>[]`. 도구 이벤트만 `matcher`로 감싼다 |
| 이벤트를 아는 방법 | stdin `hook_event_name` | 같다 | 같다(별칭) | **stdin에 없다. 명령 인자로 준다** |
| Windows의 셸 | Git Bash, 없으면 PowerShell | `cmd.exe /C` | 찾아낸 셸(pwsh, PowerShell, Git Bash) | `cmd /c` |
| 그 밖의 OS의 셸 | `sh -c` | `$SHELL -lc` | `sh -c` | `sh -c` |
| 셸 없이 실행 | `args`가 있으면 | 문서에 없다 | `args`가 있으면 | 없다 |
| 플러그인 루트 | `${CLAUDE_PLUGIN_ROOT}` | `${PLUGIN_ROOT}` | `${GROK_PLUGIN_ROOT}`, `${CLAUDE_PLUGIN_ROOT}` | 변수가 없다. 작업 디렉터리가 플러그인 루트다 |
| hook 프로세스의 작업 디렉터리 | 세션의 현재 디렉터리 | 세션 `cwd` | 워크스페이스 루트 | `hooks.json`이 있는 폴더 |
| `timeout` 단위와 기본값 | 초 | 초, 600 | 초(소수 가능), 5(Stop은 600) | 정수 초, 30 |
| 실행 전 신뢰 | `claude-code.md`를 본다 | `/hooks`에서 hook마다 신뢰 | 플러그인 신뢰 + **폴더 신뢰** | 문서에 없다(워크스페이스 hook은 폴더 신뢰) |
| hook 실패 | 통과(Git Bash 없는 Windows의 PreToolUse는 차단) | 통과 | 통과 | **미확인** |

## 2. hook 등록 문자열과 파일 위치

### 2.1 확인한 것: 확장자 없는 런처와 `.exe`를 한 폴더에 둘 때

설계 10.4절은 `bin/fullops`(POSIX 런처)와 `bin/fullops.exe`를 함께 두고 명령 문자열 하나를 세 OS에서 쓴다. 이 PC의 임시 폴더에서 같은 배치로 해 본 결과다(호스트는 쓰지 않았다).

| 실행하는 쪽 | `"…/bin/fullops" hook …` | 비고 |
|---|---|---|
| `sh -c` | 런처를 실행한다 | |
| `cmd.exe /c` | `fullops.exe`를 실행한다 | 따옴표 없는 `bin/fullops`와 `./bin/fullops`는 실패한다(`/`가 옵션으로 읽힌다). `bin\fullops`와 드라이브로 시작하는 경로는 된다 |
| PowerShell 7.7, 5.1 (`& "…"`) | **`fullops.exe`를 실행하지 않는다.** 출력도 종료 코드도 없다 | 확장자 없는 파일이 옆에 없으면 `fullops.exe`를 찾는다 |

따라서 "문자열 하나가 세 OS에서 돈다"는 **PowerShell이 실행하는 호스트(Grok, Git Bash가 없는 Claude Code)에서 성립하지 않는다.** Codex와 agy(cmd)에서는 성립한다. `design-delta.md` 1절이 미확인으로 남긴 "확장자 없는 경로가 `.exe`로 풀리는가"는 cmd에 한해 확인됐다.

### 2.2 Grok — **결정 필요**

등록할 이벤트는 SessionStart, UserPromptSubmit, PreToolUse, Stop, StopFailure다. SessionStart에는 matcher를 두지 않는다(Claude의 `startup`·`clear`는 Grok에서 절대 일치하지 않는다). `timeout`은 설계 7.1절의 값을 그대로 적는다(기본값이 5초라 반드시 적는다). PreToolUse의 matcher는 `run_terminal_command|search_replace|hashline_edit|write|spawn_subagent`다.

| 안 | 등록 | 장점 | 단점 |
|---|---|---|---|
| G-가 | 플러그인의 `hooks/hooks.json`, shell form: `"$GROK_PLUGIN_ROOT/bin/fullops" hook grok --plugin-root "$GROK_PLUGIN_ROOT"` | 플러그인 설치만으로 끝난다. macOS·Linux와 Git Bash로 도는 Windows에서 동작한다 | **PowerShell로 도는 Windows에서는 hook이 뜨지 못한다**(2.1). 실패는 통과라 작업은 막지 않지만 fullops가 꺼진 상태가 되고 이벤트마다 오류 줄이 남는다. Grok이 어느 셸을 고르는지는 미확인이다 |
| G-나 (권장) | `fullops install`이 전역 파일 `<GROK_HOME 또는 ~/.grok>/hooks/fullops.json`을 쓴다. exec form: `"command": "<홈의 절대 경로>/.fullops/bin/fullops.exe"`(그 밖의 OS는 확장자 없는 실행본), `"args": ["hook","grok"]` | 셸을 타지 않아 세 OS에서 같다. 문서에 적힌 형태이고 Orca가 자기 hook을 같은 방식으로 넣는다. 플러그인 신뢰와 무관하다(폴더 신뢰만 필요하다) | 플러그인 설치와 hook 등록이 따로다. setup 스킬이 `<패키지>/bin/fullops install`을 한 번 실행해야 한다. 플러그인을 지워도 hook 파일이 남으므로 제거 명령이 지워야 한다. S-04의 "플러그인 `VERSION`이 더 높으면 자동 설치"를 쓸 수 없다(플러그인 루트를 모른다) |
| G-다 | 플러그인의 `hooks/hooks.json`, exec form으로 이름만: `"command": "fullops"`, `"args": ["hook","grok","--plugin-root","${GROK_PLUGIN_ROOT}"]` | 등록은 플러그인에 남고 셸을 타지 않는다 | `fullops`가 `PATH`에 있어야 한다. 설계의 `install`은 `PATH`를 건드리지 않는다. Windows에서 이름만으로 `.exe`를 찾는지 미확인이다 |

어느 안이든 Grok 패키지는 Claude Code 패키지와 **따로** 만든다. 이유는 셋이다. (1) 같은 `hooks/hooks.json`을 읽으면 Grok에서 `fullops hook claude`가 불린다. (2) Claude 쪽에 넣을 `"shell"`·`"onFailure"` 가운데 `shell`은 Grok에서 "hook load error"로 뜬다. (3) SessionStart의 matcher 값이 다르다. 안내서는 "Grok은 색인을 `.grok-plugin/marketplace.json`에서 읽고 `.claude-plugin/`의 것도 받는다"고 적는다. 둘이 함께 있을 때 앞의 것을 쓴다면 배포 저장소에 `.claude-plugin/marketplace.json`(Claude 패키지)과 `.grok-plugin/marketplace.json`(Grok 패키지)을 함께 둘 수 있다(**추정**: 우선순위는 문서에 없다).

**덧붙이는 안전 장치**: Grok은 모든 hook에 환경 변수 `GROK_HOOK_EVENT`를 준다. `fullops hook claude`나 `hook codex`로 불렸는데 이 변수가 있으면 통과(`{}`)만 내게 한다. Grok이 Claude Code의 플러그인 폴더를 스스로 읽는 경우(미확인)에 판정이 두 번 나는 것을 막는다.

### 2.3 agy — **고침**

agy는 `cmd /c`와 `sh -c`를 쓰므로 2.1의 문자열이 양쪽에서 돈다. 플러그인 루트의 `hooks.json`에 이벤트마다 이벤트 이름을 인자로 적는다.

```json
{
  "fullops": {
    "SessionStart": [ { "type": "command", "timeout": 10, "command": "\"bin/fullops\" hook agy --event SessionStart --in-place" } ],
    "PreToolUse": [ { "matcher": "run_command|write_to_file|replace_file_content|multi_replace_file_content|invoke_subagent",
      "hooks": [ { "type": "command", "timeout": 8, "command": "\"bin/fullops\" hook agy --event PreToolUse --in-place" } ] } ],
    "Stop": [ { "type": "command", "timeout": 20, "command": "\"bin/fullops\" hook agy --event Stop --in-place" } ]
  }
}
```

- 경로는 따옴표로 감싸고 `./`를 붙이지 않는다(cmd에서 실패한다).
- 플러그인 폴더의 실행본을 그대로 쓴다(`--in-place`). 고정 위치를 우선하는 조건문은 cmd와 sh에 함께 통하는 문법이 없어 넣지 않는다. 전역 `~/.gemini/config/hooks.json`에 넣는 길도 있지만 그 파일은 다른 도구(Orca)와 함께 쓰는 파일이라 권하지 않는다.
- agy가 `cmd /c`에 줄을 넘기는 방식과 작업 디렉터리는 문서로만 확인했다(**미확인**).
- UserPromptSubmit에 해당하는 등록은 4.2절의 결정에 따른다.

## 3. 어댑터가 다뤄야 하는 입력 차이

`HookInput`의 자리마다 호스트가 주는 것이다. Grok은 Claude의 snake_case 이름을 별칭으로 함께 주므로 설계가 읽는 필드는 이름이 같다.

| `HookInput` | Claude Code·Codex | Grok | agy |
|---|---|---|---|
| `event` | `hook_event_name` | `hook_event_name`(값도 PascalCase) | 명령 인자 `--event` |
| `sid` | `session_id` | `session_id` | `conversationId` |
| `cwd` | `cwd` | `cwd`(세션을 띄운 폴더). 루트는 `workspaceRoot` | `workspacePaths[0]`. 프로세스의 작업 디렉터리를 쓰면 안 된다 |
| `source` | `startup`·`resume`·`clear`·`compact`·`fork` | `new`·`load`. compaction은 SessionStart를 내지 않는다 | 없다 |
| `prompt` | `prompt` | `prompt` | 이벤트가 없다. 모든 이벤트의 `lastUserInput` |
| `tool` | `tool_name`, `tool_input` | `tool_name`, `tool_input` | `toolCall.name`, `toolCall.args` |
| `stop_reason` | 없다 | `reason`: `end_turn`·`channel_closed`·`shutdown` | `terminationReason`: 정상은 `NO_TOOL_CALL` |
| `error` | Claude Code: `error`, `error_details` | `error`(종류 이름 여섯 가지), `error_details` | Stop의 `error`(`terminationReason == "ERROR"`일 때) |
| 그 밖 | - | `backgroundTasks`(camelCase만), `subagentType`, `toolInputTruncated` | `executionNum`, `fullyIdle`, `stepIdx` |

**고침**:

- 7.2절 "snake_case만 읽는다"에 "agy는 camelCase만 준다. 어댑터가 `HookInput`으로 옮긴다"를 더한다.
- `source`는 어댑터가 `new` → `startup`, `load` → `resume`으로 옮긴다. agy는 `startup`으로 둔다.
- 7.5절 Stop 공통 전제 "입력에 `reason`이 있고 `end_turn`이 아니면 전부 통과한다"는 Grok의 세션 종료 Stop을 그대로 걸러 준다. agy는 어댑터가 `NO_TOOL_CALL`을 `end_turn`으로 옮기고 나머지는 원래 값을 둔다. 사용자 취소·오류·한도 종료에서 종료를 막지 않게 된다.
- Grok의 `toolInputTruncated`가 `true`면 편집 대상과 셸 분류를 `Unknown`으로 둔다(잘린 글로 판정하지 않는다).
- 환경 변수 `ORCA_TERMINAL_HANDLE`은 두 호스트의 hook 프로세스에서도 보일 것이다(**추정**: Orca의 자기 hook 스크립트가 같은 프로세스 환경에서 `ORCA_PANE_KEY`를 읽는다).

### 3.1 도구 이름과 인자

| `ToolKind` | Claude Code | Codex | Grok | agy |
|---|---|---|---|---|
| `Shell` | `Bash`, `PowerShell` | `Bash` | `run_terminal_command` | `run_command` |
| 명령 글 | `command` | `command` | `command` | `CommandLine` |
| `Write` | `Write`(`file_path`, `content`) | - | `write`(`file_path`, `content`. 추정) | `write_to_file`(`TargetFile`, `CodeContent`. 추정) |
| `Edit` | `Edit`(`file_path`, `old_string`, `new_string`, `replace_all`) | - | `search_replace`(같은 네 이름) | `replace_file_content`(`TargetFile`, `TargetContent`, `ReplacementContent`. 추정) |
| 여러 곳 편집 | `MultiEdit` | - | `hashline_edit`(바뀐 뒤 글을 계산할 수 없다 → `Unknown`) | `multi_replace_file_content`(`ReplacementChunks[]`. 추정) |
| `Patch` | - | `apply_patch` | - | - |
| `SubAgent` | `Agent` | `spawn_agent` | `spawn_subagent`(`prompt`, `description`) | `invoke_subagent`(인자 미확인) |
| 셸 방언(Windows) | 도구 이름으로 안다 | 미확인 | PowerShell일 것이다(추정) | 미확인 |

인자 이름이 추정인 도구는, 그 이름이 입력에 없으면 편집 대상을 `Unknown`으로 둔다(통과). agy의 `invoke_subagent`는 인자 이름이 확인될 때까지 T-08이 걸리지 않는다. Grok이 Cursor preset으로 돌면 도구 이름이 `Shell`·`StrReplace`·`Write`·`Task`로 바뀌고 인자도 `path`·`contents`가 된다. v1은 다루지 않는다(모르는 도구 = 통과).

## 4. 어댑터가 다뤄야 하는 출력 차이

### 4.1 Verdict별 출력

| Verdict | Claude Code·Codex | Grok | agy |
|---|---|---|---|
| `Pass` | `{}` | `{}` | PreToolUse: `{"decision":"allow"}`. Stop: `{"decision":""}`. 그 밖: `{}` |
| `Context` (SessionStart) | `hookSpecificOutput.additionalContext` | **전할 수 없다**(최상위 `systemMessage`만 읽고, 그것은 사용자에게만 보인다) | `{"injectSteps":[{"systemMessage":{"systemMessage":t}}]}` |
| `Context` (UserPromptSubmit) | 같은 형태 | **전할 수 없다**(통과시키는 hook의 stdout을 버린다) | 이벤트가 없다 |
| `Warn` | `{"systemMessage":t}` | SessionStart에서만 확실하다. PreToolUse·Stop은 미확인 | SessionStart에서만 `{"injectSteps":[{"errorMessage":t}]}`. PreToolUse·Stop에는 통로가 없다 |
| `Deny` | `hookSpecificOutput.permissionDecision: "deny"` | 같은 형태 | `{"decision":"deny","reason":r}` |
| `Block` | `{"decision":"block","reason":r}` | 같은 형태. 한 턴에 8번까지 | `{"decision":"continue","reason":r}` |

Grok에는 설계의 표에 없는 통로가 하나 있다: PreToolUse의 `hookSpecificOutput.additionalContext`가 모델에게 간다(10,000자). 표본은 `scenarios/fixtures/host/grok/output/`과 `agy/output/`에 있다.

### 4.2 규칙별로 달라지는 것

| 규칙 | Grok | agy |
|---|---|---|
| S-00, S-01 (`Context`) | 모델에게 가지 않는다. **결정 필요**(아래 D2) | 된다 |
| S-05 (compaction 뒤 안내) | 할 수 없다. SessionStart가 오지 않는다 | 할 수 없다. `source`가 없다 |
| S-02·S-03·S-04 (효과) | 된다. 플러그인 루트는 환경 변수 `GROK_PLUGIN_ROOT`로도 안다 | 된다(SessionStart가 이어 하기에서 오는지는 미확인) |
| P-00 (턴 세기), P-02 (지표) | 된다. 깨운 턴에도 UserPromptSubmit이 온다(사람 턴으로 세어질 수 있다) | 이벤트가 없다. **결정 필요**(D4) |
| P-01 (preamble로 dispatch 묶기) | 효과는 된다. dev 모드의 `Context`는 전해지지 않는다 | `lastUserInput`으로 PreToolUse·Stop에서 묶을 수 있다. **결정 필요**(D4) |
| P-03 (`Warn`) | 전해지지 않는다. T-00·X-00이 같은 사유를 전한다 | 이벤트가 없다 |
| T-00~T-12 | 된다 | 된다. 통과 출력의 뜻은 **미확인**(D3) |
| X-00~X-07 (`Block`) | 된다 | 된다. 호스트의 강제 이어 가기 상한 값은 미확인 |
| Stop의 `Warn`(한 번 막은 뒤의 통과) | 보이는지 미확인 | 낼 수 없다. 추적에만 남긴다 |
| X-05 (b)의 백그라운드 대기 | `caps.background_wake`에 따른다(5절) | 포그라운드 |
| F-01 | StopFailure가 있다. 등록한다 | StopFailure가 없다. **결정 필요**(D5) |
| 7.6절의 "턴" | UserPromptSubmit으로 센다. Stop을 막아 이어 가는 동안에는 오지 않는다(문서) | `executionNum`을 턴으로 쓸 수 있을 것이다(**추정**) |

## 5. 능력 표 (`caps`)

| 능력 | Claude Code | Codex | Grok | agy |
|---|---|---|---|---|
| `stop_failure` | 있다 | 없다 | 있다 | 없다(Stop의 `ERROR`로 대신할 수 있다) |
| `background_wake` | 있다 | 미확인(설계는 없음으로 둔다) | **문서는 있다고 한다.** 원본은 없다고 보고 포그라운드로 기다리게 한다 | 미확인 |
| 프롬프트 이벤트 | 있다 | 있다 | 있다 | **없다** |
| 세션 시작 때 모델에게 글 | 된다 | 된다 | **안 된다** | 된다 |
| 프롬프트 때 모델에게 글 | 된다 | 된다 | **안 된다** | 이벤트가 없다 |
| compaction 뒤 SessionStart | 온다 | 온다 | 오지 않는다 | 모른다 |
| 이벤트 이름이 stdin에 | 있다 | 있다 | 있다 | **없다** |
| 하위 에이전트 도구 | `Agent` | `spawn_agent` | `spawn_subagent` | `invoke_subagent` |
| 원격 HTTP MCP | 된다 | 된다 | 된다(`--transport http`) | 된다(Streamable HTTP만) |
| Orca의 `--agent` 값 | `claude` | `codex` | `grok` | **`antigravity`** |
| Orca의 `--model`·`--effort` | 받는다 | 받는다 | 받는다 | 받는다 |
| Orca가 신뢰를 미리 처리 | 한다 | 한다 | **하지 않는다** | 한다(추정) |

`Caps`에 더할 것은 위 표의 3~7행이다. `background_wake`가 틀렸을 때의 비용은 방향이 다르다. 있다고 봤는데 깨우지 않으면 lead가 멈춘 채 사람을 기다린다. 없다고 봤는데 깨우면 턴을 붙잡는 비용만 든다. Grok은 **없음으로 시작하고 실환경에서 확인한 뒤 올리는 것을 권한다**(D6).

## 6. 설치 명령

### 6.1 플러그인

| 일 | Grok | agy |
|---|---|---|
| 등록 | `grok plugin marketplace add <owner/repo \| URL \| 경로>` | 없다(공식 마켓플레이스 하나. 제3자 등록은 미확인) |
| 설치 | `grok plugin install <이름>` (사용자가 확인한다. `--trust`는 사용자가 붙인다) | `agy plugin install <로컬 폴더 \| https://github.com/<owner>/<repo>>` |
| 갱신 | `grok plugin marketplace update` 뒤 `grok plugin update <이름>` | 다시 설치(추정) |
| 제거 | `grok plugin uninstall <이름>` | `agy plugin uninstall <폴더 이름>` |
| 확인 | `grok plugin list --json`, `grok plugin validate <경로>`, `grok inspect --json` | `agy plugin list --json`, `agy plugin validate --strict <경로>` |
| 설치 뒤 | 새 세션. 폴더 신뢰(`/hooks-trust`) | 다시 시작 |

- `--trust`는 원본처럼 **자동으로 붙이지 않는다**(보안 결정은 사용자가 한다). Grok은 신뢰하지 않은 플러그인의 hook·MCP를 돌리지 않으므로 안내 문서가 이 단계를 적어야 한다.
- `agy plugin <하위 명령>`에는 `--help`를 붙이지 않는다. `agy plugin uninstall --help`는 "`--help`라는 플러그인을 제거했다"고 답하고 `~/.gemini/config/import_manifest.json`을 새로 쓴다(`agy.md` 0절).

### 6.2 패키지 배치 — **고침** (10.4·10.5절)

```
dist/grok/fullops/                 # Grok용
  plugin.json                      # name, version, description
  skills/<이름>/SKILL.md
  .mcp.json                        # Context7 원격 HTTP
  hooks/hooks.json                 # G-가·G-다를 고를 때만. G-나면 넣지 않는다
  bin/…  VERSION  releases/*.md
dist/agy/fullops/                  # agy용
  plugin.json                      # agy 스키마의 필드만(모르는 필드는 버려지고 --strict에서 걸린다)
  hooks.json                       # 반드시 루트
  mcp_config.json                  # {"mcpServers":{"context7":{"url":"https://mcp.context7.com/mcp"}}}
  skills/<이름>/SKILL.md
  bin/…  VERSION
```

- agy 패키지에는 `hooks/hooks.json`과 `commands/`를 넣지 않는다(`validate`가 경고한다).
- Grok의 플러그인 `.mcp.json`에서 원격 HTTP를 적는 정확한 형태는 문서에 예시가 없다(**미확인**. `config.toml`의 `url`과 `grok mcp add --transport http`는 확인됐다).
- 두 호스트의 안내서는 플러그인에 바이너리를 넣는 것을 권하지 않거나 말하지 않는다. 걸러 내는 장치는 문서에 없다(**미확인**). 설계 17절 H3(바이너리 유통)과 함께 본다.

### 6.3 스킬

스킬은 플러그인의 `skills/`로 들어가므로 따로 깔 것이 없다. 플러그인 없이 스킬만 깔 때의 자리는 다음과 같다.

| | Grok | agy |
|---|---|---|
| 사용자 범위 | `<GROK_HOME 또는 ~/.grok>/skills/<이름>/` | `~/.gemini/config/skills/<이름>/` (**`~/.agents/skills`는 전역에서 읽지 않는다**) |
| 프로젝트 | `.grok/skills/`, `.agents/skills/`, `.claude/skills/` | `.agents/skills/` |
| `skills` CLI의 대상 이름 | `--agent grok` | `--agent antigravity-cli` |

원본이 부르는 형태는 `npx --yes skills@latest add <출처> --skill <이름…> --global --agent <대상…> --yes`다. 새 설계는 실행 중 Node에 기대지 않으므로(G4) 이 도구를 쓰지 않는다. 스킬만 까는 경로가 필요하면 `fullops install`이 위 폴더에 파일을 복사하면 된다.

### 6.4 MCP

| | Grok | agy |
|---|---|---|
| 플러그인에 넣기 | `.mcp.json` | `mcp_config.json`(서버 이름이 `<플러그인>_<서버>`가 된다) |
| 명령으로 등록 | `grok mcp add --transport http context7 https://mcp.context7.com/mcp [-s user\|project]` | `agy mcp add --type http context7 https://mcp.context7.com/mcp` |
| 설정 파일 | `~/.grok/config.toml`의 `[mcp_servers.context7] url = "…"` | `~/.gemini/config/mcp_config.json` |

## 7. hook이 닿지 않는 곳의 대안과 한계

| 경우 | 닿지 않는 것 | 대안 | 남는 한계 |
|---|---|---|---|
| Grok의 세션 시작·프롬프트 | S-00·S-01·S-05·P-01의 안내가 모델에게 가지 않는다 | (1) `init`이 `AGENTS.md`에 넣는 관리 블록과 스킬에 "세션을 시작하면 `fullops status`를 먼저 실행한다"를 적는다. (2) 세션의 첫 PreToolUse에서 `additionalContext`로 한 번 준다 | (1)은 모델이 따르지 않을 수 있다. (2)는 matcher에 걸리는 도구를 처음 부를 때까지 늦고, 후보가 없으면 바로 끝내는 7.1절 5단계와 부딪힌다 |
| Grok의 신뢰하지 않은 폴더 | hook 전부가 조용히 건너뛰어진다. hook이 돌지 않으니 hook으로 알릴 수도 없다 | `doctor`와 `dispatch`(grok을 배정할 때)가 `~/.grok/trusted_folders.toml`을 **읽어** 경고한다. 워크트리는 원본 체크아웃 경로로 본다(원본 `grok_trust.py --check`와 같은 판정) | 신뢰를 주는 것은 사용자다. Orca는 grok에 대해 이 단계를 대신해 주지 않는다 |
| Grok의 `!` 명령과 내장 슬래시 명령 | 어떤 hook도 불리지 않는다 | 없다 | 사람이 직접 친 명령이라 설계의 범위 밖이다 |
| agy의 IDE와 데스크톱 앱 | hook이 실행되지 않는다는 보고가 있다(CLI만) | 스킬과 규칙(`AGENTS.md`)만으로 돈다 | 거부·종료 차단·상태 기록이 없다. lint 게이트(X-01), 완료 전송 확인(X-03), 통합 대기(X-06)가 걸리지 않는다. 감싼 명령(`fullops dispatch`·`done`·`merge`)의 자체 거절만 남는다 |
| agy의 프롬프트 | 사람 턴 지표(P-02)가 정확하지 않다 | Stop의 `executionNum`과 `lastUserInput`으로 센다 | 같은 글을 두 번 보내면 구분하지 못한다 |
| 두 호스트의 PreToolUse·Stop | 사용자에게만 보이는 경고(`Warn`) | 추적에 남기고 `fullops why`로 본다 | 한 번 막은 뒤의 통과가 조용하다 |

## 8. 결정이 필요한 것

| # | 결정 | 권하는 쪽 | 이유 |
|---|---|---|---|
| D1 | Grok의 hook 등록 방식(2.2절의 G-가·G-나·G-다) | G-나 | Windows가 먼저이고 PowerShell에서 확실히 도는 것은 exec form뿐이다 |
| D2 | Grok에서 세션 시작 안내를 전하는 방법 | `AGENTS.md` 관리 블록과 스킬에 `fullops status` 실행을 적는다. SessionStart에서는 사용자에게 `systemMessage` 한 줄만 낸다 | 첫 PreToolUse에 싣는 방식은 hook 경로를 복잡하게 하고 늦게 닿는다 |
| D3 | agy의 PreToolUse 통과 출력(`allow`와 `ask`) | 실환경에서 `allow`가 권한 확인을 건너뛰지 않음을 확인한 뒤 `allow`. 건너뛴다면 `ask` | 통과가 권한을 넓히면 안 된다. 두 문서가 다르게 적는다 |
| D4 | agy의 턴과 preamble 묶기 | `PreInvocation`을 등록하지 않는다. P-01은 PreToolUse·Stop에서 `lastUserInput`으로 평가하고, 턴은 Stop의 `executionNum`으로 본다 | `PreInvocation`은 모델을 부를 때마다 불려 프로세스가 늘어난다 |
| D5 | agy의 F-01 | Stop의 `terminationReason == "ERROR"`에서 F-01을 평가한다 | StopFailure가 없고 같은 정보가 Stop에 온다 |
| D6 | Grok의 `background_wake` | 없음으로 시작하고 실환경에서 확인한 뒤 올린다 | 5절 |
| D7 | Grok 폴더 신뢰를 fullops가 어디까지 하는가 | 읽기 전용 검사와 안내만 한다. 신뢰 파일을 쓰지 않는다 | 원본은 사용자 승인 뒤 파일에 덧붙였다. 호스트의 설정 파일을 고치는 일은 줄이는 편이 낫다 |
| D8 | `fullops hook`의 `--event` 인자를 모든 호스트에 줄 것인가 | agy에만 필수로 하고 나머지는 stdin을 믿는다 | 다른 세 호스트는 stdin에 이름이 있다 |
| D9 | agy의 배포 경로 | 로컬 폴더 설치를 기본으로 문서화하고 GitHub 주소 설치는 확인한 뒤 더한다 | 제3자 마켓플레이스와 갱신 방법이 문서에 없다 |
| D10 | 호스트별 패키지 수 | Claude 마켓플레이스판, Claude 디렉터리판, Codex, Grok, agy 다섯 | 등록 파일의 자리와 형식이 다르다 |

## 9. 실환경에서 먼저 확인할 것

순서는 틀렸을 때의 피해가 큰 것부터다.

1. agy: PreToolUse에 `{"decision":"allow"}`를 냈을 때 승인이 필요한 명령을 여전히 묻는가. hook이 죽거나 시간을 넘겼을 때 도구 호출이 막히는가.
2. Grok: Windows에서 hook 명령을 어느 셸이 실행하는가. G-나의 exec form 등록이 `/hooks` 화면에 오류 없이 뜨는가.
3. 두 호스트: 바이너리가 든 플러그인이 설치되고 실행되는가(`grok plugin validate`, `agy plugin validate --strict`).
4. Grok: SessionStart·UserPromptSubmit의 출력이 정말 모델에게 가지 않는가. Stop과 PreToolUse의 `systemMessage`가 사용자에게 보이는가.
5. Grok: 백그라운드로 건 `fullops wait`가 끝났을 때 세션이 깨어나는가.
6. agy: SessionStart가 이어 하기에서도 오는가. `executionNum`이 사용자 입력마다 하나씩 느는가. 편집 도구와 `invoke_subagent`의 인자 이름.
7. 두 호스트: hook 프로세스에 `ORCA_TERMINAL_HANDLE`이 보이는가. Orca가 넘긴 첫 프롬프트(preamble)가 Grok의 UserPromptSubmit과 agy의 `lastUserInput`에 그대로 오는가.
8. Grok: Claude Code가 깐 플러그인을 Grok이 스스로 읽는가(`grok inspect --json`).
