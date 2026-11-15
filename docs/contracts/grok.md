# Grok Build 계약 (플러그인·hook·스킬·능력)

- 확인한 날: 2026-11-13~14. 이 PC의 Grok Build는 `grok 1.0.70 (f50904eaee6c) [stable]`이다(`grok --version`).
- 확인 방법: 설치본에 함께 깔리는 사용자 안내서, 모든 깊이의 `--help`, 이 PC의 `~/.grok` 설정·플러그인 폴더(읽기만), 실행 파일에 든 문자열, Orca 설치본의 코드, 원본 플러그인 v1.24.0의 문서와 스크립트를 읽었다. **세션을 띄워 hook을 실제로 돌려 보지는 않았다.** 플러그인 설치·제거·신뢰 변경도 하지 않았다.
- 표기: **사실**은 출처가 있는 것, **추정**은 미루어 본 것, **미확인**은 확인하지 못한 것이다.
- 공개 웹 문서는 읽지 않았다. 설치본의 안내서가 실행 파일과 같은 날(2026-10-25) 갱신된 것이라 그것을 정본으로 봤다.

| 약어 | 출처 |
|---|---|
| GH | `~/.grok/docs/user-guide/10-hooks.md` (hook 안내서) |
| GP | `~/.grok/docs/user-guide/09-plugins.md` (플러그인 안내서) |
| GS | `~/.grok/docs/user-guide/08-skills.md`, `16-subagents.md`, `20-background-tasks.md` |
| GM | `~/.grok/docs/user-guide/07-mcp-servers.md`, `05-configuration.md`, `14-headless-mode.md`, `22-permissions-and-safety.md` |
| HELP | `grok --help`, `grok plugin … --help`, `grok mcp … --help` |
| BIN | `~/.grok/bin/grok.exe`에 든 문자열(serde 필드 이름표). 문서에 없는 필드 이름을 확인할 때만 썼다 |
| HOME | 이 PC의 `~/.grok/config.toml`, `installed-plugins/registry.json`, `hooks/orca-status.json`, `trusted_folders.toml`(형태만) |
| ORCA | Orca 1.4.256 설치본의 `orca orchestration worker-start --help`와 `resources/app.asar.unpacked/out/` 아래 코드, `~/.orca/agent-hooks/grok-hook.cmd` |
| PY | 원본 `D:/workspace/Fullops-Squad-plugin`(v1.24.0)의 README, `docs/development.md`, `docs/releases/`, `plugins/fullops-squad/DEPENDENCIES.md`, `scripts/deps.py`·`grok_trust.py`·`flow_gate.py`·`done_gate.py`, `hooks/hooks.json`, `bin/fullops-squad.cjs`, `scripts/build.py` |
| LOCAL | 이 PC의 임시 폴더에서 `cmd.exe`·PowerShell·`sh`로 직접 해 본 것(호스트는 쓰지 않았다) |

## 1. 플러그인

### 1.1 사실

1. **플러그인 개념이 있다.** 플러그인은 스킬·슬래시 명령·에이전트·hook·MCP 서버를 묶은 폴더다. 구성은 `skills/`, `commands/`, `agents/`, `hooks/hooks.json`, `.mcp.json`, `.lsp.json`이고 `plugin.json`은 선택이다(GP "What a plugin contains").
2. **Claude Code 형식을 그대로 읽는다.** 마켓플레이스 색인은 `.grok-plugin/marketplace.json`이고 `.grok-plugin/plugin.json`과 `.claude-plugin/`의 같은 파일도 받는다(GP "Set up the repository"). 플러그인 위치도 `.claude/plugins/`의 같은 자리를 읽는다(GP "Where Grok looks for plugins"). 원본 1.24.0은 Claude Code용 색인(`.claude-plugin/marketplace.json`)과 같은 패키지 폴더(`dist/native/fullops-squad`)로 Grok에 설치돼 있다(HOME `registry.json`: `plugin_subdir: "dist/native/fullops-squad"`, 버전 1.24.0).
3. 명령(HELP, GP):

   | 일 | 명령 |
   |---|---|
   | 마켓플레이스 등록 | `grok plugin marketplace add <owner/repo \| git URL \| 로컬 경로> [--force]` |
   | 마켓플레이스 목록·갱신·제거 | `grok plugin marketplace list`, `… update [<이름>]`, `… remove <url>` |
   | 설치 | `grok plugin install <이름 \| owner/repo[@ref][#하위폴더] \| git URL \| 로컬 경로> [--trust]` |
   | 갱신 | `grok plugin update [<이름>]` (이름을 빼면 전부) |
   | 제거 | `grok plugin uninstall <이름> [--confirm] [--keep-data]` |
   | 켜기·끄기 | `grok plugin enable <이름>`, `grok plugin disable <이름>` |
   | 살펴보기 | `grok plugin list [--json] [--available]`, `grok plugin details <이름>`, `grok plugin validate [<경로>]`, `grok inspect [--json]` |

