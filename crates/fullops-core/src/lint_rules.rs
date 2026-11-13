//! lint 내장 검사와 레포 정규식 규칙(설계 8.11절 9번). 묶음 D가 채운다.
//! 아래 셋은 다른 묶음이 부르는 공개 함수다(설계 10.2절 끝의 표). 뼈대는 임시 구현만 둔다.

use crate::model::DeliverableRow;

/// 파일 머리 설명(주석·docstring 1~3줄). DOC-001과 탐색 색인이 함께 쓴다. `ext`는 점을 포함한 확장자다.
/// 임시 구현은 빈 글이다.
pub fn header_summary(_text: &str, _ext: &str) -> String {
    String::new()
}

/// `lint.json`의 `exclude`. 글이 `None`이거나 `exclude`가 없으면 내장 템플릿 `lint.json`(`assets::LINT_JSON`)의 `exclude`다.
/// 임시 구현은 빈 `GlobSet`이다(아무것도 제외하지 않는다).
pub fn lint_excludes(_lint_json: Option<&str>, _template_lint_json: &str) -> globset::GlobSet {
    globset::GlobSet::empty()
}

/// 산출물 인덱스 `docs/deliverables/README.md`의 표(열 문법은 미정-M7). 임시 구현은 빈 목록이다.
pub fn parse_deliverable_index(_text: &str) -> Vec<DeliverableRow> {
    Vec::new()
}
