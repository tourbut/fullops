//! Stop 규칙의 `check`(설계 7.5절). 묶음 B가 채운다.

use super::Finding;
use crate::facts::Facts;
use crate::model::HookInput;

/// X-00
pub(super) fn x00(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    Finding::None
}

/// X-01. 하위 작업 남음(`State::sub_open`)이면 걸리지 않는다: 하위 worker가 아직 이 워크트리에 쓰고 있다.
pub(super) fn x01(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    Finding::None
}

/// X-02
pub(super) fn x02(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    Finding::None
}

/// X-03. 하위 작업 남음이면 걸리지 않는다: 탈출구인 `fullops done`을 R-11이 거절한다.
pub(super) fn x03(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    Finding::None
}

/// X-04
pub(super) fn x04(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    Finding::None
}

/// X-05
pub(super) fn x05(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    Finding::None
}

/// X-06
pub(super) fn x06(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    Finding::None
}

/// X-07
pub(super) fn x07(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    Finding::None
}
