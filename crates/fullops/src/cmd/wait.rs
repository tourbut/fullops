//! `fullops wait`: 메시지 받기, 완료 보고 기록, 처리 표시(`--ack`), Orca가 깨울 수 없는 세션의 기다림(설계 8.6절).
//! 미리 보기(`--peek`)로 받은 것을 `state.json`에 기록한 다음에만 소비형 `check`를 부른다.

use super::dispatch::{step_refusal, usage, value};
use crate::cli::{CliError, Ctx, EXIT_OK, session_handle};
use fullops_core::flow::{self, body_shas};
use fullops_core::model::{
    AckBy, Extra, MessageId, OrcaMessage, RoleId, Run, RunId, RunOwner, Settled, SettledHow, Sha,
    State, TaskKey, Waiter, ack_by, orca_wakes,
};
use fullops_ext::{CheckMode, ExtError, Git, GitCli, Orca, OrcaCli};
use lexopt::prelude::*;
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// 기다림을 깨우는 종류. `heartbeat`·`status`는 깨우는 조건에 없다.
const WAKE_TYPES: &[&str] = &[
    "worker_done",
    "question",
    "escalation",
    "dispatch",
    "merge_ready",
    "handoff",
    "decision_gate",
];
/// 한 번의 기다림 호출 상한(미정-M3).
const WAIT_SLICE: Duration = Duration::from_secs(600);

/// Orca 호출의 실패 처리(설계 8.6절 10단계): `run-use` 뒤 한 번 다시, 그래도 실패하면 30초 뒤 다시, 세 번이면 종료 코드 4.
fn retry<T>(
    orca: &OrcaCli,
    run: &RunId,
    mut call: impl FnMut() -> Result<T, ExtError>,
) -> Result<T, CliError> {
    let mut failures = 0;
    loop {
        let result = match call() {
            Err(first) if first != ExtError::NotFound => {
                let _ = orca.run_use(run);
                call()
            }
            first => first,
        };
        match result {
            Ok(value) => return Ok(value),
            Err(ExtError::NotFound) => {
                return Err(CliError::External("Orca 실행 파일을 찾지 못했다".into()));
            }
            Err(e) => {
                failures += 1;
                if failures >= 3 {
                    return Err(CliError::External(format!(
                        "Orca 호출이 세 번 연속 실패했다: {e}"
                    )));
                }
                std::thread::sleep(Duration::from_secs(30));
            }
        }
    }
}

/// 대기 표식 `waiters/<run>.json`. 30초마다 `heartbeat`를 갱신하고 끝날 때 지운다.
struct WaiterMark {
    path: std::path::PathBuf,
    stop: Arc<AtomicBool>,
}

impl WaiterMark {
    fn start(path: std::path::PathBuf, sid: Option<fullops_core::model::SessionId>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let write = {
            let (path, sid) = (path.clone(), sid);
            move || {
                let waiter = Waiter {
                    pid: std::process::id(),
                    sid: sid.clone(),
                    heartbeat: fullops_store::now(),
                };
                let _ = fullops_store::write_json(&path, &waiter);
            }
        };
        write();
        let flag = stop.clone();
        std::thread::spawn(move || {
            let mut ticks = 0;
            while !flag.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_secs(1));
                ticks += 1;
                if ticks % 30 == 0 && !flag.load(Ordering::Relaxed) {
                    write();
                }
            }
        });
        Self { path, stop }
    }
}

impl Drop for WaiterMark {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = std::fs::remove_file(&self.path);
    }
}

/// 이번 호출에서 받은 메시지의 본문과 알림. 저장하지 않고 출력에만 쓴다.
#[derive(Default)]
struct Received {
    seen: BTreeMap<MessageId, (OrcaMessage, Vec<String>)>,
    heartbeats: BTreeSet<MessageId>,
    status: BTreeSet<MessageId>,
}

