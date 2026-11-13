//! 리뷰 필요 판정(설계 8.9절). 묶음 E가 채운다.

use crate::model::{CodeDef, FileStat, Need, NeedAction, ReviewPolicy, Sha};

/// `numstat`은 `git diff --numstat <merge_base> <head>`의 결과다.
pub fn review_need(
    _policy: &ReviewPolicy,
    _numstat: &[FileStat],
    _code: &CodeDef,
    _exclude: &globset::GlobSet,
    base: Sha,
    head: Sha,
    merge_base: Sha,
) -> Need {
    // 미구현: 리뷰를 조용히 건너뛰지 않도록 필요한 쪽으로 답한다
    Need {
        required: true,
        action: NeedAction::Ask,
        reason: "review_need가 아직 구현되지 않았습니다.".into(),
        code_files: 0,
        code_lines: 0,
        always: Vec::new(),
        base,
        head,
        merge_base,
    }
}
