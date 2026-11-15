//! Claude Code 어댑터(설계 7.2절 표의 `claude` 열).

use super::{DecodeError, Host, decode_snake, edit_op, encode_claude, joined, text, tool};
use fullops_core::model::{Dialect, EventKind, HookInput, HostId, ToolCall, ToolKind, Verdict};
use serde_json::Value;

pub struct Claude;

fn tool_of(name: &str, args: &Value) -> ToolCall {
    let shell = |dialect| ToolCall {
        command: text(args, "command"),
        ..tool(name, ToolKind::Shell(dialect))
    };
    match name {
        "Bash" => shell(Dialect::Bash),
        "PowerShell" => shell(Dialect::PowerShell),
        "Write" => ToolCall {
            file_path: text(args, "file_path"),
            content: text(args, "content"),
            ..tool(name, ToolKind::Write)
        },
        "Edit" => ToolCall {
            file_path: text(args, "file_path"),
            edits: edit_op(args, "old_string", "new_string")
                .into_iter()
                .collect(),
            ..tool(name, ToolKind::Edit)
        },
        "MultiEdit" => ToolCall {
            file_path: text(args, "file_path"),
            edits: args["edits"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|e| edit_op(e, "old_string", "new_string"))
                .collect(),
            ..tool(name, ToolKind::Edit)
        },
        "Agent" | "Task" => ToolCall {
            text: joined(args, &["prompt", "description"]),
            ..tool(name, ToolKind::SubAgent)
        },
        "Workflow" => tool(name, ToolKind::Workflow),
        _ => tool(name, ToolKind::Other),
    }
}

impl Host for Claude {
    fn id(&self) -> HostId {
        HostId::Claude
    }

    fn decode(&self, raw: &[u8], event_arg: Option<&str>) -> Result<HookInput, DecodeError> {
        decode_snake(raw, event_arg, tool_of).map(|(input, _)| input)
    }

    fn encode(&self, event: EventKind, v: &Verdict) -> String {
        encode_claude(event, v, false)
    }

    /// 모든 이벤트에서 `{}`다.
    fn pass(&self, _event_arg: Option<&str>) -> &'static str {
        super::EMPTY
    }
}
