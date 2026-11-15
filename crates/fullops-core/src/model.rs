//! 도메인 모델(설계 5절)과 파일 형식의 serde 타입(6절). 필드 이름은 docs/formats.md와 같다.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

mod hook;
mod records;
mod session;
mod state;

pub use hook::*;
pub use records::*;
pub use session::*;
pub use state::*;

/// 문자열 값을 가진 enum. serde 표기, `as_str`, `parse`를 한 번에 만든다.
macro_rules! words {
    ($(#[$m:meta])* $name:ident { $($var:ident = $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
        pub enum $name { $(#[serde(rename = $s)] $var),+ }
        impl $name {
            pub fn as_str(self) -> &'static str { match self { $(Self::$var => $s),+ } }
            pub fn parse(s: &str) -> Option<Self> { match s { $($s => Some(Self::$var),)+ _ => None } }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.as_str()) }
        }
    };
}
pub(crate) use words;

/// 식별자 문법에 맞지 않는 값.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdError {
    pub kind: &'static str,
    pub value: String,
}

impl std::fmt::Display for IdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} 문법에 맞지 않습니다: {:?}", self.kind, self.value)
    }
}

impl std::error::Error for IdError {}

/// 생성할 때 검증하는 문자열 newtype. 파일에서 읽을 때도 같은 검증을 거친다.
macro_rules! id {
    ($(#[$m:meta])* $name:ident, $check:expr) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);
        impl $name {
            pub fn new(s: impl Into<String>) -> Result<Self, IdError> {
                let s = s.into();
                let check: fn(&str) -> bool = $check;
                if check(&s) { Ok(Self(s)) } else { Err(IdError { kind: stringify!($name), value: s }) }
            }
            pub fn as_str(&self) -> &str { &self.0 }
        }
        impl TryFrom<String> for $name {
            type Error = IdError;
            fn try_from(s: String) -> Result<Self, IdError> { Self::new(s) }
        }
        impl From<$name> for String {
            fn from(v: $name) -> String { v.0 }
        }
        impl std::str::FromStr for $name {
            type Err = IdError;
            fn from_str(s: &str) -> Result<Self, IdError> { Self::new(s) }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
        }
    };
}

fn opaque(s: &str) -> bool {
    !s.is_empty() && s.len() <= 200 && !s.chars().any(|c| c.is_whitespace() || c.is_control())
}

id!(
    /// 과제 키. `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`
    TaskKey,
    |s| {
        let b = s.as_bytes();
        (1..=64).contains(&b.len())
            && b[0].is_ascii_alphanumeric()
            && b.iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
    }
);
id!(
    /// 역할 ID. `^[a-z][a-z0-9_-]{0,31}$`
    RoleId,
    |s| {
        let b = s.as_bytes();
        (1..=32).contains(&b.len())
            && b[0].is_ascii_lowercase()
            && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
    }
);
id!(
    /// 커밋 SHA. 40자 소문자 hex.
    Sha,
    |s| s.len() == 40 && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
);
id!(
    /// 세션 ID. 호스트 값은 `SessionId::from_host`로 정리해서 만든다.
    SessionId,
    |s| (1..=100).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
);
id!(
    /// Orca Run ID. 문법은 Orca 소관이라 비어 있지 않고 공백·제어 문자가 없는지만 본다.
    RunId,
    opaque
);
id!(
    /// Orca dispatch ID. Orca 응답을 받기 전의 예약 기록은 `pending:<request>`다(설계 6.2.1절).
    DispatchId,
    opaque
);
id!(
    /// Orca 메시지 ID. completion의 ID로도 쓴다.
    MessageId,
    opaque
);
id!(
    /// 결정 ID. `D` + 3자리 이상 숫자.
    DecisionId,
    |s| s.len() >= 4 && s.starts_with('D') && s[1..].bytes().all(|c| c.is_ascii_digit())
);

impl Sha {
    /// 앞 `n`자. 표시용 7자, 리뷰 폴더 이름용 12자에 쓴다.
    pub fn short(&self, n: usize) -> &str {
        self.0.get(..n).unwrap_or(&self.0)
    }
}

impl SessionId {
    /// 호스트가 준 값에서 `[A-Za-z0-9_-]` 밖 글자를 `_`로 바꾸고 100자로 자른다. 빈 값은 `None`.
    pub fn from_host(raw: &str) -> Option<Self> {
        let s: String = raw
            .chars()
            .take(100)
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if s.is_empty() { None } else { Some(Self(s)) }
    }
}

impl DecisionId {
    /// `state.json`의 `next_decision` 값으로 ID를 만든다.
    pub fn from_number(n: u32) -> Self {
        Self(format!("D{n:03}"))
    }
}

words!(Mode { Coor = "coor", Dev = "dev" });
words!(RoleKind { Worker = "worker", Design = "design", Tester = "tester", Coordinator = "coordinator" });
words!(Routing { Legacy = "legacy", Split = "split" });
words!(
    /// 순서가 있다. 뒤로 갈수록 높은 레벨이다.
    TestLevel { Minimal = "minimal", Lite = "lite", Standard = "standard", Full = "full", Exhaustive = "exhaustive" }
);
words!(
    /// 병렬 방침(설계 8.15절). `fullops.toml`의 `parallel`.
    Parallel { Off = "off", Read = "read", On = "on", Ultra = "ultra" }
);
words!(
    /// 1.24.0의 simple·implementation = Direct, design = Design, product = Product.
    RouteClass { Direct = "direct", Design = "design", Product = "product", Unresolved = "unresolved", Untracked = "untracked" }
);
words!(Purpose { Build = "build", Fix = "fix", Review = "review", Research = "research" });
words!(
    /// 과제 단계. 유도값이고 순서가 있다(설계 6.6절).
    Stage { None = "none", Routed = "routed", Assigned = "assigned", Reported = "reported",
            Reviewing = "reviewing", Merged = "merged", Done = "done" }
);
words!(
    /// 통합 위치. 유도값이다(설계 6.6절).
    Landing { MergeLeft = "merge_left", PushLeft = "push_left", Landed = "landed" }
);
words!(NeedAction { Skip = "skip", Ask = "ask", Review = "review" });
words!(
    /// `origin`과 `via`에 쓴다.
    Origin { User = "user", Bot = "bot" }
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelTuple {
    pub agent: String,
    pub model: String,
    pub effort: String,
}

/// `fullops.toml`의 `[roles.<id>]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    pub branch: String,
    pub kind: RoleKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
}

/// `fullops.toml`의 `[review]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReviewPolicy {
    pub min_code_lines: u32,
    pub min_code_files: u32,
    pub always: Vec<String>,
    pub auto: bool,
}

