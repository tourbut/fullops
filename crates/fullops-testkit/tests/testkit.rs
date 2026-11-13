//! 시험 도구 자체의 시험: 임시 레포, 가짜 실행 파일, 호스트별 입력 모양, 시나리오 형식과 구동기의 기본 동사.

use fullops_testkit::host_shapes::{Event, HOSTS, Tool, hook_call, pass_output, verdict_of};
use fullops_testkit::scenario::parse;
use fullops_testkit::{Driver, Sandbox, Setup, VERBS, bin, orca_shapes, run_scenario};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/fixtures")
}

fn json_file(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn sandbox_builds_repo_remote_and_worktrees() {
    let setup = Setup {
        roles: [("dev", "worker"), ("architecture", "design")]
            .map(|(id, kind)| (id.to_string(), kind.to_string()))
            .into(),
        ..Setup::default()
    };
    assert!(setup.config_toml().contains("parallel = \"on\""));
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
    // 사용자의 git 설정이 아니라 격리된 설정을 쓴다
    assert_eq!(
        sb.git_ok(&sb.path("lead"), &["config", "user.email"]),
        "test@example.invalid"
    );
}

#[test]
fn fake_orca_follows_script_and_records_calls() {
    let sb = Sandbox::empty();
    sb.orca_rule(json!({ "when": ["orchestration", "run-create"], "times": 1, "stdout": orca_shapes::run_created("run_1") }));
    sb.orca_rule(json!({ "when": ["orchestration", "worker-show"], "exit": 3, "stdout": "글 그대로", "stderr": "실패" }));
    let orca = |args: &[&str], terminal: Option<&str>| {
        let mut cmd = sb.command(bin("fake-orca"), &sb.home);
        cmd.args(args)
            .envs(terminal.map(|t| ("ORCA_TERMINAL_HANDLE", t)));
        sb.run(cmd, b"")
    };

    let first = orca(&["orchestration", "run-create", "--json"], Some("term_1"));
    assert_eq!(first.code, Some(0));
    assert_eq!(
        serde_json::from_str::<Value>(&first.stdout).unwrap()["result"]["runId"],
        "run_1"
    );
    // times를 다 쓴 규칙은 다시 맞지 않는다
    let second = orca(&["orchestration", "run-create", "--json"], None);
    assert_eq!(second.code, Some(1));
    assert!(second.stdout.contains("unscripted"));
    // 낱말은 순서대로 있으면 되고 사이에 다른 인자가 있어도 된다
    let show = orca(
        &["orchestration", "worker-show", "--dispatch", "ctx_1"],
        None,
    );
    assert_eq!(
        (show.code, show.stdout.as_str(), show.stderr.as_str()),
        (Some(3), "글 그대로", "실패")
    );
    assert_eq!(
        orca(&["--dispatch", "worker-show", "orchestration"], None).code,
        Some(1)
    );
    // 각본에 없는 check는 우편함으로 답한다: 비어 있으면 빈 묶음이다
    let empty = orca(
        &["orchestration", "check", "--run", "run_1", "--peek"],
        None,
    );
    assert_eq!(
        (
            empty.code,
            serde_json::from_str::<Value>(&empty.stdout).unwrap()
        ),
        (Some(0), orca_shapes::checked(vec![]))
    );

    let calls = sb.orca_calls();
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls[0]["argv"],
        json!(["orchestration", "run-create", "--json"])
    );
    assert_eq!(
        (&calls[0]["terminal"], &calls[0]["rule"]),
        (&json!("term_1"), &json!(0))
    );
    assert_eq!(
        (&calls[1]["rule"], &calls[2]["rule"], &calls[2]["exit"]),
        (&json!(null), &json!(1), &json!(3))
    );
}

