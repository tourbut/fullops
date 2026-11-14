//! Stop 규칙의 `check`(설계 7.5절). X-00부터 X-06까지 모두 평가하고 X-07은 그 가운데 걸린 것이 없을 때만 평가한다.

use super::decide::HANDOVERS;
use super::{Effect, Finding, call, hit, hit_case, known};
use crate::derive::ready;
use crate::facts::Facts;
use crate::model::{
    AckBy, Actor, DispatchId, HookInput, Known, Landing, MessageId, Plan, ReviewEvidence, RoleId,
    RunId, RunOwner, Sha, TaskKey, WorkOutcome, ack_by, orca_wakes,
};
use std::collections::BTreeSet;

/// 하위 작업 남음(설계 6.6절). dispatch가 묶이지 않았거나 `state.json`을 읽지 못하면 거짓이다.
fn sub_open(facts: &mut dyn Facts, actor: &Actor) -> bool {
    let Some(id) = &actor.dispatch else {
        return false;
    };
    facts.state().yes().is_some_and(|s| s.sub_open(id))
}

/// 준비된 행의 키와 그 계획. lead 체크아웃의 인박스 파일을 본다. S-01과 X-07이 함께 쓴다.
pub(super) fn ready_rows(facts: &mut dyn Facts) -> Known<(Vec<TaskKey>, Plan)> {
    macro_rules! get {
        ($e:expr) => {
            match $e {
                Known::Yes(v) => v.clone(),
                Known::Unknown(why) => return Known::Unknown(why),
            }
        };
    }
    let plan = get!(facts.plan());
    let state = get!(facts.state());
    let archive = get!(facts.archive_keys());
    let dir = call::root().join(HANDOVERS);
    let roles: BTreeSet<RoleId> = plan.rows.iter().filter_map(|r| r.role.clone()).collect();
    let busy: BTreeSet<RoleId> = roles
        .into_iter()
        .filter(|role| facts.path_exists(&dir.join(format!("to_{role}.md"))) == Known::Yes(true))
        .collect();
    let keys = ready(&plan, &state, &archive, &|role| busy.contains(role));
    Known::Yes((keys, plan))
}

/// X-00
pub(super) fn x00(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    match facts.policy_error() {
        Some(e) => hit(e.kind.as_str(), vec![e.detail]),
        None => Finding::None,
    }
}

/// X-01. 하위 작업 남음이면 걸리지 않는다: 하위 worker가 아직 이 워크트리에 쓰고 있다.
pub(super) fn x01(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let actor = known!(call::actor(facts));
    if sub_open(facts, &actor) {
        return Finding::None;
    }
    let changed = known!(facts.changed());
    if changed.code.is_empty() {
        return Finding::None;
    }
    let head = known!(facts.head());
    if changed.dirty.is_empty() && known!(facts.pass_covers(&head, &changed.start)) {
        return Finding::None;
    }
    call::with(|call| call.x01_hit = true);
    let dirty: Vec<String> = changed
        .dirty
        .iter()
        .map(|(path, at)| format!("{path}@{at}"))
        .collect();
    let mut files = changed
        .code
        .iter()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if changed.code.len() > 5 {
        files.push_str(&format!(" 외 {}개", changed.code.len() - 5));
    }
    if !changed.dirty.is_empty() {
        files.push_str(&format!(" (커밋하지 않은 파일 {}개)", changed.dirty.len()));
    }
    Finding::Hit {
        fp: format!("{head} {}", dirty.join(" ")),
        args: vec![changed.start.short(7).to_string(), files],
        case: "",
        // dev 직접 작업은 계속 막는다
        always: actor.direct_dev,
    }
}

/// X-02
pub(super) fn x02(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    if call::with(|call| call.x01_hit) {
        return Finding::None;
    }
    let changed = known!(facts.changed());
    if changed.code.is_empty() {
        return Finding::None;
    }
    let head = known!(facts.head());
    let need = known!(facts.review_need(&changed.start, &head));
    if !need.required || known!(facts.review_evidence(&head)) != ReviewEvidence::None {
        return Finding::None;
    }
    hit(head.as_str(), vec![head.short(7).to_string(), need.reason])
}