impl Default for ReviewPolicy {
    fn default() -> Self {
        Self {
            min_code_lines: 400,
            min_code_files: 15,
            always: Vec::new(),
            auto: false,
        }
    }
}

/// `fullops.toml`의 `[code]`. 기본값은 내장 확장자 목록이다(설계 8.11절).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CodeDef {
    pub extensions: Vec<String>,
}

/// `fullops.toml`의 `[wait]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WaitConfig {
    pub check_minutes: u64,
    pub ready_timeout_seconds: u64,
    pub log_limit: u32,
}

impl Default for WaitConfig {
    fn default() -> Self {
        Self {
            check_minutes: 60,
            ready_timeout_seconds: 90,
            log_limit: 30,
        }
    }
}

/// 검증을 통과한 `fullops.toml`(설계 6.1.1절). `config::validate`만 만든다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub schema: u32,
    pub applied_version: String,
    pub mode: Mode,
    pub primary_role: Option<RoleId>,
    pub primary_branch: Option<String>,
    pub routing: Routing,
    pub tech_plan_role: Option<RoleId>,
    pub test_level: TestLevel,
    pub parallel: Parallel,
    pub base: String,
    pub remote: Option<String>,
    pub roles: BTreeMap<RoleId, Role>,
    pub review: ReviewPolicy,
    pub code: CodeDef,
    pub wait: WaitConfig,
}

