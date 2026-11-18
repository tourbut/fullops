//! 규칙 표(설계 7.5절). 이 표가 판정 규칙의 정본이고 표 순서가 평가 순서다.
//! `check`는 이벤트별 파일에, 평가 순서와 판정 조립은 `decide.rs`에, 한 호출의 문맥은 `call.rs`에 있다.
//! 표(`RULES`)의 ID·이벤트·대상·강도·탈출구는 동결돼 있고 설계 7.5절의 표와 시험으로 대조한다.

mod call;
mod decide;
mod failure;
mod prompt;
mod session;
mod stop;
mod tool;

pub use call::{NO_SESSION, Scene};
pub use decide::{decide, decide_in};
pub use session::start_skill;

use crate::facts::Facts;
use crate::messages::MsgId;
use crate::model::{
    Actor, BoundBy, Card, DispatchId, EventKind, HookInput, MessageId, RoleKind, RuleTrace, RunId,
    Seat, TaskKey, TurnKind, Verdict,
};

/// 규칙의 강도. `Warn`은 없다: `Warn`을 결과로 내는 규칙이 없고, 한 번 막은 뒤의 통과와 능력이 없는 호스트의 S-00·S-01이
/// `Warn`을 내는 것은 `decide`의 일이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strength {
    Context,
    Effect,
    Deny,
    BlockOnce,
    BlockAlways,
}

impl Strength {
    pub fn as_str(self) -> &'static str {
        match self {
            Strength::Context => "Context",
            Strength::Effect => "Effect",
            Strength::Deny => "Deny",
            Strength::BlockOnce => "BlockOnce",
            Strength::BlockAlways => "BlockAlways",
        }
    }
}

/// 사실을 읽지 못했을 때의 판정. v1의 모든 규칙이 통과다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnUnknown {
    Pass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    None,
    /// 걸리지 않은 것으로 본다.
    Unknown,
    Hit {
        /// 한 번 막기의 지문.
        fp: String,
        /// 문구에 넣을 값.
        args: Vec<String>,
        /// 걸린 경우(T-05의 `a`~`d`, T-08의 `e`~`g`, X-05의 `a`·`b`). 없으면 빈 글이다.
        case: &'static str,
        /// 참이면 `BlockOnce` 규칙을 이번만 `BlockAlways`로 다룬다(X-01의 dev 직접 작업).
        always: bool,
    },
}

pub struct Rule {
    pub id: &'static str,
    pub event: EventKind,
    /// 좌석으로 가르는 대상. `source`, 세션의 `routed`, 호스트 능력 같은 조건은 `check`가 본다.
    pub applies: fn(&Actor) -> bool,
    pub strength: Strength,
    pub on_unknown: OnUnknown,
    pub check: fn(&HookInput, &mut dyn Facts) -> Finding,
    /// 문구 키. 값은 규칙 ID다.
    pub message: MsgId,
    /// `(경우, 탈출구 문장)`. 경우가 하나뿐이면 경우 이름은 빈 글이다. 안내·효과 규칙은 비어 있다.
    pub escapes: &'static [(&'static str, &'static str)],
    /// 설계 11.1절의 1.24.0 기능 이름. 대응이 없으면 `없음`.
    pub origin: &'static str,
}

impl Rule {
    /// `Finding::Hit.case`로 고른 탈출구. 그 경우가 없으면 첫 탈출구, 탈출구가 없으면 빈 글이다.
    pub fn escape(&self, case: &str) -> &'static str {
        self.escapes
            .iter()
            .find(|(name, _)| *name == case)
            .or(self.escapes.first())
            .map_or("", |(_, text)| text)
    }
}

fn all(_: &Actor) -> bool {
    true
}
fn lead(a: &Actor) -> bool {
    a.is_lead()
}
fn coor_lead(a: &Actor) -> bool {
    a.is_lead() && !a.direct_dev
}
fn design_seat(a: &Actor) -> bool {
    matches!(
        a.seat,
        Seat::Role {
            kind: RoleKind::Design,
            ..
        }
    )
}
// 하위 worker의 변경은 부모의 워크트리에 있고 부모의 X-01이 본다
fn seated(a: &Actor) -> bool {
    matches!(a.seat, Seat::Lead | Seat::Role { .. }) && !a.sub
}
fn direct_dev(a: &Actor) -> bool {
    a.direct_dev
}
fn bound(a: &Actor) -> bool {
    a.dispatch.is_some()
}
fn lead_or_bound(a: &Actor) -> bool {
    a.is_lead() || a.dispatch.is_some()
}

