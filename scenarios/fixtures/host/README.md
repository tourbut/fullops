# 호스트 hook 표본

호스트가 hook에 주는 입력 JSON과 hook이 내는 출력 JSON의 형식 표본이다. 근거는 `docs/contracts/claude-code.md`와 `docs/contracts/codex.md`다.

- 값은 전부 가짜다. 실제 세션의 대화나 프롬프트를 옮긴 것은 없다.
- 형식은 공식 문서의 필드 표와 예시를 따랐다. 실제 호스트에서 받아 본 것이 아니다.
- `cwd`, `transcript_path`, `file_path`는 자리 채움 값이다. 시험은 이 값을 임시 레포의 경로로 바꿔 넣는다.
- 문서에 없는 필드는 넣지 않았다. 호스트는 여기 없는 필드를 더 줄 수 있으므로 읽는 쪽은 모르는 필드를 무시해야 한다.

## `claude/` — Claude Code 2.1.323 기준

| 파일 | 내용 | 확인 수준 |
|---|---|---|
| `session_start.{startup,resume,clear,compact,fork}.json` | `source`별 SessionStart. `clear`에는 `model`이 없다(문서가 "없을 수 있다"고 한다) | 문서의 필드 표 |
| `user_prompt_submit.human.json` | 사람이 친 입력 | 문서의 예시 |
| `user_prompt_submit.orca_dispatch.json` | Orca의 배정 프롬프트. 안내 줄 + `<pasted_content id="…">` 줄 사이의 preamble | 조립한 것이다. 안내 줄과 preamble은 Orca 코드에서, 붙여 넣기 표식은 Claude Code 문서에서 왔다. 둘이 합쳐진 실제 글은 받아 보지 못했다 |
| `user_prompt_submit.orca_dispatch_bare.json` | 표식 없이 preamble만 온 경우(옛 Orca) | 위와 같다 |
| `user_prompt_submit.orca_wake.json`, `…orca_wake_unacked.json` | Orca가 터미널 세션을 깨우는 글. 줄바꿈으로 시작한다 | 문구는 Orca 코드에서 읽었다. hook의 `prompt`에 앞 줄바꿈이 남는지는 확인하지 못했다 |
| `user_prompt_submit.orca_chat_message.json` | Orca가 채팅 세션에 메시지를 통째로 넣는 경우 | 형식 함수는 Orca 코드에서 읽었다. 채팅 세션에 실제로 이 글이 들어가는지는 확인하지 못했다 |
| `pre_tool_use.bash.json`, `…bash_worker_start.json` | Bash 도구 | 문서의 예시 |
| `pre_tool_use.powershell.json` | PowerShell 도구 | 문서의 필드 표 |
| `pre_tool_use.write.json`, `…write_windows.json` | Write 도구. Windows 표본은 역슬래시 절대 경로다 | 문서의 예시 |
| `pre_tool_use.edit.json` | Edit 도구 | 문서의 필드 표 |
| `pre_tool_use.agent.json` | Agent 도구(하위 에이전트) | 문서의 필드 표 |
| `pre_tool_use.bash_in_subagent.json` | 하위 에이전트 안에서 불린 경우(`agent_id`, `agent_type`) | 문서의 공통 필드 표 |
| `stop.json`, `stop.continued.json` | Stop. 뒤의 것은 Stop hook 때문에 이미 이어 가는 중이고 백그라운드 셸 작업이 하나 있다 | 문서의 예시 |
| `stop_failure.rate_limit.json`, `…minimal.json` | StopFailure. `error`는 종류 이름이다 | 문서의 예시 |

`<task-notification>`이 든 UserPromptSubmit 표본은 없다. 그 글의 형식과 hook 입력에 실리는지를 확인하지 못했기 때문이다.

## `codex/` — Codex 0.163.0 기준

| 파일 | 내용 | 확인 수준 |
|---|---|---|
| `session_start.{startup,compact}.json` | SessionStart | 생성된 입력 스키마의 필수 필드 |
| `user_prompt_submit.human.json`, `…orca_dispatch.json` | UserPromptSubmit | 스키마. 배정 프롬프트의 글은 조립한 것이다 |
| `pre_tool_use.bash.json` | 셸 계열 도구. `tool_name`은 `Bash`다 | 문서와 스키마 |
| `pre_tool_use.apply_patch_add.json`, `…apply_patch_update.json` | `apply_patch`. 본문은 `tool_input.command`에 있다. 두 번째는 Update·Move·`@@ <문맥>`·`*** End of File`·Delete를 담는다 | 문서와 파서의 문법 주석 |
| `pre_tool_use.spawn_agent.json` | `spawn_agent`. 인자는 `message`만 넣었다 | 도구 스키마에서 `message`를 확인했다. 다른 인자는 넣지 않았다 |
| `stop.json`, `stop.continued.json` | Stop. `last_assistant_message`는 `null`일 수 있다 | 스키마 |

Windows에서 `Bash`의 `command`가 어떤 셸 문법으로 오는지는 확인하지 못했다.

## `output/` — hook이 내는 형식(두 호스트 공통)

설계 7.2절의 Verdict별 출력이다. 한 줄 JSON이다. `<…>`는 문구가 들어갈 자리다.

| 파일 | Verdict |
|---|---|
| `pass.json` | `Pass` |
| `context.session_start.json`, `context.user_prompt_submit.json` | `Context` |
| `warn.json` | `Warn` |
| `deny.pre_tool_use.json` | `Deny` |
| `block.stop.json` | `Block` |
