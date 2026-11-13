//! StopFailure 규칙의 `check`(설계 7.5절). 묶음 B가 채운다.

use super::Finding;
use crate::facts::Facts;
use crate::model::HookInput;

/// F-01
pub(super) fn f01(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    Finding::None
}