4. **플러그인은 기본으로 꺼져 있다.** `~/.grok/config.toml`의 `[plugins] enabled = ["<이름>"]`에 있어야 켜진다(GP "Turn plugins on or off in config"). 이 PC의 설정에는 `enabled = ["fullops-squad"]`가 있다(HOME).
5. **신뢰하기 전에는 hook과 MCP가 돌지 않는다.** "Install turns a plugin on. Its hooks and MCP servers stay inactive until you trust it."(GP). `--trust` 없이 `grok plugin install`을 하면 출처를 보여 주고 hook·MCP·스킬이 켜진다고 경고한 뒤 멈춘다. `--trust`를 주면 확인 없이 진행한다(GP, HELP). `~/.grok/plugins/` 아래의 플러그인은 자동으로 신뢰된다. 프로젝트의 `.grok/plugins/`는 신뢰가 필요하다.
6. 설치된 플러그인은 `~/.grok/installed-plugins/<출처에서 만든 이름>/`에 복사되고 `installed-plugins/registry.json`에 출처·버전·시각이 적힌다(HOME).
7. 세션이 시작할 때 플러그인 자동 갱신 검사가 돈다. `plugin_auto_update = false`로 끈다(GP).
8. 스킬 파일이 플러그인 폴더 밖을 가리키는 심볼릭 링크면 읽지 않는다. Windows에서 전체 경로가 260자를 넘는 스킬 파일도 건너뛴다(GP "Troubleshooting").

### 1.2 네이티브 바이너리

**사실**(GP "What this does not cover", "What a plugin contains"): 안내서는 "마켓플레이스는 프로그램을 기기에 설치하지 않는다. 도우미 바이너리가 필요하면 따로 배포하라", "plugins deliver files, not runtimes or native binaries"라고 적는다.

**추정**: 이것은 용도에 대한 안내이고 바이너리 파일을 걸러 내는 장치는 문서에 없다. 플러그인은 git 저장소나 폴더를 그대로 복사하므로 `bin/fullops.exe`를 넣으면 함께 복사될 것이다. 실제로 걸러 내는지, POSIX에서 실행 권한이 남는지는 **미확인**이다.

### 1.3 원본이 `--trust`를 자동으로 넘기지 않는 이유

**사실**(PY `DEPENDENCIES.md`): "hook·MCP 연결·폴더 신뢰는 자동 변경하지 않는다. Grok 설치에서도 `--trust`를 자동 전달하지 않는다." 1.1.0 무렵의 설치기는 `--trust`를 붙였고(원본 저장소에 남은 옛 사본에서 봤다), 지금 시험은 "어떤 호출에도 `--trust`가 없다"를 단언한다(PY `tests/npm-installer.cjs`). 신뢰는 사용자가 내리는 보안 결정이라는 것이 근거다(PY `grok_trust.py`의 머리 주석: "신뢰는 보안 결정이라 사용자가 승인한 뒤에만 `--add`를 실행한다").

## 2. hook

### 2.1 hook을 읽는 곳과 신뢰

**사실**(GH "Hook Locations"):

| 범위 | 경로 | 실행 조건 |
|---|---|---|
| 전역 | `~/.grok/hooks/*.json` | 폴더 신뢰 뒤 |
| 전역(호환) | `~/.claude/settings.json`·`settings.local.json`, `~/.cursor/hooks.json` | 폴더 신뢰 뒤. `[compat.claude] hooks = false`로 끈다 |
| 프로젝트 | `<프로젝트>/.grok/hooks/*.json`, `<프로젝트>/.claude/settings.json`, `<프로젝트>/.cursor/hooks.json` | 신뢰 필요 |
| 설정 | `~/.grok/config.toml`의 `[[hooks.<이벤트>]]` | 폴더 신뢰 뒤 |
| 플러그인 | 설치된 플러그인의 `hooks/hooks.json` | 플러그인 신뢰 + 폴더 신뢰 |

