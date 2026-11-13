//! 바깥 프로세스(git, Orca, curl). `fullops-store`를 모른다(설계 10.1절).

pub mod git;
pub mod jev;
pub mod orca;
pub mod proc;

pub use git::{Git, GitCli, MergeMode, MergeOutcome, StatusEntry, Worktree};
pub use jev::{Answer, Judge, JudgeError, Question};
pub use orca::{Orca, OrcaCli, StartRequest, Started, WorkerView};
pub use proc::{ProcResult, ProcStatus, run};

/// 바깥 도구 호출의 실패. CLI는 종료 코드 4로 끝낸다(설계 8.1절).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtError {
    /// 실행 파일을 찾지 못했다.
    NotFound,
    TimedOut,
    /// 0이 아닌 종료 코드.
    Failed {
        code: i32,
        stderr: String,
    },
    /// 응답을 해석하지 못했다(필수 필드 없음 등).
    Parse(String),
    /// 뼈대 단계의 빈 구현.
    Unimplemented,
}

impl std::fmt::Display for ExtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtError::NotFound => f.write_str("실행 파일을 찾지 못했습니다"),
            ExtError::TimedOut => f.write_str("제한 시간을 넘었습니다"),
            ExtError::Failed { code, stderr } => write!(f, "종료 코드 {code}: {}", stderr.trim()),
            ExtError::Parse(why) => write!(f, "응답을 해석하지 못했습니다: {why}"),
            ExtError::Unimplemented => f.write_str("아직 구현되지 않았습니다"),
        }
    }
}

impl std::error::Error for ExtError {}
