//! hook 입력과 도구 호출의 분류 결과, 호스트 능력 표(설계 5절, 7.2·7.4절).

use super::{Known, SessionId, words};
use std::path::PathBuf;

words!(
    /// hook 어댑터가 있는 호스트. `fullops hook <이 문자열>`의 인자이기도 하다.
    HostId { Claude = "claude", Codex = "codex", Grok = "grok", Agy = "agy" }
);
words!(EventKind {
    SessionStart = "SessionStart",
    UserPromptSubmit = "UserPromptSubmit",
    PreToolUse = "PreToolUse",
    Stop = "Stop",
    StopFailure = "StopFailure",
});

impl HostId {
    pub const ALL: [HostId; 4] = [HostId::Claude, HostId::Codex, HostId::Grok, HostId::Agy];
}

/// 호스트 능력(설계 7.2절의 능력 표). 규칙은 호스트 이름이 아니라 이 값을 본다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    /// 턴이 API 오류로 끝난 것을 hook이 안다(F-01).
    pub stop_failure: bool,
    /// 백그라운드 명령이 끝나면 세션을 깨운다(X-05).
    pub background_wake: bool,
    /// UserPromptSubmit이 온다.
    pub prompt_event: bool,
    /// SessionStart의 `Context`가 모델에게 간다.
    pub context_at_start: bool,
    /// UserPromptSubmit의 `Context`가 모델에게 간다.
    pub context_at_prompt: bool,
    /// PreToolUse·Stop에서 사용자에게만 보이는 경고(`Warn`)를 낼 수 있다.
    pub user_warn: bool,
    /// 서브에이전트 도구 이름. 병렬 안내 문구에 넣는다(8.15절).
    pub subagent_tool: &'static str,
    /// 울트라코드(Workflow)가 있는 호스트다.
    pub workflow: bool,
    /// Orca `worker-start`의 `--agent` 값.
    pub orca_agent: &'static str,
}

/// 능력 표의 정본. `Host::caps()`와 dispatch의 spec 조립이 함께 쓴다.
pub fn caps_of(host: HostId) -> Caps {
    match host {
        HostId::Claude => Caps {
            stop_failure: true,
            background_wake: true,
            prompt_event: true,
            context_at_start: true,
            context_at_prompt: true,
            user_warn: true,
            subagent_tool: "Agent",
            workflow: true,
            orca_agent: "claude",
        },
        HostId::Codex => Caps {
            stop_failure: false,
            background_wake: false,
            prompt_event: true,
            context_at_start: true,
            context_at_prompt: true,
            user_warn: true,
            subagent_tool: "spawn_agent",
            workflow: false,
            orca_agent: "codex",
        },
        HostId::Grok => Caps {
            stop_failure: true,
            // 참으로 봤는데 깨우지 않으면 lead가 멈춘다. 실환경에서 확인한 뒤 올린다(미정-M10)
            background_wake: false,
            prompt_event: true,
            context_at_start: false,
            context_at_prompt: false,
            user_warn: false,
            subagent_tool: "spawn_subagent",
            workflow: false,
            orca_agent: "grok",
        },
        HostId::Agy => Caps {
            stop_failure: true,
            background_wake: false,
            prompt_event: false,
            context_at_start: true,
            context_at_prompt: false,
            user_warn: false,
            subagent_tool: "invoke_subagent",
            workflow: false,
            orca_agent: "antigravity",
        },
    }
}

/// Orca 에이전트 ID(`routes`의 `tuples[].agent`)를 호스트로 옮긴다. 모르는 값은 `None`.
pub fn host_of_agent(orca_agent: &str) -> Option<HostId> {
    HostId::ALL
        .into_iter()
        .find(|host| caps_of(*host).orca_agent == orca_agent)
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
    Workflow,
    /// 사용자에게 묻는 질문 카드(Claude Code의 `AskUserQuestion`).
    Ask,
    Other,
}

/// 도구 호출. 어댑터가 호스트의 도구 이름과 인자를 이 꼴로 옮긴다(7.2절의 도구 대응 표). 7.4·7.5절은 이것만 본다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    /// 호스트가 준 도구 이름(추적용).
    pub name: String,
    pub kind: ToolKind,
    /// `Shell`의 명령 글, `Patch`의 패치 본문.
    pub command: Option<String>,
    /// `Write`·`Edit`의 대상 경로.
    pub file_path: Option<String>,
    /// `Write`의 전체 내용.
    pub content: Option<String>,
    /// `Edit`. 여러 곳 편집은 여러 원소다.
    pub edits: Vec<EditOp>,
    /// `SubAgent`의 지시 글(인자가 여럿이면 줄바꿈으로 이은 것).
    pub text: Option<String>,
    /// 호스트가 입력을 잘랐다(grok의 `toolInputTruncated`). 참이면 잘린 글로 판정하지 않는다.
    pub truncated: bool,
    /// `Ask`의 질문들. 모양을 읽지 못하면 비어 있다(기록도 거부도 하지 않는다).
    pub cards: Vec<Card>,
}

/// 질문 카드의 질문 하나. `options`는 선택지 label을 적힌 순서대로 담는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub question: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditOp {
    pub old: String,
    pub new: String,
    pub replace_all: bool,
}

/// 호스트 어댑터가 해석한 hook 입력(설계 7.2절의 입력 대응 표).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookInput {
    pub event: EventKind,
    pub sid: SessionId,
    pub cwd: PathBuf,
    /// SessionStart: `startup`·`clear`·`resume`·`compact`·`fork`. 어댑터가 이 어휘로 옮긴다.
    pub source: Option<String>,
    /// UserPromptSubmit의 프롬프트. agy는 모든 이벤트의 `lastUserInput`.
    pub prompt: Option<String>,
    /// PreToolUse.
    pub tool: Option<ToolCall>,
    /// Stop: 정상 종료는 `None` 또는 `"end_turn"`. 그 밖의 값이면 Stop 규칙을 평가하지 않는다.
    pub stop_reason: Option<String>,
    /// StopFailure: 오류 종류 이름(예: `rate_limit`. agy는 `error`).
    pub error: Option<String>,
    /// StopFailure: 오류 글의 앞 300자.
    pub error_details: Option<String>,
    /// 하위 에이전트 안에서 불렸을 때만 있다. 읽기만 하고 판정에 쓰지 않는다.
    pub agent_id: Option<String>,
}

/// 셸 명령 하나의 분류(설계 7.4절). `Other`와 `Unparsed`에는 어떤 규칙도 걸리지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    OrcaWorkerStart,
    /// `--spec-file`로 준 spec은 `Known::Unknown`이다(파일과 stdin을 읽지 않는다).
    OrcaTaskCreate {
        spec: Known<String>,
    },
    OrcaInject,
    OrcaTerminalSend {
        chars: usize,
    },
    OrcaCheck {
        peek: bool,
        run: Option<String>,
    },
    GitCommit {
        all: bool,
    },
    /// `paths`는 명령에 적힌 경로(작업 위치 기준) 그대로다. `-A`·`--all`·`git -C`·pathspec 파일은 레포 전체 `":/"`(`shell.rs`의 `WHOLE_REPO`) 하나로 넓힌다.
    /// `update`는 `-u`(추적 중인 파일 전부).
    GitAdd {
        paths: Vec<String>,
        update: bool,
    },
    Fullops {
        sub: String,
    },
    Other,
    Unparsed,
}
