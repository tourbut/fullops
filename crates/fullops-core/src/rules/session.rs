//! SessionStart 규칙의 `check`(설계 7.5절, 7.7절).

use super::call::{self, NO_SESSION};
use super::decide::notice_keys;
use super::{Effect, Finding, hit};
use crate::facts::Facts;
use crate::messages::hook::{self, Brief, RoleGuide};
use crate::messages::parallel_brief;
use crate::model::{
    Actor, BriefAt, DecisionStatus, HookInput, HostId, Known, Mode, Parallel, RoleId, RunOwner,
    Seat, SnapshotState,
};

/// S-01·S-06의 대상 `source`. `fork`는 갈라진 새 세션이라 `startup`과 같게 다룬다.
fn fresh(input: &HookInput) -> bool {
    matches!(
        input.source.as_deref(),
        None | Some("startup" | "clear" | "fork")
    )
}

/// 역할 안내 한 덩어리. S-01, S-05, P-01이 함께 쓴다.
pub(super) fn role_text(facts: &mut dyn Facts, actor: &Actor) -> String {
    let dev = facts.config().is_ok_and(|c| c.mode == Mode::Dev);
    let guide = match &actor.seat {
        Seat::Lead if actor.dispatch.is_some() => {
            // dispatch를 받은 기본 브랜치·주 담당 세션은 lead가 아니다. 맡은 역할로 안내한다
            match own_role(facts, actor) {
                Some(id) => {
                    return hook::role_guide(
                        &RoleGuide::Role {
                            id: id.as_str(),
                            kind: crate::model::RoleKind::Worker,
                        },
                        actor.sub,
                    );
                }
                None => RoleGuide::Other,
            }
        }
        Seat::Lead if dev => RoleGuide::DevPrimary,
        Seat::Lead => RoleGuide::Coordinator,
        Seat::Role { id, kind } => RoleGuide::Role {
            id: id.as_str(),
            kind: *kind,
        },
        Seat::Reviewer => RoleGuide::Reviewer,
        Seat::Other => RoleGuide::Other,
    };
    hook::role_guide(&guide, actor.sub)
}

/// 이 체크아웃의 역할: 역할 좌석이면 그 역할, dev 모드의 주 담당 좌석이면 `primary_role`.
pub(super) fn own_role(facts: &mut dyn Facts, actor: &Actor) -> Option<RoleId> {
    match &actor.seat {
        Seat::Role { id, .. } => Some(id.clone()),
        Seat::Lead => facts
            .config()
            .ok()
            .filter(|c| c.mode == Mode::Dev)
            .and_then(|c| c.primary_role.clone()),
        _ => None,
    }
}

fn inbox_line(facts: &mut dyn Facts, actor: &Actor) -> Option<String> {
    let role = own_role(facts, actor)?;
    let inbox = facts.inbox(&role).yes().flatten()?;
    Some(format!(
        "현재 인박스: `.fullops-squad/handovers/to_{role}.md` (과제 `{}`)",
        inbox.key
    ))
}

/// lead 항목(설계 7.7절). `state.json`이나 `PLANS.md`를 읽지 못하면 그 항목을 뺀다.
fn lead_items(facts: &mut dyn Facts, brief: &mut Brief) {
    brief.head.extend(
        [hook::LEAD_MERGE, hook::LEAD_WAIT, hook::LEAD_HUMAN]
            .iter()
            .map(|s| s.to_string()),
    );
    if let Known::Yes((keys, plan)) = super::stop::ready_rows(facts) {
        brief.ready = keys
            .iter()
            .map(|key| {
                let goal = plan.rows.iter().find(|r| &r.key == key);
                (key.to_string(), goal.map_or("", |r| &r.goal).to_string())
            })
            .collect();
        brief.delegation = plan.delegation;
    }
    let now = facts.now();
    let version = facts.config().ok().map(|c| c.applied_version.clone());
    if let Known::Yes(state) = facts.state() {
        brief.decisions = state
            .decisions
            .iter()
            .filter(|d| d.status == DecisionStatus::Open)
            .map(|d| {
                let expired = d.sha.is_some() && d.expires_at.is_some_and(|at| at < now);
                format!(
                    "- {}: {} (막는 과제: {}){}",
                    d.id,
                    d.question,
                    d.blocks.join(", "),
                    if expired {
                        " — 만료됐다. 다시 묻는다"
                    } else {
                        ""
                    }
                )
            })
            .collect();
        let pending = state.pending().count();
        let held = state
            .completions
            .iter()
            .filter(|c| c.hold.is_some() && c.landed_at.is_none())
            .count();
        if pending + held > 0 {
            brief
                .tail
                .push(format!("통합 대기 {pending}건, 보류 {held}건."));
        }
        if !state.sync_pending.is_empty() {
            let roles: Vec<&str> = state.sync_pending.keys().map(|r| r.as_str()).collect();
            brief.tail.push(format!(
                "예약된 동기화: {}(`fullops sync`).",
                roles.join(", ")
            ));
        }
        let over: Vec<String> = state
            .tasks
            .iter()
            .filter(|(_, t)| t.rounds > t.budget)
            .map(|(key, t)| format!("{key}({}/{})", t.rounds, t.budget))
            .collect();
        if !over.is_empty() {
            brief
                .tail
                .push(format!("예산을 넘은 재작업: {}.", over.join(", ")));
        }
        let runs: Vec<&str> = state
            .runs
            .iter()
            .filter(|r| r.closed_at.is_none() && r.owner == RunOwner::Lead)
            .map(|r| r.id.as_str())
            .collect();
        if !runs.is_empty() {
            brief.tail.push(format!(
                "열린 Run: {}. 새 세션은 `fullops wait --run <R>`로 Run을 이어받는다.",
                runs.join(", ")
            ));
        }
    }
    let installed = env!("CARGO_PKG_VERSION");
    if let Some(applied) = version.filter(|v| v != installed) {
        brief.tail.push(format!(
            "설치 버전 {installed}과 레포의 `applied_version` {applied}이 다르다(`fullops upgrade`)."
        ));
    }
}

