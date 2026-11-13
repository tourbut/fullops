//! 셸 명령 분류(설계 7.4절). 묶음 B가 채운다.

use crate::model::{Dialect, Intent};

/// 명령 문자열을 Intent 목록으로 나눈다. `orca_names`는 Orca 실행 파일로 볼 이름이다.
pub fn classify_shell(_command: &str, _dialect: Dialect, _orca_names: &[String]) -> Vec<Intent> {
    // 미구현: Unparsed에는 어떤 규칙도 걸리지 않는다
    vec![Intent::Unparsed]
}
