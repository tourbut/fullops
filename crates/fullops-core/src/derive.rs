//! 유도값(설계 6.6절). 저장하지 않고 계산한다. `changed`·`pass_covers`·`landing`의 순수 부분은 묶음 B가 여기에 더한다.

use crate::model::{Plan, PlanStatus, RoleId, SnapshotState, Stage, State, TaskKey, WorkOutcome};
use std::collections::BTreeSet;

/// 과제 단계. `archive`는 `archive.jsonl`에서 `landed_at`이 있는 줄의 키, `routed`는 `routes/<KEY>.json`의 존재다.
pub fn stage_of(key: &TaskKey, state: &State, archive: &BTreeSet<TaskKey>, routed: bool) -> Stage {
    let work = || {
        state
            .dispatches
            .iter()
            .filter(|d| &d.key == key && d.is_work())
    };
    let completions = || {
        state
            .completions
            .iter()
            .filter(|c| c.key.as_ref() == Some(key))
    };
    let any_dispatch = state.dispatches.iter().any(|d| &d.key == key);

    if archive.contains(key) && !any_dispatch && completions().next().is_none() {
        return Stage::Done;
    }
    let last_work = work().map(|d| d.started_at).max();
    let last_completion = completions().map(|c| c.received_at).max();
    if let (Some(started), Some(received)) = (last_work, last_completion)
        && started > received
    {
        return Stage::Assigned;
    }
    let mut landed = completions()
        .filter(|c| !c.direct && c.outcome == WorkOutcome::Succeeded)
        .peekable();
    if landed.peek().is_some() && landed.all(|c| c.landed_at.is_some()) {
        return Stage::Merged;
    }
    if state
        .snapshots
        .iter()
        .any(|s| s.state == SnapshotState::Active && s.key() == key.as_str())
    {
        return Stage::Reviewing;
    }
    if last_completion.is_some() {
        return Stage::Reported;
    }
    if last_work.is_some() {
        return Stage::Assigned;
    }
    if routed { Stage::Routed } else { Stage::None }
}

/// 준비된 행의 키를 표 순서대로 돌려준다. `inbox_exists`는 lead 체크아웃에 그 역할의 인박스가 있는지다.
pub fn ready(
    plan: &Plan,
    state: &State,
    archive: &BTreeSet<TaskKey>,
    inbox_exists: &dyn Fn(&RoleId) -> bool,
) -> Vec<TaskKey> {
    // 조건 4·5는 Assigned·Reported와만 비교하므로 routed 여부가 결과를 바꾸지 않는다
    let stage = |key: &TaskKey| stage_of(key, state, archive, false);
    plan.rows
        .iter()
        .filter(|row| {
            row.status == PlanStatus::Ready
                && !row.source.is_empty()
                && !row.role.as_ref().is_some_and(inbox_exists)
                && row.after.iter().all(|k| stage(k) >= Stage::Reported)
                && stage(&row.key) < Stage::Assigned
        })
        .map(|row| row.key.clone())
        .collect()
}
