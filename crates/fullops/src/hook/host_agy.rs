//! agy(Antigravity CLI) 어댑터(설계 7.2절 표의 `agy` 열).
//! agy는 입력에 이벤트 이름이 없어 등록 문자열이 `--event`를 준다. 키는 camelCase이고 도구 인자는 PascalCase다.
//! PreToolUse·Stop의 출력에는 `decision`이 필수다.

use super::{DecodeError, Host, clip, edit_op, os_dialect, text, tool};
use fullops_core::messages;
use fullops_core::model::{EventKind, HookInput, HostId, SessionId, ToolCall, ToolKind, Verdict};
use serde_json::{Value, json};
use std::path::PathBuf;

pub struct Agy;

/// PreToolUse의 통과 값. `allow`가 "의견 없음"으로 확인되면(미정-M11) 이 상수 하나를 바꾼다.
const PASS_PRE_TOOL_USE: &str = r#"{"decision":"ask"}"#;
/// Stop(StopFailure로 옮긴 Stop 포함)의 통과 값.
const PASS_STOP: &str = r#"{"decision":""}"#;

// 편집 도구의 인자 이름은 추정이다(미정-M11). 입력에 없으면 그 필드는 None이고 바뀐 뒤 글은 모름이 된다
fn tool_of(name: &str, args: &Value) -> ToolCall {
    let edit = |chunk: &Value| edit_op(chunk, "TargetContent", "ReplacementContent");
    match name {
        "run_command" => ToolCall {
            command: text(args, "CommandLine"),
            ..tool(name, ToolKind::Shell(os_dialect()))
        },
        "write_to_file" => ToolCall {
            file_path: text(args, "TargetFile"),
            content: text(args, "CodeContent"),
            ..tool(name, ToolKind::Write)
        },
        "replace_file_content" => ToolCall {
            file_path: text(args, "TargetFile"),
            edits: edit(args).into_iter().collect(),
            ..tool(name, ToolKind::Edit)
        },
        "multi_replace_file_content" => ToolCall {
            file_path: text(args, "TargetFile"),
            edits: args["ReplacementChunks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(edit)
                .collect(),
            ..tool(name, ToolKind::Edit)
        },
        // 인자 이름이 확인되지 않아 지시 글은 옮기지 않는다
        "invoke_subagent" => tool(name, ToolKind::SubAgent),
        _ => tool(name, ToolKind::Other),
    }
}

impl Host for Agy {
    fn id(&self) -> HostId {
        HostId::Agy
    }

    fn decode(&self, raw: &[u8], event_arg: Option<&str>) -> Result<HookInput, DecodeError> {
        let v: Value = serde_json::from_slice(raw).map_err(|_| DecodeError("json"))?;
        let mut event = event_arg
            .and_then(EventKind::parse)
            .ok_or(DecodeError("event"))?;
        let sid = text(&v, "conversationId")
            .and_then(|s| SessionId::from_host(&s))
            .ok_or(DecodeError("session"))?;
        let cwd = v["workspacePaths"][0]
            .as_str()
            .filter(|c| !c.is_empty())
            .ok_or(DecodeError("cwd"))?;
        let mut stop_reason = text(&v, "terminationReason").filter(|_| event == EventKind::Stop);
        let (mut error, mut error_details) = (None, None);
        match stop_reason.as_deref() {
            Some("NO_TOOL_CALL") => stop_reason = Some("end_turn".into()),
            // 오류로 끝난 Stop은 StopFailure로 옮긴다(F-01)
            Some("ERROR") => {
                event = EventKind::StopFailure;
                stop_reason = None;
                error = Some("error".to_string());
                error_details = text(&v, "error").filter(|e| !e.is_empty()).map(clip);
            }
            _ => {}
        }
        let call = v.get("toolCall").filter(|_| event == EventKind::PreToolUse);
        Ok(HookInput {
            event,
            sid,
            cwd: PathBuf::from(cwd),
            source: (event == EventKind::SessionStart).then(|| "startup".to_string()),
            prompt: text(&v, "lastUserInput"),
            tool: call.and_then(|call| Some(tool_of(call["name"].as_str()?, &call["args"]))),
            stop_reason,
            error,
            error_details,
            agent_id: None,
        })
    }

    fn encode(&self, event: EventKind, v: &Verdict) -> String {
        let start = event == EventKind::SessionStart;
        let out = match v {
            Verdict::Context(t) if start => {
                json!({"injectSteps": [{"systemMessage": {"systemMessage": t}}]})
            }
            Verdict::Warn(t) if start => {
                json!({"injectSteps": [{"errorMessage": messages::warn(t)}]})
            }
            Verdict::Deny { rule, reason } => {
                json!({"decision": "deny", "reason": messages::refusal(rule, reason, "")})
            }
            Verdict::Block { rule, reason, also } => {
                json!({"decision": "continue", "reason": messages::block(rule, reason, "", also)})
            }
            // 그 밖의 이벤트에는 사용자에게만 보이는 통로가 없다. 통과의 글을 낸다
            _ => {
                let event = match event {
                    // 오류로 끝난 Stop을 옮긴 것이라 출력은 Stop의 것이다
                    EventKind::StopFailure => EventKind::Stop,
                    other => other,
                };
                return self.pass(Some(event.as_str())).to_string();
            }
        };
        out.to_string()
    }

    fn pass(&self, event_arg: Option<&str>) -> &'static str {
        match event_arg {
            Some("PreToolUse") => PASS_PRE_TOOL_USE,
            Some("Stop") => PASS_STOP,
            _ => super::EMPTY,
        }
    }
}
