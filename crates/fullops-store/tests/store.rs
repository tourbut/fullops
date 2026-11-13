//! 저장 계층의 통합 시험: 레포 찾기, 설정 검증, 상태 파일의 형식·동시 갱신·복구, 세션 파일.

use fullops_core::config::PolicyKind;
use fullops_core::model::*;
use fullops_store::*;
use fullops_testkit::{Sandbox, Setup};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const SHA_A: &str = "4f0c1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b";
const SHA_B: &str = "9c1e0aa4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0";

fn same(a: &Path, b: &Path) -> bool {
    same_path(&path_text(a), &path_text(b))
}

/// git 없이 만든 체크아웃 하나. 상태 파일 시험에는 Git 디렉터리의 자리만 있으면 된다.
fn bare_repo(dir: &Path) -> Repo {
    let repo = Repo {
        root: dir.join("work"),
        git_dir: dir.join("git"),
        common_dir: dir.join("git"),
    };
    std::fs::create_dir_all(repo.squad()).unwrap();
    std::fs::create_dir_all(&repo.git_dir).unwrap();
    std::fs::write(repo.config_path(), Setup::default().config_toml()).unwrap();
    repo
}

#[test]
fn discover_handles_root_subfolder_worktree_and_inactive_repo() {
    let sb = Sandbox::with(&Setup::default());
    let (lead, dev) = (sb.path("lead"), sb.path("dev"));

    let main = discover(&lead).unwrap().expect("주 체크아웃은 활성 레포다");
    assert!(
        same(&main.root, &lead)
            && same(&main.git_dir, &lead.join(".git"))
            && same(&main.common_dir, &lead.join(".git"))
    );
    assert_eq!(head(&main).unwrap(), HeadRef::Branch("main".into()));

    // 하위 폴더와 아직 없는 경로에서도 같은 레포를 찾는다
    std::fs::create_dir_all(lead.join("src/deep")).unwrap();
    assert_eq!(
        discover(&lead.join("src/deep")).unwrap(),
        Some(main.clone())
    );
    assert_eq!(
        discover(&lead.join("src/deep/../none/x.rs")).unwrap(),
        Some(main.clone())
    );

    // 워크트리는 자기 Git 디렉터리와 공용 디렉터리를 따로 가진다
    let worktree = discover(&dev)
        .unwrap()
        .expect("역할 워크트리는 활성 레포다");
    assert!(same(&worktree.root, &dev) && same(&worktree.common_dir, &main.common_dir));
    assert!(same(&worktree.git_dir, &lead.join(".git/worktrees/dev")));
    assert!(same(&worktree.shared(), &main.shared()) && !same(&worktree.local(), &main.local()));
    assert_eq!(
        head(&worktree).unwrap(),
        HeadRef::Branch("fullops/dev".into())
    );
    sb.git_ok(&dev, &["checkout", "-q", "--detach"]);
    let tip = Sha::new(sb.git_ok(&dev, &["rev-parse", "HEAD"])).unwrap();
    assert_eq!(head(&worktree).unwrap(), HeadRef::Detached(tip));

    // 표식 없는 레포와 Git 레포가 아닌 폴더는 활성 레포가 아니다
    let plain = sb.init_repo("plain");
    assert_eq!(discover(&plain).unwrap(), None);
    assert_eq!(discover(&sb.home).unwrap(), None);
    // 활성 레포 안의 다른 레포(하위 모듈 등)는 바깥 표식을 물려받지 않는다
    std::fs::create_dir_all(lead.join("vendor/inner/.git")).unwrap();
    assert_eq!(discover(&lead.join("vendor/inner")).unwrap(), None);
}

