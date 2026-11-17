//! 유도값(설계 6.6절). 저장하지 않고 계산한다. `changed`·`pass_covers`·`landing`의 git 호출은 `facts_real`이 하고 순수 부분만 여기에 둔다.

use crate::model::{
    Completion, Plan, PlanStatus, RoleId, SnapshotState, Stage, State, TaskKey, WorkOutcome,
};
use std::collections::BTreeSet;

/// 과제 단계. `archive`는 `archive.jsonl`에서 `landed_at`이 있는 줄의 키, `routed`는 `tasks/<KEY>/route.json`의 존재다.
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

/// 그 과제의 가장 늦은 보고가 실패면 그 보고. 실패한 과제는 선행 조건을 채우지 못하고 lead에게 다시 알린다.
/// 같은 배정에 성공 보고가 있으면 실패가 뒤에 와도 실패로 보지 않는다(도착 순서로 결과가 갈리지 않게 한다. `dispatch`의 D-02와 같다).
/// 같은 키·역할에서 같은 차수를 보낸 다른 배정의 성공 보고도 그렇다(끝난 차수의 재배정이 실패해도 그 차수는 끝났다).
pub fn last_failed<'a>(key: &TaskKey, state: &'a State) -> Option<&'a Completion> {
    let round = |c: &Completion| {
        c.dispatch
            .as_ref()
            .and_then(|id| state.dispatch(id))
            .and_then(|d| d.round)
    };
    let succeeded = |c: &Completion| {
        state.completions.iter().any(|s| {
            s.outcome == WorkOutcome::Succeeded
                && ((c.dispatch.is_some() && s.dispatch == c.dispatch)
                    || (s.key == c.key
                        && s.role == c.role
                        && round(c).is_some()
                        && round(s) == round(c)))
        })
    };
    state
        .completions
        .iter()
        .filter(|c| c.key.as_ref() == Some(key))
        .max_by_key(|c| c.received_at)
        .filter(|c| c.outcome == WorkOutcome::Failed && !succeeded(c))
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
                && row
                    .after
                    .iter()
                    .all(|k| stage(k) >= Stage::Reported && last_failed(k, state).is_none())
                && stage(&row.key) < Stage::Assigned
        })
        .map(|row| row.key.clone())
        .collect()
}

/// `changed()`의 대상인가: 기록 경로(`layout::is_record`. 설계 6.6절 5번)가 아니고, 코드 파일이다(dev 직접 작업은 lint 제외에 안 걸리는 모든 파일).
pub fn is_change_target(
    path: &str,
    code: &crate::model::CodeDef,
    exclude: &globset::GlobSet,
    direct_dev: bool,
) -> bool {
    if crate::layout::is_record(path) {
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
