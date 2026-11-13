//! 흐름 명령의 순수 검사와 기록(설계 8.4~8.6절, 6.7절). 묶음 C가 채운다.

use crate::facts::Facts;
use crate::model::{
    ArchiveLine, CompletionNote, DispatchRequest, Metrics, OrcaMessage, Refusal, RoleId, Sha,
    State, TaskNewRequest, Turn,
};

/// D-01~D-10. 걸린 것을 모두 돌려준다.
pub fn check_dispatch(_req: &DispatchRequest, _facts: &mut dyn Facts) -> Vec<Refusal> {
    Vec::new()
}

/// H-01~H-04. 걸린 것을 모두 돌려준다.
pub fn check_task_new(_req: &TaskNewRequest, _facts: &mut dyn Facts) -> Vec<Refusal> {
    Vec::new()
}

/// completion 기록 규칙(설계 8.6절). `resolve`는 짧은 hex를 커밋으로 풀고 `branch_tip`은 역할 브랜치의 로컬 ref를 읽는다.
pub fn record_completion(
    _state: &mut State,
    _msg: &OrcaMessage,
    _resolve: &mut dyn FnMut(&str) -> Option<Sha>,
    _branch_tip: &mut dyn FnMut(&RoleId) -> Option<Sha>,
) -> CompletionNote {
    CompletionNote::default()
}

/// 지표(설계 6.7절).
pub fn metrics(_turns: &[Turn], _state: &State, _archive: &[ArchiveLine]) -> Metrics {
    Metrics::default()
}