#[test]
fn config_validation_follows_design() {
    let valid = "schema = 2\napplied_version = \"2.0.0\"\nmode = \"coor\"\nrouting = \"legacy\"\ntest_level = \"lite\"\n\
subagent_level = \"lite\"\nfuture_key = 1\n\n[git]\nbase = \"main\"\nremote = \"origin\"\n\n[roles.dev]\nbranch = \"fullops/dev\"\n\
kind = \"worker\"\nabout = \"서버 코드\"\n\n[roles.architecture]\nbranch = \"fullops/architecture\"\nkind = \"design\"\n";
    let config = parse_config(valid).expect("모르는 키는 무시한다");
    assert_eq!(
        (config.mode, config.base.as_str(), config.remote.as_deref()),
        (Mode::Coor, "main", Some("origin"))
    );
    assert_eq!(config.base_ref(), "refs/remotes/origin/main");
    assert_eq!(
        (
            config.review.min_code_lines,
            config.review.min_code_files,
            config.review.auto
        ),
        (400, 15, false)
    );
    assert_eq!(
        (
            config.wait.check_minutes,
            config.wait.ready_timeout_seconds,
            config.wait.log_limit
        ),
        (60, 90, 30)
    );
    assert!(config.code.extensions.iter().any(|e| e == ".rs"));
    assert_eq!(
        config.design_role().map(RoleId::as_str),
        Some("architecture")
    );

    let dev = valid.replace(
        "mode = \"coor\"",
        "mode = \"dev\"\nprimary_role = \"dev\"\nprimary_branch = \"main\"",
    );
    assert_eq!(
        parse_config(&dev).unwrap().mode_tuple(),
        (Mode::Dev, RoleId::new("dev").ok(), Some("main".into()))
    );
    let split = valid.replace(
        "routing = \"legacy\"",
        "routing = \"split\"\ntech_plan_role = \"dev\"",
    );
    assert_eq!(parse_config(&split).unwrap().routing, Routing::Split);

    let invalid = [
        (
            "schema가 2가 아님",
            valid.replace("schema = 2", "schema = 1"),
        ),
        (
            "역할 브랜치 중복",
            valid.replace("fullops/architecture", "fullops/dev"),
        ),
        (
            "design 역할 둘",
            valid.replace("kind = \"worker\"", "kind = \"design\""),
        ),
        (
            "dev 모드에 primary_role 없음",
            valid.replace(
                "mode = \"coor\"",
                "mode = \"dev\"\nprimary_branch = \"main\"",
            ),
        ),
        (
            "dev 모드에 primary_branch 없음",
            valid.replace("mode = \"coor\"", "mode = \"dev\"\nprimary_role = \"dev\""),
        ),
        (
            "primary_role이 등록 역할이 아님",
            dev.replace("primary_role = \"dev\"", "primary_role = \"art\""),
        ),
        (
            "split에 tech_plan_role 없음",
            valid.replace("routing = \"legacy\"", "routing = \"split\""),
        ),
        (
            "tech_plan_role이 design 역할",
            split.replace(
                "tech_plan_role = \"dev\"",
                "tech_plan_role = \"architecture\"",
            ),
        ),
    ];
    for (what, text) in invalid {
        assert_eq!(
            parse_config(&text).map_err(|e| e.kind),
            Err(PolicyKind::Invalid),
            "{what}"
        );
    }
    let unreadable = [
        ("TOML이 아님", "schema = = 2".to_string()),
        (
            "모르는 mode 값",
            valid.replace("mode = \"coor\"", "mode = \"auto\""),
        ),
        ("필수 키 없음", valid.replace("test_level = \"lite\"\n", "")),
        (
            "[git] 없음",
            valid.replace("[git]\nbase = \"main\"\nremote = \"origin\"\n", ""),
        ),
        ("역할 ID 문법", valid.replace("[roles.dev]", "[roles.Dev]")),
    ];
    for (what, text) in unreadable {
        assert_eq!(
            parse_config(&text).map_err(|e| e.kind),
            Err(PolicyKind::Unreadable),
            "{what}"
        );
    }

    // 작업 트리의 파일을 읽는 쪽도 같은 판정을 낸다
    let sb = Sandbox::with(&Setup::default());
    let repo = discover(&sb.path("lead")).unwrap().unwrap();
    assert!(load_config(&repo).is_ok());
    std::fs::write(repo.config_path(), "schema = 3\n").unwrap();
    assert_eq!(
        load_config(&repo).map_err(|e| e.kind),
        Err(PolicyKind::Unreadable)
    );
    assert_eq!(
        discover(&sb.path("lead")).unwrap().as_ref(),
        Some(&repo),
        "깨진 설정도 활성 표식이다"
    );
}

