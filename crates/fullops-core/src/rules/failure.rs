//! StopFailure 규칙의 `check`(설계 7.5절). agy는 오류로 끝난 Stop을 어댑터가 이 이벤트로 옮긴다.

use super::{Effect, Finding, call, hit, known};
use crate::facts::Facts;
use crate::model::HookInput;

/// F-01. dispatch당 한 번 보낸다. 오류 종류는 가리지 않는다.
pub(super) fn f01(input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let actor = known!(call::actor(facts));
    let Some(id) = actor.dispatch else {
        return Finding::None;
    };
    let recorded = facts.state().yes().and_then(|state| {
        let d = state.dispatch(&id)?;
        Some((d.settled.is_some() || d.limit_escalated, d.task_id.clone()))
    });
    let session_task = facts
        .session()
        .yes()
        .and_then(|s| s.dispatch.as_ref())
        .and_then(|d| d.task.clone());
    let (done, task) = match recorded {
        Some((done, task)) => (done, task.or(session_task)),
        // state.json에 없는 dispatch는 세션 파일의 noticed로 한 번만 보낸다
        None => (call::noticed(facts, &format!("f01:{id}")), session_task),
    };
    if done {
        return Finding::None;
    }
    let kind = input.error.as_deref().unwrap_or("error");
    let text = match input.error_details.as_deref().filter(|d| !d.is_empty()) {
        Some(details) => format!("{kind}: {details}"),
        None => kind.to_string(),
    };
    call::effect(Effect::SendLimitEscalation {
        dispatch: id.clone(),
        task,
        error: text.chars().take(300).collect(),
    });
    hit(id.as_str(), Vec::new())
}
