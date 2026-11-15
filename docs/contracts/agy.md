# agy(Antigravity CLI) 계약 (플러그인·hook·스킬·능력)

- 확인한 날: 2026-11-13~14. 실행 파일 이름은 `agy`다(`antigravity`라는 명령은 이 PC의 PATH에 없다). 버전은 `1.4.8`이다(`agy --version`).
- 확인 방법: 설치본에 든 내장 스킬의 문서, 공식 웹 문서, `agy --help` 계열, 이 PC의 `~/.gemini` 설정 폴더(읽기만), 실행 파일에 든 문자열, Orca 설치본의 코드, 원본 플러그인 v1.24.0의 문서와 스크립트를 읽었다. **세션을 띄워 hook을 실제로 돌려 보지는 않았다.** 이 PC의 agy는 로그인돼 있지 않다(CLI 로그에 "You are not logged into Antigravity"가 남는다).
- 표기: **사실**은 출처가 있는 것, **추정**은 미루어 본 것, **미확인**은 확인하지 못한 것이다.

| 약어 | 출처 |
|---|---|
| AB | `~/.gemini/antigravity-cli/builtin/skills/agy-customizations/`의 `SKILL.md`와 `docs/{hooks,plugins,skills,mcp_servers,json_configs,rules}.md`, `builtin/skills/plugin/SKILL.md` (1.4.8에 딸린 문서) |
| WH | https://antigravity.google/docs/hooks |
| WP | https://antigravity.google/docs/plugins?tab=cli (`/docs/cli/plugins/`가 이곳으로 넘어간다), https://antigravity.google/docs/build-plugins/ |
| WS | https://antigravity.google/docs/skills, https://antigravity.google/docs/cli/reference, https://antigravity.google/docs/features?tab=cli |
| HELP | `agy --help`, `agy plugin help`, `agy mcp --help`, `agy mcp add --help` |
| BIN | `agy.exe`에 든 문자열. 문서끼리 다를 때 어느 쪽이 이 버전인지 가릴 때만 썼다 |
| HOME | 이 PC의 `~/.gemini/config/{config.json,hooks.json,mcp_config.json}`(키만), `~/.gemini/config/plugins/`, `~/.gemini/antigravity-cli/settings.json` |
| ORCA | Orca 1.4.256 설치본의 코드와 `~/.orca/agent-hooks/antigravity-*.cmd`, `antigravity-hook-post.cjs` |
| FORUM | https://discuss.ai.google.dev/t/…/176814 (사용자 보고. 공식 답변은 없다) |
| PY | 원본 v1.24.0의 README, `docs/development.md`, `docs/releases/`, `scripts/deps.py`, `scripts/install.py`, `scripts/build.py` |
| LOCAL | 이 PC의 임시 폴더에서 `cmd.exe`·`sh`로 직접 해 본 것 |

## 0. 주의: `agy plugin <하위 명령> --help`는 도움말이 아니다

**사실**(이 묶음에서 겪었다): `agy plugin`의 하위 명령은 `--help`를 인자 값으로 받는다.

| 실행한 것 | 결과 |
|---|---|
| `agy plugin install --help` | `Error: install target must be a directory: --help` |
| `agy plugin list --help` | `Error: unknown list option "--help": only --json is supported` |
| `agy plugin enable --help` | `Error: plugin "--help" not found or invalid` |
| `agy plugin uninstall --help` | **`Uninstalled plugin "--help"`를 출력하고 `~/.gemini/config/import_manifest.json`을 `{"imports": null}`로 새로 썼다** |

설치된 플러그인 두 개(`agy plugin list --json`)와 `config.json`, `plugins/` 폴더는 그대로였다. 도움말은 `agy plugin help`와 `agy mcp <하위 명령> --help`만 안전하다. fullops의 설치 명령과 `doctor`는 agy의 플러그인 하위 명령에 `--help`를 붙이지 않아야 한다.

## 0.1 실환경 확인(2026-11-15, agy CLI의 `--prompt` 세션)

아래는 이 문서의 다른 절보다 앞선다. 문서와 실행 파일에서 옮긴 내용 가운데 실제와 다른 것이 있었다.