/// X-03. 하위 작업 남음이면 Orca 조회 없이 걸리지 않는다: 탈출구인 `fullops done`을 R-11이 거절한다.
pub(super) fn x03(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let actor = known!(call::actor(facts));
    let Some(id) = actor.dispatch else {
        return Finding::None;
    };
    // state.json의 기록: 없으면(또는 읽지 못하면) Orca에 묻는다
    let recorded = match facts.state() {
        Known::Yes(state) if state.sub_open(&id) => return Finding::None,
        Known::Yes(state) => state.dispatch(&id).map(|d| d.settled.is_some()),
        Known::Unknown(_) => None,
    };
    if recorded == Some(true) {
        return Finding::None;
    }
    if known!(facts.orca_settled(&id)) {
        if recorded.is_some() {
            call::effect(Effect::MarkSettled(id));
        }
        return Finding::None;
    }
    hit(id.as_str(), vec![id.to_string()])
}

/// X-04
pub(super) fn x04(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let routed = match facts.session() {
        Known::Yes(session) if !session.routed.is_empty() => session.routed.clone(),
        _ => return Finding::None,
    };
    let mut dispatched: BTreeSet<TaskKey> = known!(facts.state())
        .dispatches
        .iter()
        .filter(|d| !d.is_reservation())
        .map(|d| d.key.clone())
        .collect();
    // 병합으로 닫힌 과제는 배정 기록이 보관으로 옮겨진다. 인박스 없는 과제(설계·질문 전달)는 logs도 없어 이것만 남는다.
    // 다만 병합 뒤에 다시 분류(`route --force`)만 하고 배정하지 않은 키는 면제하지 않는다
    let landed = known!(facts.archive_landed()).clone();
    for key in &routed {
        if let Some(at) = landed.get(key)
            && facts
                .route(key)
                .yes()
                .flatten()
                .is_none_or(|route| route.created_at <= *at)
        {
            dispatched.insert(key.clone());
        }
    }
    let planned: BTreeSet<TaskKey> = known!(facts.plan())
        .rows
        .iter()
        .map(|r| r.key.clone())
        .collect();
    let roles: Vec<RoleId> = match facts.config() {
        Ok(config) => config.roles.keys().cloned().collect(),
        Err(_) => return Finding::Unknown,
    };
    let inboxes: BTreeSet<TaskKey> = roles
        .iter()
        .filter_map(|role| facts.inbox(role).yes().flatten())
        .map(|inbox| inbox.key)
        .collect();
    let logs = call::root().join(HANDOVERS).join("logs");
    // shortcut: Facts에 폴더 훑기가 없어 `logs/<KEY>.<역할>.r0~r9.md`만 본다, 차수가 10을 넘는 과제가 생기면 Facts에 목록 조회를 더한다
    let mut logged = |key: &TaskKey| {
        roles.iter().any(|role| {
            (0..10).any(|round| {
                facts.path_exists(&logs.join(format!("{key}.{role}.r{round}.md")))
                    == Known::Yes(true)
            })
        })
    };
    let left: Vec<String> = routed
        .iter()
        .filter(|key| {
            !dispatched.contains(key)
                && !inboxes.contains(key)
                && !planned.contains(key)
                && !logged(key)
        })
        .map(|key| key.to_string())
        .collect();
    if left.is_empty() {
        return Finding::None;
    }
    hit(left.join(","), vec![left.join(", ")])
}

/// X-05가 보는 Run 하나.
struct RunView {
    id: RunId,
    coordinator: Option<String>,
    /// `fullops merge`가 처리 표시까지 하는 완료 보고: `KEY(메시지 ID)`와 그 SHA.
    merge: Vec<(String, Option<Sha>)>,
    /// 처리한 뒤 `wait --ack`로 표시할 메시지 ID.
    ack: Vec<MessageId>,
    unsettled: Vec<DispatchId>,
}