macro_rules! rule {
    ($id:literal, $event:ident, $applies:ident, $strength:ident, $check:path, $origin:literal) => {
        rule!($id, $event, $applies, $strength, $check, [], $origin)
    };
    ($id:literal, $event:ident, $applies:ident, $strength:ident, $check:path, [$($escape:expr),*], $origin:literal) => {
        Rule {
            id: $id,
            event: EventKind::$event,
            applies: $applies,
            strength: Strength::$strength,
            on_unknown: OnUnknown::Pass,
            check: $check,
            message: MsgId($id),
            escapes: &[$($escape),*],
            origin: $origin,
        }
    };
}

const DISPATCH: &str = "`fullops dispatch <KEY>`";
const TASK_NEW: &str = "`fullops task new`, 다른 과제는 `fullops plan add`";

#[rustfmt::skip]
pub static RULES: &[Rule] = &[
    rule!("S-00", SessionStart, all, Context, session::s00, "미설정 레포 빈 판정, fail-open, 정책 오류 차단"),
    rule!("S-01", SessionStart, all, Context, session::s01, "SessionStart 역할 안내, 레벨, lead의 준비된 행·위임 범위·열린 결정·버전 차이"),
    rule!("S-02", SessionStart, all, Effect, session::s02, "SessionStart 세션 기준점(reflog 길이)"),
    rule!("S-03", SessionStart, lead, Effect, session::s03, "SessionStart 사라진 snapshot 닫기"),
    rule!("S-04", SessionStart, all, Effect, session::s04, "고정 hook 실행 경로와 실행본 보존"),
    rule!("S-05", SessionStart, all, Context, session::s05, "없음"),
    rule!("S-06", SessionStart, all, Context, session::s06, "SessionStart caveman 스킬 주입"),
    rule!("P-00", UserPromptSubmit, all, Effect, prompt::p00, "없음"),
    rule!("P-01", UserPromptSubmit, all, Effect, prompt::p01, "UserPromptSubmit dispatch 인식, dev 역할 안내"),
    rule!("P-02", UserPromptSubmit, lead, Effect, prompt::p02, "UserPromptSubmit 사람 턴 기록"),
    rule!("P-03", UserPromptSubmit, all, Context, prompt::p03, "미설정 레포 빈 판정, fail-open, 정책 오류 차단"),
    rule!("T-00", PreToolUse, all, Deny, tool::t00,
        [("", "설정을 고치거나 새 세션을 연다. `fullops doctor`")],
        "미설정 레포 빈 판정, fail-open, 정책 오류 차단"),
    rule!("T-01", PreToolUse, all, Deny, tool::t01, [("", DISPATCH)], "PreToolUse `--run` 없는 `worker-start`"),
    rule!("T-02", PreToolUse, all, Deny, tool::t02, [("", DISPATCH)], "PreToolUse 터미널 주입, 긴 `terminal send`"),
    rule!("T-03", PreToolUse, lead, Deny, tool::t03,
        [("", "실행 중인 worker에게 보내는 후속 지시는 `orca orchestration send --to dispatch:<ID> --subject … --body …`(Orca가 권하는 길이고 fullops가 감싸지 않는다). 새 과제나 재작업은 `fullops task reopen` → `fullops dispatch <KEY>`")],
        "PreToolUse 터미널 주입, 긴 `terminal send`"),
    rule!("T-04", PreToolUse, lead, Deny, tool::t04,
        [("", "`fullops route <KEY>` 뒤 spec에 `Task key: <KEY>`를 넣거나 `fullops dispatch`(Task는 `dispatch`가 만든다)")],
        "PreToolUse 배정 선행 조건(route, 인박스, 미통합, 기준선, 리뷰 snapshot)"),
    rule!("T-05", PreToolUse, all, Deny, tool::t05,
        [("a", TASK_NEW), ("b", TASK_NEW), ("c", "`fullops done`"),
         ("d", "같은 과제 수정은 본문만, 다른 과제는 `fullops plan add`")],
        "PreToolUse 인박스 규칙"),
    rule!("T-06", PreToolUse, coor_lead, Deny, tool::t06,
        [("", "설계 역할에 넘긴다: `fullops dispatch <KEY> --role <설계 역할>`")], "PreToolUse 지시서 작성 권한"),
    rule!("T-07", PreToolUse, design_seat, Deny, tool::t07,
        [("", "지시서에 적어 구현 역할에 넘긴다")], "PreToolUse 설계 역할 코드 수정"),
    rule!("T-08", PreToolUse, all, Deny, tool::t08,
        [("e", "직접 처리한다. 방침은 사람이 `fullops.toml`의 `parallel`로 바꾼다"),
         ("f", "지시에 `Purpose: research` 또는 `Purpose: review`와 `read only`를 적는다"),
         ("g", "읽기 전용 조사·리뷰로 좁히거나 직접 처리한다. 구현을 맡기려면 사람이 `parallel`을 `on`으로 바꾼다")],
        "PreToolUse 하위 위임 레벨(`subagent_level`)과 SessionStart·지시서의 레벨 안내(`subagent_brief`)"),
    rule!("T-09", PreToolUse, all, Deny, tool::t09,
        [("", "보고서 판정 항목에 링크하거나 `tasks/<KEY>/qa/raw/`로 옮긴다")], "PreToolUse 링크 없는 캡처 커밋, 차수 폴더"),
    rule!("T-10", PreToolUse, all, Deny, tool::t10,
        [("", "기존 `tasks/<KEY>/qa/report.md`에 절을 덧붙인다. 재작업은 `fullops task reopen`")],
        "PreToolUse 링크 없는 캡처 커밋, 차수 폴더"),
    rule!("T-11", PreToolUse, lead, Effect, tool::t11, "PreToolUse lead의 `check` 미리 보기 기록"),
    rule!("T-12", PreToolUse, all, Effect, tool::t12, "없음"),
    rule!("T-13", PreToolUse, all, Effect, tool::t13, "없음"),
    rule!("T-14", PreToolUse, bound, Deny, tool::t14,
        [("", "preamble의 `ask`로 lead에게 묻는다. 되돌릴 수 있는 선택은 묻지 않고 추천안으로 진행해 완료 보고에 적는다")], "없음"),
    rule!("T-15", PreToolUse, lead, Deny, tool::t15,
        [("", "추천하는 선택지를 1번에 두고 그 label에 `(추천)`을 붙여 다시 묻는다")], "없음"),
    rule!("T-16", PreToolUse, lead, Effect, tool::t16, "없음"),
    rule!("X-00", Stop, all, BlockOnce, stop::x00, [("", "설정을 고치거나 새 세션")],
        "미설정 레포 빈 판정, fail-open, 정책 오류 차단"),
    // dev 직접 작업이면 check가 Finding::Hit.always로 BlockAlways로 올린다(설계 7.5절 표)
    rule!("X-01", Stop, seated, BlockOnce, stop::x01,
        [("", "커밋한 뒤 `fullops lint --from <시작점>`. 적용할 검사가 없으면 이유를 말하고 다시 끝낸다(BlockOnce일 때만)")],
        "Stop lint 통과 게이트, dev 직접 작업의 리뷰 요구"),
    rule!("X-02", Stop, direct_dev, BlockAlways, stop::x02,
        [("", "`fullops review snapshot <KEY>`로 독립 리뷰를 받거나, 사용자 결정 뒤 `fullops review waive`")],
        "Stop lint 통과 게이트, dev 직접 작업의 리뷰 요구"),
    rule!("X-03", Stop, bound, BlockOnce, stop::x03,
        [("", "`fullops done`을 실행하고 출력된 본문으로 preamble의 `worker_done`을 보낸다. 판단이 필요하면 preamble의 `ask`, 막혔으면 `escalation`. 하위 worker는 `fullops done` 없이 preamble의 `worker_done`을 바로 보낸다")],
        "Stop 미정산 dispatch"),
    rule!("X-04", Stop, all, BlockOnce, stop::x04,
        [("", "`fullops task new`와 `fullops dispatch`로 배정하거나 `fullops plan add <KEY> --status \"보류(<사유>)\"`")],
        "Stop 분류 후 미배정"),
    rule!("X-05", Stop, lead_or_bound, BlockOnce, stop::x05,
        [("a", "미처리 메시지를 처리한다. 통합 대기인 완료 보고는 `fullops merge <KEY>`(또는 `fullops hold`)가 처리 표시까지 한다. 그 밖의 메시지는 처리한 뒤 `fullops wait --run <R> --ack <메시지 ID> --no-wait`"),
         ("b", "`fullops wait --run <R>`. 세션 핸들이 있는 세션에서는 Run을 이 세션에 다시 묶고 받은 것만 돌려준 뒤 바로 끝난다. 세션 핸들이 없는 세션에서는 기다린다: 백그라운드 명령이 세션을 깨우는 호스트는 백그라운드로 한 번 걸고 끝내고, 그 밖의 호스트는 포그라운드로 실행해 결과를 기다린다")],
        "Stop Run 대기(백그라운드 대기 통과)"),
    rule!("X-06", Stop, lead, BlockAlways, stop::x06,
        [("", "`fullops merge <KEY>`. 미룰 때는 `fullops hold <KEY> --reason … --owner … --resume …`. 키 없는 보고는 `fullops completion set-sha <메시지 ID> <SHA> --key <KEY>`로 키와 SHA를 적은 뒤 병합하거나 `fullops hold --completion <메시지 ID> --reason … --owner … --resume …`로 미룬다")],
        "Stop 미통합 완료 보고"),
    rule!("X-07", Stop, lead, BlockOnce, stop::x07,
        [("", "계속할지 묻지 말고 배정한다: 준비된 행은 `fullops task new` → `fullops dispatch`, 괄호에 명령이 적힌 과제는 그 명령. 기다려야 하면 `fullops plan set <KEY> \"보류(<사유>)\"` 또는 `\"대기(사람: …)\"`")],
        "Stop 준비된 대기 행"),
    rule!("F-01", StopFailure, bound, Effect, failure::f01, "StopFailure escalation"),
];

