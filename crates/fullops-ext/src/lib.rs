//! 바깥 프로세스(git, Orca, curl, 호스트 CLI). `fullops-store`를 모른다(설계 10.1절).

pub mod git;
pub mod hosts;
pub mod jev;
pub mod orca;
pub mod proc;

pub use fullops_core::model::TreeEntry;
pub use git::{Git, GitCli, MergeMode, MergeOutcome, NameStatus, StatusEntry, Worktree};
pub use hosts::{
    host_available, host_marketplaces, host_plugins, host_version, run_step, skill_roots,
};
pub use jev::{Answer, JevCurl, Judge, JudgeError, Judged, Question, QuestionKind, Usage};
pub use orca::{
    CheckMode, CheckResult, Orca, OrcaCli, StartFailure, StartOutcome, StartRequest, StartTarget,
    Started, WorkerView, orca_program,
};
pub use proc::{ProcResult, ProcStatus, run};

/// 바깥 도구 호출의 실패. CLI는 종료 코드 4로 끝낸다(설계 8.1절).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtError {
    /// 실행 파일을 찾지 못했다.
    NotFound,
    TimedOut,
    /// 0이 아닌 종료 코드, 또는 도구가 실패라고 답했다.
    Failed {
        code: i32,
        stderr: String,
    },
    /// 응답을 해석하지 못했다(필수 필드 없음 등).
    Parse(String),
}

impl std::fmt::Display for ExtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtError::NotFound => f.write_str("실행 파일을 찾지 못했습니다"),
            ExtError::TimedOut => f.write_str("제한 시간을 넘었습니다"),
            ExtError::Failed { code, stderr } => write!(f, "종료 코드 {code}: {}", stderr.trim()),
            ExtError::Parse(why) => write!(f, "응답을 해석하지 못했습니다: {why}"),
        }
    }
}

impl std::error::Error for ExtError {}
