//! 흐름 명령의 순수 검사와 기록(설계 8.4~8.6절, 6.7절). 묶음 C가 채운다.
//! 시그니처는 설계 10.2절이 정본이고 뼈대는 임시 구현만 둔다.

use crate::facts::Facts;
use crate::model::{
    ArchiveLine, DispatchRequest, MessageNote, Metrics, ModelTuple, OrcaMessage, Parallel, Refusal,
    RoleId, RunId, Sha, State, TaskNewRequest, Turn,
};

/// D-01~D-10과 D-13. 걸린 것을 모두 돌려준다. 하위 배정(`req.sub`)이면 D-04·D-08·D-09·D-13만 본다.
pub fn check_dispatch(_req: &DispatchRequest, _facts: &mut dyn Facts) -> Vec<Refusal> {
    Vec::new()
}

/// H-01~H-04. 걸린 것을 모두 돌려준다.
pub fn check_task_new(_req: &TaskNewRequest, _facts: &mut dyn Facts) -> Vec<Refusal> {
    Vec::new()
}

/// 메시지 한 건의 종류별 처리와 completion 기록 규칙(설계 8.6절 6단계). 모든 종류의 메시지를 받는다.
/// `run`은 호출할 때 준 `--run`이다(메시지에는 Run이 없다). `resolve`는 짧은 hex를 커밋으로 풀고
/// `branch_tip`은 역할 브랜치의 로컬 ref를 읽는다.
pub fn record_message(
    _state: &mut State,
    _run: &RunId,
    _msg: &OrcaMessage,
    _resolve: &mut dyn FnMut(&str) -> Option<Sha>,
    _branch_tip: &mut dyn FnMut(&RoleId) -> Option<Sha>,
) -> MessageNote {
    MessageNote::default()
}

/// 지표(설계 6.7절). `status`와 `merge`(M-13)가 잠금 밖에서 부른다. 임시 구현은 모든 값이 0이다.
pub fn metrics(_turns: &[Turn], _state: &State, _archive: &[ArchiveLine]) -> Metrics {
    Metrics::default()
}

/// Orca `worker-start`에 넘길 `--effort` 값(설계 8.5절 8단계). 기록에는 항상 tuple의 값을 쓴다.
/// 임시 구현은 tuple의 `effort`다. 미정-M13이 바꾸는 곳은 이 함수 하나다.
pub fn orca_effort(tuple: &ModelTuple, _parallel: Parallel, _new_terminal: bool) -> String {
    tuple.effort.clone()
}
