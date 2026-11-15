//! Codex 어댑터(설계 7.2절 표의 `codex` 열). StopFailure가 없고 편집은 `apply_patch` 하나다.

use super::{DecodeError, Host, decode_snake, encode_claude, joined, os_dialect, text, tool};
use fullops_core::model::{EventKind, HookInput, HostId, ToolCall, ToolKind, Verdict};
use serde_json::Value;

pub struct Codex;

fn tool_of(name: &str, args: &Value) -> ToolCall {
    match name {
        // Windows에서 `command`가 어느 셸 문법인지는 미정-M2다. 바뀌면 이 줄을 고친다
        "Bash" => ToolCall {
            command: text(args, "command"),
            ..tool(name, ToolKind::Shell(os_dialect()))
        },
        // 패치 본문이 `tool_input.command`에 온다
        "apply_patch" => ToolCall {
            command: text(args, "command"),
            ..tool(name, ToolKind::Patch)
        },
        "spawn_agent" => ToolCall {
            text: joined(args, &["message", "prompt"]),
            ..tool(name, ToolKind::SubAgent)
        },
        _ => tool(name, ToolKind::Other),
    }
}

impl Host for Codex {
    fn id(&self) -> HostId {
        HostId::Codex
    }

    fn decode(&self, raw: &[u8], event_arg: Option<&str>) -> Result<HookInput, DecodeError> {
        decode_snake(raw, event_arg, tool_of).map(|(input, _)| input)
    }

    // 출력 스키마가 모르는 최상위 필드를 허용하지 않는다. Claude Code와 같은 필드만 낸다
    fn encode(&self, event: EventKind, v: &Verdict) -> String {
        encode_claude(event, v, false)
    }

    /// 모든 이벤트에서 `{}`다.
    fn pass(&self, _event_arg: Option<&str>) -> &'static str {
        super::EMPTY
    }
}
