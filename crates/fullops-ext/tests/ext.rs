//! 바깥 도구 계층의 통합 시험: `GitCli`는 임시 레포로, `OrcaCli`는 fake-orca와 `scenarios/fixtures/orca/`의 표본으로 본다.

use fullops_core::model::{DispatchId, Known, RunId, Sha};
use fullops_ext::*;
use fullops_testkit::{Sandbox, Setup, bin, orca_shapes};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn git_cli_reads_and_writes_a_real_repo() {
    let sb = Sandbox::with(&Setup::default());
    let (lead, dev) = (sb.path("lead"), sb.path("dev"));
    let (git, dev_git) = (GitCli::cli(lead.clone()), GitCli::cli(dev.clone()));
    let main = git.rev_parse("HEAD").yes().expect("HEAD");
    assert_eq!(main.as_str(), sb.git_ok(&lead, &["rev-parse", "HEAD"]));
    assert_eq!(git.rev_parse("refs/heads/none"), Known::Unknown("git"));

    let trees = git.worktrees().yes().unwrap();
    assert_eq!(
        trees
            .iter()
            .map(|w| w.branch.as_deref())
            .collect::<Vec<_>>(),
        [Some("main"), Some("fullops/dev")]
    );
    assert_eq!(trees[0].head, main);

    // 새 파일과 고친 파일만 골라 커밋한다. 다른 변경은 건드리지 않는다
    sb.write(&dev.join("src/a.py"), "print(1)\nprint(2)\n");
    sb.write(&dev.join("notes.txt"), "커밋하지 않는 파일\n");
    let paths = strings(&["src/a.py", "src/none.py"]);
    let commit = dev_git
        .commit_only(&paths, "A-1 구현")
        .unwrap()
        .expect("커밋");
    assert_eq!(
        dev_git.commit_only(&paths, "A-1 구현"),
        Ok(None),
        "바뀐 것이 없다"
    );
    assert_eq!(dev_git.commit_only(&[], "빈 목록"), Ok(None));
    let status = dev_git.status(None).yes().unwrap();
    assert_eq!(
        status
            .iter()
            .map(|e| (e.index, e.worktree, e.path.as_str()))
            .collect::<Vec<_>>(),
        [('?', '?', "notes.txt")]
    );
    assert!(dev_git.status(Some("src")).yes().unwrap().is_empty());

    assert_eq!(
        git.is_ancestor(main.as_str(), commit.as_str()),
        Known::Yes(true)
    );
    assert_eq!(
        git.is_ancestor(commit.as_str(), main.as_str()),
        Known::Yes(false)
    );
    assert_eq!(git.is_ancestor("none", "HEAD"), Known::Unknown("git"));
    assert_eq!(
        git.merge_base("main", "fullops/dev"),
        Known::Yes(main.clone())
    );
    assert_eq!(
        git.diff_names("main", "fullops/dev"),
        Known::Yes(strings(&["src/a.py"]))
    );
    let stat = git.numstat("main", "fullops/dev").yes().unwrap();
    assert_eq!(
        (stat[0].path.as_str(), stat[0].added, stat[0].deleted),
        ("src/a.py", 2, 0)
    );
    let names = git.name_status("main", "fullops/dev").yes().unwrap();
    assert_eq!(
        (names[0].status, names[0].path.as_str(), &names[0].old_path),
        ('A', "src/a.py", &None)
    );
    assert!(
        git.diff_u0("main", "fullops/dev")
            .yes()
            .unwrap()
            .contains("+print(2)")
    );

    // blob 묶음 조회와 탐색용 읽기
    let blobs = git
        .show_blobs(&[
            ("fullops/dev".to_string(), "src/a.py".to_string()),
            ("fullops/dev".to_string(), "src/none.py".to_string()),
            ("fullops/dev".to_string(), "src".to_string()),
        ])
        .yes()
        .unwrap();
    assert_eq!(blobs, [Some(b"print(1)\nprint(2)\n".to_vec()), None, None]);
    let tree = git.ls_tree("fullops/dev").yes().unwrap();
    let entry = tree.iter().find(|e| e.path == "src/a.py").expect("ls-tree");
    assert_eq!(
        (entry.mode.as_str(), entry.kind.as_str()),
        ("100644", "blob")
    );
    let prefixes = git
        .blob_prefixes(&[entry.oid.clone(), "0".repeat(40)], 5)
        .yes()
        .unwrap();
    assert_eq!(prefixes, [Some((18, b"print".to_vec())), None]);
    assert_eq!(
        git.grep_files("fullops/dev", &strings(&["print(2)", "없는 글"])),
        Known::Yes(strings(&["src/a.py"]))
    );
    assert_eq!(
        git.grep_files("fullops/dev", &strings(&["없는 글"])),
        Known::Yes(vec![])
    );
    assert_eq!(git.grep_files("fullops/dev", &[]), Known::Yes(vec![]));

    // 이름 변경: 대상 폴더를 만들어 주고, 옛 경로와 새 경로를 함께 커밋한다
    dev_git.mv("src/a.py", "src/deep/b.py").unwrap();
    let moved = dev_git
        .commit_only(&strings(&["src/a.py", "src/deep/b.py"]), "이름 변경")
        .unwrap()
        .expect("이름 변경 커밋");
    let names = git
        .name_status(commit.as_str(), moved.as_str())
        .yes()
        .unwrap();
    assert_eq!(
        (
            names[0].status,
            names[0].path.as_str(),
            names[0].old_path.as_deref()
        ),
        ('R', "src/deep/b.py", Some("src/a.py"))
    );
    assert_eq!(
        git.diff_names(commit.as_str(), moved.as_str())
            .yes()
            .unwrap()
            .len(),
        2
    );

    // push, fetch, 병합
    dev_git
        .push("origin", "HEAD:refs/heads/fullops/dev")
        .unwrap();
    git.fetch("origin", "fullops/dev").unwrap();
    assert!(dev_git.push("no-remote", "HEAD").is_err());
    assert_eq!(
        git.merge(&lead, "fullops/dev", MergeMode::FfOnly),
        Ok(MergeOutcome::Done)
    );
    assert_eq!(git.rev_parse("HEAD"), Known::Yes(moved.clone()));

    // 충돌: 병합이 진행 중으로 남고 abort로 되돌린다
    sb.write(&lead.join("src/deep/b.py"), "lead 쪽 변경\n");
    git.commit_only(&strings(&["src/deep/b.py"]), "lead 변경")
        .unwrap()
        .unwrap();
    sb.write(&dev.join("src/deep/b.py"), "dev 쪽 변경\n");
    dev_git
        .commit_only(&strings(&["src/deep/b.py"]), "dev 변경")
        .unwrap()
        .unwrap();
    assert!(
        git.merge(&lead, "fullops/dev", MergeMode::FfOnly).is_err(),
        "ff가 아니다"
    );
    assert_eq!(
        git.merge(&lead, "fullops/dev", MergeMode::NoEdit),
        Ok(MergeOutcome::Conflict)
    );
    git.merge_abort(&lead).unwrap();
    assert!(git.status(None).yes().unwrap().is_empty());
    git.reset_keep(&lead, moved.as_str()).unwrap();
    assert_eq!(git.rev_parse("HEAD"), Known::Yes(moved.clone()));

    // update-ref는 이전 값이 맞을 때만 바꾼다
    let zero = Sha::new("0".repeat(40)).unwrap();
    git.update_ref("refs/heads/spare", &main, &zero).unwrap();
    assert!(git.update_ref("refs/heads/spare", &moved, &commit).is_err());
    git.update_ref("refs/heads/spare", &moved, &main).unwrap();
    assert_eq!(git.rev_parse("refs/heads/spare"), Known::Yes(moved.clone()));

    // 리뷰 사본용 detached 워크트리
    let snapshot = sb.root.join("snapshot");
    git.worktree_add_detached(&snapshot, moved.as_str())
        .unwrap();
    let trees = git.worktrees().yes().unwrap();
    assert_eq!(
        (trees.len(), &trees[2].branch, &trees[2].head),
        (3, &None, &moved)
    );
    git.worktree_remove(&snapshot).unwrap();
    assert_eq!(git.worktrees().yes().unwrap().len(), 2);

    // 레포가 아닌 폴더에서는 모든 읽기가 Unknown이다
    let nowhere = GitCli::hook(sb.home.clone());
    assert_eq!(nowhere.rev_parse("HEAD"), Known::Unknown("git"));
    assert_eq!(nowhere.status(None), Known::Unknown("git"));
    assert!(nowhere.fetch("origin", "main").is_err());
}