#[test]
fn orca_shapes_use_the_fixture_fields() {
    let fixture = |name: &str| json_file(&fixtures().join("orca").join(name));
    // 만든 응답의 필드는 표본에 있는 필드뿐이다(값은 다르다)
    let subset = |made: &Value, sample: &Value, what: &str| {
        for key in made.as_object().unwrap().keys() {
            assert!(sample.get(key).is_some(), "{what}: 표본에 없는 필드 {key}");
        }
    };
    let started = orca_shapes::worker_started("run_1", "ctx_1", "task_1", Some("term_1"));
    subset(
        &started["result"],
        &fixture("worker_start.ready.json")["result"],
        "worker-start",
    );
    let failed = orca_shapes::worker_start_failed("run_1", "ctx_1", "task_1");
    subset(
        &failed["result"],
        &fixture("worker_start.failed.json")["result"],
        "worker-start 실패",
    );
    subset(
        &orca_shapes::run_created("run_1")["result"],
        &fixture("run_create.json")["result"],
        "run-create",
    );
    let message = orca_shapes::message(
        "msg_1",
        "worker_done",
        "ctx_1",
        "task_1",
        "완료",
        "본문",
        Some("succeeded"),
    );
    let sample = &fixture("check.worker_done.json")["result"]["messages"][0];
    subset(&message, sample, "메시지");
    subset(&message["payload"], &sample["payload"], "payload");
    let worker = orca_shapes::worker("run_1", "ctx_1", "task_1", "term_1", true, "live");
    subset(
        &worker,
        &fixture("worker_show.settled.json")["result"],
        "worker-show",
    );
    let terminals = orca_shapes::terminals(&[("term_1", true, "/x")]);
    subset(
        &terminals["result"]["terminals"][0],
        &fixture("terminal_list.json")["result"]["terminals"][0],
        "terminal list",
    );
    subset(
        &orca_shapes::error("x", "y")["error"],
        &fixture("error.json")["error"],
        "오류",
    );
    // preamble에서 P-01이 찾는 두 ID
    let preamble = orca_shapes::preamble("ctx_1", "task_1", "Task key: A-1");
    assert!(preamble.contains("--dispatch-id ctx_1") && preamble.contains("--task-id task_1"));
}

#[test]
fn fake_host_and_fake_curl_answer_from_scripts() {
    let sb = Sandbox::empty();
    sb.install_fakes(&["claude", "npx", "curl"]);
    sb.host_rule(json!({ "when": ["claude", "plugin", "marketplace", "list"], "stdout": [{"name": "fullops", "repo": "tourbut/fullops"}] }));
    sb.curl_rule(json!({ "when": ["--config"], "times": 1, "stdout": {"answers": []} }));
    let run = |program: &str, args: &[&str], stdin: &[u8]| {
        let mut cmd = sb.command(
            sb.bin_dir
                .join(format!("{program}{}", std::env::consts::EXE_SUFFIX)),
            &sb.home,
        );
        cmd.args(args);
        sb.run(cmd, stdin)
    };

    let listed = run("claude", &["plugin", "marketplace", "list", "--json"], b"");
    assert_eq!(
        serde_json::from_str::<Value>(&listed.stdout).unwrap()[0]["name"],
        "fullops"
    );
    // 각본에 없는 호출: 버전은 가짜 값, 설치 명령은 아무것도 하지 않고 성공한다
    assert_eq!(
        run("claude", &["--version"], b"").stdout,
        "claude 0.0.0-fake\n"
    );
    let install = run(
        "npx",
        &["--yes", "skills@latest", "add", "x/y", "--skill", "caveman"],
        b"",
    );
    assert_eq!((install.code, install.stdout.as_str()), (Some(0), ""));
    let calls = sb.host_calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(
        (&calls[0]["program"], &calls[2]["program"]),
        (&json!("claude"), &json!("npx"))
    );
    assert_eq!(calls[2]["argv"][2], "add");

    // PATH 앞에 놓인 이름으로도 풀린다
    let mut by_name = sb.command("curl", &sb.home);
    by_name.args(["--config", "-"]);
    let asked = sb.run(
        by_name,
        b"url = \"https://example.invalid\"\ndata = \"{}\"\n",
    );
    assert_eq!(
        (asked.code, asked.stdout.trim()),
        (Some(0), "{\"answers\":[]}")
    );
    let refused = run("curl", &["--config", "-"], b"x");
    assert_eq!(refused.code, Some(7), "각본을 다 쓰면 연결 실패다");
    let calls = sb.curl_calls();
    assert!(
        calls[0]["stdin"]
            .as_str()
            .unwrap()
            .contains("example.invalid")
    );
    assert_eq!(
        (&calls[0]["rule"], &calls[1]["rule"]),
        (&json!(0), &json!(null))
    );
}

/// 표본 파일 이름(`pre_tool_use.bash.json`)에서 이벤트를 읽는다.
fn event_of(file: &str) -> &'static str {
    match file {
        f if f.starts_with("session_start") => "SessionStart",
        f if f.starts_with("user_prompt_submit") => "UserPromptSubmit",
        f if f.starts_with("pre_tool_use") => "PreToolUse",
        f if f.starts_with("stop_failure") => "StopFailure",
        f if f.starts_with("stop") => "Stop",
        _ => "Other",
    }
}