/// 설계 6.2.1절의 예시에 모르는 필드를 더한 것.
fn design_state() -> Value {
    json!({
      "v": 1, "seq": 42,
      "runs": [ { "id": "run_ab12", "owner": "lead", "opened_at": 1794500000, "closed_at": null } ],
      "dispatches": [ {
          "id": "dsp_01", "task_id": "task_9", "run": "run_ab12", "key": "PAY-12", "role": "dev",
          "purpose": "build", "tuple": { "agent": "claude", "model": "m", "effort": "medium" },
          "terminal": "term_7", "worktree": "C:/repo/dev", "session_mode": "fresh", "reason": null,
          "started_at": 1794500100, "bound_session": null,
          "reported": { "branch": "fullops/dev", "sha": SHA_A, "lint_head": SHA_A,
                        "followups": [ { "role": "tester", "text": "독립 QA" } ], "at": 1794503000 },
          "settled": { "how": "done", "outcome": "succeeded", "message": "msg_3", "at": 1794503050 },
          "limit_escalated": false, "future_dispatch_field": { "nested": [1, 2.5, "x"] } } ],
      "completions": [ {
          "id": "msg_3", "dispatch": "dsp_01", "key": "PAY-12", "role": "dev", "branch": "fullops/dev",
          "sha": SHA_A, "sha_source": "done", "direct": false, "outcome": "succeeded",
          "received_at": 1794503050, "hold": null, "released_hold": null,
          "landed_at": null, "drift": null } ],
      "messages": [ { "id": "msg_3", "delivery": "dlv_8", "run": "run_ab12", "type": "worker_done",
                      "dispatch": "dsp_01", "received_at": 1794503050, "acked_at": null } ],
      "tasks": { "PAY-12": { "rounds": 0, "budget": 2, "limit_redispatch": 0,
                             "last_tuple": { "agent": "claude", "model": "m", "effort": "medium" } } },
      "sync_pending": { "art": SHA_B },
      "snapshots": [ { "review": "PAY-12@4f0c1a2b3c4d", "path": "C:/repo/.fullops-review-7f3a", "head": SHA_A,
                       "base": SHA_B, "state": "active", "implementer_session": "sid-1", "reviewer_dispatch": null,
                       "owner": "lead", "created_at": 1794504000, "hold": null } ],
      "decisions": [ { "id": "D001", "status": "open", "key": "PAY-12", "question": "리뷰할까?", "options": ["진행", "생략"],
                       "blocks": ["PAY-12"], "asked_at": 1794504100, "origin": "user", "sha": SHA_A,
                       "risks": [], "allow": [], "expires_at": 1794590500,
                       "answer": null, "decided_by": null, "via": null, "closed_at": null } ],
      "next_decision": 2,
      "future_top_field": true
    })
}

