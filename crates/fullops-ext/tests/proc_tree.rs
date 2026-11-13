//! 제한 시간이 지나면 `run`이 프로세스 트리를 끝내는지 본다(설계 8.11절 8번).
//! 시험 실행 파일이 자기 자신을 부모와 손자 역할로 다시 띄운다. 셸이나 다른 도구에 기대지 않는다.

use fullops_ext::{ProcStatus, run};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const ROLE: &str = "FULLOPS_PROC_TREE_ROLE";
const DIR: &str = "FULLOPS_PROC_TREE_DIR";
/// 손자가 뜬 뒤 이만큼 살아남으면 표식을 남긴다.
const GRANDCHILD_LIVES: Duration = Duration::from_secs(6);

fn self_argv() -> Vec<String> {
    let exe = std::env::current_exe().expect("시험 실행 파일 경로");
    [
        exe.to_string_lossy().as_ref(),
        "--exact",
        "tree_role",
        "--nocapture",
    ]
    .map(String::from)
    .to_vec()
}

/// 다시 띄워진 쪽의 몸통. 평소의 시험 실행에서는 환경 변수가 없어 바로 끝난다.
#[test]
fn tree_role() {
    let (Ok(role), Some(dir)) = (
        std::env::var(ROLE),
        std::env::var_os(DIR).map(PathBuf::from),
    ) else {
        return;
    };
    if role == "parent" || role == "launcher" {
        // 손자는 출력 파이프를 물려받는다. 부모는 끝내질 때까지 기다리고, launcher는 손자를 남기고 바로 끝난다
        let argv = self_argv();
        let mut child = Command::new(&argv[0])
            .args(&argv[1..])
            .env(ROLE, "child")
            .spawn()
            .expect("손자 띄우기");
        if role == "parent" {
            let _ = child.wait();
        }
    } else {
        std::fs::write(dir.join("started"), "").expect("표식 쓰기");
        std::thread::sleep(GRANDCHILD_LIVES);
        std::fs::write(dir.join("survived"), "").expect("표식 쓰기");
    }
}

#[test]
fn timeout_kills_the_whole_tree() {
    let dir = std::env::temp_dir().join(format!("fullops-proc-tree-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("임시 폴더");
    let dir_text = dir.to_string_lossy().into_owned();

    let done = run(
        &self_argv(),
        &dir,
        Duration::from_secs(4),
        &[(ROLE, "parent"), (DIR, &dir_text)],
        None,
    );
    assert_eq!(done.status, ProcStatus::TimedOut);
    // 끝내기 전까지 나온 출력이 남는다(libtest의 머리 줄)
    assert!(done.stdout_text().contains("running 1 test"), "{done:?}");
    assert!(
        dir.join("started").exists(),
        "손자 프로세스가 제한 시간 안에 뜨지 못했다(부하가 크면 다시 돌린다)"
    );
    // 손자는 `run`이 돌아오기 전에 떴으므로, 살아 있다면 이 시간 안에 표식을 남긴다
    std::thread::sleep(GRANDCHILD_LIVES + Duration::from_secs(2));
    assert!(
        !dir.join("survived").exists(),
        "제한 시간이 지났는데 손자 프로세스가 살아남았다"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 직접 띄운 프로세스가 끝났으면, 파이프를 쥔 채 남은 자손(백그라운드 데몬)을 기다리지 않는다.
#[test]
fn exit_is_reported_while_a_daemon_still_holds_the_pipes() {
    let dir = std::env::temp_dir().join(format!("fullops-proc-daemon-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("임시 폴더");
    let dir_text = dir.to_string_lossy().into_owned();

    let done = run(
        &self_argv(),
        &dir,
        Duration::from_secs(120),
        &[(ROLE, "launcher"), (DIR, &dir_text)],
        None,
    );
    let daemon_alive = !dir.join("survived").exists();
    assert_eq!(done.status, ProcStatus::Exited(0), "{done:?}");
    assert!(done.stdout_text().contains("running 1 test"), "{done:?}");
    assert!(daemon_alive, "손자 프로세스가 끝날 때까지 기다렸다");
    // 손자가 끝난 뒤에 임시 폴더를 지운다(Windows는 살아 있는 프로세스의 작업 폴더를 지우지 못한다)
    for _ in 0..600 {
        if dir.join("survived").exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_millis(300));
    let _ = std::fs::remove_dir_all(&dir);
}
