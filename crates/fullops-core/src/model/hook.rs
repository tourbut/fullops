//! hook 입력과 도구 호출의 분류 결과(설계 7.2·7.4절).

use super::{Known, SessionId, words};
use std::path::PathBuf;

words!(HostKind { Claude = "claude", Codex = "codex" });
words!(EventKind {
    SessionStart = "SessionStart",
    UserPromptSubmit = "UserPromptSubmit",
    PreToolUse = "PreToolUse",
    Stop = "Stop",
    StopFailure = "StopFailure",
});

/// 호스트가 할 수 있는 것(설계 7.2절 표의 `caps`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    pub stop_failure: bool,
    pub background_wake: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Bash,
    PowerShell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Shell(Dialect),
    Write,
    Edit,
    Patch,
    SubAgent,
    Other,
}

/// 도구 호출. 호스트 어댑터가 `tool_input`에서 필요한 값만 뽑아 채운다(core는 JSON을 모른다).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub name: String,
    pub kind: ToolKind,
    /// 셸 명령. `Patch`에서는 패치 본문.
    pub command: Option<String>,
    /// `Write`·`Edit`의 대상 경로.
    pub file_path: Option<String>,
    /// `Write`의 새 내용.
    pub content: Option<String>,
    /// `Edit`는 하나, `MultiEdit`는 여러 개.
    pub edits: Vec<EditOp>,
    /// `SubAgent`의 글. `prompt`·`message`·`description`을 줄바꿈으로 이은 것.
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditOp {
    pub old: String,
    pub new: String,
    pub replace_all: bool,
}

/// 호스트 어댑터가 해석한 hook 입력(설계 7.2절).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookInput {
    pub event: EventKind,
    pub sid: SessionId,
    pub cwd: PathBuf,
    /// SessionStart: `startup`·`clear`·`resume`·`compact`.
    pub source: Option<String>,
    /// UserPromptSubmit.
    pub prompt: Option<String>,
    /// PreToolUse.
    pub tool: Option<ToolCall>,
    /// Stop: 입력 필드 `reason`.
    pub stop_reason: Option<String>,
    /// StopFailure: 입력 필드 `error`의 앞 300자.
    pub error: Option<String>,
}

/// 셸 명령 하나의 분류(설계 7.4절). `Other`와 `Unparsed`에는 어떤 규칙도 걸리지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    OrcaWorkerStart,
    OrcaTaskCreate { spec: Known<String> },
    OrcaInject,
    OrcaTerminalSend { chars: usize },
    OrcaCheck { peek: bool, run: Option<String> },
    GitCommit { all: bool },
    GitAdd,
    Fullops { sub: String },
    Other,
    Unparsed,
}