#[test]
fn state_keeps_design_format_and_unknown_fields() {
    let sb = Sandbox::empty();
    let repo = bare_repo(&sb.root);
    assert_eq!(
        read_state(&repo),
        Known::Yes(State::default()),
        "파일이 없으면 빈 상태다"
    );

    std::fs::create_dir_all(repo.shared()).unwrap();
    std::fs::write(repo.state_path(), design_state().to_string()).unwrap();
    let state = read_state(&repo)
        .yes()
        .expect("설계의 예시를 그대로 읽는다");
    assert_eq!(state.runs[0].owner, RunOwner::Lead);
    assert_eq!(state.pending().count(), 1);
    assert_eq!(state.snapshots[0].key(), "PAY-12");

    // 아무것도 고치지 않고 다시 쓰면 seq만 달라진다
    update_state(&repo, CLI_WAIT, None, |_| ()).unwrap();
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(repo.state_path()).unwrap()).unwrap();
    let mut expected = design_state();
    expected["seq"] = json!(43);
    assert_eq!(written, expected);

    // status.json은 같은 잠금 안에서 다시 만든다
    let plan = fullops_core::parse_plan(
        "| 과제 키 | 목표 | 담당 | 상태 | 지시서·원천 링크 |\n|---|---|---|---|---|\n| PAY-13 | 영수증 | art | 대기 | 지시 |\n",
        "",
    );
    update_state(&repo, CLI_WAIT, Some(&plan), |_| ()).unwrap();
    let status: Status = read_json(&repo.status_path())
        .yes()
        .flatten()
        .expect("status.json");
    assert_eq!(
        (status.mode, status.version.as_str()),
        (Some(Mode::Coor), env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        (
            status.tasks[0].key.as_str(),
            status.tasks[0].stage,
            status.tasks[0].sha.as_deref()
        ),
        ("PAY-12", Stage::Reviewing, Some("4f0c1a2"))
    );
    assert_eq!(
        (
            status.pending.len(),
            status.decisions.len(),
            status.dispatches[0].settled
        ),
        (1, 1, true)
    );
    assert_eq!(status.ready, [TaskKey::new("PAY-13").unwrap()]);
    assert_eq!(
        status.sync_pending.values().next().map(String::as_str),
        Some("9c1e0aa")
    );
    // lead 문맥이 아닌 쓰기는 ready의 이전 값을 유지한다
    update_state(&repo, CLI_WAIT, None, |_| ()).unwrap();
    let status: Status = read_json(&repo.status_path()).yes().flatten().unwrap();
    assert_eq!(status.ready.len(), 1);
}

#[test]
fn broken_or_newer_state_is_unknown_and_recoverable() {
    let sb = Sandbox::empty();
    let repo = bare_repo(&sb.root);
    std::fs::create_dir_all(repo.shared()).unwrap();

    std::fs::write(repo.state_path(), r#"{"v": 99, "runs": "새 형식"}"#).unwrap();
    assert_eq!(read_state(&repo), Known::Unknown("state_newer"));
    assert!(matches!(
        update_state(&repo, CLI_WAIT, None, |_| ()),
        Err(StoreError::Corrupt("state_newer"))
    ));
    assert!(
        std::fs::read_to_string(repo.state_path())
            .unwrap()
            .contains("99"),
        "새 형식의 파일은 건드리지 않는다"
    );

    std::fs::write(repo.state_path(), "{ 쓰다 만").unwrap();
    assert_eq!(read_state(&repo), Known::Unknown("state_broken"));
    assert!(matches!(
        update_state(&repo, CLI_WAIT, None, |_| ()),
        Err(StoreError::Corrupt("state_broken"))
    ));
    let moved = quarantine_state(&repo, CLI_WAIT).unwrap();
    assert!(
        moved
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("state.json.broken-")
    );
    assert_eq!(std::fs::read_to_string(moved).unwrap(), "{ 쓰다 만");
    assert_eq!(read_state(&repo), Known::Yes(State::default()));
}

/// 다른 프로세스로 도는 부분이다. 부모 시험이 이 시험 실행 파일을 환경 변수와 함께 다시 띄운다.
#[test]
fn child() {
    let Ok(job) = std::env::var("FULLOPS_STORE_CHILD") else {
        return;
    };
    let dir = PathBuf::from(std::env::var("FULLOPS_STORE_DIR").unwrap());
    let repo = bare_repo(&dir);
    match job.as_str() {
        "bump" => {
            for _ in 0..40 {
                update_state(&repo, Duration::from_secs(30), None, |s| {
                    s.next_decision += 1
                })
                .unwrap();
            }
        }
        "hold" => {
            let _guard = lock::acquire(
                &repo.shared().join("state.json.lock"),
                Duration::from_secs(5),
            )
            .unwrap();
            // 원자 교체 직전에 죽은 것처럼 임시 파일을 남긴다
            std::fs::write(
                repo.shared()
                    .join(format!("state.json.tmp.{}", std::process::id())),
                "{ 쓰다 만",
            )
            .unwrap();
            std::fs::write(dir.join("locked"), "").unwrap();
            std::thread::sleep(Duration::from_secs(120));
        }
        other => panic!("모르는 일: {other}"),
    }
}

fn spawn_child(job: &str, dir: &Path) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["child", "--exact", "--nocapture", "--test-threads=1"])
        .env("FULLOPS_STORE_CHILD", job)
        .env("FULLOPS_STORE_DIR", dir)
        .spawn()
        .unwrap()
}

