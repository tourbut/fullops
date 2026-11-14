//! 프로세스 실행기(설계 10.2절). 셸을 거치지 않는 인자 배열로 실행한다.
//! 제한 시간이 지나면 프로세스 트리를 끝낸다(설계 8.11절 8번). `run`의 시그니처는 뼈대가 동결한 그대로다.

use std::collections::BTreeMap;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcStatus {
    Exited(i32),
    TimedOut,
    /// 실행 파일을 찾지 못했거나 띄우지 못했다.
    NotFound,
}

/// `TimedOut`이면 `stdout`·`stderr`는 끝내기 전까지 나온 출력이다.
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

/// 트리를 끝낸 뒤 남은 출력을 기다리는 한도. 트리를 벗어난 자손이 파이프를 쥐고 있을 수 있어 오래 기다리지 않는다.
const KILL_WAIT: Duration = Duration::from_secs(5);
/// 출력이 제한 시간 끝에 맞춰 끝났을 때 프로세스의 종료를 마저 기다리는 최소 시간.
const EXIT_GRACE: Duration = Duration::from_millis(200);
/// 출력이 끝나기를 기다리는 동안 종료 상태를 보는 간격. 출력이 끝나면 이 간격을 기다리지 않고 바로 깬다.
const EXIT_POLL: Duration = Duration::from_millis(50);

type Shared = Arc<Mutex<Vec<u8>>>;

/// 파이프를 끝까지 읽어 공유 버퍼에 쌓는다. 다 읽으면 `done`을 버려, 받는 쪽이 끊김으로 안다.
fn read_all(pipe: Option<impl Read + Send + 'static>, done: mpsc::Sender<()>) -> Shared {
    let buf = Shared::default();
    let sink = buf.clone();
    std::thread::spawn(move || {
        let _done = done;
        let Some(mut pipe) = pipe else { return };
        let mut chunk = [0u8; 8192];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    if let (Ok(mut buf), Some(bytes)) = (sink.lock(), chunk.get(..n)) {
                        buf.extend_from_slice(bytes);
                    }
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    });
    buf
}

fn take(buf: &Shared) -> Vec<u8> {
    buf.lock()
        .map(|mut bytes| std::mem::take(&mut *bytes))
        .unwrap_or_default()
}

fn quiet(mut command: Command) {
    let _ = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// 직접 띄운 프로세스와 그 자손을 끝낸다.
#[cfg(windows)]
fn kill_tree(pid: u32) {
    // shortcut: Job Object는 unsafe FFI가 있어야 해서 taskkill로 트리를 끝낸다. 직접 띄운 프로세스가 먼저 끝난 뒤 남은 손자는 찾지 못한다, unsafe나 windows-sys를 허용하면 Job Object로 바꾼다
    let taskkill = std::env::var_os("SystemRoot")
        .map(|root| Path::new(&root).join("System32").join("taskkill.exe"))
        .filter(|path| path.is_file())
        .or_else(|| find_program("taskkill"));
    let Some(taskkill) = taskkill else { return };
    let mut command = Command::new(taskkill);
    command.args(["/T", "/F", "/PID", &pid.to_string()]);
    quiet(command);
}

/// 직접 띄운 프로세스와 그 자손을 끝낸다.
#[cfg(unix)]
fn kill_tree(pid: u32) {
    // 프로세스 그룹 ID가 직접 띄운 프로세스의 PID다(`process_group(0)`). 그 프로세스가 먼저 끝나도 그룹은 남는다.
    // libc 없이 신호를 보내려고 kill 명령을 쓴다
    let kill = find_program("kill").unwrap_or_else(|| "/bin/kill".into());
    let mut command = Command::new(kill);
    command.args(["-KILL", "--", &format!("-{pid}")]);
    quiet(command);
}

#[cfg(not(any(windows, unix)))]
fn kill_tree(_pid: u32) {}

/// 값 가운데 `-`로 시작하는 것이 있는가. 바깥 도구는 그런 값을 옵션으로 읽는다.
pub(crate) fn option_like(values: &[&str]) -> bool {
    values.iter().any(|v| v.starts_with('-'))
}

/// 찾은 결과를 `PATH` 값과 함께 기억한다. 한 명령이 git을 수십 번 부른다.
type Found = (
    Option<std::ffi::OsString>,
    BTreeMap<String, Option<PathBuf>>,
);
static FOUND: Mutex<Found> = Mutex::new((None, BTreeMap::new()));

/// PATH에서 실행 파일을 찾아 전체 경로를 돌려준다. Windows는 `PATHEXT`의 확장자를 붙여 본다
/// (`.cmd` 실행 파일은 찾은 전체 경로로 실행해야 한다).
/// 절대 경로가 아닌 PATH 항목(`.`, 빈 항목)은 보지 않는다. 그런 항목은 작업 디렉터리, 곧 레포에 놓인 파일을 띄운다.
pub fn find_program(name: &str) -> Option<PathBuf> {
    // 경로로 준 이름(`ORCA_CLI_COMMAND` 등)은 PATH를 보지 않는다
    if Path::new(name).components().count() > 1 {
        let path = PathBuf::from(name);
        return path.is_file().then_some(path);
    }
    let path = std::env::var_os("PATH")?;
    let mut found = FOUND.lock().ok()?;
    if found.0.as_ref() != Some(&path) {
        *found = (Some(path.clone()), BTreeMap::new());
    }
    if let Some(known) = found.1.get(name) {
        return known.clone();
    }
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
            .split(';')
            .filter(|e| !e.is_empty())
            .map(str::to_lowercase)
            .collect()
    } else {
        vec![String::new()]
    };
    let program = std::env::split_paths(&path)
        .filter(|dir| dir.is_absolute())
        .flat_map(|dir| {
            exts.iter()
                .map(move |ext| dir.join(format!("{name}{ext}")))
                .collect::<Vec<_>>()
        })
        .find(|candidate| runnable(candidate));
    found.1.insert(name.to_string(), program.clone());
    program
}