- **폴더 신뢰가 사용자 자신의 hook도 막는다.** 신뢰하지 않은 폴더에서는 `~/.grok/hooks/`, 설정 파일, `~/.claude/settings.json`, 플러그인의 hook이 모두 돌지 않고 조용히 건너뛴다. 조직이 내려보낸 hook만 예외다. 홈 폴더와 드라이브 루트는 신뢰된 것으로 친다.
- 신뢰는 `/hooks-trust`로 주고 `~/.grok/trusted_folders.toml`에 `[folders.'<경로>']`, `trusted = true`, `decided_at = <초>`로 적힌다(GH, HOME). 같은 신뢰가 MCP·LSP·hook·프로젝트 지시문·프로젝트 스킬을 한꺼번에 연다. 같은 저장소의 하위 폴더까지 덮고, 그 안의 다른 git 체크아웃은 별개다.
- 머리 없는 실행(`grok -p`)은 사용자 자신의 hook에 대해 폴더를 신뢰된 것으로 친다.
- `GROK_FOLDER_TRUST=0` 또는 `[folder_trust] enabled = false`로 폴더 신뢰를 통째로 끈다.
- 안내서는 "`--trust`로 띄워도 된다"고 하지만 1.0.70의 `grok --help`에는 `--trust`가 없다(**미확인**: 숨은 옵션인지 없어진 것인지).

**원본의 `grok_trust.py`가 하는 일**(PY): 워크트리 경로를 받으면 git 공용 폴더가 있는 원본 체크아웃 경로로 바꾼 뒤, `--check`는 `trusted_folders.toml`에 그 경로나 상위 폴더가 `trusted = true`로 있는지 보고(없으면 종료 코드 1), `--add`는 파일 끝에 항목을 덧붙인다. 0.7.5에서 생겼다. 근거는 "grok worker가 착수 전에 폴더 신뢰 확인 화면에서 멈췄다. grok은 워크트리에서도 원본 레포 경로의 신뢰를 요구한다"(PY `docs/releases/0.7.5.md`)다. 스킬은 grok을 배정한 역할이 있으면 배정 전에 `--check`를 돌리고 사용자에게 한 번 확인받은 뒤에만 `--add`한다.

### 2.2 등록 파일 형식, 실행 셸, 변수, 제한 시간

**사실**(GH "The Hook JSON Format", "Key Fields"):

- 형식은 Claude Code와 같다. 최상위 `hooks` 아래 이벤트 이름, 그 아래 `matcher`와 `hooks` 배열이다. 모르는 이벤트 이름은 건너뛴다.
- handler에서 읽는 필드는 `type`, `command`, `args`, `url`, `timeout`, `env`뿐이다. `if`가 있으면 그 handler를 건너뛴다. `async`·`asyncRewake`·`shell`·`statusMessage`·`headers`가 있으면 handler는 돌지만 "hook load error"가 `/hooks` 화면과 `grok inspect`에 뜬다. 그 밖의 모르는 키는 무시한다.
- `type`은 `command`와 `http`다.
- **셸**: `args`가 없으면 Unix에서는 `sh -c`, **Windows에서는 "찾아낸 Windows 셸(pwsh, Windows PowerShell, Git Bash)"**로 실행한다. `"shell": "bash"`는 받지 않는다. Git Bash로 고정하려면 환경 변수 `GROK_SHELL=bash`를 쓴다.
- **셸 없이 실행**: `args` 배열이 있으면(빈 배열도) `command`가 프로그램 하나가 되고 셸을 띄우지 않는다. 인자 안의 `$이름`·`${이름}`은 handler의 `env`, 플러그인 변수, 실행기 변수로 채운다. `$HOME`·없는 이름·`~`·따옴표는 쓴 그대로 간다.
- `command`가 경로면 **등록 JSON 파일이 있는 폴더 기준**으로 푼다. 이름만 쓰면 JSON 옆의 같은 이름 파일, 없으면 `PATH`에서 찾는다.
- Windows에서 `.exe`·`.cmd`·`.bat`·`.com` 경로나 이름은 Windows가 직접 실행한다. Git Bash 아래에서는 확장자 없는 파일을 Git Bash가 `#!` 줄을 읽어 실행한다. **PowerShell이나 cmd 아래에서 그 밖의 확장자(또는 확장자 없는 스크립트)는 메시지와 함께 실패한다.**
- **변수**: 모든 hook에 `GROK_HOOK_EVENT`(예: `pre_tool_use`), `GROK_HOOK_NAME`, `GROK_SESSION_ID`, `GROK_WORKSPACE_ROOT`, `CLAUDE_PROJECT_DIR`가 환경 변수로 주어진다. 플러그인의 hook에는 `GROK_PLUGIN_ROOT`, `GROK_PLUGIN_DATA`와 별칭 `CLAUDE_PLUGIN_ROOT`, `CLAUDE_PLUGIN_DATA`가 더 주어진다. `command`와 `url`의 `${이름}`·`$이름`은 치환된다. PowerShell에서는 아는 `$이름`을 `$env:이름`으로 바꿔 준다.
- **작업 디렉터리**는 워크스페이스 루트(Grok을 띄운 git 저장소나 워크트리의 루트)다. 세션을 띄운 폴더는 입력의 `cwd`다.
- **제한 시간**: `timeout`은 초이고 소수도 된다. 기본값은 5초이고 `Stop`·`SubagentStop`·`PostToolUse`만 600초다. 0은 기본값이다. 플러그인의 `SessionEnd`는 `timeout`과 무관하게 최대 1.5초다.
- **실패는 통과다.** 시간 초과·죽음·잘못된 출력은 기록만 하고 막지 않는다. 막는 것은 명시한 `deny`, 읽기 한도(8MiB)를 넘은 응답, 적용할 수 없는 `updatedInput`뿐이다. 종료 코드 2는 막는 신호다(PreToolUse·UserPromptSubmit·Stop).
- matcher: 글자·숫자·`_`·`-`·공백·`,`·`|`만 있으면 이름 목록(정확히 일치)이고, 그 밖이면 정규식이다. `Stop`과 `UserPromptSubmit`의 matcher는 무시된다.