| 항목 | 이 문서가 적었던 것 | 실제 |
|---|---|---|
| 파일 쓰기 도구 | `write_to_file{TargetFile, CodeContent}` | `write_file{Path, Content, Overwrite}` |
| 파일 수정 도구 | `replace_file_content{TargetFile, TargetContent, ReplacementContent}` | `edit_file{Path, Target, Replacement}` |
| 셸 도구 | `run_command{CommandLine}` | 같다(`WaitMsBeforeAsync`, `toolSummary`가 더 온다) |
| PreToolUse `matcher` | 정규식으로 추정(`a\|b`) | 정규식이 아니다. `"*"`와 도구 이름 하나는 맞고, `"a\|b"`와 `".*"`는 어떤 도구에도 맞지 않았다. 그래서 fullops는 도구 이름마다 한 항목을 등록한다(`AGY_PRE_TOOLS`) |
| 입력의 이벤트 이름·작업 위치 | 없다 | `hookEventName`과 `cwd`가 온다(`workspacePaths`도 온다). fullops는 여전히 `--event`와 `workspacePaths[0]`을 쓴다 |

- 확인한 방법: 점검 레포의 `.agents/hooks.json`에 모든 도구(`"*"`)의 PreToolUse 입력을 파일로 남기는 hook을 두고 셸·쓰기·수정을 한 번씩 시켰다. 그 입력으로 `scenarios/fixtures/host/agy/pre_tool_use.{write_file,edit_file}.json`을 만들었다.
- 고치기 전에는 fullops의 agy PreToolUse hook이 한 번도 불리지 않았다(매처 문법). 불렸더라도 쓰기·수정 도구 이름을 몰라 통과시켰을 것이다.
- 고친 뒤 같은 세션에서 `write_file`로 과제명 인박스를 만들면 T-05, `run_command`로 날것 `orca orchestration worker-start`를 실행하면 T-01로 거부되는 것을 확인했다. SessionStart의 안내는 모델에 닿고, Stop의 차단(X-07)도 동작한다.
- 플러그인 등록 문자열의 `"bin/fullops"`(확장자 없는 상대 경로)는 Windows의 `cmd /c`가 `bin/fullops.exe`로 풀어 실행한다(플러그인 루트에서 확인).

## 1. 플러그인

### 1.1 사실

1. **플러그인 개념이 있다.** 스킬·에이전트·규칙·MCP 서버·hook을 한 묶음으로 설치하고 켜고 끈다(AB `plugins.md`, WP).
2. **배치**(AB, WP "build-plugins"):

   ```
   <플러그인>/
     plugin.json            # 필수. 또는 .antigravity-plugin/plugin.json(여러 호스트용 저장소. 이쪽이 우선한다)
     hooks.json             # 선택. 반드시 플러그인 루트에 둔다
     mcp_config.json        # 선택
     skills/<이름>/SKILL.md
     agents/<이름>.md
     rules/AGENTS.md        # 플러그인이 켜져 있는 동안 늘 적용되는 규칙
     scripts/  assets/logo.svg  README.md
   ```

3. **`plugin.json`**: `name`(소문자 kebab-case. CLI와 마켓플레이스에서는 필수), `displayName`, `description`, `version`, `logo`, `suggestedPrompts`(최대 3개), `skills`(추가 스킬 폴더), `disabled`, `author`, `homepage`, `repository`, `license`, `keywords`. 스키마는 최상위 `additionalProperties: false`다(WP). 런타임은 모르는 최상위 필드를 조용히 버린다(AB `plugin/SKILL.md`). `agy plugin install`은 주석과 끝 쉼표가 없는 엄격한 JSON만 읽는다(BIN의 오류 문구).
4. **명령**(HELP `agy plugin help`, WP):

   | 일 | 명령 |
   |---|---|
   | 설치 | `agy plugin install <이름[@마켓플레이스] \| https://github.com/<owner>/<repo> \| 로컬 경로>` |
   | 제거 | `agy plugin uninstall <이름>` |
   | 켜기·끄기 | `agy plugin enable <이름>`, `agy plugin disable <이름>` |
   | 목록 | `agy plugin list [--json]` |
   | 검사 | `agy plugin validate [경로] [--strict] [--json]` |
   | 다른 도구에서 가져오기 | `agy plugin import [gemini \| claude \| 경로]` (`~/.gemini/extensions/`, `~/.claude/plugins/`) |
   | 마켓플레이스 링크 | `agy plugin link <mp> <target>` ("Generate link to a marketplace". 뜻은 **미확인**) |

