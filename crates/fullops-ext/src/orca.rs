//! Orca CLI 호출과 응답 해석(설계 9.2절). 응답 해석은 이 파일 한 곳에서 한다. 묶음 C가 채운다.
//! 인자 이름과 응답 필드(미정-M3)가 바뀌면 이 파일만 고친다.

use crate::ExtError;
use fullops_core::model::{DispatchId, OrcaMessage, RunId, WorkOutcome};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `orchestration worker-start`의 인자(설계 8.5절 8단계).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRequest {
    pub run: RunId,
    pub spec: String,
    pub worktree: PathBuf,
    pub agent: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub terminal: Option<String>,
    pub task: Option<String>,
    pub timeout_ms: u64,
}

/// 결과에 dispatch ID가 없으면 부르는 쪽이 `pending:<run>:<started_at>`으로 쓴다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    pub dispatch: Option<DispatchId>,
    pub task: Option<String>,
    pub terminal: Option<String>,
}

/// worker 응답에서 읽는 값(설계 9.2절).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerView {
    pub dispatch: Option<DispatchId>,
    pub task: Option<String>,
    pub run: Option<RunId>,
    pub outcome: Option<WorkOutcome>,
    /// `stage == "settled"`.
    pub settled: bool,
    pub liveness: Option<String>,
    pub terminal: Option<String>,
}

pub trait Orca {
    fn run_create(&self) -> Result<RunId, ExtError>;
    fn worker_start(&self, req: &StartRequest) -> Result<Started, ExtError>;
    /// `wait`가 `Some`이면 기다림 방식으로 부른다.
    fn check(
        &self,
        run: &RunId,
        peek: bool,
        wait: Option<Duration>,
    ) -> Result<Vec<OrcaMessage>, ExtError>;
    fn ack(&self, run: &RunId, deliveries: &[String]) -> Result<(), ExtError>;
    fn worker_show(&self, dispatch: &DispatchId) -> Result<WorkerView, ExtError>;
    fn worker_list(&self, run: &RunId) -> Result<Vec<WorkerView>, ExtError>;
    fn send_escalation(
        &self,
        dispatch: &DispatchId,
        task: Option<&str>,
        subject: &str,
        body: &str,
    ) -> Result<(), ExtError>;
    /// 그 워크트리에 살아 있는 터미널의 핸들.
    fn terminals_in(&self, worktree: &Path) -> Result<Vec<String>, ExtError>;
    fn run_use(&self, run: &RunId) -> Result<(), ExtError>;
}

/// Orca 실행 파일을 부르는 구현. 시험에서는 `ORCA_CLI_COMMAND`가 fake-orca를 가리킨다.
pub struct OrcaCli {
    /// 설계 9.2절의 순서로 찾은 실행 파일.
    pub program: String,
    pub cwd: PathBuf,
}

impl Orca for OrcaCli {
    fn run_create(&self) -> Result<RunId, ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn worker_start(&self, _req: &StartRequest) -> Result<Started, ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn check(
        &self,
        _run: &RunId,
        _peek: bool,
        _wait: Option<Duration>,
    ) -> Result<Vec<OrcaMessage>, ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn ack(&self, _run: &RunId, _deliveries: &[String]) -> Result<(), ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn worker_show(&self, _dispatch: &DispatchId) -> Result<WorkerView, ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn worker_list(&self, _run: &RunId) -> Result<Vec<WorkerView>, ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn send_escalation(
        &self,
        _dispatch: &DispatchId,
        _task: Option<&str>,
        _subject: &str,
        _body: &str,
    ) -> Result<(), ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn terminals_in(&self, _worktree: &Path) -> Result<Vec<String>, ExtError> {
        Err(ExtError::Unimplemented)
    }
    fn run_use(&self, _run: &RunId) -> Result<(), ExtError> {
        Err(ExtError::Unimplemented)
    }
}
