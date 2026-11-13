//! 시험 도구 자체의 시험: 임시 레포, fake-orca, 시나리오 형식과 구동기의 기본 동사.

use fullops_testkit::scenario::parse;
use fullops_testkit::{Driver, Sandbox, Setup, VERBS, bin, orca_shapes, run_scenario};
use serde_json::json;

#[test]
fn sandbox_builds_repo_remote_and_worktrees() {
    let setup = Setup {
        roles: [("dev", "worker"), ("architecture", "design")]
            .map(|(id, kind)| (id.to_string(), kind.to_string()))
            .into(),
        ..Setup::default()
    };
    let sb = Sandbox::with(&setup);
    assert_eq!(
        sb.git_ok(&sb.path("lead"), &["rev-parse", "--abbrev-ref", "HEAD"]),
        "main"
    );
    assert_eq!(
        sb.git_ok(&sb.path("dev"), &["rev-parse", "--abbrev-ref", "HEAD"]),
        "fullops/dev"
    );
    assert_eq!(
        sb.git_ok(&sb.path("architecture"), &["status", "--porcelain"]),
        ""
    );
    let remote = sb.git_ok(&sb.path("lead"), &["ls-remote", "--heads", "origin"]);
    for branch in [
        "refs/heads/main",
        "refs/heads/fullops/dev",
        "refs/heads/fullops/architecture",
    ] {
        assert!(remote.contains(branch), "{branch}이 원격에 없다: {remote}");
    }
    assert!(sb.path("lead").join(".fullops-squad/PLANS.md").is_file());
    // 사용자의 git 설정이 아니라 격리된 홈의 설정을 쓴다
    assert_eq!(
        sb.git_ok(&sb.path("lead"), &["config", "user.email"]),
        "test@example.invalid"
    );
}

#[test]
fn fake_orca_follows_script_and_records_calls() {
    let sb = Sandbox::empty();
    sb.orca_rule(json!({ "when": ["orchestration", "run-create"], "times": 1, "stdout": orca_shapes::run_created("run_1") }));
    sb.orca_rule(json!({ "when": ["orchestration", "check", "--peek"], "exit": 3, "stdout": "글 그대로", "stderr": "실패" }));
    let orca = |args: &[&str], terminal: Option<&str>| {
        let mut cmd = sb.command(bin("fake-orca"), &sb.home);
        cmd.args(args)
            .envs(terminal.map(|t| ("ORCA_TERMINAL_HANDLE", t)));
        sb.run(cmd, b"")
    };

    let first = orca(&["orchestration", "run-create", "--json"], Some("term_1"));
    assert_eq!(first.code, Some(0));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&first.stdout).unwrap()["result"]["runId"],
        "run_1"
    );
    // times를 다 쓴 규칙은 다시 맞지 않는다
    let second = orca(&["orchestration", "run-create", "--json"], None);
    assert_eq!(second.code, Some(1));
    assert!(second.stdout.contains("unscripted"));
    // 낱말은 순서대로 있으면 되고 사이에 다른 인자가 있어도 된다
    let peek = orca(
        &["orchestration", "check", "--run", "run_1", "--peek"],
        None,
    );
    assert_eq!(
        (peek.code, peek.stdout.as_str(), peek.stderr.as_str()),
        (Some(3), "글 그대로", "실패")
    );
    assert_eq!(
        orca(&["--peek", "check", "orchestration"], None).code,
        Some(1)
    );

    let calls = sb.orca_calls();
    assert_eq!(calls.len(), 4);
    assert_eq!(
        calls[0]["argv"],
        json!(["orchestration", "run-create", "--json"])
    );
    assert_eq!(
        (&calls[0]["terminal"], &calls[0]["rule"]),
        (&json!("term_1"), &json!(0))
    );
    assert_eq!(
        (&calls[1]["rule"], &calls[2]["rule"]),
        (&json!(null), &json!(1))
    );
}