**확인한 것**(LOCAL): 같은 폴더에 확장자 없는 POSIX 런처 `bin/fullops`와 `bin/fullops.exe`를 함께 두고 명령 `"…/bin/fullops" hook grok`을 실행하면, `sh`는 런처를 실행하고 `cmd.exe /c`는 `fullops.exe`를 실행한다. **PowerShell(7.7과 5.1)은 `fullops.exe`를 실행하지 않는다**(출력도 종료 코드도 없다). 확장자 없는 파일이 옆에 없을 때는 PowerShell도 `fullops.exe`를 찾는다. 설계 10.4절의 "명령 문자열 하나가 세 OS에서 돈다"는 PowerShell에서 성립하지 않는다.

**미확인**: Git Bash와 PowerShell이 모두 있는 Windows에서 Grok이 어느 셸을 고르는지. 안내서가 "`GROK_SHELL=bash`로 Git Bash에서 돌게 하라"고 따로 적은 것으로 보아 기본은 PowerShell 쪽일 것이다(**추정**). exec form의 `command`에서 `${GROK_PLUGIN_ROOT}`가 치환되는지도 문서로는 분명하지 않다(인자는 치환된다고 적혀 있다).

### 2.3 이벤트

**사실**(GH "Hook Events"):

| 설계의 이벤트 | Grok | 막을 수 있는가 | Claude Code와 다른 점 |
|---|---|---|---|
| SessionStart | `SessionStart` | 아니다 | matcher가 보는 값이 `new`(새 세션, `/new`·`/clear` 포함)와 `load`(`--resume`·`--continue`·`/resume`)다. **Claude의 `startup`·`resume`·`clear`·`compact`는 절대 일치하지 않는다.** **compaction은 SessionStart를 내지 않는다**(`PreCompact`·`PostCompact`만). 하위 에이전트의 세션에는 오지 않는다. **출력은 최상위 `systemMessage`만 읽는다** |
| UserPromptSubmit | `UserPromptSubmit` | 그렇다 | **통과시키는 hook의 stdout은 버려진다(`additionalContext` 없음).** 백그라운드 작업·하위 에이전트 완료·스케줄러가 깨운 턴에도 불리지만 그때는 관찰만 한다. 기본 제한 시간 30초(안내서의 이 절)와 5초(공통 기본값)가 함께 적혀 있어 `timeout`을 명시하는 편이 안전하다 |
| PreToolUse | `PreToolUse` | 그렇다 | 도구 이름이 다르다(2.6절). `additionalContext`를 모델에게 줄 수 있다 |
| Stop | `Stop` | 그렇다 | **세션이 끝날 때도 한 번 더 불린다**(`reason`이 `channel_closed`나 `shutdown`. 최대 1.5초, 출력은 무시). 정상 종료는 `reason == "end_turn"`이다. 사용자가 끊으면 `StopCancelled`, API 오류면 `StopFailure`가 대신 온다 |
| StopFailure | `StopFailure` | 아니다 | 있다. 출력과 종료 코드는 무시된다. `systemMessage`도 보이지 않는다 |

그 밖에 `PostToolUse`, `PostToolUseFailure`, `PermissionDenied`, `StopCancelled`, `Notification`, `SubagentStart`, `SubagentStop`, `PreCompact`, `PostCompact`, `SessionEnd`가 있다. `bash` 모드(`!`) 명령과 내장 슬래시 명령은 어떤 hook도 내지 않는다(PreToolUse로 막을 수 없다).

### 2.4 입력

**사실**(GH "Input", "Porting Claude Code stop hooks"; 필드 이름 가운데 문서에 없는 것은 BIN):

