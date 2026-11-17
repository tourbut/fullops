//! 커밋되는 기록과 묶음 사이에 주고받는 값의 형식(설계 5절, 6.1절, 8.6·8.9·8.11절, 10.2·10.3·10.7절).

use super::{
    DecisionId, DispatchId, DispatchReason, HostId, MessageId, ModelTuple, Need, Origin, Parallel,
    Purpose, RoleId, RouteClass, RunId, Sha, ShaSource, TaskKey, TestLevel, WorkOutcome, words,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

words!(DecidedBy { Jev = "jev", Fallback = "fallback", Manual = "manual" });
words!(LintStatus { Passed = "passed", Failed = "failed", Timeout = "timeout", Unrunnable = "unrunnable", Skipped = "skipped" });
words!(Severity { Error = "ERROR", Warning = "WARNING" });

/// `tasks/<KEY>/route.json`(설계 6.1.4절).
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
    /// Orca의 에이전트 ID 그대로(agy는 `antigravity`).
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

/// 인박스 front matter의 인박스 전용 필드 열 개(설계 6.1.3절). `task new`·`task reopen`만 쓴다.
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
    /// `task new` 시점의 기록이다. 판정과 안내는 작업 트리의 `fullops.toml`을 본다.
    pub parallel: Parallel,
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

/// `tasks/<KEY>/reviews/<head 12자>/accepted.json`(설계 8.9절).
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

/// `tasks/<KEY>/reviews/<head 12자>/waiver.json`(설계 8.9절). 면제는 수락이 아니다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Waiver {
    pub v: u32,
    pub key: TaskKey,
    pub base: Sha,
    pub head: Sha,
    pub reason: String,
    pub decision: DecisionId,
    /// 결정에 답한 사람(`--by`). 없으면 답이 온 길(`user`·`bot`)이다.
    pub decided_by: String,
    /// 결정의 `via`.
    #[serde(default)]
    pub via: Option<Origin>,
    /// 결정을 연 뒤의 사람 턴을 확인하고 받은 답인가(8.10절). hook이 돌지 않는 환경의 답은 거짓이다.
    #[serde(default)]
    pub human_turn: bool,
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

/// `git diff --numstat`의 한 줄. 바이너리 파일은 줄 수가 0이고 `binary`가 참이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStat {
    pub path: String,
    pub added: u32,
    pub deleted: u32,
    /// git이 줄 수를 세지 못했다.
    pub binary: bool,
}

/// `git ls-tree -r`의 한 줄(설계 10.2절). `fullops-store::update_index`도 받으므로 core에 둔다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    /// `100644` 등.
    pub mode: String,
    /// `blob` 또는 `commit`(하위 모듈).
    pub kind: String,
    pub oid: String,
    /// 레포 기준 상대 경로(`/` 구분).
    pub path: String,
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

/// S-06이 붙이는 사용자 스킬의 글(설계 10.2절 `Facts::user_skill`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillText {
    pub path: PathBuf,
    /// front matter 뒤의 본문.
    pub body: String,
}

/// 산출물 인덱스 `docs/deliverables/README.md`의 한 행(설계 10.2절). 열 문법은 미정-M7이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliverableRow {
    /// `D01` 형식.
    pub id: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    /// 원천 문서의 레포 상대 경로.
    pub sources: Vec<String>,
}

/// Orca가 준 메시지 한 건(설계 9.2절, 10.2절). 해석은 `fullops-ext::orca` 한 곳에서 한다. 메시지에 Run은 없다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrcaMessage {
    pub id: MessageId,
    /// Orca의 `type`. 없으면 `status`.
    pub kind: String,
    pub from: Option<String>,
    pub subject: Option<String>,
    /// 출력에만 쓰고 저장하지 않는다.
    pub body: Option<String>,
    /// `payload.taskId`.
    pub task_id: Option<String>,
    /// `payload.dispatchId`.
    pub dispatch_id: Option<DispatchId>,
    /// `payload.outcome`.
    pub outcome: Option<String>,
}

/// `record_message`가 completion을 만들었을 때의 기록(설계 8.6절).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompletionNote {
    pub key: Option<TaskKey>,
    pub role: Option<RoleId>,
    pub sha: Option<Sha>,
    pub sha_source: Option<ShaSource>,
    pub outcome: Option<WorkOutcome>,
}

