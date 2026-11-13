//! UserPromptSubmit 규칙의 `check`(설계 7.5절). 묶음 B가 채운다.

use super::Finding;
use crate::facts::Facts;
use crate::model::HookInput;

/// P-00
pub(super) fn p00(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    Finding::None
}

/// P-01
pub(super) fn p01(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    Finding::None
}

/// P-02
pub(super) fn p02(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    Finding::None
}

/// P-03
pub(super) fn p03(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    Finding::None
}
