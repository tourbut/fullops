//! 저장 계층의 통합 시험: 레포 찾기, 설정 검증, 상태 파일의 형식·동시 갱신·복구, 세션 파일, 설계 문서의 예시.

use fullops_core::config::{PolicyKind, RawConfig};
use fullops_core::explore::{ContextResult, FindResult, SearchIndex};
use fullops_core::model::*;
use fullops_store::*;
use fullops_testkit::{Sandbox, Setup};
use serde::Serialize;
use serde::de::DeserializeOwned;
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

/// 설계 문서에서 `marker`가 든 코드 블록(``` 사이)을 꺼낸다. 자리 채움 값은 형식에 맞는 값으로 바꾼다.
fn design_block(marker: &str) -> String {
    let design =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/design.md"))
            .unwrap()
            .replace("\r\n", "\n");
    let at = design
        .find(marker)
        .unwrap_or_else(|| panic!("설계 문서에 {marker:?}가 없다"));
    let open = design[..at].rfind("\n```").expect("여는 펜스");
    let start = open + 1 + design[open + 1..].find('\n').expect("펜스 줄 끝") + 1;
    let end = at + design[at..].find("\n```").expect("닫는 펜스");
    design[start..end]
        .replace("\"…\"", &format!("\"{SHA_A}\""))
        .replace("<기본 브랜치 SHA>", SHA_B)
        .replace("<첫 SessionStart의 HEAD>", SHA_A)
}

/// 예시 JSON을 타입으로 읽고 다시 쓰면 같은 JSON이다.
fn round_trip<T: DeserializeOwned + Serialize>(text: &str) -> T {
    let example: Value = serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    let typed: T =
        serde_json::from_value(example.clone()).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert_eq!(serde_json::to_value(&typed).unwrap(), example);
    typed
}

