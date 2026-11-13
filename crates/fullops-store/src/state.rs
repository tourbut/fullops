//! `state.json` 읽기·쓰기와 `status.json` 재생성(설계 6.2·6.5절).

use crate::files::{
    archive_keys, load_config, now, read_archive, read_json, read_text, write_json,
};
use crate::lock::{StoreError, acquire};
use crate::repo::Repo;
use crate::turns::read_turns;
use fullops_core::model::{
    DEFAULT_BUDGET, DecisionStatus, Known, Plan, RoleId, STATE_V, State, Status, StatusDecision,
    StatusDispatch, StatusPending, StatusTask, TaskKey, WorkOutcome,
};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

fn parse(text: &str) -> Result<State, &'static str> {
    #[derive(serde::Deserialize)]
    struct Version {
        v: u32,
    }
    match serde_json::from_str::<State>(text) {
        Ok(state) if state.v > STATE_V => Err("state_newer"),
        Ok(state) => Ok(state),
        // 새 형식이라 못 읽은 것과 파손을 가른다
        Err(_) => Err(match serde_json::from_str::<Version>(text) {
            Ok(version) if version.v > STATE_V => "state_newer",
            _ => "state_broken",
        }),
    }
}

/// 잠그지 않고 읽는다. 파일이 없으면 빈 상태다. 파손이거나 `v`가 더 크면 `Unknown`이다.
pub fn read_state(repo: &Repo) -> Known<State> {
    match read_text(&repo.state_path()) {
        Known::Yes(None) => Known::Yes(State::default()),
        Known::Yes(Some(text)) => match parse(&text) {
            Ok(state) => Known::Yes(state),
            Err(why) => Known::Unknown(why),
        },
        Known::Unknown(why) => Known::Unknown(why),
    }
}

/// `state.json`을 쓰는 유일한 길(설계 6.5절). 잠금 → 다시 읽기 → 수정 → 원자 교체 → `status.json` 교체 → 잠금 해제.
/// `lead_plan`은 lead 문맥에서만 준다. 주면 `status.json`의 `ready`를 다시 계산하고 안 주면 이전 값을 유지한다.
pub fn update_state<T>(
    repo: &Repo,
    wait: Duration,
    lead_plan: Option<&Plan>,
    f: impl FnOnce(&mut State) -> T,
) -> Result<T, StoreError> {
    let path = repo.state_path();
    let _guard = acquire(&lock_path(repo), wait)?;
    let mut state = match read_text(&path) {
        Known::Yes(None) => State::default(),
        Known::Yes(Some(text)) => parse(&text).map_err(StoreError::Corrupt)?,
        Known::Unknown(why) => return Err(StoreError::Corrupt(why)),
    };
    let out = f(&mut state);
    state.seq += 1;
    write_json(&path, &state)?;
    // status.json은 유도값이라 실패해도 상태 쓰기는 성공으로 둔다
    let _ = write_json(&repo.status_path(), &build_status(repo, &state, lead_plan));
    Ok(out)
}

/// 읽히지 않는 `state.json`을 `state.json.broken-<시각>`으로 옮기고 빈 상태로 시작한다(CLI와 `doctor`만 쓴다).
pub fn quarantine_state(repo: &Repo, wait: Duration) -> Result<PathBuf, StoreError> {
    let path = repo.state_path();
    let _guard = acquire(&lock_path(repo), wait)?;
    let broken = repo.shared().join(format!("state.json.broken-{}", now()));
    std::fs::rename(&path, &broken)?;
    write_json(&path, &State::default())?;
    Ok(broken)
}

fn lock_path(repo: &Repo) -> PathBuf {
    repo.shared().join("state.json.lock")
}