/// X-05
pub(super) fn x05(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let actor = known!(call::actor(facts));
    let owner = match &actor.dispatch {
        Some(id) => RunOwner::Dispatch(id.clone()),
        None => RunOwner::Lead,
    };
    let state = known!(facts.state());
    let mut runs: Vec<RunView> = state
        .runs
        .iter()
        .filter(|r| r.closed_at.is_none() && state.owns(r, &owner))
        .map(|run| {
            let (mut merge, mut ack) = (Vec::new(), Vec::new());
            for m in state.messages.iter().filter(|m| m.run == run.id) {
                if m.acked_at.is_some() {
                    continue;
                }
                if ack_by(state, &m.id) == AckBy::Merge {
                    let done = state.completions.iter().find(|c| c.id == m.id);
                    let key = done.and_then(|c| c.key.as_ref());
                    let key = key.map_or("키 없음".to_string(), |k| k.to_string());
                    merge.push((format!("{key}({})", m.id), done.and_then(|c| c.sha.clone())));
                } else {
                    ack.push(m.id.clone());
                }
            }
            let unsettled = state
                .dispatches
                .iter()
                .filter(|d| d.run == run.id && d.settled.is_none() && !d.is_reservation());
            RunView {
                id: run.id.clone(),
                coordinator: run.coordinator.clone(),
                merge,
                ack,
                unsettled: unsettled.map(|d| d.id.clone()).collect(),
            }
        })
        .collect();
    // 이미 통합된 보고(날것 git merge·push)는 미처리로 세지 않는다: 같은 호출의 X-06 효과가 처리 표시를 한다
    if owner == RunOwner::Lead {
        for run in &mut runs {
            run.merge.retain(|(_, sha)| {
                !sha.as_ref()
                    .is_some_and(|sha| facts.landing(sha) == Known::Yes(Landing::Landed))
            });
        }
    }
    let terminal = match facts.session() {
        Known::Yes(session) => session.terminal.clone(),
        Known::Unknown(_) => facts.session_handle(),
    };
    let background = facts.caps().background_wake;
    let (mut fp, mut args, mut close) = (Vec::new(), Vec::new(), Vec::new());
    let mut case = "";
    for run in runs {
        if !(run.merge.is_empty() && run.ack.is_empty()) {
            // (a) 미처리 메시지
            case = "a";
            let ack: Vec<&str> = run.ack.iter().map(|id| id.as_str()).collect();
            let merge: Vec<&str> = run.merge.iter().map(|(text, _)| text.as_str()).collect();
            fp.push(format!(
                "{}:a:{}:{}",
                run.id,
                merge.join(","),
                ack.join(",")
            ));
            let mut text = format!("Run {}에 처리하지 않은 메시지가 있다.", run.id);
            if !merge.is_empty() {
                text.push_str(&format!(
                    " `fullops merge <KEY>`로 처리할 완료 보고: {}.",
                    merge.join(", ")
                ));
            }
            if !ack.is_empty() {
                text.push_str(&format!(
                    " 처리한 뒤 `fullops wait --run {} --ack {} --no-wait`로 표시한다.",
                    run.id,
                    ack.join(" --ack ")
                ));
            }
            args.push(text);
        } else if run.unsettled.is_empty() {
            close.push(run.id);
        } else {
            // (b) 실행 중인 worker가 있는데 깨움 보장이 없다
            let woken = orca_wakes(terminal.as_deref(), run.coordinator.as_deref())
                || (background
                    && match facts.waiter_alive(&run.id) {
                        Known::Yes(alive) => alive,
                        // 대기 표식을 읽지 못하면 막지 않는다
                        Known::Unknown(_) => true,
                    });
            if woken {
                continue;
            }
            if case.is_empty() {
                case = "b";
            }
            let ids: Vec<&str> = run.unsettled.iter().map(|id| id.as_str()).collect();
            fp.push(format!("{}:b:{}", run.id, ids.join(",")));
            args.push(format!(
                "Run {}의 worker가 실행 중인데 이 세션을 깨울 수단이 없다(`fullops wait --run {}`).",
                run.id, run.id
            ));
        }
    }
    if !close.is_empty() {
        call::effect(Effect::CloseRuns(close));
    }
    if fp.is_empty() {
        return Finding::None;
    }
    hit_case(fp.join(" "), args, case)
}