5. **설치 위치**: CLI는 `~/.gemini/config/plugins/<이름>/`이다. 워크스페이스 범위는 프로젝트의 `.agents/plugins/`다(WP). 켜짐 여부는 `~/.gemini/config/config.json`의 `plugins.<폴더 이름>.enabled`에 적힌다(AB, HOME). 새로 설치한 플러그인 폴더는 다시 시작해야 보인다(AB `plugin/SKILL.md`).
6. **마켓플레이스**: 공식 마켓플레이스 이름은 `antigravity-plugins-official` 하나다. 이름으로 설치하려면 agy에 로그인돼 있어야 한다. 등록은 신청 양식으로 한다(WP). **제3자 마켓플레이스를 등록하는 방법은 문서에 없다**(**미확인**. 실행 파일에는 `marketplace.json`을 읽는 코드의 오류 문구가 있다). 원본도 "agy의 마켓플레이스 등록은 확인하지 못했다"고 적었다(PY `docs/releases/0.8.0.md`). **갱신 명령도 문서에 없다.** 다시 설치하는 것으로 본다(**추정**).
7. **신뢰·승인**: 플러그인 hook에 대한 별도 승인 절차는 문서에 없다. 변수를 요구하는 MCP가 든 플러그인은 꺼진 채로 설치되고, 켤 때 값을 묻는다(WP).

### 1.2 Claude Code 형식을 그대로 읽는가

**사실**(AB `plugins.md`의 NOTE, BIN의 `validate` 설명):

- `agy plugin install`은 **순수한** Claude Code 플러그인(`.claude-plugin/plugin.json`만 있는 것)이나 Gemini CLI 확장(`gemini-extension.json`)을 가져올 때 `hooks/hooks.json`을 `<플러그인>/hooks.json`으로 옮겨 놓는다.
- **네이티브 플러그인**(`plugin.json`, `.antigravity-plugin/plugin.json`, `.gemini-plugin/plugin.json` 가운데 하나가 있는 폴더)은 그 옮기기를 하지 않는다. **`hooks/hooks.json`은 읽지 않는다.** `validate`는 이것을 경고한다("a `hooks/hooks.json` file (which isn't read)").
- 플러그인 루트의 `hooks.json`에 Claude Code식 최상위 `"hooks"` 키가 있으면 **이름이 `hooks`인 hook 묶음 하나**로 읽힌다(`validate`가 경고한다). 이벤트 이름과 입출력 계약이 다르므로(2절) Claude Code용 hook이 그대로 동작하지는 않는다.
- `commands/` 폴더와 `workflows/`의 Markdown은 플러그인에서 읽지 않는다.
- 스킬(`skills/<이름>/SKILL.md`)과 MCP(`mcp_config.json`)는 형식이 같아 그대로 읽힌다. MCP 파일 이름은 `.mcp.json`이 아니라 `mcp_config.json`이다.

### 1.3 네이티브 바이너리

**사실**(WP "build-plugins"): `scripts/` 폴더의 파일은 마켓플레이스 묶음에서 실행 권한(0755)을 받는다. 문서는 "여러 OS에서 도는 Node.js 스크립트"를 권한다. 컴파일된 바이너리를 넣어도 되는지는 적혀 있지 않다(**미확인**). 로컬 경로나 GitHub에서 설치할 때 `bin/`의 실행 권한이 남는지도 모른다.

## 2. hook

### 2.1 사실: hook을 지원한다

agy에는 hook이 있다. 다만 이벤트 이름·입력·출력이 Claude Code와 다르다. hook은 **CLI에서만 실행된다는 보고**가 있다(FORUM: IDE 2.1.1과 데스크톱 2.5.0에서는 0회, `agy` CLI 1.1.10에서는 전역 PreToolUse hook이 모든 도구 호출에 불리고 `{"decision":"deny"}`로 막았다). 공식 문서는 플러그인의 `hooks.json`을 "CLI only"라고 적는다(WH).

### 2.2 등록 파일

**사실**(AB `hooks.md`, WH):