/// S-00
pub(super) fn s00(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    match facts.policy_error() {
        Some(e) => hit(e.kind.as_str(), vec![e.detail]),
        None => Finding::None,
    }
}

/// S-01
pub(super) fn s01(input: &HookInput, facts: &mut dyn Facts) -> Finding {
    if !fresh(input) {
        return Finding::None;
    }
    // 닿지 않는 안내는 만들지 않는다. decide가 사용자에게 한 줄만 낸다
    if !facts.caps().context_at_start {
        return hit("", Vec::new());
    }
    let (Known::Yes(actor), Ok(config)) = (call::actor(facts), facts.config().cloned()) else {
        return Finding::Unknown;
    };
    let mut brief = Brief {
        head: vec![role_text(facts, &actor)],
        parallel_on: matches!(config.parallel, Parallel::On | Parallel::Ultra),
        pstack: facts.host() == HostId::Claude,
        ..Brief::default()
    };
    if matches!(actor.seat, Seat::Lead | Seat::Role { .. }) {
        brief.head.push(format!(
            "{}\n{}\n{}",
            hook::test_level(config.test_level),
            parallel_brief(config.parallel, Some(facts.host()), BriefAt::Session),
            hook::INBOX_RULE
        ));
        brief.head.push(hook::LINT_GATE.into());
        brief.head.extend(inbox_line(facts, &actor));
    }
    if actor.is_lead() {
        lead_items(facts, &mut brief);
    }
    let missing = call::with(|call| call.scene.missing_tools.clone());
    if !missing.is_empty() {
        brief.tail.push(format!(
            "PATH에 `{}`이 없다. 그 도구에 기대는 검사와 배정이 돌지 않는다.",
            missing.join("`, `")
        ));
    }
    for key in notice_keys(facts) {
        call::effect(Effect::RecordNotice(key.to_string()));
        brief.tail.push(hook::notice(key).to_string());
    }
    call::with(|call| call.brief = Some(brief));
    hit("", Vec::new())
}

/// S-02
pub(super) fn s02(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    if !matches!(facts.session(), Known::Unknown(NO_SESSION)) {
        return Finding::None;
    }
    call::ensure_session(facts);
    hit("", Vec::new())
}

/// S-03
pub(super) fn s03(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let open = |s: &SnapshotState| matches!(s, SnapshotState::Active | SnapshotState::Creating);
    let paths: Vec<String> = facts
        .state()
        .yes()
        .map(|state| {
            let open = state.snapshots.iter().filter(|s| open(&s.state));
            open.map(|s| s.path.clone()).collect()
        })
        .unwrap_or_default();
    // 경로가 사라진 것이 있을 때만 Git 등록을 확인하러 간다
    let lost = |path: &String| facts.path_exists(std::path::Path::new(path)) == Known::Yes(false);
    if paths.iter().any(lost) {
        call::effect(Effect::CloseLostSnapshots);
    }
    call::effect(Effect::GcSessions);
    hit("", Vec::new())
}

/// S-04
pub(super) fn s04(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    if !call::with(|call| call.scene.self_install) {
        return Finding::None;
    }
    call::effect(Effect::SelfInstall);
    hit("", Vec::new())
}

/// S-05
pub(super) fn s05(input: &HookInput, facts: &mut dyn Facts) -> Finding {
    if input.source.as_deref() != Some("compact") {
        return Finding::None;
    }
    let Known::Yes(actor) = call::actor(facts) else {
        return Finding::Unknown;
    };
    let mut args = vec![role_text(facts, &actor)];
    args.extend(inbox_line(facts, &actor));
    hit("", args)
}

/// S-06
pub(super) fn s06(input: &HookInput, facts: &mut dyn Facts) -> Finding {
    if !fresh(input) || !facts.caps().context_at_start {
        return Finding::None;
    }
    match facts.user_skill("caveman") {
        Known::Yes(Some(skill)) if !skill.body.trim().is_empty() => {
            call::with(|call| call.skill = Some(skill));
            hit("", Vec::new())
        }
        _ => Finding::None,
    }
}