const SCENARIO: &str = r#"
name = "뼈대: 빈 규칙 표는 무엇이든 통과시킨다"
origin = "설계 18.1절"

[setup]
mode = "coor"
roles = { dev = "worker", architecture = "design" }
remote = true

[[step]]
act = "session_starts"
expect = { verdict = "pass", flow = "continues" }

[[step]]
act = "runs_shell"
command = "orca orchestration worker-start --spec x"
expect = { verdict = "pass" }

[[step]]
act = "writes_file"
who = "dev"
path = "src/a.py"
content = "print(1)\n"

[[step]]
act = "commits"
who = "dev"
message = "A-1 구현"

[[step]]
act = "orca_replies"
when = ["orchestration", "check"]
stdout = { ok = true, result = { messages = [] } }

[[step]]
act = "runs_fullops"
args = ["rules", "--json"]
expect = { verdict = "pass", exit = 0, flow = "continues" }

[[step]]
act = "lead_dispatches"
key = "B-2"
role = "dev"
expect = { verdict = "error", exit = 1, flow = "stopped" }

[[step]]
act = "stops"
who = "dev"
expect = { verdict = "pass" }
"#;

#[test]
fn scenario_runs_through_the_rs_driver() {
    let scenario = parse(SCENARIO).unwrap();
    assert_eq!((scenario.steps.len(), scenario.setup.roles.len()), (8, 2));
    let results = run_scenario(&scenario).unwrap();
    assert_eq!(results.len(), 8);
    assert!(results[5].output.contains("T-01"));

    // 기대와 다르면 어느 단계가 왜 다른지 알려 준다
    let wrong = parse(&SCENARIO.replace("expect = { verdict = \"pass\" }\n\n[[step]]\nact = \"writes_file\"", "expect = { verdict = \"deny\", rule = \"T-01\", escape = \"fullops dispatch\" }\n\n[[step]]\nact = \"writes_file\"")).unwrap();
    let error = run_scenario(&wrong).unwrap_err();
    assert!(
        error.contains("2단계(runs_shell)")
            && error.contains("T-01")
            && error.contains("fullops dispatch"),
        "{error}"
    );

    // 형식 오류: 모르는 동사, 빠진 인자, 모르는 기대 항목
    let mut driver = Driver::new(&Setup::default());
    let step =
        |text: &str| parse(&format!("name = \"x\"\n[[step]]\n{text}")).map(|s| s.steps[0].clone());
    assert!(
        driver
            .step(&step("act = \"flies\"").unwrap())
            .unwrap_err()
            .contains("모르는 동사")
    );
    assert!(
        driver
            .step(&step("act = \"runs_shell\"").unwrap())
            .unwrap_err()
            .contains("command")
    );
    assert!(step("act = \"stops\"\nexpect = { verdit = \"pass\" }").is_err());
    // 편집 동사는 막히지 않으면 디스크에 반영한다
    let edited = driver
        .step(&step("act = \"writes_file\"\npath = \"a.txt\"\ncontent = \"하나\"").unwrap())
        .unwrap();
    assert_eq!((edited.verdict, edited.flow()), ("pass", "continues"));
    driver
        .step(
            &step("act = \"edits_file\"\npath = \"a.txt\"\nold = \"하나\"\nnew = \"둘\"").unwrap(),
        )
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(driver.sb.path("lead").join("a.txt")).unwrap(),
        "둘"
    );
    driver
        .step(&step("act = \"deletes_file\"\npath = \"a.txt\"").unwrap())
        .unwrap();
    assert!(!driver.sb.path("lead").join("a.txt").exists());

    // 동사 목록에 있는 이름은 모두 구동기가 안다
    for verb in VERBS {
        let result = driver.step(&step(&format!("act = \"{}\"", verb.name)).unwrap());
        assert!(
            !result.is_err_and(|e| e.contains("모르는 동사")),
            "{}",
            verb.name
        );
    }
}
