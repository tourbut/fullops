//! git 호출(설계 9.1절). 구현은 묶음 E가 채운다.

use crate::ExtError;
use fullops_core::model::{FileStat, Known, Sha};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `git status --porcelain`의 한 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    /// 인덱스 칸(X).
    pub index: char,
    /// 작업 트리 칸(Y).
    pub worktree: char,
    /// 레포 기준 상대 경로(`/` 구분).
    pub path: String,
}

/// `git worktree list --porcelain`의 한 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    /// detached면 `None`. `refs/heads/`를 뗀 이름이다.
    pub branch: Option<String>,
    pub head: Sha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMode {
    FfOnly,
    NoEdit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeOutcome {
    Done,
    Conflict,
}

/// 읽기는 `Known`, 쓰기는 `Result`다. 쓰기는 CLI만 한다(설계 9.1절).
pub trait Git {
    fn is_ancestor(&self, older: &str, newer: &str) -> Known<bool>;
    fn merge_base(&self, a: &str, b: &str) -> Known<Sha>;
    fn rev_parse(&self, rev: &str) -> Known<Sha>;
    fn diff_names(&self, from: &str, to: &str) -> Known<Vec<String>>;
    fn numstat(&self, from: &str, to: &str) -> Known<Vec<FileStat>>;
    fn status(&self, pathspec: Option<&str>) -> Known<Vec<StatusEntry>>;
    /// `(rev, path)` 묶음 조회. 없는 blob은 `None`.
    fn show_blobs(&self, items: &[(String, String)]) -> Known<Vec<Option<Vec<u8>>>>;
    fn worktrees(&self) -> Known<Vec<Worktree>>;
    /// `git commit --only`. 바뀐 것이 없으면 `None`.
    fn commit_only(&self, paths: &[String], message: &str) -> Result<Option<Sha>, ExtError>;
    fn merge(&self, cwd: &Path, rev: &str, mode: MergeMode) -> Result<MergeOutcome, ExtError>;
    /// force를 쓰지 않는다.
    fn push(&self, remote: &str, refspec: &str) -> Result<(), ExtError>;
    fn fetch(&self, remote: &str, branch: &str) -> Result<(), ExtError>;
    fn mv(&self, from: &str, to: &str) -> Result<(), ExtError>;
    fn merge_abort(&self, cwd: &Path) -> Result<(), ExtError>;
    /// `git reset --keep`. M-07만 쓴다.
    fn reset_keep(&self, cwd: &Path, rev: &str) -> Result<(), ExtError>;
    /// 이전 값이 `old`일 때만 바꾼다.
    fn update_ref(&self, name: &str, new: &Sha, old: &Sha) -> Result<(), ExtError>;
    fn worktree_add_detached(&self, path: &Path, rev: &str) -> Result<(), ExtError>;
    /// force를 쓰지 않는다.
    fn worktree_remove(&self, path: &Path) -> Result<(), ExtError>;
}

/// git 실행 파일을 부르는 구현. 제한 시간은 hook 2초, CLI 30초(fetch·push·merge는 120초)다.
pub struct GitCli {
    pub repo_root: PathBuf,
    pub timeout: Duration,
}

const TODO: &str = "unimplemented";

impl Git for GitCli {
    fn is_ancestor(&self, _older: &str, _newer: &str) -> Known<bool> {
        Known::Unknown(TODO)
    }
    fn merge_base(&self, _a: &str, _b: &str) -> Known<Sha> {
        Known::Unknown(TODO)
    }
    fn rev_parse(&self, _rev: &str) -> Known<Sha> {
        Known::Unknown(TODO)
    }
    fn diff_names(&self, _from: &str, _to: &str) -> Known<Vec<String>> {
        Known::Unknown(TODO)
    }
    fn numstat(&self, _from: &str, _to: &str) -> Known<Vec<FileStat>> {
        Known::Unknown(TODO)
    }
    fn status(&self, _pathspec: Option<&str>) -> Known<Vec<StatusEntry>> {
        Known::Unknown(TODO)
    }
    fn show_blobs(&self, _items: &[(String, String)]) -> Known<Vec<Option<Vec<u8>>>> {
        Known::Unknown(TODO)
    }
    fn worktrees(&self) -> Known<Vec<Worktree>> {
        Known::Unknown(TODO)
    }
    fn commit_only(&self, _paths: &[String], _message: &str) -> Result<Option<Sha>, ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn merge(&self, _cwd: &Path, _rev: &str, _mode: MergeMode) -> Result<MergeOutcome, ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn push(&self, _remote: &str, _refspec: &str) -> Result<(), ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn fetch(&self, _remote: &str, _branch: &str) -> Result<(), ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn mv(&self, _from: &str, _to: &str) -> Result<(), ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn merge_abort(&self, _cwd: &Path) -> Result<(), ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn reset_keep(&self, _cwd: &Path, _rev: &str) -> Result<(), ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn update_ref(&self, _name: &str, _new: &Sha, _old: &Sha) -> Result<(), ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn worktree_add_detached(&self, _path: &Path, _rev: &str) -> Result<(), ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn worktree_remove(&self, _path: &Path) -> Result<(), ExtError> {
        Err(ExtError::Unimplemented)
    }
}