/// `record_message`의 결과(설계 10.2절). `wait`의 `actionable`·`absorbed`가 된다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MessageNote {
    /// `heartbeat`·`status`라 세기만 하고 `messages`에 넣지 않았다.
    pub absorbed: bool,
    /// completion을 만들었으면 그 기록. Review·Research 보고와 하위 배정의 보고에서는 만들지 않는다.
    pub completion: Option<CompletionNote>,
    /// 형식 알림. 차단이 아니다.
    pub notes: Vec<String>,
}

/// `check_dispatch`의 입력. `fullops dispatch`의 인자다(설계 8.5절). `--task`는 없다.
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
    /// `--brief`의 글. 하위 배정에서 필수이고 lead가 주면 사용법 오류다. Orca로만 가고 저장하지 않는다.
    pub brief: Option<String>,
    /// 하위 배정이다(호출자가 dispatch가 묶인 세션이다). D-04·D-08·D-09·D-13만 검사한다.
    pub sub: bool,
}

/// `check_task_new`의 입력. `fullops task new|reopen`의 인자다(설계 8.4절).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskNewRequest {
    pub role: RoleId,
    pub key: TaskKey,
    pub goal: String,
    pub reopen: bool,
}

/// 형식 버전. 이 값이 아닌 탐색 결과 파일은 없는 것으로 본다.
pub const EXPLORE_V: u32 = 1;

// ── 의존 목록과 설치 기록(설계 10.3·10.7절) ─────────────────────

/// `deps.toml`. 읽기와 필드 규칙 검증은 `fullops-store::load_deps`가 한다. 모르는 표와 필드는 무시한다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deps {
    pub schema: u32,
    /// 사용자 스킬 설치 도구의 argv 머리.
    #[serde(default = "default_skill_cli")]
    pub skill_cli: Vec<String>,
    #[serde(rename = "self")]
    pub own: DepsSelf,
    /// 이름 → 출처.
    #[serde(default)]
    pub marketplaces: BTreeMap<String, String>,
    #[serde(default)]
    pub plugins: Vec<DepsPlugin>,
    #[serde(default)]
    pub skills: Vec<DepsSkills>,
    #[serde(default)]
    pub mcp: Vec<DepsMcp>,
    #[serde(default)]
    pub optional: Vec<DepsOptional>,
}

fn default_skill_cli() -> Vec<String> {
    ["npx", "--yes", "skills@latest"].map(String::from).to_vec()
}

/// `[self]`: fullops 플러그인 자신.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepsSelf {
    pub plugin: String,
    pub marketplace: String,
    pub source: String,
    pub hosts: Vec<HostId>,
}

/// `[[plugins]]`: 호스트 플러그인. `id`는 `<이름>@<마켓플레이스>`다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepsPlugin {
    pub id: String,
    pub hosts: Vec<HostId>,
    /// 설치 뒤에 내는 주의 문구의 이름. 모르는 값은 무시한다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

/// `[[skills]]`: 사용자 스킬 묶음. `hosts`를 생략하면 네 호스트 전부다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepsSkills {
    pub source: String,
    /// 이름 목록 또는 `["*"]`(그 출처의 스킬 전부).
    pub names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<HostId>>,
    /// 기록이고 동작에 쓰지 않는다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

/// `[[mcp]]`: 원격 HTTP MCP. `url`은 `https://`로 시작한다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepsMcp {
    pub name: String,
    pub url: String,
}

/// `[[optional]]`: 설치하지 않고 안내만 한다. `kind`는 `tool`·`skill`이다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepsOptional {
    pub kind: String,
    pub name: String,
    pub source: String,
    pub note: String,
}

/// `~/.fullops/deps-state.json`: 의존 설치 기록.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepsState {
    pub v: u32,
    /// 설치한 시점의 `deps.toml`의 sha256.
    pub manifest_sha256: String,
    #[serde(default)]
    pub hosts: BTreeMap<HostId, HostInstall>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostInstall {
    pub at: i64,
    pub self_version: String,
    /// 호스트별 명령과 그 호스트가 대상인 스킬 명령이 모두 성공했다.
    pub complete: bool,
    /// 실패한 명령의 `Step.what`.
    #[serde(default)]
    pub failed: Vec<String>,
}
