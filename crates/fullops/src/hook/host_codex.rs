//! Codex 어댑터(설계 7.2절 표의 `codex` 열). 계약은 미정-M2이고 바뀌면 이 파일만 고친다. `decode`와 `encode`는 묶음 B가 채운다.

use super::{DecodeError, Host};
use fullops_core::model::{Caps, Dialect, EventKind, HookInput, HostKind, ToolKind, Verdict};

pub struct Codex;

impl Host for Codex {
    fn kind(&self) -> HostKind {
        HostKind::Codex
    }

    fn decode(&self, _raw: &[u8]) -> Result<HookInput, DecodeError> {
        Err(DecodeError("unimplemented"))
    }

    fn encode(&self, _event: EventKind, _v: &Verdict) -> String {
        "{}".to_string()
    }

    fn caps(&self) -> Caps {
        Caps {
            stop_failure: false,
            background_wake: false,
        }
    }

    fn tool_kind(&self, name: &str) -> ToolKind {
        match name {
            // 방언은 OS로 정한다
            "shell" | "exec_command" | "local_shell" => ToolKind::Shell(if cfg!(windows) {
                Dialect::PowerShell
            } else {
                Dialect::Bash
            }),
            "apply_patch" => ToolKind::Patch,
            "spawn_agent" => ToolKind::SubAgent,
            _ => ToolKind::Other,
        }
    }
}
