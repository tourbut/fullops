//! 규칙 표(설계 7.5절). 이 표가 판정 규칙의 정본이고 표 순서가 평가 순서다.
//! 뼈대에서는 모든 `check`가 `Finding::None`이다. 묶음 B가 `check`와 `decide`를 채운다.

mod failure;
mod prompt;
mod session;
mod stop;
mod tool;

use crate::facts::Facts;
use crate::messages::MsgId;
use crate::model::{
    Actor, BoundBy, DispatchId, EventKind, HookInput, MessageId, RoleKind, RuleTrace, RunId, Seat,
    TaskKey, TurnKind, Verdict,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strength {
    Context,
    /// 표의 `Warn` 결과(P-03). 설계의 Strength 목록에는 없어 더했다.
    Warn,
    Effect,
    Deny,
    BlockOnce,
    BlockAlways,
}

impl Strength {
    pub fn as_str(self) -> &'static str {
        match self {
            Strength::Context => "Context",
            Strength::Warn => "Warn",
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
    /// `fp`는 한 번 막기의 지문, `args`는 문구에 넣을 값이다.
    Hit {
        fp: String,
        args: Vec<String>,
    },
    /// 걸리지 않은 것으로 본다.
    Unknown,
}

pub struct Rule {
    pub id: &'static str,
    pub event: EventKind,
    pub applies: fn(&Actor) -> bool,
    pub strength: Strength,
    pub on_unknown: OnUnknown,
    pub check: fn(&HookInput, &mut dyn Facts) -> Finding,
    pub message: MsgId,
    /// 문구에 넣는 탈출구(다음에 칠 명령). 안내·효과 규칙은 비어 있다.
    pub escape: &'static str,
    /// 1.24.0에서 이 규칙이 대응하는 동작(설계 11.1절).
    pub origin: &'static str,
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
fn seated(a: &Actor) -> bool {
    matches!(a.seat, Seat::Lead | Seat::Role { .. })
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
    ($id:literal, $event:ident, $applies:ident, $strength:ident, $check:path, $escape:literal, $origin:literal) => {
        Rule {
            id: $id,
            event: EventKind::$event,
            applies: $applies,
            strength: Strength::$strength,
            on_unknown: OnUnknown::Pass,
            check: $check,
            message: MsgId($id),
            escape: $escape,
            origin: $origin,
        }
    };
}

#[rustfmt::skip]
pub static RULES: &[Rule] = &[
    rule!("S-00", SessionStart, all, Context, session::s00, "", "미설정 레포 빈 판정, fail-open, 정책 오류 차단"),
    rule!("S-01", SessionStart, all, Context, session::s01, "", "SessionStart 역할 안내, 레벨, lead의 준비된 행·위임 범위·열린 결정·버전 차이"),
    rule!("S-02", SessionStart, all, Effect, session::s02, "", "SessionStart 세션 기준점(reflog 길이)"),
    rule!("S-03", SessionStart, lead, Effect, session::s03, "", "SessionStart 사라진 snapshot 닫기"),
    rule!("S-04", SessionStart, all, Effect, session::s04, "", "고정 hook 실행 경로와 실행본 보존"),
    rule!("S-05", SessionStart, all, Context, session::s05, "", "없음(새 동작, 시나리오 B6)"),
    rule!("P-00", UserPromptSubmit, all, Effect, prompt::p00, "", "없음(턴 기준 한 번 막기, DF-11)"),
    rule!("P-01", UserPromptSubmit, all, Effect, prompt::p01, "", "UserPromptSubmit dispatch 인식, dev 역할 안내"),
    rule!("P-02", UserPromptSubmit, lead, Effect, prompt::p02, "", "UserPromptSubmit 사람 턴 기록"),
    rule!("P-03", UserPromptSubmit, all, Warn, prompt::p03, "", "정책 오류 차단"),
    rule!("T-00", PreToolUse, all, Deny, tool::t00,
        "설정을 고치거나 새 세션을 연다. `fullops doctor`", "정책 오류 차단"),
    rule!("T-01", PreToolUse, all, Deny, tool::t01,
        "`fullops dispatch <KEY>`", "PreToolUse `--run` 없는 worker-start(DF-1)"),
    rule!("T-02", PreToolUse, all, Deny, tool::t02,
        "`fullops dispatch <KEY>`", "PreToolUse 터미널 주입"),
    rule!("T-03", PreToolUse, lead, Deny, tool::t03,
        "후속 지시는 `fullops dispatch <KEY> --terminal <핸들>`", "PreToolUse 긴 terminal send"),
    rule!("T-04", PreToolUse, lead, Deny, tool::t04,
        "`fullops route <KEY>` 뒤 spec에 `Task key: <KEY>`를 넣거나 `fullops dispatch`",
        "PreToolUse 배정 선행 조건(날것 task-create)"),
    rule!("T-05", PreToolUse, all, Deny, tool::t05,
        "(a)(b) `fullops task new`, 다른 과제는 `fullops plan add` (c) `fullops done` (d) 같은 과제 수정은 본문만, 다른 과제는 `fullops plan add`",
        "PreToolUse 인박스 규칙"),
    rule!("T-06", PreToolUse, coor_lead, Deny, tool::t06,
        "설계 역할에 넘긴다: `fullops dispatch <KEY> --role <설계 역할>`", "PreToolUse 지시서 작성 권한"),
    rule!("T-07", PreToolUse, design_seat, Deny, tool::t07,
        "지시서에 적어 구현 역할에 넘긴다", "PreToolUse 설계 역할 코드 수정"),
    rule!("T-08", PreToolUse, all, Deny, tool::t08,
        "(e) 직접 처리한다. 레벨은 사람이 `fullops.toml`의 `subagent_level`로 바꾼다 (f) 지시에 `Purpose:`를 적는다 (g) 읽기 전용 조사·리뷰로 좁히거나 직접 처리한다",
        "PreToolUse 하위 위임 레벨(DF-6)"),
    rule!("T-09", PreToolUse, all, Deny, tool::t09,
        "보고서 판정 항목에 링크하거나 `<과제 폴더>/raw/`로 옮긴다", "PreToolUse 링크 없는 캡처 커밋"),
    rule!("T-10", PreToolUse, all, Deny, tool::t10,
        "기존 `qa-reports/<폴더>/report.md`에 절을 덧붙인다. 재작업은 `fullops task reopen`", "PreToolUse 차수 폴더"),
    rule!("T-11", PreToolUse, lead, Effect, tool::t11, "", "PreToolUse lead의 check 미리 보기 기록"),
    rule!("T-12", PreToolUse, all, Effect, tool::t12, "", "없음(CLI 호출자 판정용 current.json)"),
    rule!("X-00", Stop, all, BlockOnce, stop::x00, "설정을 고치거나 새 세션", "정책 오류 차단"),
    // dev 직접 작업이면 decide가 BlockAlways로 올린다(설계 7.5절 표)
    rule!("X-01", Stop, seated, BlockOnce, stop::x01,
        "커밋한 뒤 `fullops lint --from <시작점>`. 적용할 검사가 없으면 이유를 말하고 다시 끝낸다(BlockOnce일 때만)",
        "Stop lint 통과 게이트"),
    rule!("X-02", Stop, direct_dev, BlockAlways, stop::x02,
        "`fullops review snapshot <KEY>`로 독립 리뷰를 받거나, 사용자 결정 뒤 `fullops review waive`",
        "Stop dev 직접 작업의 리뷰 요구"),
    rule!("X-03", Stop, bound, BlockOnce, stop::x03,
        "`fullops done`을 실행하고 출력된 본문으로 preamble의 `worker_done`을 보낸다. 판단이 필요하면 preamble의 `ask`, 막혔으면 `escalation`",
        "Stop 미정산 dispatch"),
    rule!("X-04", Stop, all, BlockOnce, stop::x04,
        "`fullops task new`와 `fullops dispatch`로 배정하거나 `fullops plan add <KEY> --status \"보류(<사유>)\"`",
        "Stop 분류 후 미배정"),
    rule!("X-05", Stop, lead_or_bound, BlockOnce, stop::x05,
        "(a) 메시지를 처리하고 `fullops wait --run <R> --ack <delivery>` (b) Claude Code는 `fullops wait --run <R>`을 백그라운드로 한 번 걸고 끝낸다. Codex는 포그라운드로 실행해 결과를 기다린다",
        "Stop Run 대기(백그라운드 대기 통과)"),
    rule!("X-06", Stop, lead, BlockAlways, stop::x06,
        "`fullops merge <KEY>`. 미룰 때는 `fullops hold <KEY> --reason … --owner … --resume …`",
        "Stop 미통합 완료 보고"),
    rule!("X-07", Stop, lead, BlockOnce, stop::x07,
        "계속할지 묻지 말고 배정한다: `fullops task new` → `fullops dispatch`. 기다려야 하면 `fullops plan set <KEY> \"보류(<사유>)\"` 또는 `\"대기(사람: …)\"`",
        "Stop 준비된 대기 행"),
    rule!("F-01", StopFailure, bound, Effect, failure::f01, "", "StopFailure escalation"),
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
}

/// `decide`의 결과. `trace`는 추적 줄의 `rules`가 된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub verdict: Verdict,
    pub effects: Vec<Effect>,
    pub trace: Vec<RuleTrace>,
}

/// 이벤트 하나를 판정한다. 뼈대에서는 모든 규칙이 걸리지 않아 항상 통과다.
pub fn decide(_input: &HookInput, _facts: &mut dyn Facts) -> Outcome {
    Outcome {
        verdict: Verdict::Pass,
        effects: Vec::new(),
        trace: Vec::new(),
    }
}