fn keys(value: &Value) -> Vec<String> {
    value
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn host_shapes_follow_the_fixtures() {
    let cwd = Path::new("/home/dev/repo");
    let file = cwd.join("src/a.py");
    let events = || {
        vec![
            Event::SessionStart { source: "startup" },
            Event::UserPromptSubmit { prompt: "입력" },
            Event::PreToolUse(Tool::Shell {
                command: "npm test",
                powershell: false,
            }),
            Event::PreToolUse(Tool::Write {
                path: &file,
                content: "x",
            }),
            Event::PreToolUse(Tool::Edit {
                path: &file,
                old: "a",
                new: "b",
            }),
            Event::PreToolUse(Tool::SubAgent { prompt: "조사" }),
            Event::Stop { reason: None },
            Event::StopFailure { error: "429" },
        ]
    };
    for host in HOSTS {
        // 표본의 이벤트별 최상위 필드와 도구별 인자 이름
        let dir = fixtures().join("host").join(host);
        let mut top: Vec<(&str, Vec<String>)> = Vec::new();
        let mut tools: Vec<(String, Vec<String>)> = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if !name.ends_with(".json") {
                continue;
            }
            let sample = json_file(&path);
            top.push((event_of(&name), keys(&sample)));
            let call = if host == "agy" {
                sample["toolCall"].clone()
            } else {
                json!({"name": sample["tool_name"], "args": sample["tool_input"]})
            };
            if let Some(tool) = call["name"].as_str() {
                tools.push((tool.to_string(), keys(&call["args"])));
            }
        }
        for event in events() {
            let Some(call) = hook_call(host, "sid-1", cwd, Some("입력"), &event) else {
                continue;
            };
            // 등록 인자: agy만 --event를 준다
            assert_eq!(call.args[0], host);
            assert_eq!(call.args.len(), if host == "agy" { 3 } else { 1 }, "{host}");
            let event_name = match (&event, host) {
                (Event::SessionStart { .. }, _) => "SessionStart",
                (Event::UserPromptSubmit { .. }, _) => "UserPromptSubmit",
                (Event::PreToolUse(_), _) => "PreToolUse",
                // agy는 오류로 끝난 Stop이 StopFailure 구실을 한다
                (Event::Stop { .. }, _) | (Event::StopFailure { .. }, "agy") => "Stop",
                (Event::StopFailure { .. }, _) => "StopFailure",
            };
            if host == "agy" {
                assert_eq!(call.args[1..], ["--event", event_name]);
            } else {
                assert_eq!(call.input["hook_event_name"], event_name);
            }
            let known: Vec<&String> = top
                .iter()
                .filter(|(e, _)| *e == event_name)
                .flat_map(|(_, k)| k)
                .collect();
            for key in keys(&call.input) {
                assert!(
                    known.contains(&&key),
                    "{host} {event_name}: 표본에 없는 필드 {key}"
                );
            }
            let tool = if host == "agy" {
                call.input["toolCall"].clone()
            } else {
                json!({"name": call.input["tool_name"], "args": call.input["tool_input"]})
            };
            if let Some(name) = tool["name"].as_str() {
                let Some((_, args)) = tools.iter().find(|(t, _)| t == name) else {
                    panic!("{host}: 표본에 없는 도구 {name}");
                };
                for key in keys(&tool["args"]) {
                    assert!(args.contains(&key), "{host} {name}: 표본에 없는 인자 {key}");
                }
            }
        }
    }
    // 호스트에 없는 이벤트와 도구
    assert!(hook_call("codex", "s", cwd, None, &Event::StopFailure { error: "x" }).is_none());
    assert!(
        hook_call(
            "agy",
            "s",
            cwd,
            None,
            &Event::UserPromptSubmit { prompt: "x" }
        )
        .is_none()
    );
    assert!(
        hook_call(
            "grok",
            "s",
            cwd,
            None,
            &Event::PreToolUse(Tool::Workflow { script: "x" })
        )
        .is_none()
    );
    assert!(
        hook_call(
            "claude",
            "s",
            cwd,
            None,
            &Event::PreToolUse(Tool::Workflow { script: "x" })
        )
        .is_some()
    );
    assert!(hook_call("cursor", "s", cwd, None, &Event::Stop { reason: None }).is_none());

    // 출력 표본을 verdict 어휘로 옮긴다. 파일 이름의 첫 낱말이 판정이다
    for (host, dir) in [
        ("claude", "host/output"),
        ("codex", "host/output"),
        ("grok", "host/grok/output"),
        ("agy", "host/agy/output"),
    ] {
        for entry in std::fs::read_dir(fixtures().join(dir)).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).unwrap();
            let expected = name.split('.').next().unwrap();
            // grok은 PreToolUse의 Context를 받지만 fullops는 그 이벤트에서 Context를 내지 않는다(표본만 있다)
            assert_eq!(verdict_of(host, &text), expected, "{host} {name}");
            if expected == "pass" {
                let event = match name.as_str() {
                    "pass.pre_tool_use.json" => "PreToolUse",
                    "pass.stop.json" => "Stop",
                    _ => "SessionStart",
                };
                assert_eq!(text.trim(), pass_output(host, event), "{host} {name}");
            }
        }
    }
    assert_eq!(verdict_of("claude", "판정이 아닌 글"), "error");
    assert_eq!(pass_output("agy", "StopFailure"), r#"{"decision":""}"#);
}

