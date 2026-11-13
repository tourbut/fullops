//! `state.json` 읽기·쓰기와 `status.json` 재생성(설계 6.2·6.5절).

use crate::files::{load_config, now, read_json, read_text, write_json};
use crate::lock::{StoreError, acquire};
use crate::repo::Repo;
use fullops_core::model::{
    DEFAULT_BUDGET, DecisionStatus, Derived, Known, STATE_V, State, Status, StatusDecision,
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
/// `derived`는 그 호출이 잠금 밖에서 계산한 유도값이다(설계 6.2.3절). 넘긴 것만 `status.json`에서 바뀌고 나머지는 이전 값을 잇는다.
/// 잠금 안에서는 `state.json`, `fullops.toml`, 이전 `status.json`만 읽는다(`turns.jsonl`·`archive.jsonl`은 읽지 않는다).
pub fn update_state<T>(
    repo: &Repo,
    wait: Duration,
    derived: Option<&Derived>,
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
    let _ = write_json(&repo.status_path(), &build_status(repo, &state, derived));
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
/// state에서 바로 나오는 부분은 매번 다시 만들고, 유도값(`ready`·`pending[].landing`·`metrics`)은 넘겨받은 것만 바꾼다.
fn build_status(repo: &Repo, state: &State, derived: Option<&Derived>) -> Status {
    let previous: Option<Status> = read_json(&repo.status_path()).yes().flatten();
    // archive.jsonl을 읽지 않으므로 단계는 assigned·reported·reviewing·merged 가운데 하나다
    let no_archive = BTreeSet::new();

    let mut keys: BTreeSet<&TaskKey> = BTreeSet::new();
    keys.extend(
        state
            .dispatches
            .iter()
            .filter(|d| d.counts())
            .map(|d| &d.key),
    );
    keys.extend(state.completions.iter().filter_map(|c| c.key.as_ref()));
    let tasks = keys
        .into_iter()
        .map(|key| {
            let dispatch = state
                .dispatches
                .iter()
                .filter(|d| &d.key == key && d.counts())
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
                stage: fullops_core::stage_of(key, state, &no_archive, true),
                rounds: counters.map_or(0, |t| t.rounds),
                budget: counters.map_or(DEFAULT_BUDGET, |t| t.budget),
                sha: latest
                    .and_then(|c| c.sha.as_ref())
                    .map(|s| s.short(7).to_string()),
                held: completions().any(|c| c.hold.is_some()),
            }
        })
        .collect();

    let pending = state
        .completions
        .iter()
        .filter(|c| !c.direct && c.outcome == WorkOutcome::Succeeded && c.landed_at.is_none())
        .map(|c| {
            let sha = c.sha.as_ref().map(|s| s.short(7).to_string());
            // 넘겨받은 값이 없으면 같은 completion ID의 이전 값을 잇는다
            let carried = || {
                previous
                    .as_ref()?
                    .pending
                    .iter()
                    .find(|old| old.id == c.id)?
                    .landing
            };
            StatusPending {
                id: c.id.clone(),
                key: c.key.clone(),
                landing: derived
                    .and_then(|d| d.landing.get(c.id.as_str()).copied())
                    .or_else(carried),
                sha,
                hold_reason: c.hold.as_ref().map(|h| h.reason.clone()),
            }
        })
        .collect();

    Status {
        v: 1,
        generated_at: now(),
        mode: load_config(repo)
            .ok()
            .map(|c| c.mode)
            .or(previous.as_ref().and_then(|p| p.mode)),
        version: env!("CARGO_PKG_VERSION").to_string(),
        tasks,
        dispatches: state
            .dispatches
            .iter()
            .filter(|d| !d.is_reservation())
            .map(|d| StatusDispatch {
                key: d.key.clone(),
                role: d.role.clone(),
                purpose: d.purpose,
                agent: d.tuple.agent.clone(),
                model: d.tuple.model.clone(),
                started_at: d.started_at,
                settled: d.settled.is_some(),
                sub: d.is_sub(),
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
        ready: derived
            .and_then(|d| d.ready.clone())
            .or_else(|| previous.as_ref().map(|p| p.ready.clone()))
            .unwrap_or_default(),
        sync_pending: state
            .sync_pending
            .iter()
            .map(|(role, sha)| (role.clone(), sha.short(7).to_string()))
            .collect(),
        metrics: derived
            .and_then(|d| d.metrics.clone())
            .or(previous.map(|p| p.metrics))
            .unwrap_or_default(),
        issue: None,
    }
}