#[cfg(unix)]
fn runnable(file: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(file).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn runnable(file: &Path) -> bool {
    file.is_file()
}

/// `argv[0]`을 `cwd`에서 실행하고 끝나기를 기다린다. `env`는 물려받은 환경에 덧붙인다.
/// 제한 시간이 지나면 프로세스 트리를 끝내고 `TimedOut`을 돌려준다.
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
    // 이름만 준 실행 파일(git, orca, curl)은 PATH의 절대 경로 항목에서 찾은 전체 경로로 띄운다
    let program = match Path::new(program).components().count() {
        1 => match find_program(program) {
            Some(found) => found,
            None => return failed(ProcStatus::NotFound),
        },
        _ => PathBuf::from(program),
    };
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .envs(env.iter().copied())
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        // 제한 시간이 지나면 자손까지 끝낼 수 있게 자기 프로세스 그룹을 준다
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
    }
    let Ok(mut child) = command.spawn() else {
        return failed(ProcStatus::NotFound);
    };

    // 파이프가 차서 서로 멈추지 않도록 입출력을 스레드로 나눈다
    if let (Some(mut pipe), Some(bytes)) = (child.stdin.take(), stdin.map(<[u8]>::to_vec)) {
        std::thread::spawn(move || {
            let _ = pipe.write_all(&bytes);
        });
    }
    let (done, readers) = mpsc::channel::<()>();
    let out = read_all(child.stdout.take(), done.clone());
    let err = read_all(child.stderr.take(), done);
    // 읽는 스레드가 둘 다 끝났는가를 `limit`까지 기다려 본다
    let drained = |limit: Duration| {
        matches!(
            readers.recv_timeout(limit),
            Err(RecvTimeoutError::Disconnected)
        )
    };

    let deadline = Instant::now() + timeout;
    // 출력이 끝나는 것으로 종료를 안다(종료 상태를 폴링하는 것보다 빨리 깬다).
    // 직접 띄운 프로세스가 끝났는데 자손(백그라운드로 남는 데몬)이 파이프를 쥐고 있으면 출력이 끝나지 않는다.
    // 그래서 출력을 기다리는 사이사이 종료 상태도 본다. git·orca 호출이 남의 데몬 때문에 제한 시간까지 매달리지 않는다
    let (mut closed, mut until) = (false, deadline);
    loop {
        if !closed {
            let left = until.saturating_duration_since(Instant::now());
            closed = drained(left.min(EXIT_POLL));
            if closed {
                // 파이프를 닫고도 계속 도는 프로세스는 남은 시간 동안 기다린다. 막 끝나는 프로세스에는 짧은 여유를 준다
                until = deadline.max(Instant::now() + EXIT_GRACE);
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !closed {
                    // 직접 띄운 프로세스가 쓴 마지막 출력을 마저 읽는다. 자손의 출력은 기다리지 않는다
                    drained(EXIT_GRACE);
                }
                return ProcResult {
                    status: ProcStatus::Exited(status.code().unwrap_or(-1)),
                    stdout: take(&out),
                    stderr: take(&err),
                };
            }
            Ok(None) if Instant::now() < until => {
                if closed {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            _ => break,
        }
    }
    kill_tree(child.id());
    let _ = child.kill();
    let _ = child.wait();
    drained(KILL_WAIT);
    ProcResult {
        status: ProcStatus::TimedOut,
        stdout: take(&out),
        stderr: take(&err),
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
