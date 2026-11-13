//! 편집 도구의 대상과 결과 계산(설계 7.4절). 묶음 B가 채운다.

use crate::model::{Known, ToolCall};
use std::path::{Path, PathBuf};

/// 편집 한 건. `after`가 `Yes(None)`이면 삭제다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditTarget {
    pub path: PathBuf,
    pub before: Option<String>,
    pub after: Known<Option<String>>,
}

/// 도구 호출이 바꾸는 파일과 편집 뒤의 글. `read`는 현재 파일 글을 읽는다.
pub fn edit_targets(
    _tool: &ToolCall,
    _read: &mut dyn FnMut(&Path) -> Known<Option<String>>,
) -> Vec<EditTarget> {
    // 미구현: 대상 목록이 비면 통과다
    Vec::new()
}
