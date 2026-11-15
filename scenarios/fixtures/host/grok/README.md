# Grok Build hook 표본

Grok Build 1.0.70이 hook에 주는 입력 JSON과 hook이 내는 출력 JSON의 형식 표본이다. 근거는 `docs/contracts/grok.md`다.

- 값은 전부 가짜다. 실제 세션의 대화나 프롬프트를 옮긴 것은 없다.
- 형식은 설치본에 딸린 hook 안내서의 필드 표와 예시를 따랐다. **실제 호스트에서 받아 본 것이 아니다.**
- 실제 입력은 줄바꿈으로 끝나는 한 줄 JSON이다. 여기서는 읽기 쉽게 여러 줄로 적었다.
- 최상위 키는 camelCase이고, 안내서가 "함께 보낸다"고 적은 snake_case 별칭(`session_id`, `transcript_path`, `permission_mode`, `tool_name`, `tool_input`, `tool_use_id`, `stop_hook_active`, `last_assistant_message`, `error_details`)을 같이 넣었다. `hook_event_name`의 값은 PascalCase이고 `hookEventName`의 값은 snake_case다.
- `cwd`, `workspaceRoot`, `transcriptPath`, `file_path`는 자리 채움 값이다. 시험은 임시 레포의 경로로 바꿔 넣는다.
- 호스트는 여기 없는 필드를 더 줄 수 있다. 읽는 쪽은 모르는 필드를 무시해야 한다.

## 입력

| 파일 | 내용 | 확인 수준 |
|---|---|---|
| `session_start.new.json`, `session_start.load.json` | SessionStart. `source`는 `new`(새 세션, `/clear` 포함)와 `load`(이어 하기)다. Claude의 `startup`·`clear`·`compact`는 오지 않는다 | 값은 안내서의 matcher 설명. 필드 이름 `source`는 실행 파일의 필드 이름표에서 읽었다 |
| `user_prompt_submit.human.json` | 사람이 친 입력 | 필드 이름 `prompt`는 실행 파일의 필드 이름표에서 읽었다. 안내서에 예시가 없다 |
| `user_prompt_submit.orca_dispatch.json` | Orca의 배정 프롬프트(preamble) | 조립한 것이다. 프롬프트 글은 `codex/user_prompt_submit.orca_dispatch.json`과 같다. Orca가 argv로 넘긴 첫 프롬프트에 이 이벤트가 오는지는 확인하지 못했다 |
| `pre_tool_use.run_terminal_command.json`, `…_worker_start.json` | 셸 도구 | 안내서의 예시 |
| `pre_tool_use.run_terminal_command_background.json` | 백그라운드 명령(`block_until_ms: 0`) | 안내서의 백그라운드 설명 |
| `pre_tool_use.search_replace.json` | 편집 도구. 인자는 Claude의 `Edit`와 같은 네 이름이다 | 인자 이름은 실행 파일의 `SearchReplaceInput`에서 읽었다 |
| `pre_tool_use.write.json` | 쓰기 도구 | **추정**. 인자 `file_path`·`content`는 안내서의 다른 문장에서 미루어 봤다 |
| `pre_tool_use.spawn_subagent.json` | 하위 에이전트 도구 | 안내서의 매개변수 표 |
| `pre_tool_use.run_terminal_command_in_subagent.json` | 하위 에이전트 안에서 불린 경우(`subagentType`) | 안내서 |
| `stop.json` | 정상 종료(`reason: "end_turn"`) | 안내서 |
| `stop.continued.json` | Stop hook 때문에 이어 가는 중이고 백그라운드 셸 작업이 하나 있다. `backgroundTasks`는 camelCase로만 온다 | 안내서 |
| `stop.session_end.json` | 세션이 끝날 때 한 번 더 오는 Stop(`reason: "shutdown"`). 출력은 무시된다 | 안내서 |
| `stop_failure.rate_limit.json` | StopFailure. `error`는 종류 이름이다 | 안내서 |

도구 이벤트에는 `promptId`를 넣지 않았다(안내서가 "도구 이벤트에는 없다"고 적는다). `hashline_edit` 표본은 없다. 인자의 모양을 확인하지 못했기 때문이다.

## `output/` — hook이 내는 형식

한 줄 JSON이다. `<…>`는 문구가 들어갈 자리다.

| 파일 | Verdict | 비고 |
|---|---|---|
| `pass.json` | `Pass` | |
| `deny.pre_tool_use.json` | `Deny` | Claude Code와 같다 |
| `block.stop.json` | `Block` | Claude Code와 같다. 한 턴에 8번까지 |
| `warn.json` | `Warn` | SessionStart에서만 확실히 읽힌다 |
| `context.pre_tool_use.json` | PreToolUse에서 모델에게 주는 글 | 설계의 Verdict 표에는 없는 통로다. **SessionStart와 UserPromptSubmit의 `additionalContext`는 Grok이 읽지 않으므로 표본을 두지 않았다** |