입력은 stdin의 한 줄 JSON이다. 키는 **camelCase**이고, 아래 최상위 키는 Claude의 **snake_case 별칭도 함께 온다**: `session_id`, `transcript_path`, `permission_mode`, `tool_name`, `tool_input`, `tool_response`, `tool_use_id`, `duration_ms`, `is_interrupt`, `stop_hook_active`, `last_assistant_message`, `error_details`, `notification_type`.

| 필드 | 이벤트 | 설명 |
|---|---|---|
| `hook_event_name` | 전부 | **Claude의 PascalCase 값**(`"PreToolUse"`, `"Stop"`) |
| `hookEventName` | 전부 | Grok의 snake_case 값(`"pre_tool_use"`, `"stop"`) |
| `sessionId` / `session_id` | 전부 | 세션 ID |
| `cwd` | 전부 | 세션을 띄운 폴더(띄운 그대로의 표기) |
| `workspaceRoot` | 전부 | 워크스페이스 루트 |
| `permissionMode` / `permission_mode` | 전부 | `default`·`auto`·`plan`·`bypassPermissions` |
| `timestamp` | 전부 | hook을 보낸 시각 |
| `promptId` | 턴 범위 이벤트 | 턴 ID. 세션 범위 이벤트와 도구 이벤트에는 없다 |
| `source` | SessionStart | `new`·`load`. 값은 GH, 필드 이름은 BIN |
| `prompt` | UserPromptSubmit | 필드 이름은 BIN. 문서에는 예시가 없다 |
| `toolName` / `tool_name`, `toolInput` / `tool_input`, `toolUseId` / `tool_use_id`, `toolInputTruncated` | PreToolUse | 입력이 128KiB를 넘으면 긴 문자열이 잘리고 `toolInputTruncated`가 `true`다. 4KiB 이하 문자열은 잘리지 않는다 |
| `reason` | Stop | `end_turn`·`channel_closed`·`shutdown` |
| `stopHookActive` / `stop_hook_active`, `lastAssistantMessage` / `last_assistant_message` | Stop | Claude와 같은 뜻 |
| `backgroundTasks`, `sessionCrons` | Stop | **camelCase만 온다.** 항목은 `id`, `type`(`shell`·`monitor`·`subagent`), `status`, `command`(shell), `description`, `agentType`. 항목 안의 키도 camelCase다 |
| `error`, `errorDetails` / `error_details`, `lastAssistantMessage` | StopFailure | `error`는 `rate_limit`·`authentication_failed`·`invalid_request`·`server_error`·`max_output_tokens`·`unknown`. 용량 오류(503·529)는 `rate_limit`으로 분류된다 |
| `subagentType` | 하위 에이전트 안에서 불린 이벤트 | 주 세션에는 없다 |

**설계가 읽는 필드(`hook_event_name`, `session_id`, `cwd`, `source`, `prompt`, `tool_name`, `tool_input`, `reason`, `error`)는 Grok에서도 같은 snake_case 이름으로 읽힌다.** 원본이 "grok은 camelCase"라고 한 것은 별칭이 생기기 전의 사정으로 보인다(**추정**: 원본 0.6.0의 기록이다). 지금도 camelCase로만 오는 것은 `backgroundTasks`·`sessionCrons`·`workspaceRoot`·`promptId`·`toolInputTruncated`·`subagentType`이다. 원본이 Claude Code를 가려내는 방법이 바로 이것이다: Stop 입력에 snake_case `background_tasks`가 있으면 Claude Code로 본다(PY `flow_gate.py`의 `waiting_in_background`).

**미확인**: 깨운 턴(백그라운드 완료 등)의 UserPromptSubmit에서 `prompt`에 무엇이 오는지. Orca가 argv로 넘긴 첫 프롬프트에 UserPromptSubmit이 오는지("You submit a prompt"라고만 적혀 있다).

### 2.5 출력

**사실**(GH "Output (Blocking Hooks)", "Stop Decision Control", "Passive Hooks", "Reading a Hook's Reply"):

| 설계의 Verdict | 설계의 출력 | Grok의 처리 |
|---|---|---|
| `Pass` | `{}` | 통과한다. Stop에서 "차단을 정하지 않은 읽을 수 있는 응답"은 턴을 끝낸다 |
| `Context` (SessionStart) | `{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":t}}` | **모델에게 가지 않는다.** 관찰 이벤트는 최상위 `systemMessage`만 읽는다 |
| `Context` (UserPromptSubmit) | 같은 형태 | **버려진다.** "stdout of an allowing hook is discarded (no `additionalContext`)" |
| `Warn` | `{"systemMessage":t}` | SessionStart에서는 읽는다(사용자에게 보인다). StopFailure·PreCompact·PostCompact·SessionEnd에서는 버려진다. PreToolUse·Stop에서 보이는지는 문서에 분명하지 않다(**미확인**) |
| `Deny` | `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":r}}` | 도구 호출을 막고 사유를 모델에게 보인다. 최상위 `{"decision":"deny","reason":r}`도 된다. 둘 다 있으면 `permissionDecision`이 이긴다 |
| `Block` | `{"decision":"block","reason":r}` | 턴을 끝내지 않고 `reason`을 사용자 메시지처럼 모델에게 준다. **한 턴에 이어 가기 8번을 넘으면 턴을 끝낸다**(Claude Code와 같다). 이어 가는 동안 UserPromptSubmit은 오지 않는다 |