| 범위 | 경로 |
|---|---|
| 전역 | `~/.gemini/config/hooks.json` |
| 워크스페이스 | `<프로젝트>/.agents/hooks.json` (폴더를 신뢰해야 한다는 것은 FORUM의 보고다) |
| 플러그인 | `<플러그인 루트>/hooks.json` |

형식은 **최상위 키가 hook 묶음의 이름**이다. Claude Code의 최상위 `"hooks"`가 없다.

```json
{
  "fullops": {
    "SessionStart":  [ { "type": "command", "command": "…", "timeout": 10 } ],
    "PreToolUse":    [ { "matcher": "run_command|write_to_file", "hooks": [ { "type": "command", "command": "…", "timeout": 8 } ] } ],
    "Stop":          [ { "type": "command", "command": "…", "timeout": 20 } ]
  }
}
```

- `SessionStart`·`PreInvocation`·`PostInvocation`·`Stop`은 **handler를 바로 늘어놓는다**(matcher 묶음이 없다). `PreToolUse`·`PostToolUse`만 `matcher`와 `hooks`로 감싼다.
- 묶음에 `"enabled": false`를 두면 꺼진다. 여러 묶음의 같은 이벤트는 합쳐져 차례로 돈다.
- handler의 필드는 `type`(`command`만), `command`, `timeout`(정수 초, 기본 30)뿐이다. HTTP·프롬프트 hook은 없다. hook은 동기로 돌고 에이전트 루프를 막는다.
- **셸**: Unix는 `sh -c`, **Windows는 `cmd /c`**다. `~`는 홈 폴더로 풀어 준다.
- **작업 디렉터리는 `hooks.json`이 있는 폴더**다. 플러그인의 hook은 플러그인 루트에서 돈다. 문서는 "`${PLUGIN_ROOT}`가 아니라 상대 경로를 쓰라"고 한다(WP "build-plugins"). hook 명령에 주어지는 경로 변수는 문서에 없다(MCP 설정의 `args`·`env`·`cwd`에서만 `${PLUGIN_ROOT}`·`${PLUGIN_DATA}`가 풀린다).
- matcher는 정규식이다. `""`와 `"*"`는 전부다.
- 이 PC의 전역 `hooks.json`에는 Orca가 넣은 `orca-status` 묶음이 있다(HOME). 이벤트마다 다른 `.cmd` 파일을 등록했다.

**확인한 것**(LOCAL): 플러그인 루트에 확장자 없는 POSIX 런처 `bin/fullops`와 `bin/fullops.exe`를 함께 두면, 명령 `"bin/fullops" hook agy --event Stop`은 `cmd.exe /c`에서 `fullops.exe`를, `sh -c`에서 런처를 실행한다. 따옴표 없는 `bin/fullops …`와 `./bin/fullops …`는 cmd에서 실패한다(`/`가 옵션으로 읽힌다). agy가 `cmd /c`에 줄을 어떻게 넘기는지는 돌려 보지 않았다.

### 2.3 이벤트

**사실**(AB `hooks.md`, WH):

| 설계의 이벤트 | agy | 비고 |
|---|---|---|
| SessionStart | `SessionStart` | 새 대화가 시작할 때 한 번. `source`가 없다. 이어 하기(`--continue`, `--conversation`)에서 불리는지는 **미확인** |
| UserPromptSubmit | **없다** | 가장 가까운 것은 `PreInvocation`(모델을 부르기 직전마다. 한 턴에 여러 번)이다. 사용자의 마지막 입력은 **모든 이벤트의 공통 필드 `lastUserInput`**에 온다 |
| PreToolUse | `PreToolUse` | 도구 이름과 인자가 다르다(2.6절) |
| Stop | `Stop` | 실행 루프가 끝날 때. 끝난 이유가 `terminationReason`에 온다 |
| StopFailure | **없다** | `Stop`의 `terminationReason == "ERROR"`와 `error`가 같은 구실을 한다 |

그 밖에 `PostToolUse`, `PreInvocation`, `PostInvocation`이 있다.

### 2.4 입력