#[test]
fn concurrent_updates_from_two_processes_are_all_applied() {
    let sb = Sandbox::empty();
    let repo = bare_repo(&sb.root);
    let mut children = [spawn_child("bump", &sb.root), spawn_child("bump", &sb.root)];
    for _ in 0..20 {
        update_state(&repo, Duration::from_secs(30), None, |s| {
            s.next_decision += 1
        })
        .unwrap();
        // 잠그지 않는 읽기는 쓰는 도중에도 온전한 파일을 본다
        assert!(matches!(read_state(&repo), Known::Yes(_)));
    }
    for child in &mut children {
        assert!(child.wait().unwrap().success());
    }
    let state = read_state(&repo).yes().expect("파일이 깨지지 않는다");
    assert_eq!((state.next_decision, state.seq), (1 + 100, 100));
    assert!(
        read_json::<Status>(&repo.status_path())
            .yes()
            .flatten()
            .is_some()
    );
}

#[test]
fn lock_is_released_when_holder_dies() {
    let sb = Sandbox::empty();
    let repo = bare_repo(&sb.root);
    let mut holder = spawn_child("hold", &sb.root);
    let started = Instant::now();
    while !sb.root.join("locked").exists() {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "자식이 잠금을 잡지 못했다"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    // 잡혀 있는 동안 hook은 기한 안에 포기한다
    assert!(matches!(
        update_state(&repo, HOOK_WAIT, None, |_| ()),
        Err(StoreError::LockBusy)
    ));
    assert_eq!(
        read_state(&repo),
        Known::Yes(State::default()),
        "읽기는 잠그지 않는다"
    );

    holder.kill().unwrap();
    holder.wait().unwrap();
    update_state(&repo, Duration::from_secs(5), None, |s| s.next_decision = 7)
        .expect("죽은 프로세스의 잠금은 풀린다");
    let state = read_state(&repo).yes().unwrap();
    assert_eq!((state.next_decision, state.seq), (7, 1));
}

#[test]
fn session_file_is_created_once_and_merged() {
    let sb = Sandbox::empty();
    let repo = bare_repo(&sb.root);
    let sid = SessionId::from_host("abc/def 1").unwrap();
    assert_eq!(sid.as_str(), "abc_def_1");
    assert_eq!(read_session(&repo, &sid), Known::Yes(None));
    assert!(matches!(
        update_session(&repo, &sid, HOOK_WAIT, |_| ()),
        Err(StoreError::Missing)
    ));

    // 설계 6.3절의 예시
    let example = json!({
        "v": 1, "sid": "abc_def_1", "host": "claude", "started_at": 1794500000, "last_seen": 1794504000,
        "terminal": "term_3", "mode_at_start": ["coor", null, null], "anchor": SHA_A, "turn": 3,
        "dispatch": { "id": "dsp_01", "task": "task_9", "bound_by": "preamble" },
        "routed": ["PAY-12"],
        "blocks": { "X-07": { "fp": "9c1e", "turn": 3 } },
        "noticed": ["hook-error:io", "plan-parse"]
    });
    let session: Session = serde_json::from_value(example.clone()).unwrap();
    assert_eq!(serde_json::to_value(&session).unwrap(), example);
    assert!(create_session(&repo, &session, HOOK_WAIT).unwrap());
    let mut later = session.clone();
    later.anchor = Sha::new(SHA_B).ok();
    assert!(
        !create_session(&repo, &later, HOOK_WAIT).unwrap(),
        "이미 있으면 만들지 않는다"
    );

    update_session(&repo, &sid, HOOK_WAIT, |s| {
        s.turn += 1;
        assert!(s.notice("state-broken") && !s.notice("plan-parse"));
        s.add_routed(&TaskKey::new("PAY-12").unwrap());
        s.add_routed(&TaskKey::new("PAY-13").unwrap());
        s.bind(SessionDispatch {
            id: DispatchId::new("dsp_02").unwrap(),
            task: None,
            bound_by: BoundBy::Terminal,
        });
    })
    .unwrap();
    let saved = read_session(&repo, &sid).yes().flatten().unwrap();
    assert_eq!(
        (saved.turn, saved.routed.len(), saved.noticed.len()),
        (4, 2, 3)
    );
    assert!(saved.blocks.is_empty() && saved.anchor == Sha::new(SHA_A).ok());
}

#[test]
fn committed_records_round_trip() {
    let sb = Sandbox::empty();
    let repo = bare_repo(&sb.root);
    let (key, role) = (TaskKey::new("PAY-12").unwrap(), RoleId::new("dev").unwrap());

    // 설계 6.1.4절의 예시
    let example = json!({
        "v": 1, "key": "PAY-12", "class": "direct", "role": "dev",
        "deliverables": ["D03"], "origin": "user", "decided_by": "jev",
        "reason": "", "request_file": "docs/requests/pay-12.md", "request_sha256": "ab12",
        "probabilities": { "scope": 0.91, "role": 0.77 },
        "thresholds": { "scope": 0.8, "role": 0.6, "deliverable": 0.5 },
        "tuples": { "dev": { "agent": "claude", "model": "m", "effort": "medium", "why": "기존 패턴 구현" } },
        "created_at": 1794500000, "history": []
    });
    assert_eq!(read_route(&repo, &key), Known::Yes(None));
    write_route(&repo, &serde_json::from_value(example.clone()).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(read_route(&repo, &key).yes().flatten().unwrap()).unwrap(),
        example
    );

    assert_eq!(read_inbox(&repo, &role), Known::Yes(None));
    let inbox = format!(
        "---\nid: handover-dev\ntitle: PAY-12 — 결제 재시도 버튼\nstatus: draft\nupdated: 2026-11-13\nowner: dev\nsummary: 결제 재시도 버튼\n\
key: PAY-12\nrole: dev\ngoal: 결제 재시도 버튼\nbase: {SHA_A}\nroute: direct\nfrom: lead\nround: 0\nbudget: 2\ntest_level: lite\nsubagent_level: lite\n---\n# 본문\n"
    );
    std::fs::create_dir_all(repo.inbox_path(&role).parent().unwrap()).unwrap();
    std::fs::write(repo.inbox_path(&role), &inbox).unwrap();
    let read = read_inbox(&repo, &role).yes().flatten().expect("인박스");
    assert_eq!(
        (read.key, read.route, read.budget),
        (key.clone(), RouteClass::Direct, 2)
    );
    std::fs::write(repo.inbox_path(&role), inbox.replace("key: PAY-12\n", "")).unwrap();
    assert_eq!(read_inbox(&repo, &role), Known::Unknown("inbox_syntax"));

    assert!(
        read_plan(&repo, "").yes().unwrap().rows.is_empty(),
        "PLANS.md가 없으면 빈 계획이다"
    );

    let line = json!({"key": "PAY-12", "role": "dev", "sha": SHA_A, "received_at": 1, "landed_at": 2, "rounds": 0,
                      "dispatches": [{"purpose": "build", "started_at": 1, "reason": null}], "direct": false});
    append_archive(&repo, &serde_json::from_value(line.clone()).unwrap()).unwrap();
    let archive = read_archive(&repo).yes().unwrap();
    assert_eq!(serde_json::to_value(&archive[0]).unwrap(), line);
    assert!(archive_keys(&archive).contains(&key));
    assert_eq!(
        fullops_core::stage_of(&key, &State::default(), &archive_keys(&archive), true),
        Stage::Done
    );
}