/// 메시지 묶음을 `state.json`에 기록한다. 짧은 SHA와 역할 브랜치 tip은 잠금 밖에서 미리 푼다.
fn record(
    ctx: &Ctx,
    git: &GitCli,
    run: &RunId,
    messages: &[OrcaMessage],
    received: &mut Received,
) -> Result<(), CliError> {
    let config = ctx.config()?;
    let mut resolved: BTreeMap<String, Option<Sha>> = BTreeMap::new();
    for msg in messages.iter().filter(|m| m.kind == "worker_done") {
        for hex in body_shas(msg.body.as_deref().unwrap_or_default()) {
            let sha = git.rev_parse(&format!("{hex}^{{commit}}")).yes();
            resolved.insert(hex, sha);
        }
    }
    let tips: BTreeMap<&RoleId, Option<Sha>> = config
        .roles
        .iter()
        .map(|(id, role)| {
            (
                id,
                git.rev_parse(&format!("refs/heads/{}", role.branch)).yes(),
            )
        })
        .collect();
    let notes = ctx.update_state(None, |s| {
        messages
            .iter()
            .map(|msg| {
                flow::record_message(
                    s,
                    run,
                    msg,
                    &mut |hex| resolved.get(hex).cloned().flatten(),
                    &mut |role| tips.get(role).cloned().flatten(),
                )
            })
            .collect::<Vec<_>>()
    })?;
    for (msg, note) in messages.iter().zip(notes) {
        match (note.absorbed, msg.kind.as_str()) {
            (true, "heartbeat") => drop(received.heartbeats.insert(msg.id.clone())),
            (true, _) => drop(received.status.insert(msg.id.clone())),
            (false, _) => {
                let entry = received
                    .seen
                    .entry(msg.id.clone())
                    .or_insert_with(|| (msg.clone(), Vec::new()));
                entry.1.extend(note.notes);
            }
        }
    }
    Ok(())
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let (mut run, mut acks, mut max_minutes, mut no_wait) = (None, Vec::new(), None, false);
    while let Some(arg) = args.next()? {
        match arg {
            Long("run") => run = Some(value(&mut args)?),
            Long("ack") => acks.push(value(&mut args)?),
            Long("max-minutes") => {
                let text = value(&mut args)?;
                max_minutes = Some(
                    text.parse::<u64>()
                        .map_err(|_| usage("`--max-minutes <N>`은 0 이상의 정수다."))?,
                );
            }
            Long("no-wait") => no_wait = true,
            other => return Err(other.unexpected().into()),
        }
    }
    let (repo, config) = (ctx.repo()?, ctx.config()?);
    let caller = ctx.caller()?;
    let state = fullops_store::read_state(repo).yes().unwrap_or_default();
    let owner = match &caller.actor.dispatch {
        Some(id) => RunOwner::Dispatch(id.clone()),
        None => RunOwner::Lead,
    };

    // 1. Run 정하기
    let run = match run {
        Some(run) => RunId::new(run).map_err(|e| usage(e.to_string()))?,
        None => {
            let own: Vec<&Run> = state
                .runs
                .iter()
                .filter(|r| r.closed_at.is_none() && r.owner == owner)
                .collect();
            match own.as_slice() {
                [only] => only.id.clone(),
                [] => {
                    return Err(usage(
                        "이 세션이 소유한 열린 Run이 없다. `--run <ID>`를 준다.",
                    ));
                }
                many => {
                    let ids: Vec<&str> = many.iter().map(|r| r.id.as_str()).collect();
                    return Err(step_refusal(
                        "wait",
                        format!("열린 Run이 여럿이다: {}.", ids.join(", ")),
                        "`--run <ID>`로 고른다",
                    ));
                }
            }
        }
    };
    let orca = OrcaCli::new(repo.root.clone(), Duration::from_secs(15));
    let git = GitCli::cli(repo.root.clone());
    let handle = session_handle();

    // 2. 다시 묶기와 깨움 판정
    let mut coordinator = state
        .runs
        .iter()
        .find(|r| r.id == run)
        .and_then(|r| r.coordinator.clone());
    let known = state.runs.iter().any(|r| r.id == run);
    let rebound = match &handle {
        Some(handle) if coordinator.as_ref() != Some(handle) => match orca.run_use(&run) {
            Ok(()) => {
                coordinator = Some(handle.clone());
                true
            }
            Err(e) => {
                ctx.note(&format!(
                    "Run `{run}`을 이 세션에 다시 묶지 못했다({e}). 이 세션에서 기다린다."
                ));
                false
            }
        },
        _ => false,
    };
    let wakes = orca_wakes(handle.as_deref(), coordinator.as_deref());

    // 3. 처리 표시. Orca는 부르지 않는다
    if rebound || !known || !acks.is_empty() {
        let unknown = ctx.update_state(None, |s| {
            match s.runs.iter_mut().find(|r| r.id == run) {
                Some(found) if rebound => found.coordinator = coordinator.clone(),
                Some(_) => {}
                None => s.runs.push(Run {
                    id: run.clone(),
                    owner: owner.clone(),
                    coordinator: coordinator.clone(),
                    opened_at: fullops_store::now(),
                    closed_at: None,
                    extra: Extra::new(),
                }),
            }
            let mut unknown = Vec::new();
            for id in &acks {
                match s.messages.iter_mut().find(|m| m.id.as_str() == id) {
                    Some(m) => m.acked_at = m.acked_at.or(Some(fullops_store::now())),
                    None => unknown.push(id.clone()),
                }
            }
            unknown
        })?;
        for id in unknown {
            ctx.note(&format!("모르는 메시지 ID다: {id}"));
        }
    }

    // 4. 대기 표식
    let waiting = !no_wait && !wakes;
    let _mark = waiting.then(|| WaiterMark::start(repo.waiter_path(&run), caller.sid.clone()));
    let minutes = max_minutes.unwrap_or(config.wait.check_minutes);
    let (started, limit) = (Instant::now(), Duration::from_secs(minutes * 60));
    let mut received = Received::default();
    let (mut idle_timeout, mut expired, mut lost) = (false, false, Vec::new());
    loop {
        // 5·6. 받기: 미리 보기를 기록한 뒤에 소비한다
        let peeked = retry(&orca, &run, || orca.check(Some(&run), CheckMode::Peek))?;
        record(ctx, &git, &run, &peeked.messages, &mut received)?;
        if !peeked.messages.is_empty() {
            for _ in 0..10 {
                let got = retry(&orca, &run, || orca.check(Some(&run), CheckMode::Consume))?;
                if got.messages.is_empty() {
                    break;
                }
                record(ctx, &git, &run, &got.messages, &mut received)?;
                // 읽음 표시를 스스로 하지 않는 옛 Orca
                if let Some(delivery) = got.delivery_id {
                    let ids: Vec<&MessageId> = got.messages.iter().map(|m| &m.id).collect();
                    ctx.update_state(None, |s| {
                        for m in s.messages.iter_mut().filter(|m| ids.contains(&&m.id)) {
                            m.delivery = Some(delivery.clone());
                        }
                    })?;
                    let _ = orca.check(Some(&run), CheckMode::AckLegacy(delivery));
                }
            }
        }
        // 7. 반환 판단
        let now = fullops_store::read_state(repo).yes().unwrap_or_default();
        if !waiting
            || now
                .messages
                .iter()
                .any(|m| m.run == run && m.acked_at.is_none())
        {
            break;
        }
        // 9. 생존 확인
        if expired {
            let views = retry(&orca, &run, || orca.worker_list(&run))?;
            let dead: Vec<_> = views
                .iter()
                .filter(|v| v.liveness == "exited")
                .map(|v| v.dispatch.clone())
                .collect();
            lost = ctx.update_state(None, |s| {
                let mut lost = Vec::new();
                for d in s
                    .dispatches
                    .iter_mut()
                    .filter(|d| d.run == run && d.settled.is_none() && dead.contains(&d.id))
                {
                    d.settled = Some(Settled {
                        how: SettledHow::Lost,
                        outcome: None,
                        message: None,
                        at: fullops_store::now(),
                        extra: Extra::new(),
                    });
                    lost.push(json!({"dispatch": d.id, "key": d.key, "role": d.role}));
                }
                lost
            })?;
            idle_timeout = true;
            break;
        }
        // 8. 기다리기. keepalive 줄은 stderr라 읽고 버려진다
        let left = limit.saturating_sub(started.elapsed());
        let slice = left.clamp(Duration::from_secs(1), WAIT_SLICE);
        retry(&orca, &run, || {
            orca.check(
                Some(&run),
                CheckMode::PeekWait {
                    types: WAKE_TYPES,
                    timeout: slice,
                },
            )
        })?;
        expired = started.elapsed() >= limit;
    }

    let state = fullops_store::read_state(repo).yes().unwrap_or_default();
    let (actionable, next) = report(&state, &run, &received, wakes);
    let out = json!({
        "run": run, "idle_timeout": idle_timeout, "check_minutes": minutes, "actionable": actionable,
        "absorbed": {"heartbeats": received.heartbeats.len(), "status": received.status.len()},
        "lost": lost, "orca_wakes": wakes, "next": next,
    });
    let mut human: Vec<String> = actionable
        .iter()
        .map(|m| {
            let text = |name: &str| m[name].as_str().unwrap_or("-").to_string();
            let mut line = format!(
                "- {} {} | 과제 {} | 역할 {} | SHA {} | 처리: {}",
                text("id"),
                text("type"),
                text("key"),
                text("role"),
                text("sha"),
                text("ack_by")
            );
            if let Some(body) = m["body"].as_str().filter(|b| !b.is_empty()) {
                line.push_str(&format!("\n  {body}"));
            }
            for note in m["notes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Json::as_str)
            {
                line.push_str(&format!("\n  알림: {note}"));
            }
            line
        })
        .collect();
    if actionable.is_empty() {
        human.push("처리할 메시지가 없다.".into());
    }
    if idle_timeout {
        human.push(format!(
            "{minutes}분 동안 메시지가 없었다. 끝난 worker: {}건",
            lost.len()
        ));
    }
    human.push(format!("다음: {next}"));
    ctx.print(&human.join("\n"), out);
    Ok(EXIT_OK)
}

/// 그 Run의 처리 표시가 없는 메시지 전부와 다음 할 일(설계 8.6절의 반환).
fn report(state: &State, run: &RunId, received: &Received, wakes: bool) -> (Vec<Json>, String) {
    let (mut merges, mut ack_ids): (Vec<String>, Vec<&str>) = (Vec::new(), Vec::new());
    let mut actionable = Vec::new();
    for m in state
        .messages
        .iter()
        .filter(|m| &m.run == run && m.acked_at.is_none())
    {
        let dispatch = m.dispatch.as_ref().and_then(|id| state.dispatch(id));
        let completion = state.completions.iter().find(|c| c.id == m.id);
        let key: Option<&TaskKey> = completion
            .and_then(|c| c.key.as_ref())
            .or(dispatch.map(|d| &d.key));
        let by = ack_by(state, &m.id);
        match (by, key) {
            (AckBy::Merge, Some(key)) => {
                let step = format!("`fullops merge {key}`(처리 표시까지 한다)");
                if !merges.contains(&step) {
                    merges.push(step);
                }
            }
            (AckBy::Merge, None) => merges.push(format!(
                "`fullops completion set-sha {} <SHA> --key <KEY>` 뒤 `fullops merge <KEY>`",
                m.id
            )),
            (AckBy::Ack, _) => ack_ids.push(m.id.as_str()),
        }
        let seen = received.seen.get(&m.id);
        let outcome = completion.map(|c| c.outcome).or(dispatch
            .and_then(|d| d.settled.as_ref())
            .and_then(|s| s.outcome));
        actionable.push(json!({
            "id": m.id, "delivery": m.delivery, "type": m.kind, "ack_by": by,
            "from": seen.and_then(|(msg, _)| msg.from.clone()),
            "subject": seen.and_then(|(msg, _)| msg.subject.clone()),
            "key": key,
            "role": completion.and_then(|c| c.role.as_ref()).or(dispatch.map(|d| &d.role)),
            "sha": completion.and_then(|c| c.sha.as_ref()),
            "sha_source": completion.and_then(|c| c.sha_source),
            "outcome": outcome,
            "body": seen.and_then(|(msg, _)| msg.body.clone()),
            "notes": seen.map(|(_, notes)| notes.clone()).unwrap_or_default(),
        }));
    }
    let mut next = merges;
    if !ack_ids.is_empty() {
        let flags: Vec<String> = ack_ids.iter().map(|id| format!("--ack {id}")).collect();
        next.push(format!(
            "그 밖의 메시지는 처리한 뒤 `fullops wait --run {run} {} --no-wait`",
            flags.join(" ")
        ));
    }
    next.push(if wakes {
        "기다릴 것이 남았으면 턴을 끝낸다. Orca가 깨운다".to_string()
    } else {
        format!("`fullops wait --run {run}`로 이어 기다린다")
    });
    (actionable, next.join(". "))
}