// R2-1: 레포·설정에서 온 값(원격, 브랜치, ref, SHA)은 git의 옵션으로도, 경로·URL 원격으로도 읽히지 않는다
#[test]
fn git_cli_never_passes_values_as_options() {
    let sb = Sandbox::with(&Setup::default());
    let lead = sb.path("lead");
    let git = GitCli::cli(lead.clone());
    let evil = "--upload-pack=touch r2-marker";
    let push_evil = "--receive-pack=touch r2-marker";

    // 원격과 브랜치 자리
    assert!(git.fetch(evil, "main").is_err());
    assert!(git.fetch("origin", evil).is_err());
    assert!(git.push(evil, "main").is_err());
    assert!(git.push("origin", push_evil).is_err());
    assert!(git.push("origin", "+main").is_err(), "강제 push다");
    // 등록되지 않은 이름을 git은 경로로 읽는다. 레포에 놓인 폴더는 원격이 되지 못한다
    sb.git_ok(&lead, &["init", "-q", "--bare", "planted.git"]);
    for remote in [
        "planted.git",
        "./planted.git",
        "ext::sh -c touch% r2-marker",
    ] {
        assert!(git.fetch(remote, "main").is_err(), "{remote}");
        assert!(git.push(remote, "main").is_err(), "{remote}");
    }
    // 등록된 원격과 브랜치 이름은 그대로 된다(온전한 ref로 넘긴다)
    git.fetch("origin", "main").unwrap();
    git.push("origin", "main").unwrap();

    // 읽기는 모른다고 답하고 git을 부르지 않는다
    assert_eq!(git.is_ancestor(evil, "HEAD"), Known::Unknown("git_arg"));
    assert_eq!(git.is_ancestor("HEAD", evil), Known::Unknown("git_arg"));
    assert_eq!(git.merge_base("HEAD", "--all"), Known::Unknown("git_arg"));
    assert_eq!(git.rev_parse("--all"), Known::Unknown("git_arg"));
    assert_eq!(
        git.diff_names("--no-index", "HEAD"),
        Known::Unknown("git_arg")
    );
    assert_eq!(
        git.numstat("HEAD", "--output=r2-marker"),
        Known::Unknown("git_arg")
    );
    assert_eq!(
        git.name_status("--output=r2-marker", "HEAD"),
        Known::Unknown("git_arg")
    );
    assert_eq!(
        git.diff_u0("HEAD", "--output=r2-marker"),
        Known::Unknown("git_arg")
    );
    assert_eq!(git.ls_tree("--output=r2-marker"), Known::Unknown("git_arg"));
    let literal = strings(&["README"]);
    assert_eq!(
        git.grep_files("--open-files-in-pager=touch r2-marker", &literal),
        Known::Unknown("git_arg")
    );
    // 쓰기는 거절한다
    assert!(git.merge(&lead, "--no-verify", MergeMode::NoEdit).is_err());
    assert!(git.reset_keep(&lead, "--hard").is_err());
    let head = git.rev_parse("HEAD").yes().unwrap();
    assert!(git.update_ref("--stdin", &head, &head).is_err());
    assert!(
        git.worktree_add_detached(&sb.root.join("r2-snapshot"), "--force")
            .is_err()
    );
    assert!(!lead.join("r2-marker").exists() && !sb.root.join("r2-snapshot").exists());

    // `-`로 시작해도 옵션이 아닌 자리: grep의 리터럴, `--` 뒤의 경로
    assert_eq!(
        git.grep_files("HEAD", &strings(&["--no-such-text"])),
        Known::Yes(vec![])
    );
    sb.write(&lead.join("-dash.txt"), "이름이 -로 시작하는 파일\n");
    let paths = strings(&["-dash.txt"]);
    git.commit_only(&paths, "-m으로 시작하는 제목")
        .unwrap()
        .unwrap();
    git.mv("-dash.txt", "moved/-dash.txt").unwrap();
    assert!(lead.join("moved/-dash.txt").is_file());
    // 없는 rev가 같은 이름의 파일로 읽히지 않는다
    assert_eq!(git.diff_names("HEAD", "README.md"), Known::Unknown("git"));
}

