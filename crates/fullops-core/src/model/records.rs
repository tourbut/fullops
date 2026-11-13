//! 커밋되는 기록과 묶음 사이에 주고받는 값의 형식(설계 6.1절, 8.6·8.9·8.11절).

use super::{
    DecisionId, DispatchId, DispatchReason, MessageId, ModelTuple, Need, Origin, Purpose, RoleId,
    RouteClass, RunId, Sha, ShaSource, SubagentLevel, TaskKey, TestLevel, WorkOutcome, words,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

words!(DecidedBy { Jev = "jev", Fallback = "fallback", Manual = "manual" });
words!(LintStatus { Passed = "passed", Failed = "failed", Timeout = "timeout", Unrunnable = "unrunnable", Skipped = "skipped" });
words!(Severity { Error = "ERROR", Warning = "WARNING" });
words!(
    /// Orca 메시지 종류. 모르는 종류는 `Other`로 받는다.
    OrcaMessageKind { Heartbeat = "heartbeat", Status = "status", WorkerDone = "worker_done",
                      Question = "question", Escalation = "escalation", Other = "other" }
);

/// `routes/<KEY>.json`(설계 6.1.4절).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Route {
    pub v: u32,
    pub key: TaskKey,
    pub class: RouteClass,
    /// `unresolved`면 `null`.
    pub role: Option<RoleId>,
    #[serde(default)]
    pub deliverables: Vec<String>,
    pub origin: Origin,
    pub decided_by: DecidedBy,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub request_file: Option<String>,
    #[serde(default)]
    pub request_sha256: Option<String>,
    #[serde(default)]
    pub probabilities: BTreeMap<String, f64>,
    #[serde(default)]
    pub thresholds: BTreeMap<String, f64>,
    /// 키는 역할 ID 또는 `reviewer`. 기록일 뿐이고 다음 과제의 기본값으로 쓰지 않는다.
    #[serde(default)]
    pub tuples: BTreeMap<String, RouteTuple>,
    pub created_at: i64,
    /// 재분류 전의 객체들. 각 원소의 `history`는 비운다.
    #[serde(default)]
    pub history: Vec<Route>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteTuple {
    pub agent: String,
    pub model: String,
    pub effort: String,
    #[serde(default)]
    pub why: String,
}

impl RouteTuple {
    pub fn tuple(&self) -> ModelTuple {
        ModelTuple {
            agent: self.agent.clone(),
            model: self.model.clone(),
            effort: self.effort.clone(),
        }
    }
}

/// 인박스 front matter의 인박스 전용 필드(설계 6.1.3절). `task new`·`task reopen`만 쓴다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inbox {
    pub key: TaskKey,
    pub role: RoleId,
    pub goal: String,
    pub base: Sha,
    /// `direct`·`design`·`product`.
    pub route: RouteClass,
    /// `lead` 또는 작성한 역할 ID.
    pub from: String,
    pub round: u32,
    pub budget: u32,
    pub test_level: TestLevel,
    pub subagent_level: SubagentLevel,
}

/// `lint/pass.json`(설계 8.11절 11번). 통과했을 때만 있다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintPass {
    pub v: u32,
    /// merge-base.
    pub base: Sha,
    pub head: Sha,
    pub tree: Sha,
    pub config_sha256: String,
    pub test_level: TestLevel,
    pub at: i64,
}

/// `lint/result.json`(설계 8.11절 10번).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LintResult {
    pub v: u32,
    pub base: Sha,
    pub merge_base: Sha,
    pub head: Sha,
    pub config_sha256: String,
    pub test_level: TestLevel,
    pub started_at: i64,
    pub seconds: f64,
    pub commands: Vec<LintCommandResult>,
    pub findings: Vec<LintFinding>,
    pub errors: u32,
    pub warnings: u32,
    pub unrunnable: u32,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LintCommandResult {
    pub name: String,
    pub kind: String,
    pub run: Vec<String>,
    pub cwd: String,
    pub status: LintStatus,
    pub exit_code: Option<i32>,
    pub seconds: f64,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub output_tail: String,
    #[serde(default)]
    pub log: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintFinding {
    pub code: String,
    pub severity: Severity,
    pub path: String,
    pub line: Option<u32>,
    pub message: String,
    #[serde(default)]
    pub suggestion: String,
}

/// `reviews/<KEY>/<head 12자>/accepted.json`(설계 8.9절).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepted {
    pub v: u32,
    pub key: TaskKey,
    pub base: Sha,
    pub head: Sha,
    pub result_sha256: String,
    pub lint_sha256: String,
    pub reviewer_session: String,
    pub implementer_session: Option<String>,
    pub at: i64,
}

/// `reviews/<KEY>/<head 12자>/waiver.json`(설계 8.9절). 면제는 수락이 아니다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Waiver {
    pub v: u32,
    pub key: TaskKey,
    pub base: Sha,
    pub head: Sha,
    pub reason: String,
    pub decision: DecisionId,
    /// 항상 `user`.
    pub decided_by: String,
    pub need: Need,
    pub at: i64,
}

/// 그 head의 리뷰 증거(설계 10.2절 `Facts::review_evidence`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewEvidence {
    Accepted,
    Waived,
    None,
}

/// `git diff --numstat`의 한 줄. 바이너리 파일은 줄 수가 0이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStat {
    pub path: String,
    pub added: u32,
    pub deleted: u32,
}

/// 세션이 만든 변경(설계 6.6절 `changed()`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSet {
    /// 시작점.
    pub start: Sha,
    /// 대상 파일(레포 기준 상대 경로, `/` 구분).
    pub code: Vec<String>,
    /// 미커밋 대상 파일과 수정 시각(에폭 초).
    pub dirty: Vec<(String, i64)>,
}

/// Orca가 준 메시지 한 건(설계 9.2절). 해석은 `fullops-ext::orca` 한 곳에서 한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrcaMessage {
    pub id: MessageId,
    /// 전달 ID. ack에 쓴다.
    pub delivery: Option<String>,
    pub run: Option<RunId>,
    pub kind: OrcaMessageKind,
    pub subject: Option<String>,
    /// 출력에만 쓰고 저장하지 않는다.
    pub body: String,
    pub task_id: Option<String>,
    pub dispatch_id: Option<DispatchId>,
    pub outcome: Option<WorkOutcome>,
}

/// `record_completion`의 결과(설계 8.6절). `wait`의 `actionable` 항목이 된다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompletionNote {
    /// completion을 만들었는가. Review·Research 보고에서는 만들지 않는다.
    pub recorded: bool,
    pub key: Option<TaskKey>,
    pub role: Option<RoleId>,
    pub sha: Option<Sha>,
    pub sha_source: Option<ShaSource>,
    pub outcome: Option<WorkOutcome>,
    /// 형식 알림. 차단이 아니다.
    pub notes: Vec<String>,
}

/// `check_dispatch`의 입력. `fullops dispatch`의 인자다(설계 8.5절).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchRequest {
    pub key: TaskKey,
    pub role: Option<RoleId>,
    pub purpose: Purpose,
    pub run: Option<RunId>,
    pub terminal: Option<String>,
    pub tuple: Option<ModelTuple>,
    pub sha: Option<String>,
    pub reason: Option<DispatchReason>,
}

/// `check_task_new`의 입력. `fullops task new|reopen`의 인자다(설계 8.4절).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskNewRequest {
    pub role: RoleId,
    pub key: TaskKey,
    pub goal: String,
    pub reopen: bool,
}