**사실**(AB `hooks.md`, WH): 입력은 stdin의 JSON이고 키는 전부 **camelCase**다. **snake_case 별칭이 없고, 이벤트 이름이 입력에 없다.** 어느 이벤트인지는 등록한 명령이 스스로 알아야 한다. Orca가 이벤트마다 다른 `.cmd` 파일을 등록하고 그 안에서 환경 변수로 이벤트 이름을 정하는 것도 이 때문이다(ORCA).

| 필드 | 이벤트 | 설명 |
|---|---|---|
| `conversationId` | 전부 | 대화 ID. 설계의 세션 ID 자리다 |
| `workspacePaths` | 전부 | **배열**이다. `cwd` 필드는 없다 |
| `transcriptPath`, `artifactDirectoryPath`, `modelName` | 전부 | CLI의 기록은 `~/.gemini/antigravity-cli/brain/<conversationId>/.system_generated/logs/transcript.jsonl`이다 |
| `lastUserInput` | 전부(SessionStart의 설명에는 빠져 있다) | 사용자의 마지막 입력 |
| `toolCall.name`, `toolCall.args`, `stepIdx` | PreToolUse, PostToolUse | PostToolUse에는 실패했을 때 `error`가 더 온다 |
| `invocationNum`, `initialNumSteps` | PreInvocation, PostInvocation | |
| `executionNum` | Stop | 실행 루프의 번호 |
| `terminationReason` | Stop | `NO_TOOL_CALL`(정상 종료), `MAX_INVOCATIONS`, `MAX_FORCED_INVOCATIONS`, `MAX_TOKEN_BUDGET_EXCEEDED`, `ERROR`, `USER_CANCELED`, `TERMINAL_STEP_TYPE`, `TERMINAL_CUSTOM_HOOK`, `INJECTED_RESPONSE` |
| `error` | Stop | 오류로 끝났을 때의 글 |
| `fullyIdle` | Stop | 백그라운드 작업이 전부 끝났으면 `true` |

### 2.5 출력

**사실**(AB `hooks.md`, WH):

| 설계의 Verdict | agy의 출력 | 비고 |
|---|---|---|
| `Pass` (PreToolUse) | `{"decision":"allow"}` | **`decision`이 필수다.** 값의 뜻은 아래 |
| `Pass` (Stop) | `{"decision":""}` 또는 `continue`가 아닌 아무 값 | `decision`이 필수라고 적혀 있다 |
| `Pass` (그 밖) | `{}` | |
| `Context` (SessionStart) | `{"injectSteps":[{"systemMessage":{"systemMessage":t}}]}` | 대화 기록에 남는다. `systemMessage`는 문자열이 아니라 객체다. `{"userMessage":t}`, `{"ephemeralMessage":t}`(이번 턴 뒤 사라진다)도 있다 |
| `Context` (UserPromptSubmit) | 해당 이벤트가 없다 | `PreInvocation`의 `injectSteps`가 같은 구실을 할 수 있다 |
| `Warn` | `{"injectSteps":[{"errorMessage":t}]}` | 모델에게 보내지 않고 오류로만 보인다(WH에만 있고 AB에는 없다). `injectSteps`를 받는 이벤트(SessionStart·PreInvocation·PostInvocation)에서만 된다. **PreToolUse와 Stop에는 사용자에게만 보이는 통로가 없다** |
| `Deny` | `{"decision":"deny","reason":r}` | 도구 호출을 바로 막는다. `reason`은 사용자와 에이전트에게 보인다 |
| `Block` | `{"decision":"continue","reason":r}` | 멈추지 않고 루프로 돌아간다. `reason`은 시스템 메시지로 들어간다. 이어 가기 횟수의 상한은 `MAX_FORCED_INVOCATIONS`라는 종료 이유가 있다는 것만 안다(값은 **미확인**) |

**PreToolUse `decision` 값의 뜻이 문서마다 다르다.**

| 값 | 설치본 문서(AB) | 웹 문서(WH) |
|---|---|---|
| `allow` | "Automatically allow the tool execution" | "proceed, deferring to the next hook or default policy" |
| `auto_approve` | 없다 | 기본 정책이 물어볼 호출을 승인한다 |
| `deny` | 바로 막는다 | 같다 |
| `ask` | 사용자에게 묻는다("항상 허용" 기록을 따른다) | 같다 |
| `force_ask` | 늘 묻는다 | 같다 |
| `deny_unless_prior_grant` | 없다 | 전에 승인된 적이 없으면 막는다 |