모델에게 글을 줄 수 있는 길은 셋이다: PreToolUse의 `hookSpecificOutput.additionalContext`(도구 결과와 함께 `<system-reminder>`로 간다. 10,000자), PostToolUse의 같은 필드, Stop의 `block` 사유와 `additionalContext`(둘 다 턴을 이어 가게 한다). hook이 `deny`를 내면 같은 응답의 `additionalContext`는 버려진다.

값이 표 밖인 `decision`은 hook 실패로 처리돼 통과한다. `{`로 시작하지만 JSON이 아닌 응답도 실패(통과)다.

### 2.6 도구 이름과 인자

**사실**(GH "Tool Name Aliases", GS, GM `14-headless-mode.md`):

| 종류 | hook이 받는 `tool_name` | 인자 | matcher에서 통하는 Claude 이름 |
|---|---|---|---|
| 셸 | `run_terminal_command` | `command`. 백그라운드는 `block_until_ms: 0` | `Bash` |
| 편집 | `search_replace` | `file_path`, `old_string`, `new_string`, `replace_all`(BIN의 `SearchReplaceInput`) | `Edit`, `Write`, `MultiEdit` |
| 편집(다른 파일 도구 묶음) | `hashline_edit` | 경로와 `edits` 배열(BIN). 바뀐 뒤의 글을 계산할 수 없다 | 정규식 matcher로만 |
| 쓰기 | `write` | `file_path`, `content`(**추정**: 안내서가 "Cursor preset의 Write는 `path`·`contents`라서 `tool_input.file_path`·`content`를 읽는 hook은 아무것도 못 읽는다"고 적은 데서 미루어 봤다) | `Write` |
| 하위 에이전트 | `spawn_subagent` | `prompt`, `description`, `run_in_background`(기본 `true`), `isolation`, `resume_from`, `cwd` | `Task` |
| MCP 도구 | `<서버>__<도구>` | - | `mcp__<서버>__<도구>` |

- matcher는 Claude 이름과 Grok 이름을 모두 받는다. 예를 들어 `Bash`는 `run_terminal_command`에도 걸린다.
- 에이전트 종류가 Cursor preset(`agent_type = "cursor"`)이면 도구 이름이 `Shell`, `StrReplace`(`path`), `Write`(`path`, `contents`), `Task`로 바뀐다.
- 하위 에이전트는 한 단계만 된다. 하위 에이전트 안의 도구 호출은 `subagentType`이 붙어 같은 hook을 탄다.

**미확인**: Windows에서 `run_terminal_command`의 `command`가 어느 셸 문법인지. 안내서는 "Shell 도구의 명령을 pwsh와 Windows PowerShell로 띄운다"고 적는다(GM `22-permissions-and-safety.md`). Git Bash를 쓰는 경우가 있는지는 모른다.

## 3. 스킬

**사실**(GS `08-skills.md`):

- 사용자 범위 경로는 `~/.grok/skills/`다(`GROK_HOME`이 있으면 그 아래). 호환으로 `~/.claude/skills/`, `~/.cursor/skills/`도 읽는다. 각 단계에서 `.agents/skills/`도 함께 훑는다. 프로젝트는 `.grok/skills/`, `.claude/skills/`, `.agents/skills/`다.
- 이름이 겹치면 우선순위가 높은 자리가 이긴다. 플러그인의 스킬은 같은 이름의 사용자 스킬을 덮지 않고 `<플러그인>:<이름>`으로 남는다.
- `SKILL.md`의 frontmatter는 `name`, `description`이 기본이고 `when-to-use`, `allowed-tools`, `disable-model-invocation` 등이 더 있다. 본문은 앞 25,000 토큰만 읽는다.
- 스킬 하나가 슬래시 명령 하나가 된다(`/<이름>`).

**스킬 설치 도구**(PY `deps.py`, vercel-labs/skills의 `src/agents.ts`): 원본은 `npx --yes skills@latest add <출처> --skill <이름…> --global --agent grok --yes`를 부른다. `skills` CLI의 `grok` 대상은 프로젝트 `.grok/skills`, 전역 `<GROK_HOME 또는 ~/.grok>/skills`다. 이 PC에서는 `~/.grok/skills/<이름>`이 `~/.agents/skills/<이름>`을 가리키는 링크로 깔려 있다(HOME).

