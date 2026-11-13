//! `turns.jsonl`(설계 6.2.2절). 쓰기는 묶음 B가 채운다.

use crate::files::read_text;
use crate::repo::Repo;
use fullops_core::model::{Known, Turn};
use std::io;

/// 모든 줄. 해석하지 못한 줄은 건너뛴다.
pub fn read_turns(repo: &Repo) -> Known<Vec<Turn>> {
    read_text(&repo.turns_path()).map(|text| {
        text.unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    })
}

/// 한 줄을 `O_APPEND`로 `write` 한 번에 쓴다. 잠그지 않는다.
pub fn append_turn(_repo: &Repo, _turn: &Turn) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "append_turn은 아직 구현되지 않았습니다",
    ))
}