1.4.8의 실행 파일에는 `auto_approve`와 `deny_unless_prior_grant`가 들어 있다(BIN). 이 버전은 웹 문서의 여섯 값 쪽이고 `allow`는 "의견 없음"일 것이다(**추정**). Orca는 자기 상태 표시용 PreToolUse hook에서 `{"decision":"ask"}`를 낸다(ORCA). **`allow`가 권한 확인을 건너뛰게 하는지는 fullops가 통과를 낼 때마다 닿는 문제라 실환경에서 먼저 확인해야 한다.**

hook이 죽거나 시간을 넘기거나 잘못된 JSON을 냈을 때 agy가 통과시키는지 막는지는 문서에 없다(**미확인**). Orca의 스크립트는 다른 일을 하기 전에 먼저 유효한 응답을 출력한다.

### 2.6 도구 이름과 인자

**사실**(WH의 도구 목록, AB의 예시): 도구 이름은 `view_file`, `write_to_file`, `replace_file_content`, `multi_replace_file_content`, `list_dir`, `find_by_name`, `grep_search`, `search_web`, `read_url_content`, `run_command`, `manage_task`, `schedule`, `list_permissions`, `ask_permission`, `invoke_subagent`, `define_subagent`, `send_message`, `manage_subagents`, `ask_question`, `generate_image`다. 인자 이름은 **PascalCase**다. 문서의 예시는 `run_command`의 `CommandLine` 하나다.

| 종류 | `toolCall.name` | 인자 |
|---|---|---|
| 셸 | `run_command` | `CommandLine`(사실). 백그라운드 전환에 `WaitMsBeforeAsync`가 쓰인다(**추정**: BIN에 이름이 있다) |
| 쓰기 | `write_to_file` | `TargetFile`, `CodeContent`(**추정**: BIN에 이름이 있고 문서에는 없다) |
| 편집 | `replace_file_content` | `TargetFile`, `TargetContent`, `ReplacementContent`(**추정**) |
| 편집(여러 곳) | `multi_replace_file_content` | `TargetFile`, `ReplacementChunks[]`(**추정**) |
| 하위 에이전트 | `invoke_subagent` | **미확인** |

**미확인**: Windows에서 `run_command`의 `CommandLine`이 어느 셸 문법인지.

## 3. 스킬

**사실**(WS `docs/skills`, AB):

| 범위 | 경로 |
|---|---|
| 전역(CLI·앱 공용) | `~/.gemini/config/skills/<이름>/` |
| 전역(CLI만 더 읽는다) | `~/.gemini/antigravity-cli/skills/`, `~/.gemini/skills/` |
| 플러그인 | `~/.gemini/config/plugins/<플러그인>/skills/` |
| 워크스페이스 | `<프로젝트>/.agents/skills/<이름>/`와 저장소 루트까지의 상위 폴더들(`.agent/skills`도 받는다) |

- **전역에서 `~/.agents/skills`는 읽지 않는다.** 문서의 전역 경로 목록에 없다. 원본의 주석("agy는 전역에서 `~/.agents/skills`를 읽지 않는다")과 맞고, `skills` CLI도 antigravity 대상을 "전역에서 공용 폴더를 읽지 않는다"로 표시한다(`globalReadsCanonical: false`).
- `SKILL.md`의 frontmatter는 `name`(선택. 없으면 폴더 이름), `description`(필수)이다. CLI에서는 스킬 하나가 슬래시 명령 하나가 된다. 스킬을 더한 뒤에는 `/skills reload`나 새 대화가 필요하다.
- 규칙은 `AGENTS.md`·`GEMINI.md`(디렉터리 계층)와 `.agents/rules/*.md`다. 파일당 24,000바이트, 합계 20,000 토큰이 한도다.
- 이 PC의 `~/.gemini/config/skills/`에는 Orca가 깐 스킬 넷(`orca-cli`, `orchestration` 등)이 있다(HOME).