## 4. 능력

| 항목 | 결과 | 출처 |
|---|---|---|
| 백그라운드 명령이 끝나면 세션을 깨우는가 | **깨운다.** "Completion wakes the parent automatically." "Any of them can wake the agent for a new turn (commands and subagents on completion, monitors on events, loops on their timer)." | GS `20-background-tasks.md` |
| 원본의 판단과의 차이 | 원본은 "Codex·grok은 백그라운드 명령이 끝나도 세션을 깨우지 않는다"고 보고 포그라운드 대기를 시킨다(0.7.6의 기록). 1.0.70의 안내서와 맞지 않는다. 실제로 깨우는지는 돌려 보지 않았다 | PY `flow_gate.py`, `docs/releases/0.7.6.md` |
| 하위 에이전트 도구 | 있다. `spawn_subagent` | GS `16-subagents.md` |
| StopFailure | 있다 | GH |
| MCP 등록 | `grok mcp add [-t stdio\|http\|sse] [-s user\|project] [-e K=V] [-H "이름: 값"] <이름> <명령 또는 URL>`. 원격 HTTP를 등록할 수 있다. 설정은 `~/.grok/config.toml`의 `[mcp_servers.<이름>] url = "…"`. 플러그인의 `.mcp.json`도 읽는다 | HELP, GM `07-mcp-servers.md` |
| 다른 호스트의 MCP를 함께 읽는가 | `~/.claude.json`, `~/.cursor/mcp.json`, 프로젝트의 `.mcp.json`을 기본으로 읽는다 | GM |

## 5. Orca와의 관계

**사실**(ORCA):

- Orca의 에이전트 ID는 `grok`이다. `orca orchestration worker-start --agent grok`으로 띄운다. 도움말은 "`--model` supports Claude, Codex, Cursor, Antigravity, Grok, and Muse"라고 적고, 코드는 `--model`을 `grok -m <모델>`로, `--effort`를 `--reasoning-effort <값>`으로 넘긴다. 원본 스킬의 "`--model`을 받지 않는 CLI(grok 등)는 `terminal create --command`로 띄운다"는 지금 Orca에서는 필요 없다.
- Orca는 첫 프롬프트를 argv로 넘긴다(`grok -- <프롬프트>`. 코드의 `promptInjectionMode: "argv"`, `argvPromptSeparator: "--"`).
- **Orca는 Claude Code·Codex·Antigravity에는 띄우기 전에 워크스페이스 신뢰를 처리하는 단계(`preflightTrust`)가 있고 grok에는 없다.** grok worker가 폴더 신뢰 화면에서 멈출 수 있다는 원본의 경험과 맞는다.
- Orca는 자기 상태 표시용 hook을 `~/.grok/hooks/orca-status.json`에 직접 써 넣는다. 명령은 절대 경로의 `.cmd` 파일이고 모든 이벤트에 `timeout` 10초로 등록돼 있다. 그 스크립트는 hook 프로세스의 환경에서 `ORCA_PANE_KEY` 등을 읽는다. Grok의 hook 프로세스가 세션의 환경 변수를 물려받는다는 뜻이다. `ORCA_TERMINAL_HANDLE`도 보일 것이다(**추정**).
- preamble은 호스트와 무관하게 Orca가 조립한다(`orca.md` 3절). grok 전용 문구는 찾지 못했다.

## 6. 원본 1.24.0이 Grok에서 하는 일 (새 설계의 기준선)

**설치**(PY README, `bin/fullops-squad.cjs`, `deps.py`):

1. `grok plugin marketplace add tourbut/Fullops-Squad-plugin`, `grok plugin install fullops-squad@fullops-squad`(`--trust` 없음). 통합 설치기(`npx … install --hosts grok`)가 같은 명령을 부른다.
2. `python3 <플러그인>/scripts/deps.py --host grok`: hook 실행 사본을 `~/.fullops-squad/runtime/`에 준비하고, 전역 npm 도구(`ocr`, `context7-mcp`)를 깔고, 공통 스킬을 `skills` CLI로 Codex·Claude Code·Grok 셋에 깔고, grok에만 mattpocock 스킬과 ponytail 스킬을 더 깔고, 완료 기록을 `<GROK_HOME 또는 ~/.grok>/fullops-deps.json`에 쓴다. grok에는 의존 플러그인을 깔지 않는다.
3. 갱신은 `grok plugin marketplace update`와 `grok plugin update fullops-squad` 뒤 `deps.py`를 다시 돌린다. 떠 있는 세션은 새 세션으로 바꾼다.

