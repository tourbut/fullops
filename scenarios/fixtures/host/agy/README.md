# agy(Antigravity CLI) hook 표본

agy 1.4.8이 hook에 주는 입력 JSON과 hook이 내는 출력 JSON의 형식 표본이다. 근거는 `docs/contracts/agy.md`다.

- 값은 전부 가짜다. 실제 대화나 프롬프트를 옮긴 것은 없다.
- 형식은 설치본에 든 내장 문서와 공식 웹 문서의 필드 표·예시를 따랐다. **실제 호스트에서 받아 본 것이 아니다.**
- **입력에 이벤트 이름이 없다.** 어느 이벤트인지는 등록한 명령의 인자(`fullops hook agy --event <이름>`)로 안다. 파일 이름이 그 이벤트다.
- 키는 전부 camelCase다. `cwd`가 없고 `workspacePaths` 배열이 온다. 세션 ID 자리는 `conversationId`다.
- `workspacePaths`, `transcriptPath`, `artifactDirectoryPath`, `TargetFile`은 자리 채움 값이다. 시험은 임시 레포의 경로로 바꿔 넣는다.
- 호스트는 여기 없는 필드를 더 줄 수 있다. 읽는 쪽은 모르는 필드를 무시해야 한다.

## 입력

| 파일 | 이벤트 | 내용 | 확인 수준 |
|---|---|---|---|
| `session_start.json` | SessionStart | 공통 필드만 온다. `lastUserInput`이 없다 | 문서의 필드 목록 |
| `pre_invocation.json` | PreInvocation | 모델을 부르기 직전마다 온다. UserPromptSubmit에 가장 가까운 이벤트다 | 문서의 예시 |
| `pre_tool_use.run_command.json` | PreToolUse | 셸 도구. 명령은 `toolCall.args.CommandLine`이다 | 문서의 예시 |
| `pre_tool_use.run_command_orca_dispatch.json` | PreToolUse | `lastUserInput`에 Orca의 배정 프롬프트(preamble)가 들어 있는 경우. 프롬프트 이벤트가 없으므로 dispatch는 여기서 묶는다 | 조립한 것이다. 프롬프트 글은 `codex/user_prompt_submit.orca_dispatch.json`과 같다. preamble이 `lastUserInput`에 그대로 오는지는 확인하지 못했다 |
| `pre_tool_use.write_to_file.json` | PreToolUse | 쓰기 도구 | **추정**. 인자 `TargetFile`·`CodeContent`는 실행 파일에 든 이름이고 문서에는 없다 |
| `pre_tool_use.replace_file_content.json` | PreToolUse | 편집 도구 | **추정**. 인자 `TargetFile`·`TargetContent`·`ReplacementContent`도 같다 |
| `pre_tool_use.invoke_subagent.json` | PreToolUse | 하위 에이전트 도구 | **추정**. 도구 이름은 문서에 있다. 인자 `Prompt`는 지어낸 것이다 |
| `stop.json` | Stop | 정상 종료(`terminationReason: "NO_TOOL_CALL"`, `fullyIdle: true`) | 문서의 예시 |
| `stop.background.json` | Stop | 백그라운드 작업이 남아 있다(`fullyIdle: false`) | 문서의 필드 설명 |
| `stop.error.json` | Stop | 오류로 끝났다(`terminationReason: "ERROR"`, `error`에 글). StopFailure가 없는 agy에서 그 구실을 한다 | 문서의 필드 설명. `error`의 글은 지어낸 것이다 |
| `stop.user_canceled.json` | Stop | 사용자가 끊었다(`USER_CANCELED`) | 문서의 값 목록 |

`multi_replace_file_content` 표본은 없다. `ReplacementChunks` 항목의 모양을 확인하지 못했기 때문이다.

## `output/` — hook이 내는 형식

한 줄 JSON이다. `<…>`는 문구가 들어갈 자리다. Claude Code의 형태(`hookSpecificOutput`, `decision: "block"`)는 agy에서 통하지 않는다.

| 파일 | Verdict | 비고 |
|---|---|---|
| `pass.pre_tool_use.json` | `Pass` (PreToolUse) | `decision`이 필수다. `allow`가 권한 확인을 건너뛰게 하는지는 **확인하지 못했다**. 건너뛴다면 `ask`로 바꿔야 한다 |
| `pass.stop.json` | `Pass` (Stop) | `continue`가 아닌 값이면 멈춘다 |
| `pass.json` | `Pass` (SessionStart와 그 밖) | |
| `deny.pre_tool_use.json` | `Deny` | |
| `block.stop.json` | `Block` | `decision`은 `block`이 아니라 `continue`다 |
| `context.session_start.json` | `Context` (SessionStart) | `systemMessage`는 문자열이 아니라 객체다 |
| `warn.session_start.json` | `Warn` (SessionStart) | `errorMessage`는 웹 문서에만 있다. PreToolUse와 Stop에는 사용자에게만 보이는 통로가 없다 |