**스킬 설치 도구**(PY `deps.py`, vercel-labs/skills의 `src/agents.ts`): 원본은 `npx --yes skills@latest add <출처> --skill <이름…> --global --agent antigravity-cli --yes`를 부른다. `skills` CLI의 `antigravity-cli` 대상은 프로젝트 `.agents/skills`, 전역 `~/.gemini/config/skills`이고 `~/.gemini/antigravity-cli`가 있으면 설치된 것으로 본다. 원본의 검사는 `~/.gemini/config/skills`, `~/.gemini/antigravity-cli/skills`, `~/.gemini/skills` 세 곳에서 찾는다(1.17.0).

## 4. 능력

| 항목 | 결과 | 출처 |
|---|---|---|
| 백그라운드 명령이 끝나면 세션을 깨우는가 | **미확인.** Stop 입력의 `fullyIdle`("모든 백그라운드 작업이 끝났으면 true")로 백그라운드 작업이 있다는 것만 안다. 문서는 깨우는지 말하지 않는다 | AB, WS |
| 하위 에이전트 도구 | 있다. `invoke_subagent`(그리고 `define_subagent`, `manage_subagents`, `send_message`). 하위 에이전트는 자기 대화에서 병렬로 돈다 | WH, WS |
| StopFailure | 없다. `Stop`의 `terminationReason == "ERROR"`로 대신한다 | AB |
| MCP 등록 | `agy mcp add [--type stdio\|http] [--env K=V] [--header "이름: 값"] <이름> <명령 또는 URL> [인자…]`. 플래그는 이름 앞에 와야 한다. 전역 설정은 `~/.gemini/config/mcp_config.json`, 플러그인은 `mcp_config.json`이다. **원격은 Streamable HTTP만 된다**(`{"url": "https://…"}`. 옛 SSE는 안 된다). 플러그인의 서버 이름은 `<플러그인>_<서버>`로 바뀐다 | HELP, AB `mcp_servers.md`, WP |
| 워크스페이스 신뢰 | `~/.gemini/antigravity-cli/settings.json`의 `trustedWorkspaces`에 CLI가 물어서 적는다 | WS, HOME |

## 5. Orca와의 관계

**사실**(ORCA):

- Orca의 에이전트 ID는 **`antigravity`**다(`agy`가 아니다). `orca orchestration worker-start --agent antigravity`로 띄우고 실행 파일은 `agy`다. `--model`과 `--effort`를 받는다.
- Orca는 첫 프롬프트를 실행 옵션으로 넘긴다. 코드의 값은 `promptInjectionMode: "flag-prompt-interactive"`이고, agy의 `--prompt-interactive`(`-i`)를 쓰는 것으로 보인다(**추정**: 옵션 이름까지는 읽지 않았다).
- Orca에는 antigravity용 워크스페이스 신뢰 사전 처리(`preflightTrust: "antigravity"`)가 있다. Orca의 설정이 켜져 있으면 띄우기 전에 신뢰를 적어 준다(**추정**: 처리 함수의 이름과 호출 자리만 읽었다).
- Orca는 자기 상태 표시용 hook을 `~/.gemini/config/hooks.json`의 `orca-status` 묶음으로 넣는다. `PreInvocation`·`PostInvocation`·`Stop`·`PreToolUse`·`PostToolUse`를 등록하고, 스크립트는 Stop에 `{"decision":""}`, PreToolUse에 `{"decision":"ask"}`, 그 밖에 `{}`를 먼저 출력한 뒤 입력을 Orca에 보낸다. 스크립트가 `ORCA_PANE_KEY` 등을 환경에서 읽으므로 agy의 hook 프로세스는 세션의 환경 변수를 물려받는다.
- preamble은 호스트와 무관하게 Orca가 조립한다(`orca.md` 3절). agy에는 UserPromptSubmit이 없으므로 preamble은 `lastUserInput`으로만 볼 수 있다.

## 6. 원본 1.24.0이 agy에서 하는 일 (새 설계의 기준선)

**설치**(PY README, `docs/development.md`, `scripts/install.py`, `deps.py`):

1. 일반 설치 경로가 없다. 통합 설치기(`bin/fullops-squad.cjs`)의 대상은 Codex·Claude Code·grok 셋이고 agy는 빠져 있다. README는 "개발 체크아웃·agy 설치는 개발 문서를 따른다"고 적는다.
2. 개발 체크아웃에서 `python3 scripts/install.py --host agy`를 돌린다. 빌드한 뒤 스킬을 `skills` CLI로 `--agent antigravity-cli`에 깔고, `agy plugin install <체크아웃>/dist/native/fullops-squad`(로컬 폴더)를 실행한다. 완료 기록은 `~/.agy/fullops-deps.json`에 쓴다.

