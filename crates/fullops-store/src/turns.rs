//! `turns.jsonl`(설계 6.2.2절). 본문은 쓰지 않는다.

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
pub fn append_turn(repo: &Repo, turn: &Turn) -> io::Result<()> {
    crate::files::append_line(&repo.turns_path(), turn)
}