/// X-06
pub(super) fn x06(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let open: Vec<(MessageId, Option<TaskKey>, Option<Sha>, bool)> = known!(facts.state())
        .completions
        .iter()
        .filter(|c| !c.direct && c.outcome == WorkOutcome::Succeeded && c.landed_at.is_none())
        .map(|c| (c.id.clone(), c.key.clone(), c.sha.clone(), c.hold.is_some()))
        .collect();
    let (mut landed, mut left) = (Vec::new(), Vec::new());
    for (id, key, sha, held) in open {
        // 키가 없는 건은 `merge <KEY>`·`hold <KEY>`로 가리킬 수 없다: 탈출구가 쓰는 메시지 ID를 적는다
        let label = |what: String| match &key {
            Some(key) => format!("{key}({what})"),
            None => format!("키 없음(메시지 {id}, {what})"),
        };
        let Some(sha) = sha else {
            if !held {
                left.push(label("SHA 없음".into()));
            }
            continue;
        };
        // 통합 위치를 모르는 건은 대기로 세지 않는다
        let what = match facts.landing(&sha) {
            Known::Yes(Landing::Landed) => {
                landed.push(id.clone());
                continue;
            }
            Known::Yes(Landing::MergeLeft) => "병합 남음",
            Known::Yes(Landing::PushLeft) => "push 남음",
            Known::Unknown(_) => continue,
        };
        if !held {
            left.push(label(format!("{}, {what}", sha.short(7))));
        }
    }
    if !landed.is_empty() {
        call::effect(Effect::MarkLanded(landed));
    }
    if left.is_empty() {
        return Finding::None;
    }
    hit(left.join(","), vec![left.join(", ")])
}

/// X-07
pub(super) fn x07(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let (keys, plan) = known!(ready_rows(facts));
    let mut keys: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
    // 표에 없어도 이어 갈 일: 배정되지 않은 지시서(설계 완료 뒤의 구현 지시서)와 실패한 채 남은 과제
    let actor = known!(call::actor(facts));
    let state = facts.state().yes().cloned();
    if let (Ok(config), Some(state)) = (facts.config().cloned(), state) {
        let own = |id: &RoleId| actor.direct_dev && config.primary_role.as_ref() == Some(id);
        let mut inboxes = Vec::new();
        for (id, role) in config.roles.iter().filter(|(id, _)| !own(id)) {
            let Some(inbox) = facts.inbox(id).yes().flatten() else {
                continue;
            };
            // Facts는 역할 브랜치를 읽지 않는다: 그 키·역할을 맡았던 워크트리의 완료 기록을 본다
            let log = format!("logs/{}.{}.r{}.md", inbox.key, inbox.role, inbox.round);
            let worktree = state
                .dispatches
                .iter()
                .rev()
                .find(|d| d.key == inbox.key && d.role == inbox.role && d.counts());
            let finished = worktree.is_some_and(|d| {
                let path = std::path::Path::new(&d.worktree).join(HANDOVERS).join(&log);
                facts.path_exists(&path) == Known::Yes(true)
            });
            inboxes.push((inbox, role.kind, finished));
        }
        keys.extend(crate::flow::leftover(&state, &plan, &inboxes));
    }
    if keys.is_empty() {
        return Finding::None;
    }
    hit(keys.join(","), vec![keys.join(", ")])
}
