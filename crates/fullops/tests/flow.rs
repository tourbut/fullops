//! 흐름 명령(묶음 C)의 통합 시험. `scenarios/`의 C 몫 시나리오를 `rs` 구동기로 돌리고,
//! 시나리오 파일이 단언하지 못하는 것(fake-orca가 받은 인자와 spec, `state.json`, 실행 중에 정해지는 SHA, 강제 종료)을 본다.
//! 실제 Orca·네트워크·사용자 홈은 건드리지 않는다.

use fullops_testkit::driver::mismatches;
use fullops_testkit::scenario::{load, parse};
use fullops_testkit::{Driver, Run, Sandbox, Setup, StepResult, bin, orca_shapes, run_scenario};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

fn scenario(id: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scenarios")
        .join(format!("{id}.toml"));
    let scenario = load(&path).unwrap_or_else(|e| panic!("{e}"));
    run_scenario(&scenario).unwrap_or_else(|e| panic!("{id}: {e}"));
}

macro_rules! scenarios {
    ($($name:ident => $id:literal),+ $(,)?) => {
        $(#[test] fn $name() { scenario($id) })+
    };
}

scenarios! {
    scenario_d1 => "d1", scenario_d2 => "d2", scenario_d3 => "d3", scenario_d4 => "d4",
    scenario_d5 => "d5", scenario_d6 => "d6", scenario_d8 => "d8",
    scenario_h4 => "h4", scenario_h5 => "h5",
    scenario_i5 => "i5", scenario_i6 => "i6", scenario_i7 => "i7", scenario_j5 => "j5",
    scenario_r2 => "r2", scenario_r2_off => "r2-off", scenario_r2_ultra => "r2-ultra",
    scenario_r3 => "r3", scenario_r3_off => "r3-off", scenario_r4 => "r4",
    scenario_r6 => "r6", scenario_r6_off => "r6-off",
}

/// 단계 묶음을 돌리고 단계에 적은 기대를 단언한다.
fn steps(driver: &mut Driver, text: &str) -> Vec<StepResult> {
    let scenario = parse(&format!("name = \"flow\"\n{text}")).unwrap();
    scenario
        .steps
        .iter()
        .map(|step| {
            let result = driver.step(step).unwrap();
            let diff = step.expect.as_ref().map(|e| mismatches(e, &result));
            assert!(
                diff.as_ref().is_none_or(Vec::is_empty),
                "{}: {diff:?}\n{}",
                step.act,
                result.output
            );
            result
        })
        .collect()
}

const DISPATCH: &str = "[[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\nexpect = { verdict = \"pass\" }\n";
const SUBDISPATCH: &str = "[[step]]\nact = \"worker_subdispatches\"\nrole = \"dev\"\nkey = \"A-1\"\nbrief = \"src/b.py를 맡는다\"\nexpect = { verdict = \"pass\" }\n";

/// 인박스의 완료 보고 절을 채우고 커밋한다.
fn report(role: &str) -> String {
    format!(
        "[[step]]\nact = \"edits_file\"\nwho = \"{role}\"\npath = \".fullops-squad/handovers/to_{role}.md\"\nold = \"## 완료 보고\\n\"\n\
         new = \"## 완료 보고\\n\\n구현과 확인을 마쳤다.\\n\"\n[[step]]\nact = \"commits\"\nwho = \"{role}\"\n"
    )
}

fn state(sb: &Sandbox) -> Value {
    let path = sb.path("lead").join(".git/fullops/state.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("state.json"))
        .expect("state.json 형식")
}

fn out(run: &Run) -> Value {
    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    serde_json::from_str(run.stdout.trim()).unwrap_or_else(|e| panic!("{e}: {}", run.stdout))
}

fn calls(sb: &Sandbox, word: &str) -> Vec<Value> {
    let has = |call: &Value| {
        call["argv"]
            .as_array()
            .is_some_and(|a| a.iter().any(|x| x == word))
    };
    sb.orca_calls().into_iter().filter(has).collect()
}

/// 호출의 `--이름` 다음 값.
fn flag(call: &Value, name: &str) -> Option<String> {
    let argv = call["argv"].as_array()?;
    let at = argv.iter().position(|a| a == name)?;
    argv.get(at + 1)?.as_str().map(str::to_string)
}

fn head(sb: &Sandbox, who: &str) -> String {
    sb.git_ok(&sb.path(who), &["rev-parse", "HEAD"])
}

/// 상태 파일과 Orca 인자에 쓰는 경로 표기(`/` 구분).
fn path_slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let started = Instant::now();
    while !done() {
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "기다리다 끝났다: {what}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn dispatch_records_state_and_sends_the_spec() {
    let mut d = Driver::new(&Setup::default());
    // 원본 체크아웃의 미추적 환경 파일은 대상 워크트리에 연결된다
    d.sb.write(
        &d.sb.path("lead").join(".fullops-squad/.env"),
        "TOKEN_FOR_TEST=1\n",
    );
    steps(&mut d, DISPATCH);
    let sb = &d.sb;
    let st = state(sb);
    let dispatch = &st["dispatches"][0];
    assert_eq!(st["dispatches"].as_array().map(Vec::len), Some(1));
    for (name, value) in [
        ("id", "ctx_1"),
        ("task_id", "task_1"),
        ("terminal", "term_1"),
        ("run", "run_1"),
        ("key", "A-1"),
        ("role", "dev"),
        ("purpose", "build"),
        ("session_mode", "fresh"),
    ] {
        assert_eq!(dispatch[name], value, "{name}");
    }
    assert_eq!(
        dispatch["tuple"],
        json!({"agent": "claude", "model": "test-model", "effort": "medium"})
    );
    assert!(dispatch["sub_of"].is_null() && dispatch["settled"].is_null());
    let request = dispatch["request"].as_str().unwrap();
    assert!(
        request.starts_with("req_") && request.len() == 20,
        "{request}"
    );
    assert_eq!(st["runs"][0]["id"], "run_1");
    assert_eq!(st["runs"][0]["owner"], "lead");
    assert_eq!(st["tasks"]["A-1"]["last_tuple"]["agent"], "claude");

    let start = &calls(sb, "worker-start")[0];
    let dev = sb.path("dev").to_string_lossy().replace('\\', "/");
    assert_eq!(flag(start, "--worktree"), Some(format!("path:{dev}")));
    for (name, value) in [
        ("--run", "run_1"),
        ("--spec-file", "-"),
        ("--agent", "claude"),
        ("--model", "test-model"),
        ("--effort", "medium"),
        ("--timeout-ms", "90000"),
        ("--retry-request", request),
    ] {
        assert_eq!(flag(start, name).as_deref(), Some(value), "{name}");
    }
    let spec = start["stdin"].as_str().unwrap();
    let base = sb.git_ok(&sb.path("lead"), &["rev-parse", "HEAD~1"]);
    assert!(
        spec.starts_with(&format!(
            "Task key: A-1; Purpose: implementation; Role: dev; Handover: {dev}/.fullops-squad/handovers/to_dev.md; \
             Base: {base}; Test level: lite; Parallel: on. 병렬 방침 on: "
        )),
        "{spec}"
    );
    assert!(
        spec.contains("(`Agent`)")
            && spec.contains("`fullops dispatch A-1 --brief \"<맡길 파일 묶음과 완료 조건>\"`")
    );
    assert!(
        spec.ends_with(
            "끝나면 fullops done을 실행하고 출력된 본문으로 preamble의 worker_done을 한 번 보낸다"
        ),
        "{spec}"
    );
    assert!(!spec.contains("ultracode") && !spec.contains("Read only"));

    // 준비 커밋이 기본 브랜치·원격·대상 워크트리에 갔다
    let lead = sb.path("lead");
    assert_eq!(
        sb.git_ok(&lead, &["log", "-1", "--format=%s"]),
        "fullops: 배정 준비 A-1"
    );
    assert_eq!(
        sb.git_ok(&lead, &["rev-parse", "origin/main"]),
        head(sb, "lead")
    );
    assert!(
        sb.path("dev")
            .join(".fullops-squad/handovers/to_dev.md")
            .is_file()
    );
    assert_eq!(
        std::fs::read_to_string(sb.path("dev").join(".fullops-squad/.env")).unwrap(),
        "TOKEN_FOR_TEST=1\n"
    );
    // 이미 연결된 뒤의 `env link`는 할 일이 없다
    assert_eq!(
        out(&sb.cli(&sb.path("dev"), &["env", "link", "--json"]))["linked"],
        json!([])
    );
    let status: Value = serde_json::from_str(
        &std::fs::read_to_string(lead.join(".git/fullops/status.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(status["dispatches"][0]["key"], "A-1");
    assert_eq!(status["tasks"][0]["stage"], "assigned");
}

#[test]
fn ultra_starts_only_new_claude_terminals_with_ultracode() {
    let mut d = Driver::new(&Setup {
        parallel: "ultra".into(),
        ..Setup::default()
    });
    steps(
        &mut d,
        &format!(
            "{DISPATCH}[[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\npurpose = \"fix\"\nterminal = \"term_1\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\npurpose = \"fix\"\nagent = \"codex\"\nmodel = \"m\"\neffort = \"high\"\nexpect = {{ verdict = \"pass\" }}\n"
        ),
    );
    let starts = calls(&d.sb, "worker-start");
    assert_eq!(flag(&starts[0], "--effort").as_deref(), Some("ultracode"));
    assert!(
        starts[0]["stdin"]
            .as_str()
            .unwrap()
            .contains("울트라코드가 켜져 있으면")
    );
    // 있는 터미널에는 agent·model·effort를 넘기지 않는다
    assert_eq!(flag(&starts[1], "--terminal").as_deref(), Some("term_1"));
    assert!(flag(&starts[1], "--effort").is_none() && flag(&starts[1], "--agent").is_none());
    assert_eq!(flag(&starts[2], "--effort").as_deref(), Some("high"));
    assert!(
        starts[2]["stdin"]
            .as_str()
            .unwrap()
            .contains("(`spawn_agent`)")
    );
    // 기록에는 준 값 그대로 남는다
    let st = state(&d.sb);
    assert_eq!(st["dispatches"][0]["tuple"]["effort"], "medium");
    assert_eq!(st["dispatches"][1]["session_mode"], "retain");
}

#[test]
fn dispatch_killed_midway_starts_the_worker_once() {
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        "[[step]]\nact = \"lead_routes\"\nkey = \"A-1\"\nrole = \"dev\"\n[[step]]\nact = \"lead_writes_task\"\nkey = \"A-1\"\nrole = \"dev\"\n",
    );
    let sb = &d.sb;
    sb.orca_rule(json!({"when": ["orchestration", "run-create"], "times": 1, "stdout": orca_shapes::run_created("run_1")}));
    sb.orca_rule(json!({"when": ["orchestration", "worker-start"], "times": 1, "sleep_ms": 3000,
                        "stdout": orca_shapes::worker_started("run_1", "ctx_k", "task_k", Some("term_k"))}));
    let lead = sb.path("lead");
    let mut child = sb
        .command(bin("fullops"), &lead)
        .args(["dispatch", "A-1", "--role", "dev"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    // Orca가 요청을 받은 뒤, 응답을 받기 전에 죽인다
    wait_until("worker-start 호출", || {
        calls(sb, "worker-start").len() == 1
    });
    child.kill().unwrap();
    child.wait().unwrap();
    let st = state(sb);
    let request = st["dispatches"][0]["request"].as_str().unwrap().to_string();
    assert_eq!(st["dispatches"][0]["id"], format!("pending:{request}"));

    // 다시 실행하면 같은 요청 ID로 Orca가 기록해 둔 결과를 받는다
    let again = sb.cli(&lead, &["dispatch", "A-1", "--role", "dev", "--json"]);
    assert_eq!(out(&again)["dispatch"], "ctx_k");
    let starts = calls(sb, "worker-start");
    assert_eq!(starts.len(), 2);
    assert!(
        starts
            .iter()
            .all(|c| flag(c, "--retry-request").as_deref() == Some(&request))
    );
    assert_eq!(
        (starts[0]["replayed"].clone(), starts[1]["replayed"].clone()),
        (json!(false), json!(true))
    );
    let st = state(sb);
    assert_eq!(st["dispatches"].as_array().map(Vec::len), Some(1));
    assert_eq!(st["dispatches"][0]["id"], "ctx_k");
    assert_eq!(st["dispatches"][0]["terminal"], "term_k");
    assert_eq!(calls(sb, "run-create").len(), 1);
}

#[test]
fn refused_starts_leave_no_reservation() {
    scenario("i7");
    // 같은 흐름을 다시 돌려 요청 ID와 예약 기록을 본다
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        "[[step]]\nact = \"lead_routes\"\nkey = \"A-1\"\nrole = \"dev\"\n[[step]]\nact = \"lead_writes_task\"\nkey = \"A-1\"\nrole = \"dev\"\n",
    );
    let sb = &d.sb;
    sb.orca_rule(json!({"when": ["orchestration", "run-create"], "times": 1, "stdout": orca_shapes::run_created("run_1")}));
    sb.orca_rule(
        json!({"when": ["orchestration", "worker-start"], "times": 1, "exit": 1,
                        "stdout": orca_shapes::worker_start_failed("run_1", "ctx_f", "task_f")}),
    );
    sb.orca_rule(
        json!({"when": ["orchestration", "worker-start"], "times": 1, "exit": 1,
                        "stdout": orca_shapes::error("nested_worker_depth_exceeded", "가짜 거절")}),
    );
    let lead = sb.path("lead");
    for _ in 0..2 {
        let run = sb.cli(&lead, &["dispatch", "A-1", "--role", "dev"]);
        assert_eq!(run.code, Some(4), "{}", run.stderr);
        assert_eq!(state(sb)["dispatches"], json!([]));
    }
    // Orca 실행 파일이 없어도 예약 기록이 남지 않는다
    let missing = sb.fullops(
        &lead,
        &["dispatch", "A-1", "--role", "dev"],
        &[("ORCA_CLI_COMMAND", "fullops-no-such-orca")],
        b"",
    );
    assert_eq!(missing.code, Some(4), "{}", missing.stderr);
    assert_eq!(state(sb)["dispatches"], json!([]));
    let starts = calls(sb, "worker-start");
    assert_eq!(starts.len(), 2);
    assert_ne!(
        flag(&starts[0], "--retry-request"),
        flag(&starts[1], "--retry-request")
    );
}

#[test]
fn sub_dispatch_stays_in_the_parents_worktree() {
    let mut d = Driver::new(&Setup::default());
    steps(&mut d, DISPATCH);
    let (lead_head, dev_head) = (head(&d.sb, "lead"), head(&d.sb, "dev"));
    steps(&mut d, SUBDISPATCH);
    let sb = &d.sb;
    let st = state(sb);
    let sub = &st["dispatches"][1];
    assert_eq!(
        (
            sub["id"].clone(),
            sub["sub_of"].clone(),
            sub["key"].clone(),
            sub["role"].clone()
        ),
        (json!("ctx_2"), json!("ctx_1"), json!("A-1"), json!("dev"))
    );
    assert_eq!(sub["brief"].as_str().map(str::len), Some(16));
    assert_eq!(sub["tuple"], st["dispatches"][0]["tuple"]);
    assert_eq!(st["runs"][1]["owner"], "dispatch:ctx_1");
    assert_eq!(st["runs"][1]["coordinator"], "term_1");
    // 글은 Orca로만 가고 상태 파일에는 없다
    assert!(!st.to_string().contains("src/b.py"));
    let start = &calls(sb, "worker-start")[1];
    let dev = sb.path("dev").to_string_lossy().replace('\\', "/");
    assert_eq!(flag(start, "--worktree"), Some(format!("path:{dev}")));
    assert_eq!(flag(start, "--run").as_deref(), Some("run_2"));
    let spec = start["stdin"].as_str().unwrap();
    assert!(
        spec.contains("; Sub of: ctx_1; Scope: src/b.py를 맡는다. 병렬 방침 on:"),
        "{spec}"
    );
    assert!(
        spec.ends_with("preamble의 worker_done을 한 번 보낸다")
            && spec.contains("커밋·lint·fullops done은 하지 않는다")
    );
    assert!(
        !spec.contains("--brief"),
        "하위 worker의 spec에는 하위 배정 문장이 없다"
    );
    // 인박스·커밋·push가 없다
    assert_eq!((head(sb, "lead"), head(sb, "dev")), (lead_head, dev_head));

    // 하위 보고는 completion을 만들지 않고 부모가 처리 표시한다
    sb.orca_mail(
        "run_2",
        orca_shapes::message(
            "msg_sub",
            "worker_done",
            "ctx_2",
            "task_2",
            "끝",
            "src/b.py를 고쳤다",
            Some("succeeded"),
        ),
    );
    let got = sb.fullops(
        &sb.path("dev"),
        &["wait", "--run", "run_2", "--json"],
        &[("ORCA_TERMINAL_HANDLE", "term_1")],
        b"",
    );
    let got = out(&got);
    assert_eq!(got["orca_wakes"], true);
    assert_eq!(got["actionable"][0]["ack_by"], "ack");
    let st = state(sb);
    assert_eq!(st["completions"], json!([]));
    assert_eq!(st["dispatches"][1]["settled"]["how"], "done");
    assert_eq!(calls(sb, "--wait").len(), 0);
}

#[test]
fn wait_takes_messages_that_arrive_between_peek_and_consume() {
    let mut d = Driver::new(&Setup::default());
    steps(&mut d, DISPATCH);
    let sb = &d.sb;
    let done = orca_shapes::message(
        "msg_1",
        "worker_done",
        "ctx_1",
        "task_1",
        "끝",
        "A-1 끝",
        Some("failed"),
    );
    let question = orca_shapes::message(
        "msg_2",
        "question",
        "ctx_1",
        "task_1",
        "질문",
        "어느 쪽",
        None,
    );
    sb.orca_rule(json!({"when": ["orchestration", "check", "--peek"], "times": 1, "stdout": orca_shapes::checked(vec![done.clone()])}));
    sb.orca_rule(json!({"when": ["orchestration", "check"], "times": 1, "stdout": orca_shapes::checked(vec![done, question])}));
    let got = out(&sb.cli(
        &sb.path("lead"),
        &["wait", "--run", "run_1", "--no-wait", "--json"],
    ));
    let ids: Vec<&str> = got["actionable"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["id"].as_str())
        .collect();
    assert_eq!(ids, ["msg_1", "msg_2"]);
    // 실패 보고와 질문은 lead가 처리한 뒤 --ack로 표시한다
    assert_eq!(got["actionable"][0]["outcome"], "failed");
    assert_eq!(
        (
            got["actionable"][0]["ack_by"].clone(),
            got["actionable"][1]["ack_by"].clone()
        ),
        (json!("ack"), json!("ack"))
    );
    assert_eq!(got["actionable"][1]["body"], "어느 쪽");
    assert!(
        got["next"]
            .as_str()
            .unwrap()
            .contains("--ack msg_1 --ack msg_2 --no-wait"),
        "{got}"
    );
    let st = state(sb);
    assert_eq!(st["messages"].as_array().map(Vec::len), Some(2));
    assert!(
        !st.to_string().contains("어느 쪽"),
        "본문은 저장하지 않는다"
    );
    // 기록이 소비보다 먼저다: --peek 다음에 소비형
    let checks: Vec<bool> = calls(sb, "check")
        .iter()
        .map(|c| c["argv"].to_string().contains("--peek"))
        .collect();
    assert_eq!(checks.get(..2), Some(&[true, false][..]));
}

#[test]
fn wait_killed_before_consuming_keeps_one_record() {
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        &format!("{DISPATCH}[[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"A-1\"\n"),
    );
    let sb = &d.sb;
    let done = orca_shapes::message(
        "msg_k",
        "worker_done",
        "ctx_1",
        "task_1",
        "끝",
        "A-1 끝",
        Some("succeeded"),
    );
    sb.orca_rule(json!({"when": ["orchestration", "check", "--peek"], "times": 1, "stdout": orca_shapes::checked(vec![done.clone()])}));
    sb.orca_rule(json!({"when": ["orchestration", "check"], "times": 1, "sleep_ms": 3000, "stdout": orca_shapes::checked(vec![done.clone()])}));
    let lead = sb.path("lead");
    let mut child = sb
        .command(bin("fullops"), &lead)
        .args(["wait", "--run", "run_1"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_until("소비형 check 호출", || calls(sb, "check").len() == 2);
    child.kill().unwrap();
    child.wait().unwrap();
    // 미리 보기에서 본 메시지는 이미 state에 있다
    let st = state(sb);
    assert_eq!(st["messages"][0]["id"], "msg_k");
    assert_eq!(st["completions"][0]["sha"], head(sb, "dev"));

    // 다시 실행하면 같은 메시지가 중복 없이 남아 있다
    sb.orca_mail("run_1", done);
    let got = out(&sb.cli(&lead, &["wait", "--run", "run_1", "--no-wait", "--json"]));
    assert_eq!(got["actionable"].as_array().map(Vec::len), Some(1));
    assert_eq!(got["actionable"][0]["ack_by"], "merge");
    let st = state(sb);
    assert_eq!(
        (
            st["messages"].as_array().map(Vec::len),
            st["completions"].as_array().map(Vec::len)
        ),
        (Some(1), Some(1))
    );
}

#[test]
fn wait_absorbs_heartbeats_and_only_waits_where_orca_cannot_wake() {
    let mut d = Driver::new(&Setup::default());
    steps(&mut d, DISPATCH);
    let sb = &d.sb;
    let lead = sb.path("lead");
    for id in ["hb_1", "hb_2", "hb_3"] {
        sb.orca_mail(
            "run_1",
            orca_shapes::message(id, "heartbeat", "ctx_1", "task_1", "", "", None),
        );
    }
    sb.orca_mail(
        "run_1",
        orca_shapes::message("st_1", "status", "ctx_1", "task_1", "", "", None),
    );
    // Orca가 깨우는 세션: run-use 뒤 바로 돌아오고 폴링과 대기 표식이 없다
    sb.orca_rule(json!({"when": ["orchestration", "run-use"], "times": 1, "stdout": orca_shapes::run_created("run_1")}));
    let woken = sb.fullops(
        &lead,
        &["wait", "--run", "run_1", "--json"],
        &[("ORCA_TERMINAL_HANDLE", "term_new")],
        b"",
    );
    let woken = out(&woken);
    assert_eq!(woken["absorbed"], json!({"heartbeats": 3, "status": 1}));
    assert_eq!(
        (woken["orca_wakes"].clone(), woken["idle_timeout"].clone()),
        (json!(true), json!(false))
    );
    assert_eq!(woken["actionable"], json!([]));
    assert_eq!(state(sb)["runs"][0]["coordinator"], "term_new");
    assert_eq!(state(sb)["messages"], json!([]));
    assert_eq!(
        (calls(sb, "--wait").len(), calls(sb, "worker-list").len()),
        (0, 0)
    );
    assert!(!lead.join(".git/fullops/waiters/run_1.json").exists());

    // run-use가 실패한 세션: 대기 표식을 쓰고 기다린 뒤 생존을 확인한다. 끝난 worker는 lost로 적는다
    sb.orca_rule(json!({"when": ["orchestration", "check", "--wait"], "times": 1, "sleep_ms": 1500, "stdout": {"ok": true, "result": {"messages": [], "timedOut": true}}}));
    sb.orca_rule(json!({"when": ["orchestration", "worker-list"], "stdout": {"ok": true, "result": {"workers": [
        orca_shapes::worker("run_1", "ctx_1", "task_1", "term_1", false, "exited")]}}}));
    let child = sb
        .command(bin("fullops"), &lead)
        .args(["wait", "--run", "run_1", "--max-minutes", "0", "--json"])
        .env("ORCA_TERMINAL_HANDLE", "term_other")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    wait_until("대기 표식", || {
        lead.join(".git/fullops/waiters/run_1.json").exists()
    });
    let finished = child.wait_with_output().unwrap();
    let waited: Value = serde_json::from_slice(&finished.stdout).unwrap();
    assert_eq!(
        (waited["orca_wakes"].clone(), waited["idle_timeout"].clone()),
        (json!(false), json!(true))
    );
    assert_eq!(
        waited["lost"],
        json!([{"dispatch": "ctx_1", "key": "A-1", "role": "dev"}])
    );
    assert_eq!(state(sb)["dispatches"][0]["settled"]["how"], "lost");
    let waits = calls(sb, "--wait");
    assert_eq!(waits.len(), 1);
    assert_eq!(
        flag(&waits[0], "--types").as_deref(),
        Some("worker_done,question,escalation,dispatch,merge_ready,handoff,decision_gate")
    );
    assert!(
        !lead.join(".git/fullops/waiters/run_1.json").exists(),
        "끝날 때 지운다"
    );
}

#[test]
fn done_without_hooks_reaches_the_lead_with_a_sha() {
    let mut d = Driver::new(&Setup {
        roles: [("dev", "worker"), ("qa", "worker")]
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .into(),
        ..Setup::default()
    });
    // 세션 핸들이 dispatch의 터미널과 같은 worker
    steps(
        &mut d,
        &format!(
            "{DISPATCH}[[step]]\nact = \"file\"\nwho = \"dev\"\npath = \"src/a.py\"\ncontent = \"print(1)\\n\"\n{}\
             [[step]]\nact = \"runs_fullops\"\nwho = \"dev\"\nraw = true\nargs = [\"done\", \"--no-checks\", \"시험\", \"--followup\", \"qa: 독립 QA\"]\nexpect = {{ verdict = \"pass\" }}\n",
            report("dev")
        ),
    );
    let st = state(&d.sb);
    let reported = &st["dispatches"][0]["reported"];
    assert_eq!(
        (reported["sha"].clone(), reported["branch"].clone()),
        (json!(head(&d.sb, "dev")), json!("fullops/dev"))
    );
    assert_eq!(
        reported["followups"],
        json!([{"role": "qa", "text": "독립 QA"}])
    );
    assert!(reported["lint_head"].is_null());
    assert_eq!(
        d.sb.git_ok(&d.sb.path("dev"), &["log", "-1", "--format=%B"]),
        "fullops: 완료 기록 A-1 (dev)\n\nlint 생략: 시험"
    );
    assert!(
        d.sb.path("dev")
            .join(".fullops-squad/handovers/logs/A-1.dev.r0.md")
            .is_file()
    );
    assert_eq!(
        d.sb.git_ok(&d.sb.path("dev"), &["rev-parse", "origin/fullops/dev"]),
        head(&d.sb, "dev")
    );

    // dispatch 기록이 없는 worker: 본문으로만 전달된다
    let finished = steps(
        &mut d,
        &format!(
            "[[step]]\nact = \"lead_routes\"\nkey = \"C-3\"\nrole = \"qa\"\n[[step]]\nact = \"lead_writes_task\"\nkey = \"C-3\"\nrole = \"qa\"\n\
             [[step]]\nact = \"commits\"\n[[step]]\nact = \"git\"\nwho = \"qa\"\nargs = [\"merge\", \"-q\", \"main\"]\n{}\
             [[step]]\nact = \"runs_fullops\"\nwho = \"qa\"\nraw = true\nargs = [\"done\"]\nexpect = {{ verdict = \"pass\", escape = \"dispatch를 찾지 못해\" }}\n",
            report("qa")
        ),
    );
    let sb = &d.sb;
    let body = finished
        .last()
        .unwrap()
        .output
        .lines()
        .find(|l| l.starts_with("[완료] C-3"))
        .unwrap()
        .to_string();
    assert!(
        body.contains(&format!("SHA {}", head(sb, "qa"))) && body.contains("브랜치 fullops/qa"),
        "{body}"
    );

    sb.orca_mail(
        "run_1",
        orca_shapes::message(
            "msg_a",
            "worker_done",
            "ctx_1",
            "task_1",
            "끝",
            "",
            Some("succeeded"),
        ),
    );
    sb.orca_mail(
        "run_1",
        orca_shapes::message(
            "msg_c",
            "worker_done",
            "ctx_gone",
            "task_gone",
            "끝",
            &body,
            Some("succeeded"),
        ),
    );
    let got = out(&sb.cli(
        &sb.path("lead"),
        &["wait", "--run", "run_1", "--no-wait", "--json"],
    ));
    let by_id = |id: &str| {
        got["actionable"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == id)
            .cloned()
            .unwrap()
    };
    let (a, c) = (by_id("msg_a"), by_id("msg_c"));
    assert_eq!(
        (a["sha"].clone(), a["sha_source"].clone(), a["key"].clone()),
        (json!(head(sb, "dev")), json!("done"), json!("A-1"))
    );
    assert_eq!(
        (c["sha"].clone(), c["sha_source"].clone(), c["key"].clone()),
        (json!(head(sb, "qa")), json!("body"), json!("C-3"))
    );
    assert_eq!(
        (a["ack_by"].clone(), c["ack_by"].clone()),
        (json!("merge"), json!("merge"))
    );
    let next = got["next"].as_str().unwrap();
    assert!(
        next.contains("`fullops merge A-1`") && next.contains("`fullops merge C-3`"),
        "{next}"
    );
}

#[test]
fn done_interrupted_at_push_finishes_on_rerun() {
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        &format!(
            "{DISPATCH}[[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"A-1\"\n{}",
            report("dev")
        ),
    );
    let dev = d.sb.path("dev");
    let origin = d.sb.git_ok(&dev, &["remote", "get-url", "origin"]);
    let missing = d.sb.root.join("missing.git").to_string_lossy().into_owned();
    d.sb.git_ok(&dev, &["remote", "set-url", "origin", &missing]);
    // push에서 끊긴다: 완료 기록 커밋은 남고 state에는 아직 없다
    steps(
        &mut d,
        "[[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nno_checks = \"시험\"\nexpect = { verdict = \"external\", exit = 4 }\n",
    );
    assert_eq!(
        d.sb.git_ok(&dev, &["log", "-1", "--format=%s"]),
        "fullops: 완료 기록 A-1 (dev)"
    );
    assert!(state(&d.sb)["dispatches"][0]["reported"].is_null());
    let recorded = head(&d.sb, "dev");

    // 다시 실행하면 R-06을 건너뛰고 push와 기록, 본문 출력을 마친다
    d.sb.git_ok(&dev, &["remote", "set-url", "origin", &origin]);
    steps(
        &mut d,
        "[[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nexpect = { verdict = \"pass\", escape = \"[완료] A-1 | 브랜치 fullops/dev | SHA \" }\n",
    );
    assert_eq!(head(&d.sb, "dev"), recorded, "완료 기록 커밋은 하나다");
    assert_eq!(state(&d.sb)["dispatches"][0]["reported"]["sha"], recorded);
    assert_eq!(
        d.sb.git_ok(&dev, &["rev-parse", "origin/fullops/dev"]),
        recorded
    );
}

#[test]
fn review_dispatch_goes_to_the_snapshot_in_a_separate_session() {
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        &format!(
            "{DISPATCH}[[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"A-1\"\n{}\
             [[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nno_checks = \"시험\"\n[[step]]\nact = \"orca_sends\"\n[[step]]\nact = \"lead_waits\"\n\
             [[step]]\nact = \"runs_fullops\"\nargs = [\"route\", \"A-1\", \"--tuple-only\", \"--role\", \"reviewer\", \"--agent\", \"codex\", \"--model\", \"rm\", \"--effort\", \"high\"]\nexpect = {{ verdict = \"pass\" }}\n",
            report("dev")
        ),
    );
    // `review snapshot`(묶음 E)이 하는 일을 손으로 한다: detached 워크트리와 state의 active snapshot
    let sha = head(&d.sb, "dev");
    let lead = d.sb.path("lead");
    let snapshot =
        d.sb.root
            .join("review-1")
            .to_string_lossy()
            .replace('\\', "/");
    d.sb.git_ok(
        &lead,
        &["worktree", "add", "-q", "--detach", &snapshot, &sha],
    );
    let state_path = lead.join(".git/fullops/state.json");
    let mut st = state(&d.sb);
    st["snapshots"] = json!([{"review": format!("A-1@{}", &sha[..12]), "path": snapshot, "head": sha, "base": head(&d.sb, "lead"),
        "state": "active", "implementer_session": "sid-dev", "reviewer_dispatch": null, "owner": "lead", "created_at": 1, "hold": null}]);
    std::fs::write(&state_path, st.to_string()).unwrap();

    let review = |extra: &str, expect: &str| {
        format!(
            "[[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\npurpose = \"review\"\nsha = \"{sha}\"\nbare = true\n{extra}expect = {expect}\n"
        )
    };
    steps(
        &mut d,
        &review(
            "terminal = \"term_1\"\n",
            "{ verdict = \"deny\", rule = \"D-06\" }",
        ),
    );
    steps(&mut d, &review("", "{ verdict = \"pass\" }"));
    let start = calls(&d.sb, "worker-start").pop().unwrap();
    assert_eq!(flag(&start, "--worktree"), Some(format!("path:{snapshot}")));
    assert_eq!(flag(&start, "--agent").as_deref(), Some("codex"));
    let spec = start["stdin"].as_str().unwrap();
    assert!(spec.contains(&format!("; Review SHA: {sha}; Implementer session: sid-dev; Reviewer session: separate; Review folder: ")), "{spec}");
    assert!(
        spec.contains(&format!(
            ".fullops-squad/reviews/A-1/{}; Read only: true. ",
            &sha[..12]
        )) && spec.contains("Purpose: review")
    );
    assert!(!spec.contains("Handover:") && spec.contains("[리뷰] A-1"));
    // 리뷰 보고는 completion을 만들지 않는다
    let reviewer = state(&d.sb)["dispatches"][1].clone();
    assert_eq!(
        (
            reviewer["purpose"].clone(),
            reviewer["tuple"]["agent"].clone()
        ),
        (json!("review"), json!("codex"))
    );
    let (id, task) = (
        reviewer["id"].as_str().unwrap(),
        reviewer["task_id"].as_str().unwrap(),
    );
    d.sb.orca_mail(
        "run_1",
        orca_shapes::message(
            "msg_r",
            "worker_done",
            id,
            task,
            "리뷰",
            "[리뷰] A-1",
            Some("succeeded"),
        ),
    );
    let got = out(&d
        .sb
        .cli(&lead, &["wait", "--run", "run_1", "--no-wait", "--json"]));
    let review_msg = got["actionable"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == "msg_r")
        .unwrap();
    assert_eq!(review_msg["ack_by"], "ack");
    assert_eq!(
        state(&d.sb)["completions"].as_array().map(Vec::len),
        Some(1)
    );

    // snapshot이 깨끗하지 않으면 배정하지 않는다
    d.sb.write(Path::new(&snapshot).join("stray.txt").as_path(), "x\n");
    steps(
        &mut d,
        &review(
            "",
            "{ verdict = \"deny\", rule = \"D-06\", escape = \"fullops review snapshot A-1\" }",
        ),
    );
}

#[test]
fn route_classifies_through_curl_and_falls_back() {
    let mut d = Driver::new(&Setup {
        roles: [("dev", "worker"), ("architecture", "design")]
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .into(),
        ..Setup::default()
    });
    steps(&mut d, "[[step]]\nact = \"fakes\"\nnames = [\"curl\"]\n");
    let sb = &d.sb;
    let lead = sb.path("lead");
    let reply = |simple: f64, design: f64| {
        json!({"model": "jev-1.14.0", "usage": {"input_tokens": 120, "output_tokens": 9},
               "answers": {"scope": {"type": "choice", "choice": "simple", "probabilities": {"simple": simple, "design": design}, "confidence": 0.8},
                           "role": {"type": "choice", "choice": "dev", "probabilities": {"dev": 1.0}, "confidence": 1.0}}})
    };
    for stdout in [reply(0.9, 0.1), reply(0.5, 0.5), reply(0.9, 0.6)] {
        sb.curl_rule(json!({"when": ["--config", "-"], "times": 1, "stdout": stdout}));
    }
    // 가짜 curl이 PATH 맨 앞에 있어야 한다(실제 curl로 나가지 않는다)
    assert!(
        sb.bin_dir
            .join(format!("curl{}", std::env::consts::EXE_SUFFIX))
            .is_file()
    );
    let online = [("FULLOPS_OFFLINE", "0"), ("TYPESAFE_API_KEY", "test-key")];
    let route = |key: &str, request: &str, env: &[(&str, &str)], more: &[&str]| {
        let mut args = vec!["route", key, "--request", request, "--json"];
        args.extend(more);
        let run = sb.fullops(&lead, &args, env, b"");
        (out(&run), run.stderr)
    };

    // 확신이 높으면 그 역할에 바로 간다
    let (direct, _) = route("A-1", "로그인 버튼의 색을 바꾼다", &online, &[]);
    assert_eq!(
        (
            direct["class"].clone(),
            direct["role"].clone(),
            direct["decided_by"].clone()
        ),
        (json!("direct"), json!("dev"), json!("jev"))
    );
    assert_eq!(direct["probabilities"], json!({"scope": 0.9, "role": 1.0}));
    let sent = &sb.curl_calls()[0];
    let argv = sent["argv"].to_string();
    assert!(
        argv.contains("--config") && !argv.contains("test-key") && !argv.contains("로그인"),
        "키와 본문은 인자에 없다: {argv}"
    );
    let config = sent["stdin"].as_str().unwrap();
    assert!(
        config.contains("url = \"https://api.typesafe.ai/v1/systemone\"")
            && config.contains("header = \"Authorization: Bearer test-key\"")
    );
    assert!(
        config.contains("\\\"model\\\":\\\"jev-latest\\\"")
            && config.contains("로그인 버튼의 색을 바꾼다")
            && config.contains("\\\"criteria\\\":{\\\"design\\\":null,\\\"simple\\\":null}"),
        "{config}"
    );

    // 같은 요청은 캐시에서 읽는다
    let (cached, _) = route("A-1", "로그인 버튼의 색을 바꾼다", &online, &["--force"]);
    assert_eq!(
        (
            cached["decided_by"].clone(),
            cached["history"].as_array().map(Vec::len)
        ),
        (json!("jev"), Some(1))
    );
    assert_eq!(sb.curl_calls().len(), 1);

    // 확신이 낮으면 설계 역할로 간다
    let (design, _) = route("B-2", "결제 구조를 다시 잡는다", &online, &[]);
    assert_eq!(
        (design["class"].clone(), design["role"].clone()),
        (json!("design"), json!("architecture"))
    );

    // 잘못된 분포, 키 없음, 오프라인, 비밀값이 든 요청은 모두 폴백이고 종료 코드 0이다
    let (wrong, warned) = route("C-3", "분포가 틀린 답을 받는다", &online, &[]);
    assert_eq!(
        (wrong["class"].clone(), wrong["decided_by"].clone()),
        (json!("design"), json!("fallback"))
    );
    assert!(warned.contains("폴백"), "{warned}");
    assert_eq!(sb.curl_calls().len(), 3);
    for (key, request, env) in [
        ("D-4", "키가 없다", &[("FULLOPS_OFFLINE", "0")][..]),
        ("E-5", "오프라인이다", &[][..]),
        ("F-6", "password: hunter2 를 고친다", &online[..]),
    ] {
        let (fallback, _) = route(key, request, env, &[]);
        assert_eq!(fallback["decided_by"], "fallback", "{key}");
    }
    assert_eq!(sb.curl_calls().len(), 3, "폴백은 curl을 부르지 않는다");

    // 레포의 .fullops-squad/.env에서 찾은 키는 그 제공자의 엔드포인트로 간다
    sb.write(
        &lead.join(".fullops-squad/.env"),
        "# 키\nOPENROUTER_API_KEY=\"or-key\"\n",
    );
    sb.curl_rule(json!({"when": ["--config", "-"], "times": 1, "stdout": reply(0.95, 0.05)}));
    let (routed, _) = route(
        "G-7",
        "레포의 키로 묻는다",
        &[("FULLOPS_OFFLINE", "0")],
        &[],
    );
    assert_eq!(routed["decided_by"], "jev");
    let config = sb.curl_calls()[3]["stdin"].as_str().unwrap().to_string();
    assert!(
        config.contains("https://openrouter.ai/api/v1/systemone")
            && config.contains("Bearer or-key")
            && config.contains("~typesafe/jev-latest"),
        "{config}"
    );
}

#[test]
fn route_rules_and_tuple_records() {
    let sb = Sandbox::with(&Setup::default());
    let (lead, dev) = (sb.path("lead"), sb.path("dev"));
    let run = |cwd: &Path, args: &[&str]| sb.cli(cwd, args);
    let refused = |run: Run, rule: &str| {
        assert_eq!(run.code, Some(2), "{}", run.stderr);
        assert!(
            run.stderr.contains(&format!("FullOps[{rule}]")),
            "{}",
            run.stderr
        );
    };
    refused(
        run(
            &dev,
            &[
                "route",
                "A-1",
                "--request",
                "x",
                "--class",
                "direct",
                "--role",
                "dev",
            ],
        ),
        "RT-01",
    );
    refused(run(&lead, &["route", "A 1", "--request", "x"]), "RT-02");
    refused(
        run(&lead, &["route", "A-1", "--request", &"가".repeat(4001)]),
        "RT-03",
    );
    refused(
        run(
            &lead,
            &["route", "A-1", "--request", "x", "--class", "direct"],
        ),
        "RT-04",
    );
    refused(
        run(
            &lead,
            &[
                "route",
                "A-1",
                "--request",
                "x",
                "--class",
                "direct",
                "--role",
                "dev",
                "--agent",
                "claude",
            ],
        ),
        "RT-05",
    );

    let made = out(&run(
        &lead,
        &[
            "route",
            "A-1",
            "--request",
            "x",
            "--class",
            "direct",
            "--role",
            "dev",
            "--deliverable",
            "D03",
            "--json",
        ],
    ));
    assert_eq!(
        (
            made["class"].clone(),
            made["decided_by"].clone(),
            made["deliverables"].clone()
        ),
        (json!("direct"), json!("manual"), json!(["D03"]))
    );
    assert_eq!(made["requires_packet"], false);
    // 기록이 있으면 호출하지 않고 기존 기록을 낸다
    let kept = out(&run(
        &lead,
        &[
            "route",
            "A-1",
            "--request",
            "다른 글",
            "--class",
            "unresolved",
            "--json",
        ],
    ));
    assert_eq!(kept["class"], "direct");
    // 재작업 키는 새로 분류하지 않는다
    let rework = run(
        &lead,
        &[
            "route",
            "A-1-FIX",
            "--request",
            "x",
            "--class",
            "direct",
            "--role",
            "dev",
        ],
    );
    assert!(
        rework.stderr.contains("fullops task reopen"),
        "{}",
        rework.stderr
    );
    refused(rework, "RT-02");
    assert_eq!(
        run(
            &lead,
            &[
                "route",
                "A-1-FIX",
                "--request",
                "x",
                "--class",
                "direct",
                "--role",
                "dev",
                "--distinct"
            ]
        )
        .code,
        Some(0)
    );

    // --tuple-only는 분류 없이 tuple만 쓴다. 기록이 없으면 untracked다
    let tuple = out(&run(
        &lead,
        &[
            "route",
            "R-9",
            "--tuple-only",
            "--role",
            "reviewer",
            "--agent",
            "antigravity",
            "--model",
            "m",
            "--effort",
            "high",
            "--why",
            "리뷰",
            "--json",
        ],
    ));
    assert_eq!(tuple["class"], "untracked");
    assert_eq!(
        tuple["tuples"]["reviewer"],
        json!({"agent": "antigravity", "model": "m", "effort": "high", "why": "리뷰"})
    );

    // 1.24.0 기록에서 켜져 있던 requires_packet만 옮기고, 재분류는 값을 잇는다
    sb.write(
        &lead.join(".fullops-squad/docs/evaluations/jev/P-6-route.json"),
        "{\"requires_packet\": true}",
    );
    let moved = out(&run(
        &lead,
        &[
            "route",
            "P-6",
            "--request",
            "x",
            "--class",
            "direct",
            "--role",
            "dev",
            "--json",
        ],
    ));
    assert_eq!(moved["requires_packet"], true);
    let again = out(&run(
        &lead,
        &[
            "route",
            "P-6",
            "--request",
            "y",
            "--class",
            "unresolved",
            "--force",
            "--json",
        ],
    ));
    assert_eq!(
        (
            again["requires_packet"].clone(),
            again["class"].clone(),
            again["history"][0]["class"].clone()
        ),
        (json!(true), json!("unresolved"), json!("direct"))
    );
}

#[test]
fn task_new_reopen_and_recover() {
    let mut d = Driver::new(&Setup {
        roles: [("dev", "worker"), ("architecture", "design")]
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .into(),
        ..Setup::default()
    });
    // design 과제의 타 역할 지시서는 lead가 쓰지 못한다
    steps(
        &mut d,
        "[[step]]\nact = \"lead_routes\"\nkey = \"Z-9\"\nrole = \"architecture\"\nclass = \"design\"\n\
         [[step]]\nact = \"lead_writes_task\"\nkey = \"Z-9\"\nrole = \"dev\"\nexpect = { verdict = \"deny\", rule = \"H-02\" }\n\
         [[step]]\nact = \"lead_writes_task\"\nkey = \"Q-0\"\nrole = \"dev\"\nexpect = { verdict = \"deny\", rule = \"H-01\", escape = \"fullops route\" }\n",
    );
    steps(&mut d, DISPATCH);
    let sb = &d.sb;
    let (lead, dev) = (sb.path("lead"), sb.path("dev"));
    let inbox = std::fs::read_to_string(dev.join(".fullops-squad/handovers/to_dev.md")).unwrap();
    let (front, body) = fullops_core::parse_front_matter(&inbox).unwrap();
    let parsed = fullops_core::frontmatter::inbox_of(&front).expect("인박스 전용 필드 열 개");
    assert_eq!(
        (
            parsed.key.as_str(),
            parsed.role.as_str(),
            parsed.round,
            parsed.budget,
            parsed.from.as_str()
        ),
        ("A-1", "dev", 0, 2, "lead")
    );
    assert_eq!(front.get("id"), Some("handover-dev"));
    assert_eq!(front.get("title"), Some("A-1 — A-1 구현"));
    assert!(inbox[body..].starts_with("# A-1 — A-1 구현\n") && inbox.contains("## 완료 보고"));

    // recover: 미커밋 변경을 WIP 커밋으로 보존하고 기본 브랜치를 병합한다
    sb.write(&dev.join("src/wip.py"), "print(1)\n");
    sb.write(&lead.join("docs/later.md"), "뒤에 온 문서\n");
    sb.git_ok(&lead, &["add", "-A"]);
    sb.git_ok(&lead, &["commit", "-q", "-m", "기본 브랜치가 앞서 간다"]);
    let recovered = out(&sb.cli(
        &dev,
        &["task", "recover", "--role", "dev", "--key", "A-1", "--json"],
    ));
    assert!(recovered["wip"].is_string() && recovered["packet"].is_null());
    assert_eq!(
        sb.git_ok(&dev, &["log", "--format=%s", "--grep", "wip"]),
        "wip(dev): A-1"
    );
    assert!(
        dev.join("docs/later.md").is_file()
            && sb.git_ok(&dev, &["status", "--porcelain"]).is_empty()
    );

    // 충돌하면 병합을 되돌리고 WIP 커밋은 남긴다
    sb.write(&lead.join("README.md"), "lead가 고친 글\n");
    sb.git_ok(&lead, &["commit", "-q", "-am", "README"]);
    sb.write(&dev.join("README.md"), "worker가 고친 글\n");
    let conflict = sb.cli(&dev, &["task", "recover", "--role", "dev", "--key", "A-1"]);
    assert_eq!(conflict.code, Some(3), "{}", conflict.stderr);
    assert_eq!(
        sb.git_ok(&dev, &["log", "-1", "--format=%s"]),
        "wip(dev): A-1"
    );
    assert!(
        sb.git(&dev, &["rev-parse", "--verify", "-q", "MERGE_HEAD"])
            .code
            != Some(0)
    );

    // reopen: 완료 기록이 있어야 하고 차수와 재작업 수가 오른다
    let none = sb.cli(
        &lead,
        &[
            "task", "reopen", "--role", "dev", "--key", "A-1", "--goal", "수정",
        ],
    );
    assert_eq!(none.code, Some(2), "인박스가 남아 있다: {}", none.stderr);
    std::fs::rename(lead.join(".fullops-squad/handovers/to_dev.md"), {
        std::fs::create_dir_all(lead.join(".fullops-squad/handovers/logs")).unwrap();
        lead.join(".fullops-squad/handovers/logs/A-1.dev.r0.md")
    })
    .unwrap();
    let dup = sb.cli(
        &lead,
        &[
            "task", "new", "--role", "dev", "--key", "A-1", "--goal", "다시",
        ],
    );
    assert!(
        dup.code == Some(2)
            && dup.stderr.contains("FullOps[H-04]")
            && dup.stderr.contains("fullops task reopen"),
        "{}",
        dup.stderr
    );
    let reopened = out(&sb.cli(
        &lead,
        &[
            "task", "reopen", "--role", "dev", "--key", "A-1", "--goal", "수정", "--json",
        ],
    ));
    assert_eq!(reopened["round"], 1);
    assert_eq!(state(sb)["tasks"]["A-1"]["rounds"], 1);
}

#[test]
fn plan_edits_only_the_task_table() {
    let sb = Sandbox::with(&Setup::default());
    let lead = sb.path("lead");
    let run = |args: &[&str]| sb.cli(&lead, args);
    assert_eq!(
        run(&[
            "plan",
            "add",
            "A-1",
            "--role",
            "dev",
            "--goal",
            "목표 | 파이프",
            "--source",
            "사용자 지시"
        ])
        .code,
        Some(0)
    );
    assert_eq!(
        run(&[
            "plan",
            "add",
            "B-2",
            "--role",
            "dev",
            "--goal",
            "둘째",
            "--source",
            "사용자 지시",
            "--after",
            "A-1,C-3"
        ])
        .code,
        Some(0)
    );
    assert_eq!(
        run(&[
            "plan",
            "add",
            "C-3",
            "--role",
            "dev",
            "--goal",
            "셋째",
            "--source",
            "사용자 지시",
            "--status",
            "대기(사람: 배포)"
        ])
        .code,
        Some(0)
    );
    assert_eq!(
        run(&[
            "plan", "add", "A-1", "--role", "dev", "--goal", "중복", "--source", "x"
        ])
        .code,
        Some(1)
    );
    let rows = out(&run(&["plan", "list", "--json"]))["rows"].clone();
    assert_eq!(rows.as_array().map(Vec::len), Some(3));
    assert_eq!(
        (
            rows[0]["goal"].clone(),
            rows[0]["status"].clone(),
            rows[0]["role"].clone()
        ),
        (json!("목표 | 파이프"), json!("대기"), json!("dev"))
    );
    assert_eq!(rows[1]["after"], json!(["A-1", "C-3"]));
    assert_eq!(rows[2]["status"], "대기(사람: 배포)");
    // 선행이 끝나지 않은 행과 사람 대기 행은 준비된 행이 아니다
    assert_eq!(
        out(&run(&["plan", "ready", "--json"]))["ready"],
        json!(["A-1"])
    );
    assert_eq!(run(&["plan", "set", "A-1", "보류(나중에)"]).code, Some(0));
    assert_eq!(out(&run(&["plan", "ready", "--json"]))["ready"], json!([]));
    assert_eq!(out(&run(&["plan", "check", "--json"]))["errors"], json!([]));
    let text = std::fs::read_to_string(lead.join(".fullops-squad/PLANS.md")).unwrap();
    assert!(text.starts_with("# 현재 작업\n\n| 과제 키 | 목표 | 담당 | 상태 | 지시서·원천 링크 |\n|---|---|---|---|---|\n| A-1 | 목표 \\| 파이프 | dev | 보류(나중에) | 사용자 지시 |\n"), "{text}");
    assert!(text.ends_with("\n\n## 위임 범위\n\n"), "{text}");
}

#[test]
fn status_reports_metrics_landing_and_the_first_lines() {
    let mut d = Driver::new(&Setup::default());
    steps(&mut d, &format!("{DISPATCH}{SUBDISPATCH}"));
    let sb = &d.sb;
    let (lead, dev) = (sb.path("lead"), sb.path("dev"));
    // hook(묶음 B)이 남기는 턴 줄을 손으로 둔다
    let turns: String = [
        "human",
        "subagent",
        "subagent",
        "subagent",
        "workflow",
        "machine:orca-message",
    ]
    .iter()
    .map(|kind| format!("{{\"at\":1,\"sid\":\"sid-lead\",\"kind\":\"{kind}\"}}\n"))
    .collect();
    sb.write(&lead.join(".git/fullops/turns.jsonl"), &turns);
    let status = out(&sb.cli(&lead, &["status", "--json"]));
    let m = &status["metrics"];
    assert_eq!(
        (
            m["human_turns"].clone(),
            m["subagent_calls"].clone(),
            m["workflow_runs"].clone(),
            m["sub_dispatches"].clone()
        ),
        (json!(1), json!(3), json!(1), json!(1))
    );
    assert!(m["at"].as_i64().unwrap() > 0 && m["turns_per_task"].is_null());
    assert_eq!(status["dispatches"][1]["sub"], true);
    assert_eq!(
        status["tasks"].as_array().map(Vec::len),
        Some(1),
        "하위 배정은 과제로 세지 않는다"
    );

    // 첫머리: 역할, 인박스, 병렬 방침. lead에게는 열린 Run이 이어진다
    let human = sb.cli(&lead, &["status"]).stdout;
    assert!(
        human.starts_with("역할: lead")
            && human.contains("병렬 방침 on: 이 과제는 병렬 진행을 요청한다")
            && human.contains("열린 Run: run_1"),
        "{human}"
    );
    let worker = sb
        .fullops(
            &dev,
            &["status"],
            &[("ORCA_TERMINAL_HANDLE", "term_2")],
            b"",
        )
        .stdout;
    assert!(
        worker.starts_with("역할: dev(worker)")
            && worker.contains("하위 worker다")
            && worker.contains("인박스: .fullops-squad/handovers/to_dev.md (과제 A-1)"),
        "{worker}"
    );

    // 통합 위치: 병합 전 → push 전 → 통합됨(처리 표시까지)
    sb.write(&dev.join("src/a.py"), "print(1)\n");
    sb.git_ok(&dev, &["add", "-A"]);
    sb.git_ok(&dev, &["commit", "-q", "-m", "A-1 구현"]);
    sb.orca_mail(
        "run_1",
        orca_shapes::message(
            "msg_1",
            "worker_done",
            "ctx_1",
            "task_1",
            "끝",
            "",
            Some("succeeded"),
        ),
    );
    out(&sb.cli(&lead, &["wait", "--run", "run_1", "--no-wait", "--json"]));
    let landing =
        |sb: &Sandbox| out(&sb.cli(&lead, &["status", "--json"]))["pending"][0]["landing"].clone();
    assert_eq!(landing(sb), "merge_left");
    sb.git_ok(&lead, &["merge", "-q", "--no-edit", "fullops/dev"]);
    assert_eq!(landing(sb), "push_left");
    sb.git_ok(&lead, &["push", "-q", "origin", "main"]);
    let done = out(&sb.cli(&lead, &["status", "--json"]));
    assert_eq!(done["pending"], json!([]));
    let st = state(sb);
    assert!(st["completions"][0]["landed_at"].is_i64() && st["messages"][0]["acked_at"].is_i64());
    assert_eq!(done["tasks"][0]["stage"], "merged");
}

#[test]
fn dev_direct_done_records_a_direct_completion() {
    let mut d = Driver::new(&Setup {
        mode: "dev".into(),
        primary_role: Some("dev".into()),
        ..Setup::default()
    });
    // dev 주 담당은 route 없이 자기 인박스를 만들고 직접 구현한다
    steps(
        &mut d,
        &format!(
            r#"[[step]]
act = "runs_fullops"
args = ["task", "new", "--role", "dev", "--key", "A-1", "--goal", "직접 구현"]
expect = {{ verdict = "pass" }}
[[step]]
act = "file"
path = "src/a.py"
content = "print(1)\n"
{}[[step]]
act = "runs_fullops"
args = ["done"]
expect = {{ verdict = "deny", rule = "R-03" }}
[[step]]
act = "runs_fullops"
args = ["done", "--no-checks", "시험", "--json"]
expect = {{ verdict = "pass" }}
"#,
            report("lead").replace("to_lead.md", "to_dev.md")
        ),
    );
    let sb = &d.sb;
    let lead = sb.path("lead");
    let sha = head(sb, "lead");
    let st = state(sb);
    let c = &st["completions"][0];
    assert_eq!(
        (
            c["id"].clone(),
            c["direct"].clone(),
            c["key"].clone(),
            c["sha"].clone()
        ),
        (
            json!(format!("direct:{}", &sha[..12])),
            json!(true),
            json!("A-1"),
            json!(sha)
        )
    );
    assert_eq!(
        sb.git_ok(&lead, &["rev-parse", "origin/main"]),
        sha,
        "주 담당의 브랜치로 push한다"
    );
    assert!(
        lead.join(".fullops-squad/handovers/logs/A-1.dev.r0.md")
            .is_file()
    );
    // 직접 작업은 통합 대기로 세지 않는다
    assert_eq!(
        out(&sb.cli(&lead, &["status", "--json"]))["pending"],
        json!([])
    );
    // 보낼 것이 없는 워크트리에서는 알리기만 한다
    let idle = sb.cli(&lead, &["done"]);
    assert_eq!(idle.code, Some(0), "{}", idle.stderr);
}

#[test]
fn dispatch_publishes_from_a_coordinator_branch_and_guards_the_baseline() {
    let roles = [
        ("dev", "worker"),
        ("art", "worker"),
        ("coor", "coordinator"),
    ];
    let sb = Sandbox::with(&Setup {
        roles: roles.map(|(a, b)| (a.to_string(), b.to_string())).into(),
        ..Setup::default()
    });
    let (lead, coor) = (sb.path("lead"), sb.path("coor"));
    let prepare = |key: &str, role: &str, agent: &str| {
        let routed = sb.cli(
            &coor,
            &[
                "route",
                key,
                "--request",
                "요청",
                "--class",
                "direct",
                "--role",
                role,
                "--agent",
                agent,
                "--model",
                "m",
                "--effort",
                "medium",
            ],
        );
        assert_eq!(routed.code, Some(0), "{}", routed.stderr);
        let tasked = sb.cli(
            &coor,
            &[
                "task", "new", "--role", role, "--key", key, "--goal", "목표",
            ],
        );
        assert_eq!(tasked.code, Some(0), "{}", tasked.stderr);
    };
    let started = |n: u32| {
        sb.orca_rule(json!({"when": ["orchestration", "worker-start"], "times": 1,
            "stdout": orca_shapes::worker_started("run_1", &format!("ctx_{n}"), &format!("task_{n}"), Some(&format!("term_{n}")))}));
    };
    sb.orca_rule(json!({"when": ["orchestration", "run-create"], "times": 1, "stdout": orca_shapes::run_created("run_1")}));

    // coordinator 브랜치의 준비 커밋이 기본 브랜치 워크트리(ff)와 원격, 대상 워크트리로 간다
    prepare("A-1", "dev", "claude");
    started(1);
    let run = sb.cli(&coor, &["dispatch", "A-1"]);
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert_eq!(head(&sb, "lead"), head(&sb, "coor"));
    assert_eq!(
        sb.git_ok(&lead, &["rev-parse", "origin/main"]),
        head(&sb, "coor")
    );
    assert!(
        sb.path("dev")
            .join(".fullops-squad/handovers/to_dev.md")
            .is_file()
    );

    // 기본 브랜치 워크트리가 더러우면 게시하지 않는다
    prepare("B-2", "art", "grok");
    sb.write(&lead.join("stray.txt"), "x\n");
    let dirty = sb.cli(&coor, &["dispatch", "B-2"]);
    assert!(
        dirty.code == Some(2) && dirty.stderr.contains("FullOps[publish]"),
        "{}",
        dirty.stderr
    );
    std::fs::remove_file(lead.join("stray.txt")).unwrap();

    // grok worker는 원본 레포의 폴더 신뢰가 없으면 알린다(막지 않는다)
    started(2);
    let untrusted = sb.cli(&coor, &["dispatch", "B-2"]);
    assert!(
        untrusted.code == Some(0)
            && untrusted
                .stderr
                .contains("grok worker는 원본 레포의 폴더 신뢰가 필요하다"),
        "{}",
        untrusted.stderr
    );
    let grok_home = sb.root.join("grok-home");
    let root = path_slashes(&sb.root);
    sb.write(
        &grok_home.join("trusted_folders.toml"),
        &format!("[folders.'{root}']\ntrusted = true\ndecided_at = 1\n"),
    );
    started(3);
    let trusted = sb.fullops(
        &coor,
        &["dispatch", "B-2", "--purpose", "fix"],
        &[("GROK_HOME", &grok_home.to_string_lossy())],
        b"",
    );
    assert!(
        trusted.code == Some(0) && !trusted.stderr.contains("폴더 신뢰"),
        "{}",
        trusted.stderr
    );

    // 원격의 기본 브랜치가 앞서 있으면 대상이 통합 기준을 담지 못한다
    sb.git_ok(&lead, &["checkout", "-q", "-b", "ahead"]);
    sb.write(&lead.join("docs/ahead.md"), "원격에만 있는 문서\n");
    sb.git_ok(&lead, &["add", "-A"]);
    sb.git_ok(&lead, &["commit", "-q", "-m", "원격이 앞서 간다"]);
    sb.git_ok(&lead, &["push", "-q", "origin", "ahead:main"]);
    sb.git_ok(&lead, &["checkout", "-q", "main"]);
    let behind = sb.cli(&coor, &["dispatch", "B-2", "--purpose", "fix"]);
    assert!(
        behind.code == Some(2)
            && behind.stderr.contains("FullOps[D-11]")
            && behind.stderr.contains("fullops settle"),
        "{}",
        behind.stderr
    );
    assert_eq!(calls(&sb, "worker-start").len(), 3);
}

#[test]
fn dispatch_stops_when_the_target_conflicts_with_the_base() {
    let mut d = Driver::new(&Setup::default());
    steps(&mut d, DISPATCH);
    let sb = &d.sb;
    let (lead, dev) = (sb.path("lead"), sb.path("dev"));
    for (dir, text) in [(&lead, "lead가 고친 글\n"), (&dev, "worker가 고친 글\n")] {
        sb.write(&dir.join("README.md"), text);
        sb.git_ok(dir, &["commit", "-q", "-am", "README"]);
    }
    let run = sb.cli(&lead, &["dispatch", "A-1", "--purpose", "fix"]);
    assert_eq!(run.code, Some(3), "{}", run.stderr);
    assert!(
        run.stderr.contains("fullops sync --role dev"),
        "{}",
        run.stderr
    );
    // 병합은 되돌리고 동기화를 예약한다. worker는 뜨지 않는다
    assert!(
        sb.git(&dev, &["rev-parse", "--verify", "-q", "MERGE_HEAD"])
            .code
            != Some(0)
    );
    assert_eq!(state(sb)["sync_pending"]["dev"], head(sb, "lead"));
    assert_eq!(calls(sb, "worker-start").len(), 1);
}
