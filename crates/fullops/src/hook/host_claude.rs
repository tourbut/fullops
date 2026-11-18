//! Claude Code 어댑터(설계 7.2절 표의 `claude` 열).

use super::{DecodeError, Host, decode_snake, edit_op, encode_claude, joined, text, tool};
use fullops_core::model::{
    Card, Dialect, EventKind, HookInput, HostId, ToolCall, ToolKind, Verdict,
};
use serde_json::Value;

pub struct Claude;

/// `AskUserQuestion`의 질문 글과 선택지 label(`questions[].question`, `questions[].options[].label`).
/// 모양이 다른 질문이 하나라도 있으면 빈 목록이다: 기록도 거부도 하지 않는다.
fn cards_of(args: &Value) -> Vec<Card> {
    let card = |q: &Value| {
        let labels = q["options"].as_array().filter(|o| !o.is_empty())?;
        Some(Card {
            question: text(q, "question").filter(|t| !t.trim().is_empty())?,
            options: labels
                .iter()
                .map(|o| text(o, "label"))
                .collect::<Option<_>>()?,
        })
    };
    let questions = args["questions"].as_array();
    questions
        .and_then(|list| list.iter().map(card).collect())
        .unwrap_or_default()
}

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
        "AskUserQuestion" => ToolCall {
            cards: cards_of(args),
            ..tool(name, ToolKind::Ask)
        },
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
