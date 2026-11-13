//! 프로세스 실행기(설계 10.2절). 셸을 거치지 않는 인자 배열로 실행한다.
//! 뼈대의 기본 구현이다. 프로세스 트리 종료(Windows Job Object, POSIX 프로세스 그룹)는 묶음 D가 더한다.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcStatus {
    Exited(i32),
    TimedOut,
    /// 실행 파일을 찾지 못했거나 띄우지 못했다.
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcResult {
    pub status: ProcStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl ProcResult {
    pub fn ok(&self) -> bool {
        self.status == ProcStatus::Exited(0)
    }

    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

fn failed(status: ProcStatus) -> ProcResult {
    ProcResult {
        status,
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

fn read_all(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    })
}

/// `argv[0]`을 `cwd`에서 실행하고 끝나기를 기다린다. `env`는 물려받은 환경에 덧붙인다.
pub fn run(
    argv: &[String],
    cwd: &Path,
    timeout: Duration,
    env: &[(&str, &str)],
    stdin: Option<&[u8]>,
) -> ProcResult {
    let Some((program, args)) = argv.split_first() else {
        return failed(ProcStatus::NotFound);
    };
    let spawned = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .envs(env.iter().copied())
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let Ok(mut child) = spawned else {
        return failed(ProcStatus::NotFound);
    };

    // 파이프가 차서 서로 멈추지 않도록 입출력을 스레드로 나눈다
    if let (Some(mut pipe), Some(bytes)) = (child.stdin.take(), stdin.map(<[u8]>::to_vec)) {
        std::thread::spawn(move || {
            let _ = pipe.write_all(&bytes);
        });
    }
    let out = read_all(child.stdout.take());
    let err = read_all(child.stderr.take());

    let deadline = Instant::now() + timeout;
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(-1),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(2)),
            _ => {
                // shortcut: 직접 띄운 프로세스만 끝낸다, 하위 프로세스까지 끝내야 하는 lint 실행기(묶음 D)에서 트리 종료로 바꾼다
                let _ = child.kill();
                let _ = child.wait();
                return failed(ProcStatus::TimedOut);
            }
        }
    };
    ProcResult {
        status: ProcStatus::Exited(code),
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_passes_stdin_and_reports_missing() {
        let cwd = std::env::temp_dir();
        let git = |args: &[&str]| -> Vec<String> {
            std::iter::once("git")
                .chain(args.iter().copied())
                .map(String::from)
                .collect()
        };
        let ok = run(
            &git(&["--version"]),
            &cwd,
            Duration::from_secs(20),
            &[],
            None,
        );
        assert!(ok.ok() && ok.stdout_text().starts_with("git version"));
        let hashed = run(
            &git(&["hash-object", "--stdin"]),
            &cwd,
            Duration::from_secs(20),
            &[],
            Some(b"fullops\n"),
        );
        assert_eq!(hashed.stdout_text().trim().len(), 40);
        let missing = run(
            &["fullops-no-such-program".to_string()],
            &cwd,
            Duration::from_secs(1),
            &[],
            None,
        );
        assert_eq!(missing.status, ProcStatus::NotFound);
        assert_eq!(
            run(&[], &cwd, Duration::from_secs(1), &[], None).status,
            ProcStatus::NotFound
        );
    }
}
