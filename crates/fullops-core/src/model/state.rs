//! `state.json`, `archive.jsonl`, `status.json`의 형식(설계 6.2절).

use super::{
    DecisionId, DispatchId, Extra, Json, Landing, MessageId, Mode, ModelTuple, Origin, Purpose,
    RoleId, RunId, SessionId, Sha, Stage, TaskKey, words,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 이 바이너리가 아는 `state.json`의 `v`.
pub const STATE_V: u32 = 1;

words!(SessionMode { Fresh = "fresh", Retain = "retain" });
words!(DispatchReason { Limit = "limit" });
words!(SettledHow { Done = "done", Escalated = "escalated", Lost = "lost" });
words!(
    /// 완료 보고의 결과. Orca 메시지에 값이 없으면 `succeeded`다.
    WorkOutcome { Succeeded = "succeeded", Failed = "failed" }
);
words!(ShaSource { Done = "done", Body = "body", BranchTip = "branch_tip", Manual = "manual" });
words!(
    /// `state.json`에 넣는 메시지 종류. heartbeat·status는 넣지 않는다.
    MessageType { WorkerDone = "worker_done", Question = "question", Escalation = "escalation" }
);
words!(SnapshotState { Creating = "creating", Active = "active", Removed = "removed", Abandoned = "abandoned", Missing = "missing" });
words!(DecisionStatus { Open = "open", Answered = "answered", Dropped = "dropped" });

/// Run과 snapshot의 소유자. `"lead"` 또는 `"dispatch:<DispatchId>"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum RunOwner {
    Lead,
    Dispatch(DispatchId),
}

impl TryFrom<String> for RunOwner {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        if s == "lead" {
            return Ok(RunOwner::Lead);
        }
        s.strip_prefix("dispatch:")
            .and_then(|id| DispatchId::new(id).ok())
            .map(RunOwner::Dispatch)
            .ok_or_else(|| format!("owner 값을 읽을 수 없습니다: {s:?}"))
    }
}

