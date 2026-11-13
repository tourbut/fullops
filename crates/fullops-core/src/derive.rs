//! 유도값(설계 6.6절). 저장하지 않고 계산한다. `changed`·`pass_covers`·`landing`의 git 호출은 `facts_real`이 하고 순수 부분만 여기에 둔다.

use crate::model::{Plan, PlanStatus, RoleId, SnapshotState, Stage, State, TaskKey, WorkOutcome};
use std::collections::BTreeSet;

/// 과제 단계. `archive`는 `archive.jsonl`에서 `landed_at`이 있는 줄의 키, `routed`는 `routes/<KEY>.json`의 존재다.
pub fn stage_of(key: &TaskKey, state: &State, archive: &BTreeSet<TaskKey>, routed: bool) -> Stage {
    let work = || {
        state
            .dispatches
            .iter()
            .filter(|d| &d.key == key && d.is_work() && d.counts())
    };
    let completions = || {
        state
            .completions
            .iter()
            .filter(|c| c.key.as_ref() == Some(key))
    };
    // 하위 배정과 예약 기록은 이 판정에서 없는 것으로 본다
    let any_dispatch = state.dispatches.iter().any(|d| &d.key == key && d.counts());

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

/// 기록 경로(설계 6.6절 5번). `changed()`의 대상에서 항상 빼고, lint 통과 뒤의 기록 전용 커밋을 가릴 때 쓴다.
pub const RECORD_PATHS: [&str; 5] = [
    ".fullops-squad/handovers/",
    ".fullops-squad/docs/evaluations/",
    ".fullops-squad/routes/",
    ".fullops-squad/reviews/",
    ".fullops-squad/PLANS.md",
];

pub fn is_record_path(path: &str) -> bool {
    RECORD_PATHS.iter().any(|record| path.starts_with(record))
}

/// `changed()`의 대상인가: 기록 경로가 아니고, 코드 파일이다(dev 직접 작업은 lint 제외에 안 걸리는 모든 파일).
pub fn is_change_target(
    path: &str,
    code: &crate::model::CodeDef,
    exclude: &globset::GlobSet,
    direct_dev: bool,
) -> bool {
    if is_record_path(path) {
        return false;
    }
    if direct_dev {
        !exclude.is_match(path)
    } else {
        crate::config::is_code(path, code, exclude)
    }
}

/// 통합 위치(설계 6.6절). `on_base`는 통합 기준 ref의 조상인가, `on_local`은 원격이 있을 때 `refs/heads/<base>`의 조상인가다.
pub fn landing_of(on_base: bool, on_local: Option<bool>) -> crate::model::Landing {
    use crate::model::Landing;
    match (on_base, on_local) {
        (true, _) => Landing::Landed,
        (false, Some(true)) => Landing::PushLeft,
        _ => Landing::MergeLeft,
    }
}
