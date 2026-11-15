//! `trace/<sid>.jsonl`(설계 6.3절). 통과가 아닌 판정, 오류, 시간 초과를 남긴다.

use crate::files::{append_line, write_atomic};
use crate::repo::Repo;
use fullops_core::model::{SessionId, TraceLine};

/// 추적 파일의 한도. 넘으면 앞 절반을 버린다(설계 6.8절).
const LIMIT: u64 = 1024 * 1024;

/// 추적 한 줄을 더한다. 실패는 버린다(hook의 판정에 영향을 주지 않는다).
pub fn append_trace(repo: &Repo, sid: &SessionId, line: &TraceLine) {
    let path = repo.trace_path(sid);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > LIMIT)
        && let Ok(bytes) = std::fs::read(&path)
    {
        // 줄 중간에서 자르지 않는다
        let half = bytes.len() / 2;
        let cut = bytes
            .get(half..)
            .and_then(|rest| rest.iter().position(|b| *b == b'\n'))
            .map_or(bytes.len(), |at| half + at + 1);
        let _ = write_atomic(&path, bytes.get(cut..).unwrap_or_default());
    }
    let _ = append_line(&path, line);
}

/// 그 세션의 추적 줄 전부. 해석하지 못한 줄은 건너뛴다. `fullops why`가 읽는다.
pub fn read_trace(repo: &Repo, sid: &SessionId) -> Vec<TraceLine> {
    std::fs::read_to_string(repo.trace_path(sid))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}
