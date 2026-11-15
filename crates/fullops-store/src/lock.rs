//! OS 파일 잠금(설계 6.5절). 프로세스가 죽으면 OS가 풀어 준다.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum StoreError {
    /// 기한 안에 잠금을 얻지 못했다. hook은 쓰기를 건너뛴다.
    LockBusy,
    /// 고칠 파일이 없다(세션 파일).
    Missing,
    /// 파일을 해석하지 못했다. 글은 `Known::Unknown`의 사유와 같다(`state_broken`, `state_newer` 등).
    Corrupt(&'static str),
    Io(io::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::LockBusy => f.write_str("다른 프로세스가 상태 파일을 쓰는 중입니다"),
            StoreError::Missing => f.write_str("파일이 없습니다"),
            StoreError::Corrupt(why) => write!(f, "파일을 읽을 수 없습니다({why})"),
            StoreError::Io(e) => write!(f, "입출력 오류: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        StoreError::Io(e)
    }
}

/// hook의 잠금 대기 한도.
pub const HOOK_WAIT: Duration = Duration::from_millis(300);
/// CLI의 잠금 대기 한도.
pub const CLI_WAIT: Duration = Duration::from_secs(10);

/// 잡고 있는 동안 잠금이 유지된다. 버리면 풀린다.
#[derive(Debug)]
pub struct LockGuard {
    _file: File,
}

/// 잠금 파일을 열고 10ms 간격으로 잠금을 시도한다.
pub fn acquire(path: &Path, wait: Duration) -> Result<LockGuard, StoreError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    let deadline = Instant::now() + wait;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(LockGuard { _file: file }),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(TryLockError::WouldBlock) => return Err(StoreError::LockBusy),
            Err(TryLockError::Error(e)) => return Err(StoreError::Io(e)),
        }
    }
}
