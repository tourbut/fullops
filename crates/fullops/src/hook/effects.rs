//! 효과 실행(설계 7.1절 7·8번). 세션 파일의 기록은 판정을 내기 전에, 나머지는 판정을 낸 뒤에 한다.
//! 실패는 추적에만 남긴다. 바깥 호출(Orca, git)은 잠금 밖에서 하고 `state.json`은 한 번에 쓴다.

use super::facts_real::RealFacts;
use fullops_core::Effect;
use fullops_core::facts::Facts;
use fullops_core::model::{
    BlockMark, BoundBy, Current, DispatchId, Extra, Known, MessageId, OrcaMessage, RunId, RunOwner,
    SESSION_V, Session, SessionDispatch, Settled, SettledHow, Sha, SnapshotState, State, Turn,
    WorkOutcome, same_path,
};
use fullops_ext::{CheckMode, Git, Orca};
use fullops_store::{HOOK_WAIT, StoreError};
use std::time::{Duration, Instant};

const DAY: i64 = 86_400;
/// 세션 파일·추적·실행 표식을 지우는 기준(설계 6.8절).
const SESSION_TTL: i64 = 14 * DAY;
const LIMIT_SUBJECT: &str = "[fullops:limit] worker 턴이 API 오류로 끝남";

fn kind(error: &StoreError) -> &'static str {
    match error {
        StoreError::LockBusy => "lock_busy",
        StoreError::Missing => "no_session",
        StoreError::Corrupt(why) => *why,
        StoreError::Io(_) => "io",
    }
}

fn is_session_effect(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::CreateSession
            | Effect::TouchSession
            | Effect::BumpTurn
            | Effect::BindDispatch { .. }
            | Effect::RecordBlock { .. }
            | Effect::RecordNotice(_)
            | Effect::AppendRouted(_)
            | Effect::SetPromptFp(_)
    )
}