/// Orca 패널과 봇이 읽는 요약(설계 6.2.3절). 잠금 안에서 부른다.
fn build_status(repo: &Repo, state: &State, lead_plan: Option<&Plan>) -> Status {
    let previous: Option<Status> = read_json(&repo.status_path()).yes().flatten();
    let archive = read_archive(repo).yes().unwrap_or_default();
    let archived = archive_keys(&archive);
    let turns = read_turns(repo).yes().unwrap_or_default();

    let mut keys: BTreeSet<&TaskKey> = state.tasks.keys().collect();
    keys.extend(state.dispatches.iter().map(|d| &d.key));
    keys.extend(state.completions.iter().filter_map(|c| c.key.as_ref()));
    let tasks = keys
        .into_iter()
        .map(|key| {
            let dispatch = state
                .dispatches
                .iter()
                .filter(|d| &d.key == key)
                .max_by_key(|d| d.started_at);
            let completions = || {
                state
                    .completions
                    .iter()
                    .filter(|c| c.key.as_ref() == Some(key))
            };
            let latest = completions().max_by_key(|c| c.received_at);
            let counters = state.tasks.get(key);
            StatusTask {
                key: key.clone(),
                role: dispatch
                    .map(|d| d.role.clone())
                    .or_else(|| latest.and_then(|c| c.role.clone())),
                stage: fullops_core::stage_of(
                    key,
                    state,
                    &archived,
                    repo.route_path(key).is_file(),
                ),
                rounds: counters.map_or(0, |t| t.rounds),
                budget: counters.map_or(DEFAULT_BUDGET, |t| t.budget),
                sha: latest
                    .and_then(|c| c.sha.as_ref())
                    .map(|s| s.short(7).to_string()),
                held: completions().any(|c| c.hold.is_some() && c.landed_at.is_none()),
            }
        })
        .collect();

    let pending = state
        .completions
        .iter()
        .filter(|c| !c.direct && c.outcome == WorkOutcome::Succeeded && c.landed_at.is_none())
        .map(|c| {
            let sha = c.sha.as_ref().map(|s| s.short(7).to_string());
            // landing은 git이 있어야 계산할 수 있어 여기서는 같은 건의 이전 값만 잇는다
            let landing = previous
                .as_ref()
                .and_then(|p| {
                    p.pending
                        .iter()
                        .find(|old| old.key == c.key && old.sha == sha && sha.is_some())
                })
                .and_then(|old| old.landing);
            StatusPending {
                key: c.key.clone(),
                sha,
                landing,
                hold_reason: c.hold.as_ref().map(|h| h.reason.clone()),
            }
        })
        .collect();

    let config = load_config(repo).ok();
    let inbox_exists = |role: &RoleId| repo.inbox_path(role).is_file();
    Status {
        v: 1,
        generated_at: now(),
        mode: config
            .map(|c| c.mode)
            .or(previous.as_ref().and_then(|p| p.mode)),
        version: env!("CARGO_PKG_VERSION").to_string(),
        tasks,
        dispatches: state
            .dispatches
            .iter()
            .map(|d| StatusDispatch {
                key: d.key.clone(),
                role: d.role.clone(),
                purpose: d.purpose,
                agent: d.tuple.agent.clone(),
                model: d.tuple.model.clone(),
                started_at: d.started_at,
                settled: d.settled.is_some(),
            })
            .collect(),
        pending,
        decisions: state
            .decisions
            .iter()
            .filter(|d| d.status == DecisionStatus::Open)
            .map(|d| StatusDecision {
                id: d.id.clone(),
                key: d.key.clone(),
                question: d.question.clone(),
                options: d.options.clone(),
                asked_at: d.asked_at,
                expires_at: d.expires_at,
            })
            .collect(),
        ready: match lead_plan {
            Some(plan) => fullops_core::ready(plan, state, &archived, &inbox_exists),
            None => previous.map(|p| p.ready).unwrap_or_default(),
        },
        sync_pending: state
            .sync_pending
            .iter()
            .map(|(role, sha)| (role.clone(), sha.short(7).to_string()))
            .collect(),
        metrics: fullops_core::metrics(&turns, state, &archive),
        issue: None,
    }
}