**패키지**: Claude Code·Codex·Grok과 같은 폴더(`dist/native/fullops-squad`)다. 루트에 `plugin.json`이 있고 hook은 `hooks/hooks.json`에만 있다. `mcp_config.json`(Context7 stdio)이 agy용으로 들어 있다.

**실제로 도는 것**(등록 파일과 1.2절의 계약을 맞춰 본 것이다):

| 원본의 것 | agy에서 |
|---|---|
| hook 전부 | **돌지 않는다.** 루트에 `plugin.json`이 있어 네이티브 플러그인으로 읽히고, 네이티브 플러그인의 `hooks/hooks.json`은 읽지 않는다. 설령 읽혔어도 이벤트·입력·출력이 달라 판정이 통하지 않는다 |
| 스킬 10개 | 플러그인의 `skills/`로 읽힌다. 스킬 문서가 가리키는 Python 스크립트는 그대로 쓸 수 있다 |
| MCP(Context7) | `mcp_config.json`으로 읽힌다 |
| 규칙 | 레포의 `AGENTS.md`·`GEMINI.md`가 agy의 규칙으로 읽힌다 |

정리하면 원본은 agy에 **스킬과 MCP만** 제공한다. 세션 시작 안내, 거부, 종료 차단, 상태 기록이 전부 없다. 원본의 기록에도 "실제 agy 세션으로는 확인하지 않았다"(1.17.0)가 남아 있다. 이 PC의 agy에는 fullops가 설치돼 있지 않다(`agy plugin list --json`에 없고 `~/.agy`도 없다).

## 7. 확인하지 못한 것

| 항목 | 이유 | 확인하는 방법 |
|---|---|---|
| hook이 이 문서대로 실제로 불리는지 | 세션을 띄우지 않았고 로그인돼 있지 않다 | 실환경 점검에서 이벤트별 입력을 한 번씩 받아 본다 |
| PreToolUse에서 `{"decision":"allow"}`가 권한 확인을 건너뛰게 하는지 | 두 문서가 다르게 적는다 | 승인이 필요한 명령에 `allow`를 내고 묻는지 본다. 건너뛴다면 통과 출력을 `ask`로 바꾼다 |
| hook이 죽거나 시간을 넘겼을 때 통과인지 | 문서에 없다 | 일부러 종료 코드 1과 빈 출력을 내 본다 |
| 플러그인 hook의 작업 디렉터리가 플러그인 루트인지, `cmd /c`에 줄이 어떻게 넘어가는지 | 문서로만 확인했다 | `"bin/fullops" hook agy --event SessionStart`를 등록해 본다 |
| 바이너리가 든 플러그인이 설치되는지, POSIX에서 실행 권한이 남는지 | 설치를 하지 않았다 | `agy plugin validate --strict <패키지>` 뒤 로컬 경로로 한 번 설치한다 |
| 제3자 마켓플레이스 등록, `agy plugin link`의 뜻, 갱신 방법 | 문서에 없다 | 공식 문서의 Marketplace 안내를 다시 본다 |
| SessionStart가 이어 하기에서 불리는지, `executionNum`이 사용자 턴마다 하나씩 느는지 | 문서에 없다 | 입력을 받아 본다 |
| 편집 도구와 `invoke_subagent`의 인자 | 문서에 `CommandLine`만 있다 | PreToolUse 입력을 받아 본다 |
| 백그라운드 명령이 끝났을 때 세션이 깨어나는지 | 문서에 없다 | `fullops wait`를 백그라운드로 걸어 본다 |
| Windows에서 `run_command`의 셸 문법 | 문서에 없다 | Intent 분류 결과를 본다 |
| `agy plugin uninstall --help`가 새로 쓴 `import_manifest.json`의 이전 내용 | 파일의 만든 시각과 고친 시각이 같아 그때 처음 생긴 것으로 보이지만 확인할 길이 없다 | 사용자가 `agy plugin import`를 쓴 적이 있으면 가져온 기록을 한 번 확인한다 |