/// 세션 파일에 쓰는 효과를 한 번에 적용한다. 기록하지 못한 한 번 막기의 규칙 ID를 돌려준다(그 차단은 경고가 된다).
pub fn before_output(
    facts: &mut RealFacts,
    effects: &[Effect],
    errors: &mut Vec<&'static str>,
) -> Vec<&'static str> {
    let bind = facts.bind.clone();
    if bind.is_none() && !effects.iter().any(is_session_effect) {
        return Vec::new();
    }
    let blocks = || {
        effects
            .iter()
            .filter_map(|e| match e {
                Effect::RecordBlock { rule, .. } => Some(*rule),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let now = facts.now();
    if facts.session_missing() {
        // S-02와 같은 내용으로 만든다. 설정을 읽지 못하면 mode_at_start를 쓸 수 없어 만들지 않는다
        let Ok(mode_at_start) = facts.config().map(|c| c.mode_tuple()) else {
            errors.push("no_session");
            return blocks();
        };
        let session = Session {
            v: SESSION_V,
            sid: facts.sid.clone(),
            host: facts.host(),
            started_at: now,
            last_seen: now,
            terminal: facts.session_handle(),
            mode_at_start,
            anchor: facts.head().yes(),
            turn: 0,
            prompt_fp: None,
            dispatch: None,
            routed: Vec::new(),
            blocks: Default::default(),
            noticed: Vec::new(),
            extra: Extra::new(),
        };
        if let Err(e) = fullops_store::create_session(&facts.repo, &session, HOOK_WAIT) {
            errors.push(kind(&e));
            return blocks();
        }
    }
    let changes = bind.is_some()
        || effects
            .iter()
            .any(|e| is_session_effect(e) && *e != Effect::CreateSession);
    if !changes {
        return Vec::new();
    }
    let dispatch = |id: &DispatchId, task: &Option<String>, by| SessionDispatch {
        id: id.clone(),
        task: task.clone(),
        bound_by: by,
        extra: Extra::new(),
    };
    let written = fullops_store::update_session(&facts.repo, &facts.sid, HOOK_WAIT, |s| {
        if let (Some((id, task)), None) = (&bind, &s.dispatch) {
            s.bind(dispatch(id, task, BoundBy::Terminal));
        }
        for effect in effects {
            match effect {
                Effect::TouchSession => s.last_seen = now,
                Effect::BumpTurn => s.turn += 1,
                Effect::BindDispatch { id, task, by } => s.bind(dispatch(id, task, *by)),
                Effect::RecordBlock { rule, fp } => {
                    let mark = BlockMark {
                        fp: fp.clone(),
                        turn: s.turn,
                        extra: Extra::new(),
                    };
                    s.blocks.insert(rule.to_string(), mark);
                }
                Effect::RecordNotice(key) => {
                    s.notice(key);
                }
                Effect::AppendRouted(key) => s.add_routed(key),
                Effect::SetPromptFp(fp) => s.prompt_fp = Some(fp.clone()),
                _ => {}
            }
        }
    });
    match written {
        Ok(()) => Vec::new(),
        Err(e) => {
            errors.push(kind(&e));
            blocks()
        }
    }
}

/// `state.json`에 쓸 것. 바깥 호출을 끝낸 뒤 잠금 안에서 한 번에 적용한다.
enum Write {
    Bound(DispatchId),
    Landed(Vec<MessageId>),
    Settled(DispatchId, WorkOutcome),
    CloseRuns(Vec<RunId>),
    /// Git에 등록된 워크트리의 경로. 여기에도 없고 경로도 사라진 snapshot을 닫는다.
    LostSnapshots(Vec<String>),
    Escalated(DispatchId),
    Peeked(Option<RunId>, Vec<OrcaMessage>),
}

fn apply(facts: &RealFacts, state: &mut State, write: &Write, now: i64) {
    match write {
        Write::Bound(id) => {
            if let Some(d) = state.dispatch_mut(id) {
                d.bound_session = Some(facts.sid.clone());
            }
        }
        Write::Landed(ids) => {
            for c in &mut state.completions {
                if ids.contains(&c.id) && c.landed_at.is_none() {
                    c.landed_at = Some(now);
                    if let Some(hold) = c.hold.take() {
                        c.released_hold = Some(hold);
                    }
                }
            }
            // 날것 git merge·push로 통합한 보고도 이것으로 처리 표시가 된다
            for m in &mut state.messages {
                if ids.contains(&m.id) && m.acked_at.is_none() {
                    m.acked_at = Some(now);
                }
            }
        }
        Write::Settled(id, outcome) => {
            if let Some(d) = state.dispatch_mut(id).filter(|d| d.settled.is_none()) {
                d.settled = Some(Settled {
                    how: SettledHow::Done,
                    outcome: Some(*outcome),
                    message: None,
                    at: now,
                    extra: Extra::new(),
                });
            }
        }
        Write::CloseRuns(ids) => {
            for run in &mut state.runs {
                if ids.contains(&run.id) && run.closed_at.is_none() {
                    run.closed_at = Some(now);
                }
            }
        }
        Write::LostSnapshots(registered) => {
            for snapshot in &mut state.snapshots {
                let open = matches!(
                    snapshot.state,
                    SnapshotState::Active | SnapshotState::Creating
                );
                let gone = !std::path::Path::new(&snapshot.path).exists()
                    && !registered.iter().any(|p| same_path(p, &snapshot.path));
                if open && gone {
                    snapshot.state = SnapshotState::Missing;
                }
            }
        }
        Write::Escalated(id) => {
            if let Some(d) = state.dispatch_mut(id) {
                d.limit_escalated = true;
            }
        }
        Write::Peeked(run, messages) => {
            // 메시지에는 Run이 없다. 명령이 준 --run, 없으면 lead의 열린 Run이 하나일 때 그것이다
            let run = run.clone().or_else(|| {
                let mut open = state
                    .runs
                    .iter()
                    .filter(|r| r.closed_at.is_none() && r.owner == RunOwner::Lead);
                match (open.next(), open.next()) {
                    (Some(only), None) => Some(only.id.clone()),
                    _ => None,
                }
            });
            let Some(run) = run else {
                return;
            };
            let git = fullops_ext::GitCli::hook(facts.repo.root.clone());
            let mut resolve =
                |hex: &str| -> Option<Sha> { git.rev_parse(&format!("{hex}^{{commit}}")).yes() };
            let branches: Vec<(String, String)> = facts
                .branches()
                .into_iter()
                .map(|(role, branch)| (role, format!("refs/heads/{branch}")))
                .collect();
            let mut tip = |role: &fullops_core::model::RoleId| -> Option<Sha> {
                let (_, name) = branches.iter().find(|(id, _)| id == role.as_str())?;
                git.rev_parse(name).yes()
            };
            for message in messages {
                fullops_core::record_message(state, &run, message, &mut resolve, &mut tip);
            }
        }
    }
}

/// `last_seen`이 14일 지난 세션의 파일을 지운다. 50ms 예산을 넘으면 그만둔다.
fn gc_sessions(facts: &RealFacts, now: i64) {
    let started = Instant::now();
    let dir = facts.repo.local().join("sessions");
    for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        if started.elapsed() > Duration::from_millis(50) {
            break;
        }
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let Known::Yes(Some(session)) = fullops_store::read_json::<Session>(&path) else {
            continue;
        };
        if now - session.last_seen <= SESSION_TTL || session.sid == facts.sid {
            continue;
        }
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.lock"));
        let _ = std::fs::remove_dir_all(path.with_extension("seen"));
        let _ = std::fs::remove_file(facts.repo.trace_path(&session.sid));
    }
}

/// 판정을 낸 뒤의 효과. 남은 시간은 감시 스레드가 지킨다.
pub fn after_output(facts: &mut RealFacts, effects: &[Effect], errors: &mut Vec<&'static str>) {
    let now = facts.now();
    let in_state = |facts: &mut RealFacts, id: &DispatchId| {
        facts
            .state()
            .yes()
            .is_some_and(|state| state.dispatch(id).is_some())
    };
    let mut writes: Vec<Write> = Vec::new();
    if let Some((id, _)) = facts.bind.clone()
        && in_state(facts, &id)
    {
        writes.push(Write::Bound(id));
    }
    for effect in effects {
        match effect {
            Effect::BindDispatch { id, .. } => {
                if in_state(facts, id) {
                    writes.push(Write::Bound(id.clone()));
                }
            }
            // 지표 줄과 current.json은 실패해도 무시한다
            Effect::RecordTurn(kind) => {
                let turn = Turn {
                    at: now,
                    sid: facts.sid.clone(),
                    kind: *kind,
                };
                let _ = fullops_store::append_turn(&facts.repo, &turn);
            }
            Effect::StampCurrent => {
                let current = Current {
                    sid: facts.sid.clone(),
                    terminal: facts.session_handle(),
                    at: now,
                };
                let _ = fullops_store::write_current(&facts.repo, &current);
            }
            Effect::MarkLanded(ids) => writes.push(Write::Landed(ids.clone())),
            Effect::MarkSettled(id) => {
                let outcome = if facts.settled_failed {
                    WorkOutcome::Failed
                } else {
                    WorkOutcome::Succeeded
                };
                writes.push(Write::Settled(id.clone(), outcome));
            }
            Effect::CloseRuns(ids) => {
                for run in ids {
                    let _ = std::fs::remove_file(facts.repo.waiter_path(run));
                }
                writes.push(Write::CloseRuns(ids.clone()));
            }
            Effect::CloseLostSnapshots => match facts.git(|g| g.worktrees()) {
                Known::Yes(list) => writes.push(Write::LostSnapshots(
                    list.iter()
                        .map(|w| fullops_store::path_text(&w.path))
                        .collect(),
                )),
                Known::Unknown(why) => errors.push(why),
            },
            Effect::GcSessions => gc_sessions(facts, now),
            Effect::SelfInstall => {
                // 실행본 단계(`install --only runtime`과 같은 일)는 플러그인 루트의 실행본에 맡긴다
                if let Some(root) = facts.plugin_root.clone() {
                    let name = if cfg!(windows) {
                        "fullops.exe"
                    } else {
                        "fullops"
                    };
                    let exe = root.join("bin").join(name).to_string_lossy().into_owned();
                    let argv = [exe, "install".into(), "--only".into(), "runtime".into()];
                    let out = fullops_ext::run(&argv, &root, Duration::from_secs(3), &[], None);
                    if !out.ok() {
                        errors.push("self_install");
                    }
                }
            }
            Effect::PeekCompletions { run } => {
                let peeked = facts
                    .orca(Duration::from_millis(2500))
                    .map(|orca| orca.check(run.as_ref(), CheckMode::Peek));
                match peeked {
                    Some(Ok(result)) if !result.messages.is_empty() => {
                        writes.push(Write::Peeked(run.clone(), result.messages))
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => errors.push("orca"),
                    None => errors.push("orca_cap"),
                }
            }
            Effect::SendLimitEscalation {
                dispatch,
                task,
                error,
            } => {
                let body = format!(
                    "{error}. Task {}, Dispatch {dispatch}.",
                    task.as_deref().unwrap_or("-")
                );
                let sent = facts.orca(Duration::from_secs(6)).map(|orca| {
                    orca.send_escalation(dispatch, task.as_deref(), LIMIT_SUBJECT, &body)
                });
                match sent {
                    Some(Ok(())) if in_state(facts, dispatch) => {
                        writes.push(Write::Escalated(dispatch.clone()))
                    }
                    // state.json에 없는 dispatch는 세션 파일에 한 번 보냈다고 적는다
                    Some(Ok(())) => {
                        let key = format!("f01:{dispatch}");
                        let noted = fullops_store::update_session(
                            &facts.repo,
                            &facts.sid,
                            HOOK_WAIT,
                            |s| s.notice(&key),
                        );
                        if let Err(e) = noted {
                            errors.push(kind(&e));
                        }
                    }
                    Some(Err(_)) => errors.push("orca"),
                    None => errors.push("orca_cap"),
                }
            }
            _ => {}
        }
    }
    if writes.is_empty() {
        return;
    }
    let derived = facts.derived();
    let facts = &*facts;
    let written = fullops_store::update_state(&facts.repo, HOOK_WAIT, Some(&derived), |state| {
        for write in &writes {
            apply(facts, state, write, now);
        }
    });
    if let Err(e) = written {
        errors.push(kind(&e));
    }
}