**패키지**: Claude Code와 **같은 폴더**(`dist/native/fullops-squad`)다. `hooks/hooks.json`(이벤트 9종), 스킬 10개, `.mcp.json`(Context7 stdio)이 들어 있다. grok 전용 등록 파일은 없다.

**실제로 도는 것**(등록 파일과 2절의 계약을 맞춰 본 것이다. 세션에서 확인한 것이 아니다):

| 원본의 hook | Grok에서 |
|---|---|
| SessionStart `caveman_start`, `flow_gate start` (matcher `startup\|clear`) | **불리지 않는다.** Grok의 값은 `new`·`load`다. `flow_gate start`가 하던 `.env` 연결도 되지 않는다(**추정**) |
| SessionStart `done_gate start` (matcher 없음) | 불린다. 그러나 출력이 `additionalContext`라 **모델에게 가지 않는다** |
| UserPromptSubmit `flow_gate prompt` | 불린다. 상태 기록은 된다. 문맥 주입은 버려진다 |
| PreToolUse `flow_gate tool` | 불린다. matcher에 `run_terminal_command`·`search_replace`·`spawn_subagent`가 있다. 거부가 동작한다 |
| PostToolUse `flow_gate post` | 불린다. `additionalContext`는 모델에게 간다. 긴 출력 줄이기는 "Codex·grok에서 동작하지 않는다"(1.22.0의 기록) |
| Stop `done_gate stop`, `flow_gate stop` | 불린다. 차단이 동작한다. `done_gate`는 `reason`이 `end_turn`이 아니면 통과한다(주석: "grok의 세션 종료 Stop"). 백그라운드 대기 판정은 snake_case `background_tasks`만 보므로 Grok에서는 늘 "포그라운드로 기다려라"가 나온다 |
| StopFailure `flow_gate failure` | 불린다(**추정**). 1.24.0의 기록은 "grok에서 동작하는지는 확인하지 못했다"다 |
| SessionEnd `issue_mode session-end` | 불린다. 플러그인의 SessionEnd는 최대 1.5초다 |

정리하면 원본은 Grok에 **hook 전부를 등록하지만, 거부·종료 차단·상태 기록만 유효하고 세션 시작 안내와 프롬프트 문맥은 모델에게 닿지 않는다.** 스킬과 MCP는 다 돈다. 폴더 신뢰는 사용자가 한 번 줘야 한다.

## 7. 확인하지 못한 것

| 항목 | 이유 | 확인하는 방법 |
|---|---|---|
| hook이 이 문서대로 실제로 불리는지(입력 필드, 별칭, SessionStart·UserPromptSubmit의 출력 처리) | 세션을 띄우지 않았다 | 실환경 점검에서 `FULLOPS_TRACE=1`로 입력을 한 번씩 받아 본다 |
| Windows에서 hook 명령을 실행하는 셸의 선택 순서 | 문서가 "찾아낸 셸"이라고만 한다 | `/hooks` 화면과 hook의 `$PSVersionTable`·`$BASH_VERSION` 출력으로 본다 |
| exec form의 `command`에서 `${GROK_PLUGIN_ROOT}`가 치환되는지 | 문서는 인자의 치환만 분명히 적는다 | `grok plugin validate`와 `/hooks` 화면 |
| 바이너리가 든 플러그인이 설치되는지, POSIX에서 실행 권한이 남는지 | 설치를 하지 않았다 | `grok plugin validate <패키지>` 뒤 로컬 경로로 한 번 설치한다 |
| `fullops-squad@fullops-squad` 표기의 뜻 | 도움말은 `@`를 ref라고 하고 원본은 `이름@마켓플레이스`로 쓴다. 원본에서 동작한다는 기록만 있다 | 이름만으로 설치해 본다 |
| Grok이 Claude Code가 깐 플러그인(`~/.claude/plugins/` 아래)을 스스로 읽는지 | 문서는 "`.claude/plugins/`의 같은 자리도 된다"고만 한다. 읽는다면 Claude용 fullops의 hook이 Grok에서 `hook claude`로 불릴 수 있다 | `grok inspect --json`의 플러그인 목록 |
| `write`·`hashline_edit`의 정확한 인자 | 문서에 표가 없다 | PreToolUse 입력을 받아 본다 |
| Windows에서 `run_terminal_command`의 셸 문법 | 문서에 PowerShell만 나온다 | Intent 분류 결과를 본다 |
| 백그라운드 명령이 끝났을 때 실제로 세션이 깨어나는지 | 안내서로만 확인했다. 원본의 판단과 다르다 | `fullops wait`를 백그라운드로 걸어 본다 |
| `grok --trust` 옵션 | 안내서에는 있고 도움말에는 없다 | 해당 없음(쓰지 않는다) |