#[test]
fn design_examples_round_trip_through_the_types() {
    // 6.2.1절 state.json, 6.2.3절 status.json, 6.3절 세션 파일과 탐색 색인, 6.1.4절 route, 6.1.5절 탐색 결과
    let state: State = round_trip(&design_block("{ \"v\": 1, \"seq\": 42,"));
    assert_eq!(state.runs[0].coordinator.as_deref(), Some("term_3"));
    assert_eq!(state.dispatches[0].request, "req_5f2c9a1b7d3e4f60");
    assert!(state.dispatches[0].counts() && state.pending().count() == 1);
    let status: Status = round_trip(&design_block("{ \"v\": 1, \"generated_at\": 1794504200"));
    assert_eq!(
        (status.metrics.subagent_calls, status.dispatches[0].sub),
        (31, false)
    );
    let session: Session = round_trip(&design_block(
        "{ \"v\": 1, \"sid\": \"…\", \"host\": \"claude\"",
    ));
    assert_eq!((session.host, session.prompt_fp), (HostId::Claude, None));
    let _: Route = round_trip(&design_block(
        "{ \"v\": 1, \"key\": \"PAY-12\", \"class\": \"direct\"",
    ));
    let index: SearchIndex = round_trip(&design_block("\"parser\": \"map-v1\", \"config_sha256\""));
    assert!(index.files[0].searchable);
    let find: FindResult = round_trip(&design_block("\"options\": { \"scope\": \"code\""));
    assert_eq!(find.candidates[0].basis, "batch");
    let context: ContextResult =
        round_trip(&design_block("\"options\": { \"paths\": [\"docs/pay.md\"]"));
    assert_eq!(
        context.candidates[0].decision,
        fullops_core::explore::Decision::Keep
    );
    // 6.3절의 추적 줄, 6.2.2절의 턴 줄, 6.2.1절의 archive 줄, 10.3절의 설치 기록
    let design = design_block("{ \"v\": 1, \"seq\": 42,");
    assert!(!design.is_empty());
    let trace = r#"{"t":1794504000,"v":1,"bin":"2.0.0","host":"claude","ev":"PreToolUse","actor":"lead","intents":["OrcaWorkerStart"],"rules":[{"id":"T-01","verdict":"deny","fp":"9c1e"}],"git":0,"orca":0,"ms":7,"err":null}"#;
    let _: TraceLine = round_trip(trace);
    for kind in [
        "human",
        "machine:orca-message",
        "machine:task-notification",
        "subagent",
        "workflow",
    ] {
        let _: Turn =
            round_trip(&json!({"at": 1794500000, "sid": "sid-1", "kind": kind}).to_string());
    }
    let archive = json!({"key": "PAY-12", "role": "dev", "sha": SHA_A, "received_at": 1, "landed_at": 2, "rounds": 0,
                         "dispatches": [{"purpose": "build", "started_at": 1, "reason": null, "sub": false}], "direct": false});
    let _: ArchiveLine = round_trip(&archive.to_string());
    let deps_state = r#"{"v":1,"manifest_sha256":"ab12","hosts":{"claude":{"at":1794500000,"self_version":"2.0.0","complete":true,"failed":[]}}}"#;
    let deps_state: DepsState = round_trip(deps_state);
    assert!(deps_state.hosts[&HostId::Claude].complete);

    // 6.1.1절 fullops.toml과 10.7절 deps.toml: 읽고 다시 쓴 글을 읽으면 같은 값이다
    let config_text = design_block("schema = 2\napplied_version");
    let config = parse_config(&config_text).expect("설계의 fullops.toml 예시");
    assert_eq!(
        (config.parallel, config.test_level),
        (Parallel::On, TestLevel::Lite)
    );
    assert_eq!(config.code.extensions, [".py", ".ts"]);
    let raw: RawConfig = toml::from_str(&config_text).unwrap();
    assert_eq!(
        toml::from_str::<RawConfig>(&toml::to_string(&raw).unwrap()).unwrap(),
        raw
    );
    let deps_text = design_block("skill_cli = [\"npx\", \"--yes\", \"skills@latest\"]");
    let deps = parse_deps(&deps_text).expect("설계의 deps.toml 예시");
    assert_eq!(
        (
            deps.plugins.len(),
            deps.skills.len(),
            deps.mcp.len(),
            deps.optional.len()
        ),
        (3, 9, 1, 4)
    );
    assert_eq!(deps.own.hosts, HostId::ALL);
    assert_eq!(parse_deps(&toml::to_string(&deps).unwrap()), Ok(deps));
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
parallel = \"on\"\nfuture_key = 1\n\n[git]\nbase = \"main\"\nremote = \"origin\"\n\n[roles.dev]\nbranch = \"fullops/dev\"\n\
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
    for (word, parallel) in [
        ("off", Parallel::Off),
        ("read", Parallel::Read),
        ("ultra", Parallel::Ultra),
    ] {
        let text = valid.replace("parallel = \"on\"", &format!("parallel = \"{word}\""));
        assert_eq!(parse_config(&text).unwrap().parallel, parallel);
    }

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
    let local = valid.replace("remote = \"origin\"\n", "");
    assert_eq!(parse_config(&local).unwrap().base_ref(), "refs/heads/main");

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
        (
            "tech_plan_role이 등록 역할이 아님",
            split.replace("tech_plan_role = \"dev\"", "tech_plan_role = \"art\""),
        ),
        (
            "remote가 빈 문자열",
            valid.replace("remote = \"origin\"", "remote = \"\""),
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
        ("parallel 없음", valid.replace("parallel = \"on\"\n", "")),
        (
            "1.24.0의 레벨 값",
            valid.replace("parallel = \"on\"", "parallel = \"lite\""),
        ),
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

    // 기준 시점의 설정: 없으면 내장 기본값, 읽히지 않거나 검증을 어기면 기본값과 fell_back
    let custom =
        format!("{valid}\n[review]\nmin_code_lines = 10\n").replace("\"lite\"", "\"full\"");
    let base = baseline(Some(&custom));
    assert_eq!(
        (base.review.min_code_lines, base.test_level, base.fell_back),
        (10, TestLevel::Full, false)
    );
    let missing = baseline(None);
    assert_eq!(
        (
            missing.review.min_code_lines,
            missing.test_level,
            missing.fell_back
        ),
        (400, TestLevel::Lite, false)
    );
    assert!(missing.code.extensions.iter().any(|e| e == ".py"));
    for broken in ["schema = = 2", &valid.replace("schema = 2", "schema = 1")] {
        let fell = baseline(Some(broken));
        assert_eq!((fell.review.min_code_lines, fell.fell_back), (400, true));
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
    let mut state: Value = serde_json::from_str(&design_block("{ \"v\": 1, \"seq\": 42,")).unwrap();
    state["future_top_field"] = json!(true);
    state["dispatches"][0]["future_dispatch_field"] = json!({ "nested": [1, 2.5, "x"] });
    state["runs"][0]["future_run_field"] = json!(null);
    state
}

fn status_of(repo: &Repo) -> Status {
    read_json(&repo.status_path())
        .yes()
        .flatten()
        .expect("status.json")
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
    assert_eq!(
        ack_by(&state, &MessageId::new("msg_3").unwrap()),
        AckBy::Merge
    );

    // 아무것도 고치지 않고 다시 쓰면 seq만 달라진다
    update_state(&repo, CLI_WAIT, None, |_| ()).unwrap();
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(repo.state_path()).unwrap()).unwrap();
    let mut expected = design_state();
    expected["seq"] = json!(43);
    assert_eq!(written, expected);

    // status.json의 state에서 나오는 부분은 같은 잠금 안에서 매번 다시 만든다
    let status = status_of(&repo);
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
    assert_eq!(
        status.sync_pending.values().next().map(String::as_str),
        Some("9c1e0aa")
    );
    // 유도값은 넘겨받기 전에는 비어 있다
    assert!(status.ready.is_empty() && status.pending[0].landing.is_none());
    assert_eq!(status.metrics, Metrics::default());
}

#[test]
fn status_changes_only_the_derived_values_it_was_given() {
    let sb = Sandbox::empty();
    let repo = bare_repo(&sb.root);
    std::fs::create_dir_all(repo.shared()).unwrap();
    std::fs::write(repo.state_path(), design_state().to_string()).unwrap();
    // 영구히 자라는 파일은 잠금 안에서 읽지 않는다: 깨진 줄이 있어도 상관없다
    std::fs::write(repo.archive_path(), "{ 쓰다 만\n").unwrap();
    std::fs::write(repo.turns_path(), "{ 쓰다 만\n").unwrap();

    let ready = vec![TaskKey::new("PAY-13").unwrap()];
    let derived = Derived {
        ready: Some(ready.clone()),
        landing: [("msg_3".to_string(), Landing::PushLeft)].into(),
        metrics: Some(Metrics {
            at: 7,
            human_turns: 12,
            subagent_calls: 31,
            ..Metrics::default()
        }),
    };
    update_state(&repo, CLI_WAIT, Some(&derived), |_| ()).unwrap();
    let status = status_of(&repo);
    assert_eq!(status.ready, ready);
    assert_eq!(status.pending[0].landing, Some(Landing::PushLeft));
    assert_eq!((status.metrics.at, status.metrics.subagent_calls), (7, 31));

    // 넘기지 않은 쓰기(hook, lead가 아닌 CLI)는 이전 값을 잇는다
    // landing은 completion ID로 잇는다: SHA가 없는 건도 이어진다
    update_state(&repo, CLI_WAIT, None, |s| s.completions[0].sha = None).unwrap();
    let kept = status_of(&repo);
    assert_eq!(
        (kept.pending[0].id.as_str(), kept.pending[0].sha.as_deref()),
        ("msg_3", None)
    );
    assert_eq!(
        (
            kept.ready.clone(),
            kept.pending[0].landing,
            kept.metrics.human_turns
        ),
        (ready, Some(Landing::PushLeft), 12)
    );
    // 넘긴 것만 바뀐다
    let only_landing = Derived {
        landing: [("msg_3".to_string(), Landing::MergeLeft)].into(),
        ..Derived::default()
    };
    update_state(&repo, CLI_WAIT, Some(&only_landing), |_| ()).unwrap();
    let status = status_of(&repo);
    assert_eq!(
        (
            status.pending[0].landing,
            status.ready.len(),
            status.metrics.at
        ),
        (Some(Landing::MergeLeft), 1, 7)
    );

    // 하위 배정과 예약 기록: tasks에서 빠지고, 예약 기록은 dispatches에도 없다
    update_state(&repo, CLI_WAIT, None, |s| {
        let mut sub = s.dispatches[0].clone();
        sub.id = DispatchId::new("ctx_sub").unwrap();
        sub.key = TaskKey::new("SUB-1").unwrap();
        sub.sub_of = Some(s.dispatches[0].id.clone());
        sub.settled = None;
        let mut reserved = sub.clone();
        reserved.id = DispatchId::new("pending:req_1").unwrap();
        reserved.key = TaskKey::new("RES-1").unwrap();
        reserved.sub_of = None;
        s.dispatches.extend([sub, reserved]);
    })
    .unwrap();
    let status = status_of(&repo);
    assert_eq!(status.tasks.len(), 1);
    assert_eq!(
        status.dispatches.iter().map(|d| d.sub).collect::<Vec<_>>(),
        [false, true]
    );
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

    // R4-8: `v`는 그대로인데 이 실행본이 모르는 값이 든 파일(JSON 문법은 맞다)도 옮기지 않는다
    let unknown_value = r#"{"v": 1, "decisions": [{"id": "D-1", "status": "deferred"}]}"#;
    std::fs::write(repo.state_path(), unknown_value).unwrap();
    assert_eq!(read_state(&repo), Known::Unknown("state_newer"));
    assert!(matches!(
        update_state(&repo, CLI_WAIT, None, |_| ()),
        Err(StoreError::Corrupt("state_newer"))
    ));
    assert_eq!(
        std::fs::read_to_string(repo.state_path()).unwrap(),
        unknown_value
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
fn session_file_is_created_once_merged_and_keeps_unknown_fields() {
    let sb = Sandbox::empty();
    let repo = bare_repo(&sb.root);
    let sid = SessionId::from_host("abc/def 1").unwrap();
    assert_eq!(sid.as_str(), "abc_def_1");
    assert_eq!(read_session(&repo, &sid), Known::Yes(None));
    assert!(matches!(
        update_session(&repo, &sid, HOOK_WAIT, |_| ()),
        Err(StoreError::Missing)
    ));

    // 설계 6.3절의 예시에 모르는 필드를 더한 것
    let mut example: Value = serde_json::from_str(&design_block(
        "{ \"v\": 1, \"sid\": \"…\", \"host\": \"claude\"",
    ))
    .unwrap();
    example["sid"] = json!("abc_def_1");
    example["future_field"] = json!({ "a": [1] });
    example["dispatch"]["future_field"] = json!(true);
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
        s.prompt_fp = Some("0123456789abcdef".into());
        assert!(s.notice("state-broken") && !s.notice("plan-parse"));
        s.add_routed(&TaskKey::new("PAY-12").unwrap());
        s.add_routed(&TaskKey::new("PAY-13").unwrap());
        s.bind(SessionDispatch {
            id: DispatchId::new("dsp_02").unwrap(),
            task: None,
            bound_by: BoundBy::Terminal,
            extra: Extra::new(),
        });
    })
    .unwrap();
    let saved = read_session(&repo, &sid).yes().flatten().unwrap();
    assert_eq!(
        (saved.turn, saved.routed.len(), saved.noticed.len()),
        (4, 2, 3)
    );
    assert!(saved.blocks.is_empty() && saved.anchor == Sha::new(SHA_A).ok());
    assert_eq!(saved.extra["future_field"], json!({ "a": [1] }));
    assert_eq!(saved.prompt_fp.as_deref(), Some("0123456789abcdef"));
    for host in ["codex", "grok", "agy"] {
        example["host"] = json!(host);
        assert!(
            serde_json::from_value::<Session>(example.clone()).is_ok(),
            "{host}"
        );
    }
}

#[test]
fn committed_records_round_trip() {
    let sb = Sandbox::empty();
    let repo = bare_repo(&sb.root);
    let (key, role) = (TaskKey::new("PAY-12").unwrap(), RoleId::new("dev").unwrap());

    assert_eq!(read_route(&repo, &key), Known::Yes(None));
    let example: Value = serde_json::from_str(&design_block(
        "{ \"v\": 1, \"key\": \"PAY-12\", \"class\": \"direct\"",
    ))
    .unwrap();
    write_route(&repo, &serde_json::from_value(example.clone()).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(read_route(&repo, &key).yes().flatten().unwrap()).unwrap(),
        example
    );
    // 2.1.0에서 뗀 requires_packet이 남은 기록(2.0.x)도 읽힌다. 그 키는 버린다
    let mut old = example.clone();
    old["requires_packet"] = json!(true);
    let read: Route = serde_json::from_value(old).unwrap();
    assert_eq!(serde_json::to_value(&read).unwrap(), example);

    assert_eq!(read_inbox(&repo, &role), Known::Yes(None));
    let inbox = format!(
        "---\nid: handover-dev\ntitle: \"PAY-12 — 결제: 재시도 버튼\"\nstatus: draft\nupdated: 2026-11-13\nowner: dev\nsummary: 결제 재시도 버튼\n\
key: PAY-12\nrole: dev\ngoal: 결제 재시도 버튼\nbase: {SHA_A}\nroute: direct   # direct | design | product\nfrom: lead\nround: 0\nbudget: 2\n\
test_level: lite\nparallel: on\n---\n# 본문\n"
    );
    std::fs::create_dir_all(repo.inbox_path(&role).parent().unwrap()).unwrap();
    std::fs::write(repo.inbox_path(&role), &inbox).unwrap();
    let read = read_inbox(&repo, &role).yes().flatten().expect("인박스");
    assert_eq!(
        (read.key, read.route, read.budget, read.parallel),
        (key.clone(), RouteClass::Direct, 2, Parallel::On)
    );
    for broken in [
        inbox.replace("key: PAY-12\n", ""),
        inbox.replace("parallel: on", "subagent_level: lite"),
        inbox.replace("title: \"PAY-12", "title: \"PAY-12 \\x"),
    ] {
        std::fs::write(repo.inbox_path(&role), broken).unwrap();
        assert_eq!(read_inbox(&repo, &role), Known::Unknown("inbox_syntax"));
    }

    assert!(
        read_plan(&repo, "").yes().unwrap().rows.is_empty(),
        "PLANS.md가 없으면 빈 계획이다"
    );

    // archive.jsonl: 키는 줄 훑기로 뽑는다. landed_at이 없는 줄과 결정 줄, 깨진 줄은 세지 않는다
    assert_eq!(archive_keys(&repo), Known::Yes(Default::default()));
    let line = |key: &str, landed: Value| {
        json!({"key": key, "role": "dev", "sha": SHA_A, "received_at": 1, "landed_at": landed, "rounds": 0,
               "dispatches": [{"purpose": "build", "started_at": 1, "reason": null, "sub": true}], "direct": false})
    };
    append_archive(
        &repo,
        &serde_json::from_value(line("PAY-12", json!(2))).unwrap(),
    )
    .unwrap();
    append_archive(
        &repo,
        &serde_json::from_value(line("PAY-13", json!(null))).unwrap(),
    )
    .unwrap();
    append_line(&repo.archive_path(), &json!({"decision": {"id": "D001"}})).unwrap();
    append_line(&repo.archive_path(), &json!("깨진 줄")).unwrap();
    let archive = read_archive(&repo).yes().unwrap();
    assert_eq!(
        serde_json::to_value(&archive[0]).unwrap(),
        line("PAY-12", json!(2))
    );
    let keys = archive_keys(&repo).yes().unwrap();
    assert_eq!(
        keys.iter().map(TaskKey::as_str).collect::<Vec<_>>(),
        ["PAY-12"]
    );
    assert_eq!(
        fullops_core::stage_of(&key, &State::default(), &keys, true),
        Stage::Done
    );
}

#[test]
fn deps_toml_is_found_in_order_and_validated() {
    let example = design_block("skill_cli = [\"npx\", \"--yes\", \"skills@latest\"]");
    let builtin = example.replace(
        "source = \"tourbut/fullops\"",
        "source = \"builtin/fullops\"",
    );
    let sb = Sandbox::empty();
    let root = sb.root.join("plugin");
    std::fs::create_dir_all(&root).unwrap();

    // 플러그인 루트에 없으면 내장 사본이다(이 시험은 사용자 홈을 바꾸지 않으므로 홈의 사본이 있으면 건너뛴다)
    if fullops_home().is_none_or(|home| !home.join("deps.toml").exists()) {
        let (deps, path) = load_deps(Some(&root), &builtin).unwrap();
        assert_eq!(
            (deps.own.source.as_str(), path),
            ("builtin/fullops", PathBuf::new())
        );
    }
    std::fs::write(root.join("deps.toml"), &example).unwrap();
    let (deps, path) = load_deps(Some(&root), &builtin).unwrap();
    assert_eq!(
        (deps.own.source.as_str(), path),
        ("tourbut/fullops", root.join("deps.toml"))
    );
    assert_eq!(deps.skill_cli, ["npx", "--yes", "skills@latest"]);
    // 찾은 파일이 규칙에 맞지 않으면 내장 사본으로 넘어가지 않는다
    std::fs::write(
        root.join("deps.toml"),
        example.replace("schema = 1", "schema = 2"),
    )
    .unwrap();
    assert_eq!(load_deps(Some(&root), &builtin), Err(DepsError::Schema(2)));

    let invalid = [
        (
            "모르는 마켓플레이스",
            example.replace("pstack@cursor-plugins", "pstack@nowhere"),
        ),
        (
            "플러그인에 agy",
            example.replace(
                "hosts = [\"claude\", \"codex\"]",
                "hosts = [\"claude\", \"agy\"]",
            ),
        ),
        (
            "https가 아닌 MCP",
            example.replace("https://mcp.context7.com/mcp", "http://localhost/mcp"),
        ),
        (
            "모르는 optional 종류",
            example.replacen("kind = \"tool\"", "kind = \"binary\"", 1),
        ),
        (
            "빈 skill_cli",
            example.replace("[\"npx\", \"--yes\", \"skills@latest\"]", "[]"),
        ),
    ];
    for (what, text) in invalid {
        assert!(
            matches!(parse_deps(&text), Err(DepsError::Invalid(_))),
            "{what}"
        );
    }
    let unreadable = [
        ("TOML이 아님", "schema = = 1".to_string()),
        ("schema 없음", example.replace("schema = 1\n", "")),
        ("[self] 없음", example.replace("[self]", "[other]")),
        (
            "모르는 호스트",
            example.replace("hosts = [\"grok\", \"agy\"]", "hosts = [\"cursor\"]"),
        ),
    ];
    for (what, text) in unreadable {
        assert!(
            matches!(parse_deps(&text), Err(DepsError::Parse(_))),
            "{what}"
        );
    }
    // 모르는 표와 필드는 무시하고 skill_cli는 생략할 수 있다
    let extra = format!(
        "{}\n[future]\nx = 1\n",
        example.replace(
            "skill_cli = [\"npx\", \"--yes\", \"skills@latest\"]",
            "other = 1"
        )
    );
    assert_eq!(
        parse_deps(&extra).unwrap().skill_cli,
        ["npx", "--yes", "skills@latest"]
    );
}

#[test]
fn caps_table_matches_design() {
    // 설계 7.2절의 능력 표를 읽어 caps_of와 견준다
    let design =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/design.md"))
            .unwrap();
    let table = design
        .split("| `Caps` | `claude` | `codex` | `grok` | `agy` |")
        .nth(1)
        .expect("능력 표");
    let mut rows = 0;
    for line in table.lines().skip(2).take_while(|l| l.starts_with('|')) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let field = cells[1].trim_matches('`');
        for (host, cell) in HostId::ALL.into_iter().zip(&cells[2..6]) {
            let caps = caps_of(host);
            let flag = |value: bool| if value { "참" } else { "거짓" };
            let actual = match field {
                "stop_failure" => flag(caps.stop_failure),
                "background_wake" => flag(caps.background_wake),
                "prompt_event" => flag(caps.prompt_event),
                "context_at_start" => flag(caps.context_at_start),
                "context_at_prompt" => flag(caps.context_at_prompt),
                "user_warn" => flag(caps.user_warn),
                "workflow" => flag(caps.workflow),
                "subagent_tool" => caps.subagent_tool,
                "orca_agent" => caps.orca_agent,
                other => panic!("표에 모르는 능력이 있다: {other}"),
            };
            // 칸은 값 뒤에 괄호로 사유를 붙일 수 있다
            assert!(
                cell.trim_matches('`').starts_with(actual)
                    || cell.starts_with(&format!("`{actual}`")),
                "{field} {host}: {cell}"
            );
            assert_eq!(host_of_agent(caps.orca_agent), Some(host));
        }
        rows += 1;
    }
    assert_eq!(rows, 9, "Caps의 필드는 아홉이다");
    assert_eq!(host_of_agent("cursor"), None);
    assert!(orca_wakes(Some("term_3"), Some("term_3")));
    assert!(
        !orca_wakes(Some("term_3"), Some("term_4"))
            && !orca_wakes(None, Some("term_3"))
            && !orca_wakes(Some(""), Some(""))
    );
}