// R2-7: 이름만 준 실행 파일은 PATH의 절대 경로 항목에서만 찾는다
#[test]
fn programs_are_found_only_in_absolute_path_entries() {
    let found = hosts::find_program("git").expect("PATH에 git이 있다");
    assert!(found.is_absolute());
    assert_eq!(hosts::find_program("fullops-no-such-program"), None);
    // 경로로 준 이름은 PATH를 보지 않는다
    let exe = found.to_string_lossy().into_owned();
    assert_eq!(hosts::find_program(&exe), Some(found));
    let cwd = std::env::temp_dir();
    let argv = strings(&["git", "--version"]);
    assert!(run(&argv, &cwd, Duration::from_secs(20), &[], None).ok());
}

// R2-1의 같은 원칙: 기록·응답에서 온 값이 `-`로 시작하면 Orca를 부르지 않는다
#[test]
fn orca_cli_never_passes_values_as_options() {
    let sb = Sandbox::with(&Setup::default());
    let orca = orca(&sb);
    let run_id: RunId = "--help".parse().unwrap();
    let dispatch: DispatchId = "--help".parse().unwrap();
    let good: DispatchId = "ctx_1".parse().unwrap();
    assert!(orca.run_create("--help").is_err());
    assert!(orca.check(Some(&run_id), CheckMode::Peek).is_err());
    assert!(
        orca.check(None, CheckMode::AckLegacy("--all".into()))
            .is_err()
    );
    assert!(orca.worker_show(&dispatch).is_err());
    assert!(orca.worker_list(&run_id).is_err());
    assert!(orca.run_use(&run_id).is_err());
    assert!(orca.send_escalation(&good, None, "제목", "--body").is_err());
    assert!(
        orca.send_escalation(&good, Some("--task"), "제목", "본문")
            .is_err()
    );
    let request = |agent: &str, model: Option<&str>, request: &str| StartRequest {
        run: "run_1".parse().unwrap(),
        spec: "spec".into(),
        worktree: sb.path("dev"),
        target: StartTarget::NewTerminal {
            agent: agent.into(),
            model: model.map(str::to_string),
            effort: None,
        },
        ready_timeout: Duration::from_secs(5),
        request: request.into(),
    };
    for req in [
        request("--agent", None, "req-1"),
        request("claude", Some("--model"), "req-1"),
        request("claude", None, "--retry"),
    ] {
        assert!(orca.worker_start(&req).is_err());
    }
    assert!(sb.orca_calls().is_empty(), "fake-orca가 호출을 받았다");
}

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scenarios/fixtures/orca")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// fake-orca를 환경 변수 없이 띄운다: 작업 디렉터리가 각본 폴더다.
fn orca(sb: &Sandbox) -> OrcaCli {
    OrcaCli {
        program: bin("fake-orca").to_string_lossy().into_owned(),
        cwd: sb.orca_dir.clone(),
        timeout: Duration::from_secs(30),
    }
}

