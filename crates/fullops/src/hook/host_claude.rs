//! Claude Code 어댑터(설계 7.2절 표의 `claude` 열). `decode`와 `encode`는 묶음 B가 채운다.

use super::{DecodeError, Host};
use fullops_core::model::{Caps, Dialect, EventKind, HookInput, HostKind, ToolKind, Verdict};

pub struct Claude;

impl Host for Claude {
    fn kind(&self) -> HostKind {
        HostKind::Claude
    }

    fn decode(&self, _raw: &[u8]) -> Result<HookInput, DecodeError> {
        Err(DecodeError("unimplemented"))
    }

    fn encode(&self, _event: EventKind, _v: &Verdict) -> String {
        "{}".to_string()
    }

    fn caps(&self) -> Caps {
        Caps {
            stop_failure: true,
            background_wake: true,
        }
    }

    fn tool_kind(&self, name: &str) -> ToolKind {
        match name {
            "Bash" => ToolKind::Shell(Dialect::Bash),
            "PowerShell" => ToolKind::Shell(Dialect::PowerShell),
            "Write" => ToolKind::Write,
            "Edit" | "MultiEdit" => ToolKind::Edit,
            "Agent" | "Task" => ToolKind::SubAgent,
            _ => ToolKind::Other,
        }
    }
}