/// 규칙 ID로 표의 행을 찾는다. 문구에 탈출구를 넣을 때 쓴다.
pub fn rule(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.id == id)
}

/// hook이 판정 뒤에 실행하는 효과(설계 10.2절). 실행은 `fullops/src/hook/effects.rs`가 한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    CreateSession,
    TouchSession,
    BumpTurn,
    BindDispatch {
        id: DispatchId,
        task: Option<String>,
        by: BoundBy,
    },
    RecordBlock {
        rule: &'static str,
        fp: String,
    },
    RecordNotice(String),
    AppendRouted(TaskKey),
    RecordTurn(TurnKind),
    StampCurrent,
    MarkLanded(Vec<MessageId>),
    MarkSettled(DispatchId),
    CloseRuns(Vec<RunId>),
    CloseLostSnapshots,
    GcSessions,
    SelfInstall,
    PeekCompletions {
        run: Option<RunId>,
    },
    SendLimitEscalation {
        dispatch: DispatchId,
        task: Option<String>,
        error: String,
    },
    /// 프롬프트 이벤트가 없는 호스트의 `prompt_fp`(설계 7.5절). 값은 사용자 입력의 sha256 앞 16자다.
    SetPromptFp(String),
    /// T-16: 질문 카드의 질문마다 열린 결정으로 적는다. 키는 질문 글에서 알아낸 과제 키다.
    RecordCards(Vec<(Card, Option<TaskKey>)>),
}

/// `decide`의 결과. `trace`는 추적 줄의 `rules`가 된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub verdict: Verdict,
    pub effects: Vec<Effect>,
    pub trace: Vec<RuleTrace>,
}

/// 경우를 나누지 않는 규칙의 걸림.
pub(crate) fn hit(fp: impl Into<String>, args: Vec<String>) -> Finding {
    hit_case(fp, args, "")
}

pub(crate) fn hit_case(fp: impl Into<String>, args: Vec<String>, case: &'static str) -> Finding {
    Finding::Hit {
        fp: fp.into(),
        args,
        case,
        always: false,
    }
}

/// 사실을 읽지 못하면 그 규칙은 걸리지 않은 것으로 본다(`Finding::Unknown`).
macro_rules! known {
    ($e:expr) => {
        match $e {
            crate::model::Known::Yes(value) => value,
            crate::model::Known::Unknown(_) => return crate::rules::Finding::Unknown,
        }
    };
}
pub(crate) use known;