/// 세션 파일의 `mode_at_start`. `[mode, primary_role, primary_branch]`.
pub type ModeTuple = (Mode, Option<RoleId>, Option<String>);

impl Config {
    pub fn mode_tuple(&self) -> ModeTuple {
        (
            self.mode,
            self.primary_role.clone(),
            self.primary_branch.clone(),
        )
    }

    /// 통합 기준 ref(설계 4절).
    pub fn base_ref(&self) -> String {
        match &self.remote {
            Some(remote) => format!("refs/remotes/{remote}/{}", self.base),
            None => format!("refs/heads/{}", self.base),
        }
    }

    /// kind가 `design`인 역할. 최대 하나다.
    pub fn design_role(&self) -> Option<&RoleId> {
        self.roles
            .iter()
            .find(|(_, r)| r.kind == RoleKind::Design)
            .map(|(id, _)| id)
    }

    pub fn role_of_branch(&self, branch: &str) -> Option<(&RoleId, &Role)> {
        self.roles.iter().find(|(_, r)| r.branch == branch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seat {
    Lead,
    Role { id: RoleId, kind: RoleKind },
    Reviewer,
    Other,
}

/// 호출자(설계 7.3절). `facts::resolve_actor`가 만든다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub seat: Seat,
    pub dispatch: Option<DispatchId>,
    /// `mode == Dev && is_lead()`
    pub direct_dev: bool,
    /// 묶인 dispatch가 하위 배정이다(`state.json`의 그 dispatch에 `sub_of`가 있다). 하위 worker다.
    pub sub: bool,
}

impl Actor {
    pub fn is_lead(&self) -> bool {
        self.seat == Seat::Lead && self.dispatch.is_none()
    }

    /// 추적 줄의 `actor` 값. `lead`, `role:<id>`, `reviewer`, `other`이고 dispatch가 묶였으면 `+dispatch`가, 하위 worker면 `+sub`가 붙는다.
    pub fn label(&self) -> String {
        let seat = match &self.seat {
            Seat::Lead => "lead".to_string(),
            Seat::Role { id, .. } => format!("role:{id}"),
            Seat::Reviewer => "reviewer".to_string(),
            Seat::Other => "other".to_string(),
        };
        match (self.dispatch.is_some(), self.sub) {
            (_, true) => format!("{seat}+sub"),
            (true, false) => format!("{seat}+dispatch"),
            (false, false) => seat,
        }
    }
}

/// `<git-dir>/HEAD`의 내용.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadRef {
    Branch(String),
    Detached(Sha),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanStatus {
    Ready,
    WaitHuman(String),
    Held(String),
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRow {
    pub key: TaskKey,
    pub goal: String,
    pub role: Option<RoleId>,
    pub status: PlanStatus,
    pub after: Vec<TaskKey>,
    pub source: String,
    /// 1부터 세는 줄 번호.
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    pub rows: Vec<PlanRow>,
    pub delegation: Vec<String>,
    /// 키 문법에 안 맞아 버린 행의 줄 번호.
    pub parse_errors: Vec<u32>,
}

/// 리뷰 필요 판정(설계 8.9절). `waiver.json`에도 그대로 들어간다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Need {
    pub required: bool,
    pub action: NeedAction,
    pub reason: String,
    pub code_files: u32,
    pub code_lines: u32,
    pub always: Vec<String>,
    pub base: Sha,
    pub head: Sha,
    pub merge_base: Sha,
}

/// 모든 바깥 사실의 형태. `Unknown`의 글은 추적에 남기는 짧은 사유다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Known<T> {
    Yes(T),
    Unknown(&'static str),
}

impl<T> Known<T> {
    pub fn yes(self) -> Option<T> {
        match self {
            Known::Yes(v) => Some(v),
            Known::Unknown(_) => None,
        }
    }

    pub fn as_ref(&self) -> Known<&T> {
        match self {
            Known::Yes(v) => Known::Yes(v),
            Known::Unknown(why) => Known::Unknown(why),
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Known<U> {
        match self {
            Known::Yes(v) => Known::Yes(f(v)),
            Known::Unknown(why) => Known::Unknown(why),
        }
    }
}

/// hook 판정(설계 5절, 출력 형태는 7.2절).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Context(String),
    Warn(String),
    Deny {
        rule: &'static str,
        reason: String,
    },
    Block {
        rule: &'static str,
        reason: String,
        also: Vec<&'static str>,
    },
}

/// CLI 거절 한 건(설계 8.1절). `exit`는 2(거절) 또는 3(사람 손 필요)이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub rule: &'static str,
    pub reason: String,
    pub escape: String,
    pub exit: u8,
}

/// `#[serde(flatten)]`으로 받아 다시 쓰는 모르는 필드 묶음(설계 5절).
pub type Extra = BTreeMap<String, serde_json::Value>;

/// 기준 시점(merge-base 커밋)의 검사 기준(설계 6.1.1절). `config::baseline`이 만든다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baseline {
    pub review: ReviewPolicy,
    pub code: CodeDef,
    pub test_level: TestLevel,
    /// 그 커밋의 설정을 읽지 못해 내장 기본값을 썼다. `lint`·`review need`·`merge`가 한 줄 알린다.
    pub fell_back: bool,
}

/// `parallel_brief`가 문구를 넣는 자리(설계 8.15절).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BriefAt<'a> {
    /// S-01과 `fullops status`의 첫머리.
    Session,
    /// dispatch의 spec. `command`는 spec에 적는 실행 명령(`messages::spec_command`), `key`는 그 과제의 키,
    /// `sub`는 하위 배정이다(하위 배정의 spec에는 하위 배정 문장을 붙이지 않는다).
    Spec {
        command: &'a str,
        key: &'a TaskKey,
        sub: bool,
    },
}

