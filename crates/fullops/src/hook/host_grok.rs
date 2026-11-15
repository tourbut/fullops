//! Grok Build 어댑터(설계 7.2절 표의 `grok` 열). Claude Code의 snake_case 별칭으로 읽고,
//! camelCase로만 오는 `toolInputTruncated`·`subagentType`만 그 이름으로 읽는다.

use super::{
    DecodeError, Host, decode_snake, edit_op, encode_claude, joined, os_dialect, text, tool,
};
use fullops_core::model::{EventKind, HookInput, HostId, ToolCall, ToolKind, Verdict};
use serde_json::Value;

pub struct Grok;

fn tool_of(name: &str, args: &Value) -> ToolCall {
    let write = || ToolCall {
        file_path: text(args, "file_path"),
        content: text(args, "content"),
        ..tool(name, ToolKind::Write)
    };
    match name {
        // Windows에서의 셸 문법은 미정-M10이다
        "run_terminal_command" => ToolCall {
            command: text(args, "command"),
            ..tool(name, ToolKind::Shell(os_dialect()))
        },
        "write" => write(),
        // `content`가 문자열이면 전체 쓰기다
        "search_replace" if args["content"].is_string() => write(),
        "search_replace" => ToolCall {
            file_path: text(args, "file_path"),
            edits: edit_op(args, "old_string", "new_string")
                .into_iter()
                .collect(),
            ..tool(name, ToolKind::Edit)
        },
        // 바뀐 뒤 글을 계산할 수 없다. 경로만 옮긴다
        "hashline_edit" => ToolCall {
            file_path: text(args, "file_path"),
            ..tool(name, ToolKind::Edit)
        },
        "spawn_subagent" => ToolCall {
            text: joined(args, &["prompt", "description"]),
            ..tool(name, ToolKind::SubAgent)
        },
        // Cursor preset의 도구 이름(Shell·StrReplace·Task)은 v1에서 다루지 않는다
        _ => tool(name, ToolKind::Other),
    }
}

impl Host for Grok {
    fn id(&self) -> HostId {
        HostId::Grok
    }

    fn decode(&self, raw: &[u8], event_arg: Option<&str>) -> Result<HookInput, DecodeError> {
        let (mut input, v) = decode_snake(raw, event_arg, tool_of)?;
        input.source = input.source.map(|source| match source.as_str() {
            "new" => "startup".to_string(),
            "load" => "resume".to_string(),
            _ => source,
        });
        if input.event == EventKind::Stop {
            input.stop_reason = text(&v, "reason");
        }
        input.agent_id = text(&v, "subagentType");
        if let Some(tool) = &mut input.tool {
            tool.truncated = v["toolInputTruncated"] == true;
        }
        Ok(input)
    }

    // SessionStart 밖의 경고와 모든 문맥은 닿지 않는다(`{}`를 내고 추적에 남긴다)
    fn encode(&self, event: EventKind, v: &Verdict) -> String {
        encode_claude(event, v, true)
    }

    /// 모든 이벤트에서 `{}`다.
    fn pass(&self, _event_arg: Option<&str>) -> &'static str {
        super::EMPTY
    }
}