impl From<RunOwner> for String {
    fn from(owner: RunOwner) -> String {
        match owner {
            RunOwner::Lead => "lead".to_string(),
            RunOwner::Dispatch(id) => format!("dispatch:{id}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub v: u32,
    /// `update_state`가 쓸 때마다 1 올린다.
    #[serde(default)]
    pub seq: u64,
    #[serde(default)]
    pub runs: Vec<Run>,
    #[serde(default)]
    pub dispatches: Vec<Dispatch>,
    #[serde(default)]
    pub completions: Vec<Completion>,
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tasks: BTreeMap<TaskKey, TaskCounters>,
    /// 역할 → 반영해야 할 기본 브랜치 SHA.
    #[serde(default)]
    pub sync_pending: BTreeMap<RoleId, Sha>,
    #[serde(default)]
    pub snapshots: Vec<Snapshot>,
    #[serde(default)]
    pub decisions: Vec<Decision>,
    /// 다음 결정 ID의 번호.
    #[serde(default = "one")]
    pub next_decision: u32,
    #[serde(flatten)]
    pub extra: Extra,
}

fn one() -> u32 {
    1
}

impl Default for State {
    fn default() -> Self {
        Self {
            v: STATE_V,
            seq: 0,
            runs: Vec::new(),
            dispatches: Vec::new(),
            completions: Vec::new(),
            messages: Vec::new(),
            tasks: BTreeMap::new(),
            sync_pending: BTreeMap::new(),
            snapshots: Vec::new(),
            decisions: Vec::new(),
            next_decision: 1,
            extra: Extra::new(),
        }
    }
}

impl State {
    pub fn dispatch(&self, id: &DispatchId) -> Option<&Dispatch> {
        self.dispatches.iter().find(|d| &d.id == id)
    }

    pub fn dispatch_mut(&mut self, id: &DispatchId) -> Option<&mut Dispatch> {
        self.dispatches.iter_mut().find(|d| &d.id == id)
    }

    /// 통합 대기 completion(설계 6.6절).
    pub fn pending(&self) -> impl Iterator<Item = &Completion> {
        self.completions.iter().filter(|c| c.is_pending())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub owner: RunOwner,
    pub opened_at: i64,
    pub closed_at: Option<i64>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dispatch {
    pub id: DispatchId,
    /// Orca task ID.
    pub task_id: Option<String>,
    pub run: RunId,
    pub key: TaskKey,
    pub role: RoleId,
    pub purpose: Purpose,
    pub tuple: ModelTuple,
    pub terminal: Option<String>,
    /// 대상 워크트리. `path_text` 표기.
    pub worktree: String,
    pub session_mode: SessionMode,
    pub reason: Option<DispatchReason>,
    pub started_at: i64,
    pub bound_session: Option<SessionId>,
    pub reported: Option<Reported>,
    pub settled: Option<Settled>,
    #[serde(default)]
    pub limit_escalated: bool,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Dispatch {
    /// Build·Fix dispatch인가. completion은 이 둘에서만 생긴다.
    pub fn is_work(&self) -> bool {
        matches!(self.purpose, Purpose::Build | Purpose::Fix)
    }
}

/// `fullops done`이 쓴 완료 준비 기록.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reported {
    pub branch: String,
    pub sha: Sha,
    /// `lint/pass.json`의 head. `--no-checks`로 끝냈으면 `null`.
    pub lint_head: Option<Sha>,
    #[serde(default)]
    pub followups: Vec<Followup>,
    pub at: i64,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Followup {
    pub role: RoleId,
    pub text: String,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settled {
    pub how: SettledHow,
    /// `how == done`일 때만 있다.
    #[serde(default)]
    pub outcome: Option<WorkOutcome>,
    #[serde(default)]
    pub message: Option<MessageId>,
    pub at: i64,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Completion {
    /// 보고 메시지의 ID. dev 직접 작업은 `direct:<sha 앞 12자>`.
    pub id: MessageId,
    pub dispatch: Option<DispatchId>,
    /// 과제 키를 못 찾았으면 `null`(설계 8.6절).
    pub key: Option<TaskKey>,
    pub role: Option<RoleId>,
    pub branch: Option<String>,
    /// `null`이면 통합 대기로 센다.
    pub sha: Option<Sha>,
    pub sha_source: Option<ShaSource>,
    #[serde(default)]
    pub direct: bool,
    pub outcome: WorkOutcome,
    pub received_at: i64,
    pub hold: Option<Hold>,
    pub released_hold: Option<Hold>,
    pub landed_at: Option<i64>,
    pub drift: Option<u32>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Completion {
    /// 통합 대기인가: 직접 작업이 아니고, 성공했고, 보류가 없고, 아직 통합되지 않았다.
    pub fn is_pending(&self) -> bool {
        !self.direct
            && self.outcome == WorkOutcome::Succeeded
            && self.hold.is_none()
            && self.landed_at.is_none()
    }
}

/// 보류. 사유·담당·재개 조건 셋을 다 가진다(설계 5절 불변식 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hold {
    pub reason: String,
    pub owner: String,
    pub resume: String,
    pub at: i64,
    #[serde(flatten)]
    pub extra: Extra,
}

/// 처리할 메시지. 본문은 넣지 않는다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub delivery: Option<String>,
    pub run: RunId,
    #[serde(rename = "type")]
    pub kind: MessageType,
    pub dispatch: Option<DispatchId>,
    pub received_at: i64,
    /// `null`이면 미처리다.
    pub acked_at: Option<i64>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `tasks[<KEY>]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskCounters {
    #[serde(default)]
    pub rounds: u32,
    #[serde(default = "default_budget")]
    pub budget: u32,
    #[serde(default)]
    pub limit_redispatch: u32,
    #[serde(default)]
    pub last_tuple: Option<ModelTuple>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// 반복 예산의 기본값(설계 8.4절).
pub const DEFAULT_BUDGET: u32 = 2;

fn default_budget() -> u32 {
    DEFAULT_BUDGET
}

impl Default for TaskCounters {
    fn default() -> Self {
        Self {
            rounds: 0,
            budget: DEFAULT_BUDGET,
            limit_redispatch: 0,
            last_tuple: None,
            extra: Extra::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// `<KEY>@<head 앞 12자>`.
    pub review: String,
    /// snapshot 워크트리. `path_text` 표기.
    pub path: String,
    pub head: Sha,
    pub base: Sha,
    pub state: SnapshotState,
    pub implementer_session: Option<String>,
    pub reviewer_dispatch: Option<DispatchId>,
    pub owner: RunOwner,
    pub created_at: i64,
    /// 정리하지 못한 사유.
    pub hold: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Snapshot {
    /// `review`의 과제 키 부분.
    pub fn key(&self) -> &str {
        self.review
            .split_once('@')
            .map_or(&self.review, |(key, _)| key)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub id: DecisionId,
    pub status: DecisionStatus,
    pub key: Option<TaskKey>,
    pub question: String,
    #[serde(default)]
    pub options: Vec<String>,
    /// 멈출 과제 키 또는 메시지 ID.
    #[serde(default)]
    pub blocks: Vec<String>,
    pub asked_at: i64,
    pub origin: Origin,
    /// 있으면 답도 같은 SHA를 내야 하고 24시간 뒤 만료된다.
    pub sha: Option<Sha>,
    #[serde(default)]
    pub risks: Vec<String>,
    /// 있으면 답하는 사람이 이 안에 있어야 한다.
    #[serde(default)]
    pub allow: Vec<String>,
    pub expires_at: Option<i64>,
    pub answer: Option<String>,
    pub decided_by: Option<String>,
    pub via: Option<Origin>,
    pub closed_at: Option<i64>,
    /// `decision add --repository`. 설계 6.2.1절 예시에 없어 값이 있을 때만 쓴다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// `decision drop --reason`. 설계 6.2.1절 예시에 없어 값이 있을 때만 쓴다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_reason: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `archive.jsonl`의 한 줄. 과제 줄이거나 `{"decision":{…}}` 줄이다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArchiveLine {
    Task(ArchivedTask),
    Decision { decision: Decision },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchivedTask {
    pub key: TaskKey,
    pub role: Option<RoleId>,
    pub sha: Option<Sha>,
    pub received_at: i64,
    pub landed_at: Option<i64>,
    #[serde(default)]
    pub rounds: u32,
    #[serde(default)]
    pub dispatches: Vec<ArchivedDispatch>,
    #[serde(default)]
    pub direct: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchivedDispatch {
    pub purpose: Purpose,
    pub started_at: i64,
    pub reason: Option<DispatchReason>,
}

/// `status.json`(설계 6.2.3절). token, 본문, 프롬프트는 넣지 않는다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub v: u32,
    pub generated_at: i64,
    /// 설정을 읽지 못했으면 이전 값, 그것도 없으면 `null`.
    pub mode: Option<Mode>,
    /// 이 파일을 쓴 실행본의 버전.
    pub version: String,
    pub tasks: Vec<StatusTask>,
    pub dispatches: Vec<StatusDispatch>,
    pub pending: Vec<StatusPending>,
    pub decisions: Vec<StatusDecision>,
    pub ready: Vec<TaskKey>,
    /// 역할 → 기본 브랜치 SHA 앞 7자.
    pub sync_pending: BTreeMap<RoleId, String>,
    pub metrics: Metrics,
    /// v1에서는 항상 `null`(이슈 자동 모드 자리).
    pub issue: Option<Json>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusTask {
    pub key: TaskKey,
    pub role: Option<RoleId>,
    pub stage: Stage,
    pub rounds: u32,
    pub budget: u32,
    /// 가장 늦은 completion의 SHA 앞 7자.
    pub sha: Option<String>,
    pub held: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusDispatch {
    pub key: TaskKey,
    pub role: RoleId,
    pub purpose: Purpose,
    pub agent: String,
    pub model: String,
    pub started_at: i64,
    pub settled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusPending {
    pub key: Option<TaskKey>,
    /// SHA 앞 7자. SHA가 없으면 `null`.
    pub sha: Option<String>,
    /// 같은 SHA의 이전 값을 유지한다. 계산된 적이 없으면 `null`.
    pub landing: Option<Landing>,
    pub hold_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusDecision {
    pub id: DecisionId,
    pub key: Option<TaskKey>,
    pub question: String,
    pub options: Vec<String>,
    pub asked_at: i64,
    pub expires_at: Option<i64>,
}

/// 지표(설계 6.7절). 어떤 규칙의 조건으로도 쓰지 않는다.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Metrics {
    /// 첫 사람 턴의 시각. 사람 턴이 없으면 0.
    pub since: i64,
    pub human_turns: u32,
    pub done_tasks: u32,
    /// 완료 과제가 0이면 `null`.
    pub turns_per_task: Option<f64>,
    pub unattended_dispatches: u32,
    pub unattended_tasks: u32,
}