fn reply(sb: &Sandbox, when: &[&str], stdout: Value, exit: u8) {
    sb.orca_rule(json!({ "when": when, "times": 1, "stdout": stdout, "exit": exit }));
}

fn last_argv(sb: &Sandbox) -> Vec<String> {
    let calls = sb.orca_calls();
    let argv = calls.last().expect("호출 기록")["argv"]
        .as_array()
        .unwrap()
        .clone();
    argv.iter()
        .map(|a| a.as_str().unwrap().to_string())
        .collect()
}

fn has(argv: &[String], window: &[&str]) -> bool {
    argv.windows(window.len()).any(|w| w == window)
}

#[test]
fn orca_cli_reads_the_fixture_shapes() {
    let sb = Sandbox::empty();
    let cli = orca(&sb);
    let run = RunId::new("run_0a1b2c3d4e5f").unwrap();
    let dispatch = DispatchId::new("ctx_2a3b4c5d6e7f").unwrap();

    reply(
        &sb,
        &["orchestration", "run-create"],
        fixture("run_create.json"),
        0,
    );
    assert_eq!(
        cli.run_create("fullops PAY-12: 결제 재시도"),
        Ok(run.clone())
    );
    let argv = last_argv(&sb);
    assert!(
        has(&argv, &["--objective", "fullops PAY-12: 결제 재시도"])
            && argv.last().unwrap() == "--json"
    );
    reply(
        &sb,
        &["orchestration", "run-use"],
        fixture("run_use.json"),
        0,
    );
    assert_eq!(cli.run_use(&run), Ok(()));
    assert!(has(&last_argv(&sb), &["run-use", "--id", run.as_str()]));

    // worker-start: 성공 여부는 종료 코드로 본다
    let request = |target, request: &str| StartRequest {
        run: run.clone(),
        spec: "Task key: PAY-12; Purpose: implementation".into(),
        worktree: sb.root.join("dev"),
        target,
        ready_timeout: Duration::from_secs(90),
        request: request.into(),
    };
    let new_terminal = StartTarget::NewTerminal {
        agent: "claude".into(),
        model: Some("m".into()),
        effort: Some("ultracode".into()),
    };
    reply(
        &sb,
        &["orchestration", "worker-start"],
        fixture("worker_start.ready.json"),
        0,
    );
    let StartOutcome::Started(started) = cli
        .worker_start(&request(new_terminal.clone(), "req_1"))
        .unwrap()
    else {
        panic!("ready는 성공이다");
    };
    assert_eq!(
        (
            started.dispatch.clone(),
            started.task.as_str(),
            started.run.clone()
        ),
        (dispatch.clone(), "task_1a2b3c4d5e6f", run.clone())
    );
    assert_eq!(
        (
            started.state.as_str(),
            started.terminal.is_some(),
            started.run_created
        ),
        ("ready", true, false)
    );
    let argv = last_argv(&sb);
    assert!(has(
        &argv,
        &[
            "orchestration",
            "worker-start",
            "--run",
            run.as_str(),
            "--spec-file",
            "-",
            "--worktree"
        ]
    ));
    assert!(
        argv.iter()
            .any(|a| a.starts_with("path:") && a.ends_with("/dev") && !a.contains('\\'))
    );
    assert!(has(
        &argv,
        &[
            "--agent",
            "claude",
            "--model",
            "m",
            "--effort",
            "ultracode",
            "--timeout-ms",
            "90000",
            "--retry-request",
            "req_1",
            "--json"
        ]
    ));
    assert!(
        !argv
            .iter()
            .any(|a| a == "--task" || a == "--spec" || a == "--parent")
    );
    assert_eq!(
        sb.orca_calls().last().unwrap()["stdin"],
        "Task key: PAY-12; Purpose: implementation"
    );
    // 같은 요청 ID로 다시 부르면 기록된 결과를 받는다(새 worker가 뜨지 않는다)
    let again = cli
        .worker_start(&request(new_terminal.clone(), "req_1"))
        .unwrap();
    assert_eq!(again, StartOutcome::Started(started));
    assert_eq!(sb.orca_calls().last().unwrap()["replayed"], true);

    reply(
        &sb,
        &["orchestration", "worker-start"],
        fixture("worker_start.unobserved.json"),
        0,
    );
    let existing = StartTarget::Existing {
        terminal: "term_9".into(),
    };
    assert!(matches!(
        cli.worker_start(&request(existing, "req_2")).unwrap(),
        StartOutcome::Started(Started { ref state, .. }) if state == "outcome_unknown"
    ));
    let argv = last_argv(&sb);
    assert!(
        has(&argv, &["--terminal", "term_9"])
            && !argv.iter().any(|a| a == "--agent" || a == "--model")
    );
    // 모델이 없으면 effort도 붙이지 않는다
    let bare = StartTarget::NewTerminal {
        agent: "antigravity".into(),
        model: None,
        effort: Some("high".into()),
    };
    reply(
        &sb,
        &["orchestration", "worker-start"],
        orca_shapes::worker_started("run_1", "ctx_1", "task_1", None),
        0,
    );
    assert!(matches!(
        cli.worker_start(&request(bare, "req_3")).unwrap(),
        StartOutcome::Started(Started { terminal: None, .. })
    ));
    let argv = last_argv(&sb);
    assert!(
        has(&argv, &["--agent", "antigravity", "--timeout-ms"])
            && !argv.iter().any(|a| a == "--effort")
    );

    reply(
        &sb,
        &["orchestration", "worker-start"],
        fixture("worker_start.failed.json"),
        1,
    );
    let StartOutcome::Failed(failure) = cli
        .worker_start(&request(new_terminal.clone(), "req_4"))
        .unwrap()
    else {
        panic!("result가 있는 실패다");
    };
    assert_eq!(
        (
            failure.dispatch,
            failure.failed_stage.as_deref(),
            failure.residual.len()
        ),
        (Some(dispatch.clone()), Some("terminal_readying"), 1)
    );
    assert!(failure.last_error.is_some() && failure.next.is_some());
    // Orca가 error로 거절하면 시작되지 않은 것이 확실한 실패다
    reply(
        &sb,
        &["orchestration", "worker-start"],
        fixture("error.json"),
        1,
    );
    assert!(matches!(
        cli.worker_start(&request(new_terminal.clone(), "req_5")),
        Err(ExtError::Failed { code: 1, ref stderr }) if stderr.starts_with("invalid_argument")
    ));
    // 결과를 모른다: JSON이 아닌 출력, 필수 필드가 없는 영수증
    reply(
        &sb,
        &["orchestration", "worker-start"],
        json!("연결이 끊겼다"),
        0,
    );
    assert!(matches!(
        cli.worker_start(&request(new_terminal.clone(), "req_6")),
        Ok(StartOutcome::Unknown(_))
    ));
    reply(
        &sb,
        &["orchestration", "worker-start"],
        json!({"ok": true, "result": {"runId": "run_1"}}),
        0,
    );
    assert!(matches!(
        cli.worker_start(&request(new_terminal, "req_7")),
        Ok(StartOutcome::Unknown(_))
    ));

    // check: 메시지 한 건의 해석
    reply(
        &sb,
        &["orchestration", "check"],
        fixture("check.mixed.json"),
        0,
    );
    let mixed = cli.check(Some(&run), CheckMode::Peek).unwrap();
    assert!(has(
        &last_argv(&sb),
        &["check", "--run", run.as_str(), "--peek", "--json"]
    ));
    let kinds: Vec<&str> = mixed.messages.iter().map(|m| m.kind.as_str()).collect();
    assert_eq!(
        kinds,
        [
            "heartbeat",
            "status",
            "question",
            "escalation",
            "worker_done"
        ]
    );
    assert_eq!(
        (
            mixed.messages[0].body.as_deref(),
            mixed.messages[1].dispatch_id.as_ref()
        ),
        (None, None)
    );
    assert!(
        mixed.messages[3]
            .subject
            .as_deref()
            .unwrap()
            .starts_with("[fullops:limit]")
    );
    let done = &mixed.messages[4];
    assert_eq!(
        (
            done.id.as_str(),
            done.dispatch_id.as_ref(),
            done.task_id.as_deref(),
            done.outcome.as_deref()
        ),
        (
            "msg_3a4b5c6d7e8f",
            Some(&dispatch),
            Some("task_1a2b3c4d5e6f"),
            Some("succeeded")
        )
    );
    assert_eq!(
        (
            done.from.as_deref(),
            mixed.delivery_id.as_deref(),
            mixed.timed_out
        ),
        (Some("ctx_2a3b4c5d6e7f"), None, false)
    );
    reply(
        &sb,
        &["orchestration", "check"],
        fixture("check.worker_done_failed.json"),
        0,
    );
    assert_eq!(
        cli.check(None, CheckMode::Consume).unwrap().messages[0]
            .outcome
            .as_deref(),
        Some("failed")
    );
    let argv = last_argv(&sb);
    assert!(!argv.iter().any(|a| a == "--run" || a == "--peek"));
    reply(
        &sb,
        &["orchestration", "check"],
        fixture("check.legacy_delivery.json"),
        0,
    );
    assert_eq!(
        cli.check(Some(&run), CheckMode::Consume)
            .unwrap()
            .delivery_id
            .as_deref(),
        Some("delivery_4a5b6c7d8e9f")
    );
    reply(
        &sb,
        &["orchestration", "check"],
        fixture("check.timed_out.json"),
        0,
    );
    let waited = cli
        .check(
            Some(&run),
            CheckMode::PeekWait {
                types: &["worker_done", "question"],
                timeout: Duration::from_secs(5),
            },
        )
        .unwrap();
    assert!(waited.timed_out && waited.messages.is_empty());
    assert!(has(
        &last_argv(&sb),
        &[
            "--peek",
            "--wait",
            "--types",
            "worker_done,question",
            "--timeout-ms",
            "5000",
            "--json"
        ]
    ));
    reply(
        &sb,
        &["orchestration", "check"],
        fixture("check.empty.json"),
        0,
    );
    assert!(
        cli.check(Some(&run), CheckMode::AckLegacy("delivery_1".into()))
            .unwrap()
            .messages
            .is_empty()
    );
    assert!(has(&last_argv(&sb), &["--ack", "delivery_1"]));
    // payload가 문자열이면 세 값은 없는 것으로 본다. 취소와 연결 끊김, 오류는 실패다
    reply(
        &sb,
        &["orchestration", "check"],
        json!({"ok": true, "result": {"messages": [{"id": "msg_1", "payload": "글"}]}}),
        0,
    );
    let odd = cli.check(Some(&run), CheckMode::Peek).unwrap();
    assert_eq!(
        (
            odd.messages[0].kind.as_str(),
            odd.messages[0].dispatch_id.as_ref()
        ),
        ("status", None)
    );
    reply(
        &sb,
        &["orchestration", "check"],
        json!({"ok": true, "result": {"messages": [], "connectionLost": true}}),
        0,
    );
    assert!(cli.check(Some(&run), CheckMode::Peek).is_err());
    reply(&sb, &["orchestration", "check"], fixture("error.json"), 1);
    assert!(cli.check(Some(&run), CheckMode::Peek).is_err());
    reply(
        &sb,
        &["orchestration", "check"],
        json!({"ok": true, "result": {"messages": [{"type": "status"}]}}),
        0,
    );
    assert!(
        matches!(
            cli.check(Some(&run), CheckMode::Peek),
            Err(ExtError::Parse(_))
        ),
        "id가 없는 메시지"
    );

    // 우편함: --peek는 읽음으로 바꾸지 않고 소비형은 바꾼다
    sb.orca_mail(
        run.as_str(),
        orca_shapes::message(
            "msg_9",
            "worker_done",
            "ctx_1",
            "task_1",
            "완료",
            "[완료] A-1",
            Some("succeeded"),
        ),
    );
    sb.orca_mail(
        "run_other",
        orca_shapes::message("msg_8", "question", "ctx_2", "task_2", "질문", "?", None),
    );
    for _ in 0..2 {
        assert_eq!(
            cli.check(Some(&run), CheckMode::Peek)
                .unwrap()
                .messages
                .len(),
            1
        );
    }
    assert_eq!(
        cli.check(Some(&run), CheckMode::Consume).unwrap().messages[0]
            .id
            .as_str(),
        "msg_9"
    );
    assert!(
        cli.check(Some(&run), CheckMode::Consume)
            .unwrap()
            .messages
            .is_empty()
    );
    let idle = cli
        .check(
            Some(&run),
            CheckMode::PeekWait {
                types: &["worker_done"],
                timeout: Duration::from_secs(1),
            },
        )
        .unwrap();
    assert!(idle.timed_out);

    // worker-show와 worker-list: 정산 판정과 liveness의 첫 낱말
    for (name, settled, liveness) in [
        ("live", false, "live"),
        ("settled", true, "live"),
        ("exited", false, "exited"),
    ] {
        reply(
            &sb,
            &["orchestration", "worker-show"],
            fixture(&format!("worker_show.{name}.json")),
            0,
        );
        let view = cli.worker_show(&dispatch).unwrap();
        assert_eq!(
            (
                view.settled(),
                view.liveness.as_str(),
                view.outcome.as_deref()
            ),
            (settled, liveness, None),
            "{name}"
        );
        assert!(view.dispatch == dispatch && view.terminal.is_some() && view.stage.is_some());
    }
    assert!(has(
        &last_argv(&sb),
        &["worker-show", "--dispatch", dispatch.as_str()]
    ));
    let unverifiable = orca_shapes::worker(
        "run_1",
        "ctx_1",
        "task_1",
        "term_1",
        false,
        "unverifiable (terminal gone)",
    );
    reply(
        &sb,
        &["orchestration", "worker-show"],
        json!({"ok": true, "result": unverifiable}),
        0,
    );
    assert_eq!(cli.worker_show(&dispatch).unwrap().liveness, "unverifiable");
    reply(
        &sb,
        &["orchestration", "worker-show"],
        json!({"ok": true, "result": {"state": "ready"}}),
        0,
    );
    assert!(matches!(
        cli.worker_show(&dispatch),
        Err(ExtError::Parse(_))
    ));

    let mut first_page = fixture("worker_list.json");
    first_page["result"]["nextCursor"] = json!("cursor_2");
    reply(&sb, &["orchestration", "worker-list"], first_page, 0);
    reply(
        &sb,
        &["orchestration", "worker-list", "--cursor", "cursor_2"],
        fixture("worker_list.json"),
        0,
    );
    let workers = cli.worker_list(&run).unwrap();
    assert_eq!(workers.len(), 4, "nextCursor가 있으면 이어 읽는다");
    assert_eq!(
        (
            workers[0].settled(),
            workers[0].failed(),
            workers[0].stage.as_deref()
        ),
        (true, false, None)
    );
    assert_eq!(
        (workers[1].settled(), workers[1].liveness.as_str()),
        (false, "exited")
    );

    reply(
        &sb,
        &["orchestration", "send"],
        fixture("send.escalation.json"),
        0,
    );
    assert_eq!(
        cli.send_escalation(
            &dispatch,
            Some("task_1"),
            "[fullops:limit] worker 턴이 API 오류로 끝남",
            "rate_limit: 429"
        ),
        Ok(())
    );
    let argv = last_argv(&sb);
    assert!(has(
        &argv,
        &["orchestration", "send", "--type", "escalation", "--subject"]
    ));
    assert!(has(
        &argv,
        &[
            "--dispatch-id",
            dispatch.as_str(),
            "--task-id",
            "task_1",
            "--json"
        ]
    ));
    reply(
        &sb,
        &["orchestration", "send"],
        fixture("send.escalation.json"),
        0,
    );
    cli.send_escalation(&dispatch, None, "제목", "본문")
        .unwrap();
    assert!(!last_argv(&sb).iter().any(|a| a == "--task-id"));

    // terminal 명령은 여러 줄 형태다. connected인 터미널만 센다
    let mut terminals = fixture("terminal_list.json");
    let gone = json!({"handle": "term_gone", "connected": false, "worktreePath": "/x"});
    terminals["result"]["terminals"]
        .as_array_mut()
        .unwrap()
        .push(gone);
    sb.orca_rule(json!({ "when": ["terminal", "list"], "times": 1, "stdout": serde_json::to_string_pretty(&terminals).unwrap() }));
    assert_eq!(
        cli.terminals_in(&sb.root.join("dev")),
        Ok(strings(&["term_00000000-0000-4000-8000-000000000002"]))
    );
    let argv = last_argv(&sb);
    assert!(
        has(&argv, &["terminal", "list", "--worktree"])
            && argv.iter().any(|a| a.starts_with("path:"))
    );

    // 실행 파일이 없으면 다른 실행 파일로 바꾸지 않는다
    let missing = OrcaCli {
        program: "fullops-no-such-orca".into(),
        ..orca(&sb)
    };
    assert_eq!(missing.run_create("x"), Err(ExtError::NotFound));
    assert_eq!(missing.worker_show(&dispatch), Err(ExtError::NotFound));
    // 제한 시간을 넘긴 worker-start는 결과를 모르는 것이다
    sb.orca_rule(json!({ "when": ["orchestration", "run-use"], "sleep_ms": 3000, "stdout": fixture("run_use.json") }));
    let hasty = OrcaCli {
        timeout: Duration::from_millis(200),
        ..orca(&sb)
    };
    assert_eq!(hasty.run_use(&run), Err(ExtError::TimedOut));
}