const SCENARIO: &str = r#"
name = "뼈대: 빈 규칙 표는 무엇이든 통과시킨다"
origin = "설계 18.1절"

[setup]
mode = "coor"
roles = { dev = "worker", architecture = "design" }
remote = true
parallel = "on"

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
env = { ORCA_AGENT_SESSION_ID = "term_lead" }
expect = { verdict = "pass", exit = 0, flow = "continues" }

[[step]]
act = "lead_dispatches"
key = "B-2"
role = "dev"
expect = { verdict = "error", exit = 1, flow = "stopped" }

[[step]]
act = "lead_merges"
key = "B-2"
pending = "E"
expect = { verdict = "pass" }

[[step]]
act = "stops"
who = "dev"
expect = { verdict = "pass" }
"#;

#[test]
fn scenario_runs_through_the_rs_driver() {
    let scenario = parse(SCENARIO).unwrap();
    assert_eq!((scenario.steps.len(), scenario.setup.roles.len()), (9, 2));
    let results = run_scenario(&scenario).unwrap();
    assert_eq!(results.len(), 9);
    assert!(results[5].output.contains("T-01"));
    // pending이 적힌 단계는 실행하지 않고 보류로 적는다
    assert_eq!(
        (results[7].verdict, results[7].output.as_str()),
        ("pending", "보류: 묶음 E")
    );

    // 같은 시나리오가 네 호스트의 입력 모양과 등록 인자로 돈다
    for host in ["codex", "grok", "agy"] {
        let on_host = parse(&SCENARIO.replace(
            "parallel = \"on\"",
            &format!("parallel = \"read\"\nhost = \"{host}\""),
        ))
        .unwrap();
        run_scenario(&on_host).unwrap_or_else(|e| panic!("{host}: {e}"));
    }

    // 기대와 다르면 어느 단계가 왜 다른지 알려 준다
    let wrong = parse(&SCENARIO.replace("expect = { verdict = \"pass\" }\n\n[[step]]\nact = \"writes_file\"", "expect = { verdict = \"deny\", rule = \"T-01\", escape = \"fullops dispatch\" }\n\n[[step]]\nact = \"writes_file\"")).unwrap();
    let error = run_scenario(&wrong).unwrap_err();
    assert!(
        error.contains("2단계(runs_shell)")
            && error.contains("T-01")
            && error.contains("fullops dispatch"),
        "{error}"
    );

    // 형식 오류: 모르는 동사, 빠진 인자, 모르는 기대 항목, 모르는 setup 키
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
    assert!(parse("name = \"x\"\n[setup]\nsubagent_level = \"lite\"\n").is_err());
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

#[test]
fn design_scenario_example_is_valid() {
    // 설계 12.1절의 예시가 이 형식으로 읽힌다(동사와 기대 항목까지)
    let design =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/design.md"))
            .unwrap()
            .replace("\r\n", "\n");
    let start = design
        .find("name = \"미통합 결과를 두고 다른 과제를 배정한다\"")
        .expect("12.1절의 예시");
    let end = start + design[start..].find("\n```").unwrap();
    let scenario = parse(&design[start..end]).expect("설계의 시나리오 예시");
    assert_eq!(
        (
            scenario.steps.len(),
            scenario.setup.mode.as_str(),
            scenario.setup.roles.len()
        ),
        (4, "coor", 2)
    );
    for step in &scenario.steps {
        assert!(VERBS.iter().any(|v| v.name == step.act), "{}", step.act);
    }
    let expect = scenario.steps[1].expect.as_ref().unwrap();
    assert_eq!(
        (
            expect.verdict.as_deref(),
            expect.rule.as_deref(),
            expect.flow.as_deref()
        ),
        (Some("deny"), Some("D-03"), Some("stopped"))
    );
}