words!(
    /// 메시지의 처리 표시를 누가 하는가(설계 8.6절). `Merge`: 통합 대기인 완료 보고라 `merge`(M-10)·`hold`·통합 확인이 표시한다.
    /// `Ack`: 그 밖 전부. 처리한 뒤 `fullops wait --ack`로 표시한다.
    AckBy { Merge = "merge", Ack = "ack" }
);

/// X-05의 "Orca가 깨운다": 이 세션이 그 Run에 묶인 조율자인가. 둘 다 있고 같으면 참이다.
/// `wait`의 깨움 판정과 `dispatch`의 다음 할 일 안내가 같은 함수를 쓴다(미정-M5가 바꾸는 곳은 여기 하나다).
pub fn orca_wakes(session_handle: Option<&str>, coordinator: Option<&str>) -> bool {
    matches!((session_handle, coordinator), (Some(a), Some(b)) if !a.is_empty() && a == b)
}

/// `state.json`만 보고 그 메시지의 처리 표시를 누가 하는지 정한다(설계 8.6절의 `ack_by`).
pub fn ack_by(state: &State, message: &MessageId) -> AckBy {
    let done = state
        .messages
        .iter()
        .any(|m| &m.id == message && m.kind == "worker_done");
    let pending = state
        .completions
        .iter()
        .any(|c| &c.id == message && c.is_pending());
    if done && pending {
        AckBy::Merge
    } else {
        AckBy::Ack
    }
}

/// sha256의 소문자 hex 64자. 지문(`brief`, `prompt_fp`는 앞 16자)과 `input_sha256` 계산이 함께 쓴다.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 상태 파일에 쓰는 경로 표기. 구분자는 `/`이고 Windows의 `\\?\` 접두어와 끝의 `/`를 뗀다.
pub fn path_text(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let s = s.strip_prefix("//?/").unwrap_or(&s);
    let trimmed = s.trim_end_matches('/');
    if trimmed.is_empty() {
        s.to_string()
    } else {
        trimmed.to_string()
    }
}

/// `path_text` 표기 두 개가 같은 경로인지. Windows에서는 대소문자를 가리지 않는다.
pub fn same_path(a: &str, b: &str) -> bool {
    let (a, b) = (path_text(Path::new(a)), path_text(Path::new(b)));
    if cfg!(windows) {
        a.eq_ignore_ascii_case(&b)
    } else {
        a == b
    }
}