#[test]
fn judge_stub_and_script_judge() {
    use fullops_testkit::ScriptJudge;
    let questions = [Question {
        id: "scope".into(),
        kind: QuestionKind::Choice {
            options: strings(&["direct", "design"]),
        },
        instructions: "분류".into(),
    }];
    // `JevCurl`은 묶음 C의 구현으로 바뀌어 여기서 부르지 않는다(환경의 실제 키로 나가지 않게 한다).
    // fake-curl을 타는 시험은 `crates/fullops/tests/flow.rs`에 있다.

    let judge = ScriptJudge::new(vec![
        json!({"answers": [{"id": "scope", "choice": "direct", "probabilities": {"direct": 0.9, "design": 0.1}}, {"id": "relevant", "value": 0.8}],
               "usage": {"input_tokens": 10, "output_tokens": 2, "cost": 0.001}, "model": "jev-test"}),
        json!({"error": "no_key"}),
    ]);
    let judged = judge.ask("{\"task\":\"x\"}", &questions).unwrap();
    assert!(
        matches!(&judged.answers[0], Answer::Choice { choice, probabilities, .. } if choice == "direct" && probabilities["direct"] == 0.9)
    );
    assert_eq!(
        judged.answers[1],
        Answer::Noul {
            id: "relevant".into(),
            value: 0.8
        }
    );
    assert_eq!(
        (
            judged.usage.input_tokens,
            judged.usage.cost,
            judged.model.as_str(),
            judged.cached
        ),
        (10, Some(0.001), "jev-test", false)
    );
    assert_eq!(judge.ask("{}", &questions), Err(JudgeError::NoKey));
    assert_eq!(
        judge.ask("{}", &questions),
        Err(JudgeError::Offline),
        "각본을 다 쓰면 오프라인이다"
    );
    assert_eq!(judge.asked.borrow().len(), 3);
    assert_eq!(judge.asked.borrow()[0].1, ["scope"]);
}
