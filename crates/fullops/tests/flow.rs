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
    let results = run_scenario(&scenario).unwrap_or_else(|e| panic!("{id}: {e}"));
    // 다른 묶음에 기대던 단계까지 실제로 돈다
    assert!(results.iter().all(|r| r.verdict != "pending"), "{id}");
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
        request.len() == 36
            && request.split('-').map(str::len).eq([8, 4, 4, 4, 12])
            && request.as_bytes()[14] == b'4',
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
            "{DISPATCH}[[step]]\nact = \"orca_sends\"\n[[step]]\nact = \"lead_waits\"\nno_wait = true\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\npurpose = \"fix\"\nterminal = \"term_1\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"orca_sends\"\n[[step]]\nact = \"lead_waits\"\nno_wait = true\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\npurpose = \"fix\"\nagent = \"codex\"\nmodel = \"m\"\neffort = \"high\"\nexpect = {{ verdict = \"pass\" }}\n"
        ),
    );
    // 같은 과제의 배정을 다시 내려면 앞의 것이 정산돼야 한다(D-10). 그래서 앞 배정의 보고를 받고 다음을 냈다
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
    let next = got["next"].as_str().unwrap();
    assert!(next.contains("--ack msg_1 --no-wait"), "{got}");
    assert!(
        next.contains("`orca orchestration reply --id msg_2 --body \"<답>\"` 뒤 `fullops wait --run run_1 --ack msg_2 --no-wait`"),
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

/// worker 질문을 받은 lead가 `wait`의 다음 안내만 따라 답하고 표시까지 닫는다(R6-4). hook은 날것 `reply`를 막지 않는다.
#[test]
fn wait_question_next_leads_to_reply_and_ack() {
    let mut d = Driver::new(&Setup::default());
    steps(&mut d, DISPATCH);
    let sb = &d.sb;
    sb.orca_mail(
        "run_1",
        orca_shapes::message(
            "msg_q",
            "question",
            "ctx_1",
            "task_1",
            "질문",
            "어느 쪽",
            None,
        ),
    );
    let lead = sb.path("lead");
    let got = out(&sb.cli(&lead, &["wait", "--run", "run_1", "--no-wait", "--json"]));
    let next = got["next"].as_str().unwrap();
    let pick = |start: &str| {
        let from = next
            .find(start)
            .unwrap_or_else(|| panic!("{start}: {next}"))
            + 1;
        next[from..from + next[from..].find('`').unwrap()].to_string()
    };
    let (reply, ack) = (pick("`orca orchestration reply"), pick("`fullops wait"));
    let decision = pick("`fullops decision add");
    assert_eq!(
        decision,
        "fullops decision add --question \"<질문>\" --key A-1 --blocks A-1 --blocks msg_q"
    );
    // lead의 PreToolUse가 안내된 답하기 명령을 그대로 통과시킨다
    let event = json!({"hook_event_name": "PreToolUse", "session_id": "lead", "cwd": lead,
        "tool_name": "Bash", "tool_input": {"command": reply.replace("<답>", "공개 API는 그대로 둔다")}});
    let hook = sb.hook(&["claude"], &sb.home, &event);
    assert_eq!(
        (hook.code, hook.stdout.trim()),
        (Some(0), "{}"),
        "{}",
        hook.stderr
    );
    // 사용자 결정 길도 받아들여진다
    let decision = decision.replace("<질문>", "범위를 넓힐까");
    let words: Vec<&str> = decision
        .split('"')
        .enumerate()
        .flat_map(|(i, part)| {
            if i % 2 == 1 {
                vec![part]
            } else {
                part.split_whitespace().collect()
            }
        })
        .skip(1)
        .collect();
    out(&sb.cli(&lead, &[&words[..], &["--json"]].concat()));
    let ack: Vec<&str> = ack.split_whitespace().skip(1).chain(["--json"]).collect();
    out(&sb.cli(&lead, &ack));
    let st = state(sb);
    let q = st["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == "msg_q")
        .unwrap();
    assert!(q["acked_at"].is_u64(), "{st}");
    assert_eq!(
        st["decisions"][0]["blocks"],
        json!(["A-1", "msg_q"]),
        "{st}"
    );
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
    // 끊긴 worker의 배정을 정산해 같은 배정을 다시 낼 수 있게 한다(D-10)
    assert_eq!(state(sb)["dispatches"][0]["settled"]["how"], "lost");
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
    // 하위 worker는 같은 워크트리의 다른 호스트 세션이다. 그 세션의 hook이 지나간 뒤의 CLI가 하위 worker로 읽힌다
    let worker = steps(
        &mut d,
        "[[step]]\nact = \"runs_fullops\"\nwho = \"dev\"\nsession = \"sub\"\nargs = [\"status\"]\n\
         env = { ORCA_TERMINAL_HANDLE = \"term_2\" }\nexpect = { verdict = \"pass\" }\n",
    )
    .remove(0)
    .output;
    let sb = &d.sb;
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
        &["dispatch", "B-2", "--purpose", "fix", "--reason", "limit"],
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
    // 같은 수정 배정이 실행 중이므로 한도 재배정으로 다시 낸다(D-10의 예외)
    let behind = sb.cli(
        &coor,
        &["dispatch", "B-2", "--purpose", "fix", "--reason", "limit"],
    );
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
    // 같은 과제의 배정이 실행 중이므로 한도 재배정으로 다시 낸다(D-10의 예외)
    let run = sb.cli(
        &lead,
        &["dispatch", "A-1", "--purpose", "fix", "--reason", "limit"],
    );
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

/// K4: 과제 3개가 route → task → dispatch → 작업 → lint → done → wait → merge로 실제로 이어진다.
/// 시나리오 파일의 기대에 더해 끝난 뒤의 저장소와 상태를 본다.
#[test]
fn k4_three_tasks_run_end_to_end() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/k4.toml");
    let k4 = load(&path).unwrap_or_else(|e| panic!("{e}"));
    let mut d = Driver::new(&k4.setup);
    for (index, step) in k4.steps.iter().enumerate() {
        assert!(step.pending.is_none(), "{}단계가 보류다", index + 1);
        let result = d.step(step).unwrap();
        let diff = step.expect.as_ref().map(|e| mismatches(e, &result));
        assert!(
            diff.as_ref().is_none_or(Vec::is_empty),
            "{}단계({}): {diff:?}\n{}",
            index + 1,
            step.act,
            result.output
        );
    }
    let sb = &d.sb;
    let lead = sb.path("lead");
    // 두 과제의 코드가 기본 브랜치와 원격에 있다
    let files = sb.git_ok(&lead, &["ls-tree", "-r", "--name-only", "origin/main"]);
    assert!(
        files.contains("src/a-1.py") && files.contains("src/b-2.py"),
        "{files}"
    );
    assert_eq!(
        sb.git_ok(&lead, &["rev-parse", "main"]),
        sb.git_ok(&lead, &["rev-parse", "origin/main"])
    );
    assert_eq!(sb.git_ok(&lead, &["status", "--porcelain"]), "");
    // 배정은 과제마다 한 번, 사람 결정은 없다
    assert_eq!(calls(sb, "worker-start").len(), 2);
    let st = state(sb);
    assert_eq!(st["decisions"].as_array().map_or(0, Vec::len), 0, "{st}");
    let status = out(&sb.cli(&lead, &["status", "--json"]));
    assert_eq!(status["pending"], json!([]), "{status}");
    let plans = std::fs::read_to_string(lead.join(".fullops-squad/PLANS.md")).unwrap();
    assert!(
        plans.contains("| C-3 |") && plans.contains("대기(사람: 배포)"),
        "{plans}"
    );
}

// ── 검증(R4·R6)에서 찾은 결함의 회귀 시험 ─────────────────────────────

fn roles(items: &[(&str, &str)]) -> Setup {
    Setup {
        roles: items
            .iter()
            .map(|(id, kind)| (id.to_string(), kind.to_string()))
            .collect(),
        ..Setup::default()
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// 리뷰어가 lead 체크아웃의 `reviews/A-1/<head 12자>/result.json`에 결론을 적는다.
fn review(sb: &Sandbox, sha: &str, conclusion: &str) {
    review_of(sb, "A-1", sha, conclusion)
}

fn review_of(sb: &Sandbox, key: &str, sha: &str, conclusion: &str) {
    let path = sb
        .path("lead")
        .join(".fullops-squad/reviews")
        .join(key)
        .join(&sha[..12])
        .join("result.json");
    let mut result: Value = serde_json::from_str(&read(&path)).expect("result.json");
    result["reviewer_session"] = json!("sid-reviewer");
    result["conclusion"] = json!(conclusion);
    for file in result["files"].as_array_mut().unwrap() {
        file["review_status"] = json!("reviewed");
    }
    result["findings"] = json!([]);
    std::fs::write(&path, serde_json::to_string_pretty(&result).unwrap()).unwrap();
}

/// worker가 큰 변경을 커밋하고 완료 보고를 채워 `done`까지 한 뒤, lead가 그 보고를 받는다.
fn round(lines: usize, message: &str) -> String {
    round_of("A-1", lines, message)
}

fn round_of(key: &str, lines: usize, message: &str) -> String {
    format!(
        "[[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"{key}\"\npath = \"src/big.py\"\nlines = {lines}\n{}\
         [[step]]\nact = \"worker_lints\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n\
         [[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n\
         [[step]]\nact = \"orca_sends\"\nid = \"{message}\"\n\
         [[step]]\nact = \"lead_waits\"\nexpect = {{ verdict = \"pass\" }}\n",
        report("dev")
    )
}

/// R4-1·R4-3: 리뷰 reject 뒤 사람 없이 이어진다. reopen → fix 배정 → 수정 → done → 새 SHA 리뷰 → 병합.
/// 이전 차수의 완료 기록은 한 글자도 바뀌지 않고, 수정 차수의 지시서와 완료 기록이 남는다.
#[test]
fn review_reject_reopens_and_the_fix_round_merges() {
    let mut d = Driver::new(&Setup::default());
    steps(&mut d, &format!("{DISPATCH}{}", round(400, "msg_r0")));
    let (lead, dev) = (d.sb.path("lead"), d.sb.path("dev"));
    let logs = ".fullops-squad/handovers/logs";
    let inbox = ".fullops-squad/handovers/to_dev.md";
    let old = head(&d.sb, "dev");
    let record = read(&dev.join(logs).join("A-1.dev.r0.md"));
    assert!(record.contains("구현과 확인을 마쳤다"), "{record}");

    // 리뷰어가 reject했다: 병합은 거절되고 탈출구는 수정 배정이다
    let snapshot = d.sb.cli(&lead, &["review", "snapshot", "A-1"]);
    assert_eq!(
        snapshot.code,
        Some(0),
        "{}{}",
        snapshot.stdout,
        snapshot.stderr
    );
    review(&d.sb, &old, "reject");
    let merge = d.sb.cli(&lead, &["merge", "A-1"]);
    assert_eq!(merge.code, Some(2), "{}{}", merge.stdout, merge.stderr);
    assert!(
        merge.stderr.contains("FullOps[M-04]") && merge.stderr.contains("fullops task reopen"),
        "{}",
        merge.stderr
    );

    // 그 탈출구가 실제로 통한다: 병합 전인데도 다음 차수 인박스가 생긴다
    steps(
        &mut d,
        "[[step]]\nact = \"runs_fullops\"\nargs = [\"task\", \"reopen\", \"--role\", \"dev\", \"--key\", \"A-1\", \"--goal\", \"리뷰 지적 수정\"]\nexpect = { verdict = \"pass\" }\n",
    );
    assert_eq!(read(&lead.join(logs).join("A-1.dev.r0.md")), record);
    let fresh = read(&lead.join(inbox));
    assert!(
        fresh.contains("round: 1") && fresh.contains("리뷰 지적 수정"),
        "{fresh}"
    );
    d.sb.write(
        &lead.join(inbox),
        &fresh.replacen(
            "- [ ] 구체적 작업과 수정할 파일",
            "- [ ] 경계 검사를 넣는다",
            1,
        ),
    );
    steps(
        &mut d,
        "[[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\npurpose = \"fix\"\nbare = true\nexpect = { verdict = \"pass\" }\n",
    );

    // 수정 지시는 worker 워크트리의 인박스로 갔고 spec의 Handover가 그 파일을 가리킨다
    let delivered = read(&dev.join(inbox));
    assert!(
        delivered.contains("round: 1") && delivered.contains("경계 검사를 넣는다"),
        "{delivered}"
    );
    assert_eq!(read(&dev.join(logs).join("A-1.dev.r0.md")), record);
    let starts = calls(&d.sb, "worker-start");
    let spec = starts.last().unwrap()["stdin"].as_str().unwrap();
    assert!(spec.contains("Purpose: fix"), "{spec}");
    let handover = spec.split("Handover: ").nth(1).unwrap();
    let handover = handover.split(';').next().unwrap();
    assert!(Path::new(handover).is_file(), "{handover}");

    // worker의 done이 R-04·R-06을 거쳐 새 차수의 완료 기록을 남긴다
    let waited = steps(&mut d, &round(401, "msg_r1")).pop().unwrap();
    let new = head(&d.sb, "dev");
    assert_eq!(
        d.sb.git_ok(&dev, &["log", "-1", "--format=%s"]),
        "fullops: 완료 기록 A-1 (dev)"
    );
    let second = read(&dev.join(logs).join("A-1.dev.r1.md"));
    assert!(second.contains("경계 검사를 넣는다") && !dev.join(inbox).exists());
    assert_eq!(read(&dev.join(logs).join("A-1.dev.r0.md")), record);
    // 같은 과제의 보고가 둘이면 안내가 고를 ID를 준다
    assert!(
        waited
            .output
            .contains("`fullops merge A-1 --completion msg_r1`"),
        "{}",
        waited.output
    );

    // 새 SHA를 리뷰하고 병합한다
    d.sb.orca_rule(json!({ "when": ["terminal", "list"], "stdout": orca_shapes::terminals(&[]) }));
    for args in [
        vec![
            "review",
            "cleanup",
            "A-1",
            "--sha",
            old.as_str(),
            "--finished",
        ],
        vec!["review", "snapshot", "A-1", "--sha", new.as_str()],
    ] {
        let run = d.sb.cli(&lead, &args);
        assert_eq!(run.code, Some(0), "{args:?}: {}{}", run.stdout, run.stderr);
    }
    review(&d.sb, &new, "accept");
    let checked =
        d.sb.cli(&lead, &["review", "check", "A-1", "--sha", new.as_str()]);
    assert_eq!(
        checked.code,
        Some(0),
        "{}{}",
        checked.stdout,
        checked.stderr
    );
    steps(
        &mut d,
        "[[step]]\nact = \"runs_fullops\"\nargs = [\"merge\", \"A-1\", \"--completion\", \"msg_r1\"]\nexpect = { verdict = \"pass\" }\n",
    );
    for name in ["A-1.dev.r0.md", "A-1.dev.r1.md"] {
        assert!(lead.join(logs).join(name).is_file(), "{name}");
    }
    assert_eq!(read(&lead.join(logs).join("A-1.dev.r0.md")), record);
    assert!(!lead.join(inbox).exists());
    // 앞선 보고(병합한 SHA의 조상)도 함께 닫혀 다음 배정이 바로 이어진다
    let st = state(&d.sb);
    let open = |c: &&Value| c["landed_at"].is_null();
    assert_eq!(
        st["completions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(open)
            .count(),
        0,
        "{}",
        st["completions"]
    );
    steps(
        &mut d,
        "[[step]]\nact = \"lead_dispatches\"\nkey = \"B-1\"\nrole = \"dev\"\nexpect = { verdict = \"pass\" }\n",
    );
}

/// R4-1: 병합 뒤 lint가 실패해(M-07) 되돌린 과제도 같은 길로 이어진다. reopen → fix 배정 → 수정 → done → 병합.
#[test]
fn lint_failure_after_merge_reopens_and_the_fix_round_merges() {
    let mut d = Driver::new(&Setup::default());
    let code = |text: &str| {
        format!(
            "[[step]]\nact = \"file\"\nwho = \"dev\"\npath = \"src/a.py\"\ncontent = \"{text}\\n\"\n[[step]]\nact = \"commits\"\nwho = \"dev\"\n{}\
             [[step]]\nact = \"worker_lints\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n",
            report("dev")
        )
    };
    // worker가 보고한 뒤 기본 브랜치에 그 코드를 막는 검사가 들어온다
    let rule = r#"{"schema_version":1,"commands":[],"exclude":[],"rules":[{"code":"OLD-001","description":"옛 API","pattern":"OLD_API","severity":"ERROR"}]}"#;
    steps(
        &mut d,
        &format!(
            "{DISPATCH}{}[[step]]\nact = \"orca_sends\"\nid = \"msg_r0\"\n[[step]]\nact = \"lead_waits\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"file\"\npath = \".fullops-squad/lint/lint.json\"\ncontent = '''{rule}'''\n\
             [[step]]\nact = \"commits\"\nmessage = \"옛 API 검사\"\n\
             [[step]]\nact = \"git\"\nargs = [\"push\", \"-q\", \"origin\", \"main\"]\n\
             [[step]]\nact = \"lead_merges\"\nkey = \"A-1\"\n\
             expect = {{ verdict = \"deny\", rule = \"M-07\", escape = \"fullops task reopen --role dev --key A-1\" }}\n\
             [[step]]\nact = \"runs_fullops\"\nargs = [\"task\", \"reopen\", \"--role\", \"dev\", \"--key\", \"A-1\", \"--goal\", \"옛 API를 걷어낸다\"]\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\npurpose = \"fix\"\nbare = true\nexpect = {{ verdict = \"pass\" }}\n\
             {}[[step]]\nact = \"orca_sends\"\nid = \"msg_r1\"\n[[step]]\nact = \"lead_waits\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"runs_fullops\"\nargs = [\"merge\", \"A-1\", \"--completion\", \"msg_r1\"]\nexpect = {{ verdict = \"pass\" }}\n",
            code("OLD_API()"),
            code("NEW_API()")
        ),
    );
    let logs = d.sb.path("lead").join(".fullops-squad/handovers/logs");
    assert!(read(&logs.join("A-1.dev.r1.md")).contains("옛 API를 걷어낸다"));
    assert!(read(&logs.join("A-1.dev.r0.md")).contains("구현과 확인을 마쳤다"));
    let st = state(&d.sb);
    let open = |c: &&Value| c["landed_at"].is_null();
    let completions = st["completions"].as_array().unwrap();
    assert_eq!(
        completions.iter().filter(open).count(),
        0,
        "{completions:?}"
    );
}

/// R4-1: 끝난 차수의 인박스로 그대로 배정하면 지시서가 worker에게 가지 않으므로 거절하고 reopen을 안내한다.
#[test]
fn finished_round_inbox_is_not_dispatched_again() {
    let mut d = Driver::new(&Setup::default());
    steps(&mut d, &format!("{DISPATCH}{}", round(3, "msg_r0")));
    let dev = d.sb.path("dev");
    let record = read(&dev.join(".fullops-squad/handovers/logs/A-1.dev.r0.md"));
    let refused = steps(
        &mut d,
        "[[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\npurpose = \"fix\"\nbare = true\n\
         expect = { verdict = \"deny\", rule = \"D-02\", escape = \"fullops task reopen --role dev --key A-1\" }\n",
    );
    assert!(
        refused[0].output.contains("완료 기록"),
        "{}",
        refused[0].output
    );
    assert_eq!(calls(&d.sb, "worker-start").len(), 1);
    assert_eq!(
        read(&dev.join(".fullops-squad/handovers/logs/A-1.dev.r0.md")),
        record
    );
    // 다음 차수를 연 뒤 앞 차수의 보고를 먼저 병합해도 기록과 새 인박스가 섞이지 않는다
    let lead = d.sb.path("lead");
    steps(
        &mut d,
        "[[step]]\nact = \"runs_fullops\"\nargs = [\"task\", \"reopen\", \"--role\", \"dev\", \"--key\", \"A-1\", \"--goal\", \"후속 수정\"]\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_merges\"\nkey = \"A-1\"\nexpect = { verdict = \"pass\" }\n",
    );
    assert_eq!(
        read(&lead.join(".fullops-squad/handovers/logs/A-1.dev.r0.md")),
        record
    );
    let next = read(&lead.join(".fullops-squad/handovers/to_dev.md"));
    assert!(
        next.contains("round: 1")
            && next.contains("후속 수정")
            && !next.contains("구현과 확인을 마쳤다"),
        "{next}"
    );
    // 보고를 받기 전의 재오픈은 서로를 가리키지 않는 탈출구로 거절된다
    let mut early = Driver::new(&Setup::default());
    steps(&mut early, DISPATCH);
    let run = early.sb.cli(
        &early.sb.path("lead"),
        &[
            "task", "reopen", "--role", "dev", "--key", "A-1", "--goal", "수정",
        ],
    );
    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(
        run.stderr.contains("FullOps[H-03]")
            && run.stderr.contains("fullops wait")
            && !run.stderr.contains("fullops task new"),
        "{}",
        run.stderr
    );
}

/// R4-1: 수정 배정의 동기화가 미뤄져 `fullops sync`가 대신 반영해도 새 인박스가 이전 차수의 완료 기록에 섞이지 않는다.
#[test]
fn sync_after_reopen_keeps_the_record_apart_from_the_new_inbox() {
    let mut d = Driver::new(&Setup::default());
    steps(&mut d, &format!("{DISPATCH}{}", round(3, "msg_r0")));
    let (lead, dev) = (d.sb.path("lead"), d.sb.path("dev"));
    let log = dev.join(".fullops-squad/handovers/logs/A-1.dev.r0.md");
    let record = read(&log);
    // worker 워크트리가 더러워 배정의 동기화가 멈춘다
    d.sb.write(&dev.join("stray.txt"), "치우지 않은 파일\n");
    steps(
        &mut d,
        "[[step]]\nact = \"runs_fullops\"\nargs = [\"task\", \"reopen\", \"--role\", \"dev\", \"--key\", \"A-1\", \"--goal\", \"수정\"]\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\npurpose = \"fix\"\nbare = true\nexpect = { verdict = \"deny\", rule = \"sync\", escape = \"fullops sync --role dev\" }\n",
    );
    std::fs::remove_file(dev.join("stray.txt")).unwrap();
    let synced = d.sb.cli(&lead, &["sync", "--role", "dev"]);
    assert_eq!(synced.code, Some(0), "{}{}", synced.stdout, synced.stderr);
    assert_eq!(read(&log), record);
    let inbox = read(&dev.join(".fullops-squad/handovers/to_dev.md"));
    assert!(inbox.contains("round: 1"), "{inbox}");
}

/// R4-2: 미리 보기만 기록되고 Orca에서 읽음이 되지 않은 보고가 병합·보관 뒤 다시 와도 통합 대기로 되살아나지 않는다.
#[test]
fn merged_report_peeked_again_does_not_block_the_next_dispatch() {
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        "[[step]]\nact = \"worker_reports\"\nrole = \"dev\"\nkey = \"A-1\"\nexpect = { verdict = \"pass\" }\n",
    );
    // 소비형 check가 일어나지 않았다: Orca에는 안 읽음으로 남았다
    std::fs::remove_file(d.sb.orca_dir.join("mail.read")).unwrap();
    let got = steps(
        &mut d,
        "[[step]]\nact = \"lead_merges\"\nkey = \"A-1\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_waits\"\nno_wait = true\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_dispatches\"\nkey = \"B-1\"\nrole = \"dev\"\nexpect = { verdict = \"pass\" }\n",
    );
    assert!(
        got[1].output.contains("처리할 메시지가 없다"),
        "{}",
        got[1].output
    );
    let st = state(&d.sb);
    let pending = |c: &&Value| c["landed_at"].is_null();
    assert_eq!(
        st["completions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(pending)
            .count(),
        0,
        "{}",
        st["completions"]
    );
}

/// R4-4: 성공한 배정을 다시 실행해도 같은 워크트리에 worker가 하나 더 뜨지 않는다. 하위 배정도 같다.
#[test]
fn dispatch_rerun_after_success_starts_no_second_worker() {
    let mut d = Driver::new(&Setup::default());
    steps(&mut d, DISPATCH);
    steps(
        &mut d,
        "[[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\nbare = true\n\
         expect = { verdict = \"deny\", rule = \"D-10\", escape = \"fullops wait --run run_1\" }\n",
    );
    assert_eq!(calls(&d.sb, "worker-start").len(), 1);
    assert_eq!(state(&d.sb)["dispatches"].as_array().map(Vec::len), Some(1));

    let other = SUBDISPATCH.replace("src/b.py", "src/c.py");
    let again = SUBDISPATCH.replace("\"pass\"", "\"deny\", rule = \"D-10\"");
    steps(
        &mut d,
        &format!(
            "[[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"A-1\"\n{SUBDISPATCH}{again}{other}"
        ),
    );
    assert_eq!(calls(&d.sb, "worker-start").len(), 3);
}

/// R4-5: 읽을 수 없는 인박스를 '없음'으로 보고 R-04·R-10·R-06을 건너뛰지 않는다.
#[test]
fn done_refuses_an_inbox_it_cannot_read() {
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        &format!(
            "{DISPATCH}[[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"A-1\"\n\
             [[step]]\nact = \"edits_file\"\nwho = \"dev\"\npath = \".fullops-squad/handovers/to_dev.md\"\nold = \"round: 0\"\nnew = \"round: 1차\"\n\
             [[step]]\nact = \"commits\"\nwho = \"dev\"\n\
             [[step]]\nact = \"worker_lints\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nexpect = {{ verdict = \"deny\", rule = \"R-04\", escape = \"round\" }}\n"
        ),
    );
    let dev = d.sb.path("dev");
    assert!(dev.join(".fullops-squad/handovers/to_dev.md").is_file());
    assert!(
        !dev.join(".fullops-squad/handovers/logs/A-1.dev.r0.md")
            .exists()
    );
    assert!(state(&d.sb)["dispatches"][0]["reported"].is_null());
    // 실패 보고는 그대로 보낼 수 있다
    steps(
        &mut d,
        "[[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nfailed = \"인박스를 읽을 수 없다\"\nexpect = { verdict = \"pass\" }\n",
    );

    // 읽지 못한 분류 기록도 '없음'이 아니다: 패킷 검사를 건너뛰지 않고, 재분류가 기록을 덮어쓰지 않는다
    let lead = d.sb.path("lead");
    let route = lead.join(".fullops-squad/routes/A-1.json");
    d.sb.write(&route, "{ 쓰다 만");
    let checked =
        d.sb.cli(&lead, &["packet", "check", "A-1", "--role", "dev"]);
    assert_eq!(
        checked.code,
        Some(2),
        "{}{}",
        checked.stdout,
        checked.stderr
    );
    assert!(
        checked.stderr.contains("FullOps[packet]"),
        "{}",
        checked.stderr
    );
    let rerouted = d.sb.cli(
        &lead,
        &[
            "route",
            "A-1",
            "--request",
            "다시",
            "--class",
            "direct",
            "--role",
            "dev",
            "--force",
        ],
    );
    assert_ne!(rerouted.code, Some(0), "{}", rerouted.stdout);
    assert_eq!(read(&route), "{ 쓰다 만");
}

/// R4-6·R4-7: `plan add`·`plan set`은 고치지 않는 줄의 줄 끝 표기를 바꾸지 않고, BOM 뒤에서 시작하는 표도 읽는다.
#[test]
fn plan_keeps_line_endings_and_reads_a_table_after_a_bom() {
    let sb = Sandbox::with(&Setup::default());
    let lead = sb.path("lead");
    let plan = lead.join(".fullops-squad/PLANS.md");
    let row = "| A-1 | 첫째 | Dev | 대기 | 요청 |\r\n";
    let crlf = format!(
        "# 현재 작업\r\n\r\n| 과제 키 | 목표 | 담당 | 상태 | 지시서·원천 링크 |\r\n|---|---|---|---|---|\r\n{row}\r\n## 위임 범위\r\n\r\n- 묻지 말고 끝까지\r\n"
    );
    std::fs::write(&plan, &crlf).unwrap();
    let ok = |args: &[&str]| {
        let run = sb.cli(&lead, args);
        assert_eq!(run.code, Some(0), "{args:?}: {}{}", run.stdout, run.stderr);
        run.stdout
    };
    ok(&[
        "plan", "add", "B-2", "--role", "dev", "--goal", "둘째", "--source", "요청",
    ]);
    let added = crlf.replace(row, &format!("{row}| B-2 | 둘째 | dev | 대기 | 요청 |\r\n"));
    assert_eq!(read(&plan), added);
    ok(&["plan", "set", "A-1", "보류(사람 확인)"]);
    assert_eq!(
        read(&plan),
        added.replace(row, "| A-1 | 첫째 | Dev | 보류(사람 확인) | 요청 |\r\n")
    );

    let bom =
        "\u{feff}| 과제 | 상태 | 담당 | 원천 |\n|---|---|---|---|\n| C-3 | 대기 | dev | 요청 |\n";
    std::fs::write(&plan, bom).unwrap();
    assert!(ok(&["plan", "list"]).contains("C-3"));
    ok(&[
        "plan", "add", "D-4", "--role", "dev", "--goal", "넷째", "--source", "요청",
    ]);
    assert_eq!(read(&plan), format!("{bom}| D-4 | 대기 | dev | 요청 |\n"));
    assert!(ok(&["plan", "ready"]).contains("C-3"));
}

/// R4-8: `v`는 같은데 이 실행본이 모르는 값이 든 `state.json`은 옮기지 않고 거절한다.
#[test]
fn state_with_an_unknown_value_is_refused_not_quarantined() {
    let sb = Sandbox::with(&Setup::default());
    let lead = sb.path("lead");
    let add = |question: &str| {
        sb.cli(
            &lead,
            &[
                "decision",
                "add",
                "--question",
                question,
                "--option",
                "a",
                "--option",
                "b",
            ],
        )
    };
    assert_eq!(add("q0").code, Some(0));
    let dir = lead.join(".git/fullops");
    let path = dir.join("state.json");
    let unknown = read(&path).replacen("\"status\": \"open\"", "\"status\": \"deferred\"", 1);
    assert!(unknown.contains("deferred"));
    std::fs::write(&path, &unknown).unwrap();
    let run = add("q1");
    assert_eq!(run.code, Some(1), "{}", run.stderr);
    assert!(run.stderr.contains("새 형식"), "{}", run.stderr);
    assert_eq!(read(&path), unknown);
    let moved = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().contains("broken"));
    assert!(!moved);
}

/// R6-1: `route --request "<글>"`로 분류한 설계 과제를 인박스 없이 배정하면 설계 역할이 요청 글을 받는다.
#[test]
fn design_dispatch_without_an_inbox_carries_the_request() {
    let mut d = Driver::new(&roles(&[("dev", "worker"), ("architecture", "design")]));
    let text = "결제 화면에 재시도 버튼을 넣고 실패 사유를 보여 준다";
    let route = |key: &str| {
        format!(
            "[[step]]\nact = \"runs_fullops\"\nargs = [\"route\", \"{key}\", \"--request\", \"{text}\", \"--class\", \"design\", \"--role\", \"architecture\", \"--agent\", \"claude\", \"--model\", \"m\", \"--effort\", \"medium\"]\nexpect = {{ verdict = \"pass\" }}\n"
        )
    };
    steps(
        &mut d,
        &format!(
            "{}[[step]]\nact = \"lead_dispatches\"\nkey = \"F-9\"\nrole = \"architecture\"\nbare = true\nexpect = {{ verdict = \"pass\" }}\n",
            route("F-9")
        ),
    );
    let file = ".fullops-squad/routes/F-9.request.md";
    let starts = calls(&d.sb, "worker-start");
    let spec = starts.last().unwrap()["stdin"].as_str().unwrap();
    assert!(spec.contains(&format!("Request: {file}")), "{spec}");
    assert_eq!(read(&d.sb.path("architecture").join(file)), text);
    let lead = d.sb.path("lead");
    let recorded: Value =
        serde_json::from_str(&read(&lead.join(".fullops-squad/routes/F-9.json"))).unwrap();
    assert_eq!(recorded["request_file"], file);

    // 비밀값 무늬가 있는 글은 커밋되는 자리에 쓰지 않고 Git 공용 디렉터리에 남긴다
    let secret = "API_KEY=abcd1234 로 결제 호출을 다시 시도한다";
    let run = d.sb.cli(
        &lead,
        &[
            "route",
            "F-7",
            "--request",
            secret,
            "--class",
            "design",
            "--role",
            "architecture",
            "--json",
        ],
    );
    let local = out(&run)["request_file"].as_str().unwrap().to_string();
    assert!(!lead.join(".fullops-squad/routes/F-7.request.md").exists());
    assert!(local.contains("/.git/fullops/requests/F-7.md"), "{local}");
    assert_eq!(read(Path::new(&local)), secret);

    // 요청 글이 없는 기록(옛 실행본이 쓴 것)으로는 인박스 없는 설계 배정을 하지 않는다
    steps(&mut d, &route("F-8"));
    let path = lead.join(".fullops-squad/routes/F-8.json");
    let mut old: Value = serde_json::from_str(&read(&path)).unwrap();
    old["request_file"] = Value::Null;
    std::fs::write(&path, old.to_string()).unwrap();
    steps(
        &mut d,
        "[[step]]\nact = \"lead_dispatches\"\nkey = \"F-8\"\nrole = \"architecture\"\nbare = true\n\
         expect = { verdict = \"deny\", rule = \"D-02\", escape = \"fullops route F-8 --force --request\" }\n",
    );
}

/// R6-2: 설계 완료를 병합하면 설계 역할이 쓴 구현 지시서의 배정이 안내되고, 배정할 때까지 lead의 Stop이 한 번 막는다.
#[test]
fn design_merge_leads_to_dispatching_the_written_inbox() {
    let mut d = Driver::new(&roles(&[("dev", "worker"), ("architecture", "design")]));
    let got = steps(
        &mut d,
        "[[step]]\nact = \"lead_routes\"\nkey = \"F-1\"\nrole = \"architecture\"\nclass = \"design\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_dispatches\"\nkey = \"F-1\"\nrole = \"architecture\"\nbare = true\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"session_starts\"\nwho = \"architecture\"\n\
         [[step]]\nact = \"preamble_arrives\"\nwho = \"architecture\"\n\
         [[step]]\nact = \"writes_file\"\nwho = \"architecture\"\npath = \".fullops-squad/docs/design-docs/f-1.md\"\ncontent = \"---\\nstatus: draft\\n---\\n# F-1 설계\\n\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"runs_fullops\"\nwho = \"architecture\"\nargs = [\"task\", \"new\", \"--role\", \"dev\", \"--key\", \"F-1\", \"--goal\", \"F-1 구현\"]\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"commits\"\nwho = \"architecture\"\n\
         [[step]]\nact = \"runs_fullops\"\nwho = \"architecture\"\nargs = [\"lint\"]\n\
         [[step]]\nact = \"runs_fullops\"\nwho = \"architecture\"\nargs = [\"done\"]\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"orca_sends\"\n\
         [[step]]\nact = \"lead_waits\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_merges\"\nkey = \"F-1\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"stops\"\nexpect = { verdict = \"block\", rule = \"X-07\" }\n",
    );
    let next = "`fullops dispatch F-1 --role dev";
    let (merged, stopped) = (&got[got.len() - 2], &got[got.len() - 1]);
    assert!(merged.output.contains(next), "{}", merged.output);
    assert!(stopped.output.contains(next), "{}", stopped.output);
    let lead = d.sb.path("lead");
    let status = d.sb.cli(&lead, &["status"]);
    assert!(status.stdout.contains(next), "{}", status.stdout);

    // C2-3: X-07의 탈출구대로 PLANS.md에서 미루면 더 묻지 않고, 되돌리면 다시 안내한다
    let cli = |args: &[&str]| {
        let run = d.sb.cli(&lead, args);
        assert_eq!(run.code, Some(0), "{args:?}: {}{}", run.stdout, run.stderr);
        run.stdout
    };
    cli(&[
        "plan",
        "add",
        "F-1",
        "--role",
        "dev",
        "--goal",
        "F-1 구현",
        "--source",
        "설계",
    ]);
    cli(&["plan", "set", "F-1", "보류(사람 확인)"]);
    assert!(!cli(&["status"]).contains("이어서 할 일"));
    cli(&["plan", "set", "F-1", "대기"]);
    assert!(cli(&["status"]).contains(next));

    // 안내대로 배정하면 더 묻지 않는다
    steps(
        &mut d,
        "[[step]]\nact = \"lead_dispatches\"\nkey = \"F-1\"\nrole = \"dev\"\nbare = true\nagent = \"claude\"\nmodel = \"m\"\neffort = \"medium\"\nexpect = { verdict = \"pass\" }\n",
    );
    let status = d.sb.cli(&lead, &["status"]);
    assert!(!status.stdout.contains("이어서 할 일"), "{}", status.stdout);
    assert!(
        d.sb.path("dev")
            .join(".fullops-squad/handovers/to_dev.md")
            .is_file()
    );
}

/// (F6): 인박스 없이 받은 설계 과제(설계 문서만 만든 과제, 질문 전달 과제)를 같은 lead 세션에서 병합까지 하면
/// 배정 기록이 보관으로 옮겨져도 X-04가 '분류만 하고 배정하지 않은 과제'로 막지 않는다.
#[test]
fn merged_design_task_without_an_inbox_does_not_trip_x04() {
    let mut d = Driver::new(&roles(&[("dev", "worker"), ("architecture", "design")]));
    steps(
        &mut d,
        "[[step]]\nact = \"session_starts\"\n\
         [[step]]\nact = \"lead_routes\"\nkey = \"F-1-Q1\"\nrole = \"architecture\"\nclass = \"design\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_dispatches\"\nkey = \"F-1-Q1\"\nrole = \"architecture\"\nbare = true\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"session_starts\"\nwho = \"architecture\"\n\
         [[step]]\nact = \"preamble_arrives\"\nwho = \"architecture\"\n\
         [[step]]\nact = \"writes_file\"\nwho = \"architecture\"\npath = \".fullops-squad/docs/design-docs/f-1-q1.md\"\ncontent = \"---\\nstatus: draft\\n---\\n# 답\\n\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"commits\"\nwho = \"architecture\"\n\
         [[step]]\nact = \"runs_fullops\"\nwho = \"architecture\"\nargs = [\"lint\"]\n\
         [[step]]\nact = \"runs_fullops\"\nwho = \"architecture\"\nargs = [\"done\"]\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"orca_sends\"\n\
         [[step]]\nact = \"lead_waits\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_merges\"\nkey = \"F-1-Q1\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"stops\"\nexpect = { verdict = \"pass\" }\n",
    );
    // 병합 뒤 새 lead 세션에서 다시 분류만 하고 배정하지 않은 키는 보관 기록으로 면제되지 않는다(시각은 초 단위)
    std::thread::sleep(std::time::Duration::from_millis(1100));
    steps(
        &mut d,
        "[[step]]\nact = \"session_starts\"\nsession = \"lead-2\"\n\
         [[step]]\nact = \"runs_fullops\"\nargs = [\"route\", \"F-1-Q1\", \"--force\", \"--request\", \"바뀐 요청\", \"--class\", \"design\", \"--role\", \"architecture\", \"--reason\", \"요청 바뀜\"]\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"stops\"\nexpect = { verdict = \"block\", rule = \"X-04\" }\n",
    );
}

/// R6-6: lead의 `status`는 worker가 만든 하위 Run을 이어받을 Run으로 보여 주지 않고, `wait --run`으로도 가로채지 못한다.
#[test]
fn lead_does_not_take_over_a_workers_sub_run() {
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        &format!(
            "{DISPATCH}[[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"A-1\"\n{SUBDISPATCH}"
        ),
    );
    let lead = d.sb.path("lead");
    let status = d.sb.cli(&lead, &["status"]);
    assert!(
        status.stdout.contains("열린 Run: run_1 (") && !status.stdout.contains("run_2"),
        "{}",
        status.stdout
    );
    let before = state(&d.sb)["runs"].clone();
    let taken = d.sb.cli(&lead, &["wait", "--run", "run_2", "--no-wait"]);
    assert_eq!(taken.code, Some(2), "{}{}", taken.stdout, taken.stderr);
    assert!(taken.stderr.contains("FullOps[wait]"), "{}", taken.stderr);
    assert_eq!(state(&d.sb)["runs"], before);
    assert!(calls(&d.sb, "run-use").is_empty());
}

/// R6-7: 실패 보고는 처리 표시 뒤에도 다시 배정하거나 미룰 때까지 안내되고, 그 과제를 선행으로 둔 행은 준비되지 않는다.
#[test]
fn failed_report_is_followed_up_until_redispatched_or_put_off() {
    let mut d = Driver::new(&roles(&[("art", "worker"), ("dev", "worker")]));
    let got = steps(
        &mut d,
        "[[step]]\nact = \"lead_plans\"\nkey = \"B-1\"\nrole = \"art\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_plans\"\nkey = \"C-1\"\nrole = \"dev\"\nafter = \"B-1\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_dispatches\"\nkey = \"B-1\"\nrole = \"art\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"session_starts\"\nwho = \"art\"\n\
         [[step]]\nact = \"preamble_arrives\"\nwho = \"art\"\n\
         [[step]]\nact = \"runs_fullops\"\nwho = \"art\"\nargs = [\"done\", \"--failed\", \"빌드 도구가 없다\"]\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"orca_sends\"\nid = \"msg_f\"\noutcome = \"failed\"\nbody = \"[실패] B-1 | 사유: 빌드 도구가 없다\"\n\
         [[step]]\nact = \"lead_waits\"\nno_wait = true\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_waits\"\nack = [\"msg_f\"]\nno_wait = true\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"stops\"\nexpect = { verdict = \"block\", rule = \"X-07\" }\n",
    );
    let next = "`fullops dispatch B-1 --role art`";
    let (waited, stopped) = (&got[got.len() - 3], &got[got.len() - 1]);
    assert!(waited.output.contains(next), "{}", waited.output);
    assert!(
        stopped.output.contains(next) && stopped.output.contains("보류"),
        "{}",
        stopped.output
    );
    let lead = d.sb.path("lead");
    let cli = |args: &[&str]| {
        let run = d.sb.cli(&lead, args);
        assert_eq!(run.code, Some(0), "{args:?}: {}{}", run.stdout, run.stderr);
        run.stdout
    };
    // 선행 과제가 실패했으면 뒤 과제는 준비된 행이 아니다
    assert!(!cli(&["plan", "ready"]).contains("C-1"));
    assert!(cli(&["status"]).contains(next));
    // 미루면 더 묻지 않는다
    cli(&["plan", "set", "B-1", "보류(도구 설치 대기)"]);
    assert!(!cli(&["status"]).contains("이어서 할 일"));
    cli(&["plan", "set", "B-1", "대기"]);
    assert!(cli(&["status"]).contains(next));
    // 다시 배정해도 더 묻지 않는다
    steps(
        &mut d,
        "[[step]]\nact = \"lead_dispatches\"\nkey = \"B-1\"\nrole = \"art\"\nbare = true\nexpect = { verdict = \"pass\" }\n",
    );
    let status = d.sb.cli(&lead, &["status"]);
    assert!(!status.stdout.contains("이어서 할 일"), "{}", status.stdout);
}

// ── 검증(C2)에서 찾은 결함의 회귀 시험 ─────────────────────────────

/// Orca가 그 배정의 한도 escalation을 보내고 lead가 받는다.
fn escalate(dispatch: &str) -> String {
    format!(
        "[[step]]\nact = \"orca_sends\"\ntype = \"escalation\"\ndispatch = \"{dispatch}\"\nsubject = \"[fullops:limit] 사용 한도\"\n\
         [[step]]\nact = \"lead_waits\"\nno_wait = true\nexpect = {{ verdict = \"pass\" }}\n"
    )
}

/// C2-1: `done`을 마치고 `worker_done`만 못 보낸 채 한도로 멈춘 차수는 한도 재배정이 D-02로 막히지 않고,
/// 새 worker의 `done` 다시 실행이 R-06을 건너뛰어 보고를 끝낸다. 보고를 받은 차수의 재배정은 여전히 D-02다.
#[test]
fn limit_redispatch_after_an_unsent_done_finishes_the_report() {
    unsent_done(false);
    unsent_done(true);
}

/// `advance`: 그 사이 기본 브랜치가 앞서 가 재배정의 동기화가 병합 커밋을 만든다.
fn unsent_done(advance: bool) {
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        &format!(
            "{DISPATCH}[[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"A-1\"\n{}\
             [[step]]\nact = \"worker_lints\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n{}",
            report("dev"),
            escalate("ctx_1")
        ),
    );
    let dev = d.sb.path("dev");
    let logs = ".fullops-squad/handovers/logs";
    let record = read(&dev.join(logs).join("A-1.dev.r0.md"));
    let reported = head(&d.sb, "dev");
    if advance {
        steps(
            &mut d,
            "[[step]]\nact = \"file\"\npath = \"docs/later.md\"\ncontent = \"나중 문서\\n\"\n[[step]]\nact = \"commits\"\nmessage = \"문서\"\n",
        );
    }
    // 끝난 차수지만 보고가 오지 않았다: task reopen은 H-03으로 막히므로 탈출구는 재배정이다
    steps(
        &mut d,
        "[[step]]\nact = \"runs_fullops\"\nargs = [\"task\", \"reopen\", \"--role\", \"dev\", \"--key\", \"A-1\", \"--goal\", \"수정\"]\nexpect = { verdict = \"deny\", rule = \"H-03\" }\n\
         [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\nbare = true\nreason = \"limit\"\nexpect = { verdict = \"pass\" }\n",
    );
    // spec의 Handover는 그 차수의 완료 기록을 가리킨다
    let starts = calls(&d.sb, "worker-start");
    assert_eq!(starts.len(), 2);
    let spec = starts.last().unwrap()["stdin"].as_str().unwrap();
    let handover = spec.split("Handover: ").nth(1).unwrap();
    let handover = handover.split(';').next().unwrap();
    assert!(handover.ends_with("logs/A-1.dev.r0.md"), "{spec}");
    assert!(Path::new(handover).is_file(), "{handover}");

    // 새 worker가 done을 다시 실행한다: 완료 기록은 다시 만들지 않고 새 배정에 reported를 쓴다.
    // 받아 온 기본 브랜치가 있으면 그것까지 덮는 lint를 먼저 요구한다(R-03)
    let lint = "[[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nexpect = { verdict = \"deny\", rule = \"R-03\" }\n\
                [[step]]\nact = \"worker_lints\"\nrole = \"dev\"\nexpect = { verdict = \"pass\" }\n";
    let finished = steps(
        &mut d,
        &format!(
            "[[step]]\nact = \"session_starts\"\nwho = \"dev\"\n[[step]]\nact = \"preamble_arrives\"\nwho = \"dev\"\n{}\
             [[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n",
            if advance { lint } else { "" }
        ),
    );
    let body = &finished.last().unwrap().output;
    assert!(body.contains("[완료] A-1"), "{body}");
    assert_eq!(read(&dev.join(logs).join("A-1.dev.r0.md")), record);
    let subjects =
        d.sb.git_ok(&dev, &["log", "--format=%s", &format!("{reported}..HEAD")]);
    assert!(!subjects.contains("완료 기록"), "{subjects}");
    let st = state(&d.sb);
    assert!(st["dispatches"][1]["reported"]["sha"].is_string(), "{st}");

    // 보고를 받으면 그 차수는 끝났다: 같은 인박스로 다시 배정하면 D-02이고, 보고는 병합된다
    steps(
        &mut d,
        "[[step]]\nact = \"orca_sends\"\nid = \"msg_r0\"\n[[step]]\nact = \"lead_waits\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\npurpose = \"fix\"\nbare = true\n\
         expect = { verdict = \"deny\", rule = \"D-02\", escape = \"fullops task reopen --role dev --key A-1\" }\n\
         [[step]]\nact = \"lead_merges\"\nkey = \"A-1\"\nexpect = { verdict = \"pass\" }\n",
    );
    assert_eq!(calls(&d.sb, "worker-start").len(), 2);
    assert_eq!(
        read(&d.sb.path("lead").join(logs).join("A-1.dev.r0.md")),
        record
    );
}

/// C2-2: 한도로 다시 배정된 worker는 전임자의 하위 Run을 이어받아(소유자가 바뀐다) 하위 worker의 보고를 받고,
/// 그 보고를 처리할 때까지 Stop(X-05)과 `done`(R-11)이 그 Run을 본다. lead는 여전히 가져가지 못한다.
#[test]
fn redispatched_worker_inherits_the_predecessors_sub_run() {
    let mut d = Driver::new(&Setup::default());
    // 새 worker는 새 터미널의 새 세션(w2)이다
    let w2 = |step: &str| {
        format!("[[step]]\nact = \"runs_fullops\"\nwho = \"dev\"\nsession = \"w2\"\n{step}")
    };
    steps(
        &mut d,
        &format!(
            "{DISPATCH}[[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"A-1\"\n{SUBDISPATCH}{}\
             [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\nbare = true\nreason = \"limit\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"orca_sends\"\nrun = \"run_2\"\ndispatch = \"ctx_2\"\nid = \"msg_sub\"\nbody = \"src/b.py를 마쳤다\"\n\
             [[step]]\nact = \"session_starts\"\nwho = \"dev\"\nsession = \"w2\"\n[[step]]\nact = \"preamble_arrives\"\nwho = \"dev\"\nsession = \"w2\"\n",
            escalate("ctx_1")
        ),
    );
    let lead = d.sb.path("lead");
    let st = state(&d.sb);
    let heir = st["dispatches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["reason"] == "limit")
        .unwrap()["id"]
        .clone();
    let owner = |sb: &Sandbox| {
        let st = state(sb);
        let run = st["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "run_2")
            .cloned();
        run.map(|r| r["owner"].clone()).unwrap_or_default()
    };
    // 새 worker의 Stop은 전임자의 하위 Run을 본다
    let stopped = steps(
        &mut d,
        "[[step]]\nact = \"stops\"\nwho = \"dev\"\nsession = \"w2\"\nexpect = { verdict = \"block\", rule = \"X-05\", escape = \"fullops wait --run run_2\" }\n",
    );
    assert!(stopped[0].output.contains("run_2"), "{}", stopped[0].output);
    // lead는 가져가지 못한다
    let taken = d.sb.cli(&lead, &["wait", "--run", "run_2", "--no-wait"]);
    assert_eq!(taken.code, Some(2), "{}{}", taken.stdout, taken.stderr);
    assert_eq!(owner(&d.sb), "dispatch:ctx_1");

    // 새 worker는 `--run` 없이도 그 Run을 이어받아 하위 worker의 보고를 받는다
    let got = steps(
        &mut d,
        &w2("args = [\"wait\", \"--no-wait\"]\nexpect = { verdict = \"pass\" }\n"),
    );
    assert!(got[0].output.contains("msg_sub"), "{}", got[0].output);
    assert_eq!(owner(&d.sb), format!("dispatch:{}", heir.as_str().unwrap()));
    let st = state(&d.sb);
    let sub = st["dispatches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == "ctx_2")
        .unwrap();
    assert_eq!(sub["settled"]["how"], "done", "{sub}");
    let taken = d.sb.cli(&lead, &["wait", "--run", "run_2", "--no-wait"]);
    assert_eq!(taken.code, Some(2), "{}{}", taken.stdout, taken.stderr);
    assert!(taken.stderr.contains("FullOps[wait]"), "{}", taken.stderr);
    let status = d.sb.cli(&lead, &["status"]);
    assert!(!status.stdout.contains("run_2"), "{}", status.stdout);

    // 하위 보고를 처리 표시할 때까지 done은 R-11, 표시한 뒤에는 보고를 끝낸다
    steps(
        &mut d,
        &format!(
            "{}{}{}{}{}",
            report("dev"),
            w2("args = [\"lint\"]\nexpect = { verdict = \"pass\" }\n"),
            w2(
                "args = [\"done\"]\nexpect = { verdict = \"deny\", rule = \"R-11\", escape = \"fullops wait --run run_2\" }\n"
            ),
            w2(
                "args = [\"wait\", \"--run\", \"run_2\", \"--ack\", \"msg_sub\", \"--no-wait\"]\nexpect = { verdict = \"pass\" }\n"
            ),
            w2("args = [\"done\"]\nexpect = { verdict = \"pass\" }\n"),
        ),
    );
    assert!(
        state(&d.sb)["dispatches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["id"] == heir && x["reported"].is_object())
    );
}

// ── 검증(C3)에서 찾은 결함의 회귀 시험 ─────────────────────────────

/// 설계 역할이 dev 지시서를 써서 병합되고 lead가 그 지시서를 dev에 배정하기까지.
const DESIGNED: &str = "[[step]]\nact = \"lead_routes\"\nkey = \"F-1\"\nrole = \"architecture\"\nclass = \"design\"\nexpect = { verdict = \"pass\" }\n\
     [[step]]\nact = \"lead_dispatches\"\nkey = \"F-1\"\nrole = \"architecture\"\nbare = true\nexpect = { verdict = \"pass\" }\n\
     [[step]]\nact = \"session_starts\"\nwho = \"architecture\"\n\
     [[step]]\nact = \"preamble_arrives\"\nwho = \"architecture\"\n\
     [[step]]\nact = \"writes_file\"\nwho = \"architecture\"\npath = \".fullops-squad/docs/design-docs/f-1.md\"\ncontent = \"---\\nstatus: draft\\n---\\n# F-1 설계\\n\"\nexpect = { verdict = \"pass\" }\n\
     [[step]]\nact = \"runs_fullops\"\nwho = \"architecture\"\nargs = [\"task\", \"new\", \"--role\", \"dev\", \"--key\", \"F-1\", \"--goal\", \"F-1 구현\"]\nexpect = { verdict = \"pass\" }\n\
     [[step]]\nact = \"commits\"\nwho = \"architecture\"\n\
     [[step]]\nact = \"runs_fullops\"\nwho = \"architecture\"\nargs = [\"lint\"]\n\
     [[step]]\nact = \"runs_fullops\"\nwho = \"architecture\"\nargs = [\"done\"]\nexpect = { verdict = \"pass\" }\n\
     [[step]]\nact = \"orca_sends\"\n\
     [[step]]\nact = \"lead_waits\"\nexpect = { verdict = \"pass\" }\n\
     [[step]]\nact = \"lead_merges\"\nkey = \"F-1\"\nexpect = { verdict = \"pass\" }\n\
     [[step]]\nact = \"lead_dispatches\"\nkey = \"F-1\"\nrole = \"dev\"\nbare = true\nagent = \"claude\"\nmodel = \"m\"\neffort = \"medium\"\nexpect = { verdict = \"pass\" }\n";

/// C3-1: 설계 과제의 구현이 리뷰 reject를 받으면 안내된 수정 배정이 그대로 통한다.
/// 설계 → 구현 → reject → lead의 reopen → fix 배정 → 수정 → 새 SHA 리뷰 → 병합.
/// 설계 역할은 코드를 고치지 않으므로 설계 역할 배정의 spec에는 반려된 SHA가 실리지 않는다.
#[test]
fn design_task_reject_reopens_and_the_fix_round_merges() {
    let mut d = Driver::new(&roles(&[("dev", "worker"), ("architecture", "design")]));
    steps(
        &mut d,
        &format!("{DESIGNED}{}", round_of("F-1", 400, "msg_r0")),
    );
    let lead = d.sb.path("lead");
    let old = head(&d.sb, "dev");
    let cli = |d: &Driver, args: &[&str]| d.sb.cli(&lead, args);
    let snapshot = cli(&d, &["review", "snapshot", "F-1"]);
    assert_eq!(
        snapshot.code,
        Some(0),
        "{}{}",
        snapshot.stdout,
        snapshot.stderr
    );
    review_of(&d.sb, "F-1", &old, "reject");

    // 안내는 구현 역할을 가리키는 수정 배정이다
    let fix = "`fullops dispatch F-1 --role dev --purpose fix`";
    let checked = cli(&d, &["review", "check", "F-1", "--sha", &old]);
    assert_eq!(
        checked.code,
        Some(0),
        "{}{}",
        checked.stdout,
        checked.stderr
    );
    assert!(
        checked
            .stdout
            .contains("fullops task reopen --role dev --key F-1")
            && checked.stdout.contains(fix),
        "{}",
        checked.stdout
    );
    let merge = cli(&d, &["merge", "F-1"]);
    assert!(
        merge.stderr.contains("FullOps[M-04]") && merge.stderr.contains("--purpose fix"),
        "{}",
        merge.stderr
    );

    let planned = cli(
        &d,
        &["dispatch", "F-1", "--role", "architecture", "--dry-run"],
    );
    assert!(
        !planned.stdout.contains(&old) && !planned.stdout.contains("Rejected"),
        "{}{}",
        planned.stdout,
        planned.stderr
    );

    // 안내대로: lead가 다음 차수를 열고 수정을 배정한다
    let reopened = steps(
        &mut d,
        "[[step]]\nact = \"runs_fullops\"\nargs = [\"task\", \"reopen\", \"--role\", \"dev\", \"--key\", \"F-1\", \"--goal\", \"리뷰 지적 수정\"]\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_dispatches\"\nkey = \"F-1\"\nrole = \"dev\"\npurpose = \"fix\"\nbare = true\nexpect = { verdict = \"pass\" }\n",
    );
    assert!(reopened[0].output.contains(fix), "{}", reopened[0].output);
    steps(&mut d, &round_of("F-1", 401, "msg_r1"));
    let new = head(&d.sb, "dev");
    d.sb.orca_rule(json!({ "when": ["terminal", "list"], "stdout": orca_shapes::terminals(&[]) }));
    for args in [
        vec![
            "review",
            "cleanup",
            "F-1",
            "--sha",
            old.as_str(),
            "--finished",
        ],
        vec!["review", "snapshot", "F-1", "--sha", new.as_str()],
    ] {
        let run = cli(&d, &args);
        assert_eq!(run.code, Some(0), "{args:?}: {}{}", run.stdout, run.stderr);
    }
    review_of(&d.sb, "F-1", &new, "accept");
    let checked = cli(&d, &["review", "check", "F-1", "--sha", &new]);
    assert_eq!(
        checked.code,
        Some(0),
        "{}{}",
        checked.stdout,
        checked.stderr
    );
    steps(
        &mut d,
        "[[step]]\nact = \"runs_fullops\"\nargs = [\"merge\", \"F-1\", \"--completion\", \"msg_r1\"]\nexpect = { verdict = \"pass\" }\n",
    );
    let logs = lead.join(".fullops-squad/handovers/logs");
    assert!(read(&logs.join("F-1.dev.r1.md")).contains("리뷰 지적 수정"));
    let status = cli(&d, &["status"]);
    assert!(!status.stdout.contains("이어서 할 일"), "{}", status.stdout);

    // C3 3-4: 모두 병합된 설계 과제의 재작업 요청(RT-02)은 lead의 수정 차수를 안내하고,
    // 요청이 바뀐 경우에만 요청 글을 남기는 `route --force` → 설계 역할 배정을 덧붙인다. 그 길도 통한다
    let rework = cli(
        &d,
        &[
            "route",
            "F-1-FIX",
            "--request",
            "F-1의 경계 버그를 고친다",
            "--class",
            "design",
            "--role",
            "architecture",
        ],
    );
    assert_eq!(rework.code, Some(2), "{}{}", rework.stdout, rework.stderr);
    assert!(
        rework.stderr.contains("FullOps[RT-02]")
            && rework.stderr.contains("`fullops task reopen --role")
            && rework
                .stderr
                .contains("`fullops route F-1 --force --request \"<바뀐 요청>\" --class design --role architecture`")
            && rework
                .stderr
                .contains("`fullops dispatch F-1 --role architecture`"),
        "{}",
        rework.stderr
    );
    let routed = cli(
        &d,
        &[
            "route",
            "F-1",
            "--force",
            "--request",
            "F-1의 경계 버그를 고친다",
            "--class",
            "design",
            "--role",
            "architecture",
        ],
    );
    assert_eq!(routed.code, Some(0), "{}{}", routed.stdout, routed.stderr);
    let planned = cli(
        &d,
        &["dispatch", "F-1", "--role", "architecture", "--dry-run"],
    );
    assert_eq!(
        planned.code,
        Some(0),
        "{}{}",
        planned.stdout,
        planned.stderr
    );
    assert!(
        planned
            .stdout
            .contains("Request: .fullops-squad/routes/F-1.request.md")
            && !planned.stdout.contains("Rejected SHA"),
        "{}",
        planned.stdout
    );
    assert!(
        read(&lead.join(".fullops-squad/routes/F-1.request.md")).contains("경계 버그"),
        "{}",
        planned.stdout
    );
}

/// C3-2: `worker_done` 뒤에 온 한도 escalation은 끝난 차수를 되살리지 않는다. 재배정은 D-02로 막히고 worker는 하나다.
#[test]
fn limit_escalation_after_worker_done_keeps_the_round_finished() {
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        &format!(
            "{DISPATCH}[[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"A-1\"\n{}\
             [[step]]\nact = \"worker_lints\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"orca_sends\"\nid = \"msg_r0\"\n{}\
             [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\nbare = true\nreason = \"limit\"\n\
             expect = {{ verdict = \"deny\", rule = \"D-02\", escape = \"fullops task reopen --role dev --key A-1\" }}\n",
            report("dev"),
            escalate("ctx_1")
        ),
    );
    assert_eq!(calls(&d.sb, "worker-start").len(), 1);
    let st = state(&d.sb);
    let first = &st["dispatches"][0];
    assert_eq!(first["settled"]["how"], "done", "{first}");
    assert_eq!(first["limit_escalated"], true, "{first}");
    // 보고 하나가 그대로 병합된다
    steps(
        &mut d,
        "[[step]]\nact = \"lead_merges\"\nkey = \"A-1\"\nexpect = { verdict = \"pass\" }\n",
    );
}

/// C3-3: 같은 키·역할의 build가 끝나지 않았으면 fix 배정도 D-10이다(같은 워크트리에 worker 둘).
/// 수정 차수가 실패하면 안내는 reopen 출력과 같은 수정 배정 명령이다.
#[test]
fn build_and_fix_of_the_same_task_are_one_assignment() {
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        &format!(
            "{DISPATCH}[[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\npurpose = \"fix\"\nbare = true\n\
             expect = {{ verdict = \"deny\", rule = \"D-10\" }}\n{}\
             [[step]]\nact = \"runs_fullops\"\nargs = [\"task\", \"reopen\", \"--role\", \"dev\", \"--key\", \"A-1\", \"--goal\", \"수정\"]\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\npurpose = \"fix\"\nbare = true\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\nbare = true\nexpect = {{ verdict = \"deny\", rule = \"D-10\" }}\n\
             [[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"A-1\"\n\
             [[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nfailed = \"도구가 없다\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"orca_sends\"\nid = \"msg_f\"\noutcome = \"failed\"\nbody = \"[실패] A-1 | 사유: 도구가 없다\"\n\
             [[step]]\nact = \"lead_waits\"\nexpect = {{ verdict = \"pass\" }}\n",
            round(3, "msg_r0")
        ),
    );
    assert_eq!(calls(&d.sb, "worker-start").len(), 2);
    let status = d.sb.cli(&d.sb.path("lead"), &["status"]);
    assert!(
        status
            .stdout
            .contains("`fullops dispatch A-1 --role dev --purpose fix`"),
        "{}",
        status.stdout
    );
}

// ── 검증(C3 2바퀴)에서 찾은 결함의 회귀 시험 ─────────────────────────────

/// 설계 과제의 구현(dev)이 reject를 받은 데까지.
fn designed_and_rejected(d: &mut Driver) -> String {
    steps(d, &format!("{DESIGNED}{}", round_of("F-1", 400, "msg_r0")));
    let lead = d.sb.path("lead");
    let old = head(&d.sb, "dev");
    let snapshot = d.sb.cli(&lead, &["review", "snapshot", "F-1"]);
    assert_eq!(
        snapshot.code,
        Some(0),
        "{}{}",
        snapshot.stdout,
        snapshot.stderr
    );
    review_of(&d.sb, "F-1", &old, "reject");
    old
}

/// C3-4·C3-7: 설계 과제의 수정 배정 안내(M-04, RT-02)는 반려된 보고를 쓴 역할(dev)을 적는다. 설계 역할도 `<역할>`도 아니다.
#[test]
fn design_task_fix_guidance_names_the_reported_role() {
    let mut d = Driver::new(&roles(&[("dev", "worker"), ("architecture", "design")]));
    designed_and_rejected(&mut d);
    let lead = d.sb.path("lead");
    let fix = [
        "`fullops task reopen --role dev --key F-1",
        "`fullops dispatch F-1 --role dev --purpose fix`",
    ];
    let merge = d.sb.cli(&lead, &["merge", "F-1"]);
    assert!(
        merge.stderr.contains("FullOps[M-04]")
            && fix.iter().all(|f| merge.stderr.contains(f))
            && !merge.stderr.contains("<역할>"),
        "{}",
        merge.stderr
    );
    let rework = d.sb.cli(
        &lead,
        &[
            "route",
            "F-1-FIX",
            "--request",
            "리뷰 지적 수정",
            "--class",
            "design",
            "--role",
            "architecture",
        ],
    );
    assert_eq!(rework.code, Some(2), "{}{}", rework.stdout, rework.stderr);
    assert!(
        rework.stderr.contains("FullOps[RT-02]")
            && fix.iter().all(|f| rework.stderr.contains(f))
            && !rework.stderr.contains("reopen --role architecture"),
        "{}",
        rework.stderr
    );
}

/// C3-6: 다음 차수가 열린 설계 과제에서 Stop → merge(M-04) → reopen의 안내가 열린 dev 차수의 배정으로 이어진다.
/// (b) 설계 역할이 다음 차수를 써 병합된 경우, (a) lead가 연 수정 차수가 실패한 경우.
#[test]
fn open_round_of_a_design_task_leads_to_dispatching_it() {
    let reopen_then_dispatch = |d: &mut Driver| {
        let lead = d.sb.path("lead");
        let merge = d.sb.cli(&lead, &["merge", "F-1"]);
        assert!(
            merge.stderr.contains("FullOps[M-04]")
                && merge.stderr.contains("task reopen --role dev --key F-1"),
            "{}",
            merge.stderr
        );
        let reopen = d.sb.cli(
            &lead,
            &[
                "task",
                "reopen",
                "--role",
                "dev",
                "--key",
                "F-1",
                "--goal",
                "리뷰 지적 수정",
            ],
        );
        assert_eq!(reopen.code, Some(2), "{}{}", reopen.stdout, reopen.stderr);
        assert!(
            reopen.stderr.contains("FullOps[H-03]")
                && reopen
                    .stderr
                    .contains("`fullops dispatch F-1 --role dev --purpose fix`")
                && !reopen.stderr.contains("FullOps[H-02]")
                && !reopen.stderr.contains("--role architecture"),
            "{}",
            reopen.stderr
        );
        steps(
            d,
            "[[step]]\nact = \"lead_dispatches\"\nkey = \"F-1\"\nrole = \"dev\"\npurpose = \"fix\"\nbare = true\nexpect = { verdict = \"pass\" }\n",
        );
    };

    // (b) 설계 역할이 다음 차수의 지시서를 쓰고 병합됐다
    let mut d = Driver::new(&roles(&[("dev", "worker"), ("architecture", "design")]));
    designed_and_rejected(&mut d);
    let a2 = |step: &str| {
        format!(
            "[[step]]\nact = \"runs_fullops\"\nwho = \"architecture\"\nsession = \"a2\"\n{step}"
        )
    };
    steps(
        &mut d,
        &format!(
            "[[step]]\nact = \"lead_dispatches\"\nkey = \"F-1\"\nrole = \"architecture\"\nbare = true\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"session_starts\"\nwho = \"architecture\"\nsession = \"a2\"\n\
             [[step]]\nact = \"preamble_arrives\"\nwho = \"architecture\"\nsession = \"a2\"\n{}\
             [[step]]\nact = \"commits\"\nwho = \"architecture\"\n{}{}\
             [[step]]\nact = \"orca_sends\"\nid = \"msg_a1\"\n\
             [[step]]\nact = \"lead_waits\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"runs_fullops\"\nargs = [\"merge\", \"F-1\", \"--completion\", \"msg_a1\"]\nexpect = {{ verdict = \"pass\" }}\n",
            a2(
                "args = [\"task\", \"reopen\", \"--role\", \"dev\", \"--key\", \"F-1\", \"--goal\", \"리뷰 지적 수정\"]\nexpect = { verdict = \"pass\" }\n"
            ),
            a2("args = [\"lint\"]\n"),
            a2("args = [\"done\"]\nexpect = { verdict = \"pass\" }\n"),
        ),
    );
    reopen_then_dispatch(&mut d);

    // (a) lead가 연 수정 차수가 실패했다
    let mut d = Driver::new(&roles(&[("dev", "worker"), ("architecture", "design")]));
    designed_and_rejected(&mut d);
    steps(
        &mut d,
        "[[step]]\nact = \"runs_fullops\"\nargs = [\"task\", \"reopen\", \"--role\", \"dev\", \"--key\", \"F-1\", \"--goal\", \"리뷰 지적 수정\"]\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_dispatches\"\nkey = \"F-1\"\nrole = \"dev\"\npurpose = \"fix\"\nbare = true\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"F-1\"\n\
         [[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nfailed = \"도구가 없다\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"orca_sends\"\nid = \"msg_f\"\noutcome = \"failed\"\nbody = \"[실패] F-1 | 사유: 도구가 없다\"\n\
         [[step]]\nact = \"lead_waits\"\nexpect = { verdict = \"pass\" }\n",
    );
    reopen_then_dispatch(&mut d);
}

/// C3-5: 설계 역할이 쓴 N차 지시서는 앞 차수의 보고 수로 배정 여부를 정하지 않는다.
/// 앞 차수가 보관됐거나(A) 같은 차수의 보고가 둘이어도(B) 배정 안내가 나온다. 역할 브랜치에 그 차수의 완료 기록이 있으면 끝난 차수다.
#[test]
fn design_written_round_is_listed_after_archive_and_duplicate_reports() {
    use fullops_core::model::{
        Inbox, Parallel, Plan, RoleId, RoleKind, RouteClass, Sha, State, TaskKey, TestLevel,
    };
    let inbox = Inbox {
        key: TaskKey::new("F-1").unwrap(),
        role: RoleId::new("dev").unwrap(),
        goal: "g".into(),
        base: Sha::new("a".repeat(40)).unwrap(),
        route: RouteClass::Design,
        from: "architecture".into(),
        round: 2,
        budget: 2,
        test_level: TestLevel::Lite,
        parallel: Parallel::On,
    };
    let dispatch = |id: &str, at: i64| {
        json!({"id": id, "request": "r", "task_id": "t", "run": "run_1", "key": "F-1", "role": "dev", "purpose": "build",
            "tuple": {"agent": "claude", "model": "m", "effort": "medium"}, "terminal": null, "worktree": "w",
            "session_mode": "fresh", "reason": null, "started_at": at, "bound_session": null, "sub_of": null,
            "brief": null, "reported": null,
            "settled": {"how": "done", "outcome": "succeeded", "message": null, "at": at + 1}})
    };
    let report = |id: &str, at: i64| {
        json!({"id": id, "dispatch": null, "key": "F-1", "role": "dev", "branch": null, "sha": null,
            "sha_source": null, "outcome": "succeeded", "received_at": at, "hold": null,
            "released_hold": null, "landed_at": null, "drift": null})
    };
    let listed = |dispatches: Value, completions: Value, finished: bool| {
        let state: State = serde_json::from_value(
            json!({"v": 1, "dispatches": dispatches, "completions": completions}),
        )
        .unwrap();
        let boxes = [(inbox.clone(), RoleKind::Worker, finished)];
        let out = fullops_core::flow::leftover(&state, &Plan::default(), &boxes);
        out.iter().any(|line| line.contains("--role dev"))
    };
    // A: 0차는 병합·보관됐다. 1차의 배정(30)과 보고(40)만 남았다
    assert!(listed(
        json!([dispatch("ctx_2", 30)]),
        json!([report("msg_r1", 40)]),
        false
    ));
    // B: 0차의 보고가 둘(20, 21), 1차의 배정(50)과 보고(60)
    let two = json!([
        dispatch("ctx_1", 10),
        dispatch("ctx_2", 15),
        dispatch("ctx_3", 50)
    ]);
    let reports = json!([
        report("msg_a", 20),
        report("msg_b", 21),
        report("msg_r1", 60)
    ]);
    assert!(listed(two.clone(), reports.clone(), false));
    // 2차를 배정했으면 나오지 않고, 그 차수가 역할 브랜치에서 끝났어도 나오지 않는다
    let mut assigned = two.as_array().unwrap().clone();
    assigned.push(dispatch("ctx_4", 70));
    assert!(!listed(Value::Array(assigned), reports.clone(), false));
    assert!(!listed(two, reports, true));
}

// ── 검증(C3 3바퀴)에서 찾은 결함의 회귀 시험 ─────────────────────────────

/// C3 3-1: `done`이 완료 기록을 커밋한 뒤(R-06) push가 실패해(R-07) 실패 보고를 보낸 차수.
/// status와 H-03이 가리키는 재배정이 D-02로 막히지 않고 그 완료 기록을 지시서로 다시 보내며, worker의 `done` 다시 실행이 보고를 끝낸다.
#[test]
fn failed_report_after_done_redispatches_the_record() {
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        &format!(
            "{DISPATCH}[[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"A-1\"\n{}\
             [[step]]\nact = \"worker_lints\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"orca_sends\"\nid = \"msg_f\"\noutcome = \"failed\"\nbody = \"[실패] A-1 | 사유: 푸시 실패\"\n\
             [[step]]\nact = \"lead_waits\"\nexpect = {{ verdict = \"pass\" }}\n",
            report("dev")
        ),
    );
    let lead = d.sb.path("lead");
    let logs = ".fullops-squad/handovers/logs";
    let record = read(&d.sb.path("dev").join(logs).join("A-1.dev.r0.md"));
    let next = "`fullops dispatch A-1 --role dev`";
    let status = d.sb.cli(&lead, &["status"]);
    assert!(status.stdout.contains(next), "{}", status.stdout);
    let reopen = d.sb.cli(
        &lead,
        &[
            "task", "reopen", "--role", "dev", "--key", "A-1", "--goal", "수정",
        ],
    );
    assert_eq!(reopen.code, Some(2), "{}{}", reopen.stdout, reopen.stderr);
    assert!(
        reopen.stderr.contains("FullOps[H-03]") && reopen.stderr.contains(next),
        "{}",
        reopen.stderr
    );
    // 안내대로 다시 배정하면 그 차수의 완료 기록이 지시서로 간다
    steps(
        &mut d,
        "[[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\nbare = true\nexpect = { verdict = \"pass\" }\n",
    );
    let starts = calls(&d.sb, "worker-start");
    assert_eq!(starts.len(), 2);
    let spec = starts.last().unwrap()["stdin"].as_str().unwrap();
    let handover = spec.split("Handover: ").nth(1).unwrap();
    let handover = handover.split(';').next().unwrap();
    assert!(handover.ends_with("logs/A-1.dev.r0.md"), "{spec}");
    // 새 worker의 done 다시 실행이 보고를 끝내고, 그 보고가 병합된다. 보고를 받은 차수의 재배정은 다시 D-02다
    steps(
        &mut d,
        "[[step]]\nact = \"session_starts\"\nwho = \"dev\"\n[[step]]\nact = \"preamble_arrives\"\nwho = \"dev\"\n\
         [[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"orca_sends\"\nid = \"msg_r0\"\n[[step]]\nact = \"lead_waits\"\nexpect = { verdict = \"pass\" }\n\
         [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\nbare = true\n\
         expect = { verdict = \"deny\", rule = \"D-02\", escape = \"fullops task reopen --role dev --key A-1\" }\n\
         [[step]]\nact = \"lead_merges\"\nkey = \"A-1\"\nexpect = { verdict = \"pass\" }\n",
    );
    assert_eq!(calls(&d.sb, "worker-start").len(), 2);
    assert_eq!(read(&lead.join(logs).join("A-1.dev.r0.md")), record);
}

/// C3 3-3·C4-1: 설계 역할이 다음 차수의 지시서를 보낸 보고(msg_a1)가 병합 전이면 RT-02는 그 보고의 병합을 먼저 안내하고
/// 수정 배정은 반려된 보고를 쓴 dev를 적는다. 그 전에 lead가 reopen하면 H-03으로 거절되고 0차 완료 기록은 그대로다.
/// 안내대로 병합하면 설계 역할이 쓴 1차 지시서가 남고 그 차수가 dev에 배정된다.
#[test]
fn rework_waits_for_the_design_roles_pending_brief() {
    let mut d = Driver::new(&roles(&[("dev", "worker"), ("architecture", "design")]));
    designed_and_rejected(&mut d);
    let record = read(
        &d.sb
            .path("dev")
            .join(".fullops-squad/handovers/logs/F-1.dev.r0.md"),
    );
    let a2 = |step: &str| {
        format!(
            "[[step]]
act = \"runs_fullops\"
who = \"architecture\"
session = \"a2\"
{step}"
        )
    };
    steps(
        &mut d,
        &format!(
            "[[step]]
act = \"lead_dispatches\"
key = \"F-1\"
role = \"architecture\"
bare = true
expect = {{ verdict = \"pass\" }}
             [[step]]
act = \"session_starts\"
who = \"architecture\"
session = \"a2\"
             [[step]]
act = \"preamble_arrives\"
who = \"architecture\"
session = \"a2\"
{}",
            a2(
                "args = [\"task\", \"reopen\", \"--role\", \"dev\", \"--key\", \"F-1\", \"--goal\", \"설계 변경 반영\"]
expect = { verdict = \"pass\" }
"
            ),
        ),
    );
    // 설계 역할이 1차 지시서 본문을 채운다
    let brief =
        d.sb.path("architecture")
            .join(".fullops-squad/handovers/to_dev.md");
    let text = format!(
        "{}
- [ ] 설계가 새로 정한 경계 규칙
",
        read(&brief)
    );
    d.sb.write(&brief, &text);
    steps(
        &mut d,
        &format!(
            "[[step]]
act = \"commits\"
who = \"architecture\"
{}{}             [[step]]
act = \"orca_sends\"
id = \"msg_a1\"
             [[step]]
act = \"lead_waits\"
expect = {{ verdict = \"pass\" }}
",
            a2("args = [\"lint\"]
"),
            a2("args = [\"done\"]
expect = { verdict = \"pass\" }
"),
        ),
    );
    let lead = d.sb.path("lead");
    let rework = d.sb.cli(
        &lead,
        &[
            "route",
            "F-1-FIX",
            "--request",
            "리뷰 지적 수정",
            "--class",
            "design",
            "--role",
            "architecture",
        ],
    );
    assert_eq!(rework.code, Some(2), "{}{}", rework.stdout, rework.stderr);
    assert!(
        rework.stderr.contains("FullOps[RT-02]")
            && rework
                .stderr
                .contains("`fullops merge F-1 --completion msg_a1`")
            && rework
                .stderr
                .contains("`fullops task reopen --role dev --key F-1")
            && !rework.stderr.contains("reopen --role architecture"),
        "{}",
        rework.stderr
    );

    // 병합 전에 lead가 reopen하면 거절되고 기록·인박스는 그대로다
    let logs = lead.join(".fullops-squad/handovers/logs");
    let inbox = lead.join(".fullops-squad/handovers/to_dev.md");
    let before = read(&inbox);
    let reopen = d.sb.cli(
        &lead,
        &[
            "task", "reopen", "--role", "dev", "--key", "F-1", "--goal", "수정",
        ],
    );
    assert_eq!(reopen.code, Some(2), "{}{}", reopen.stdout, reopen.stderr);
    assert!(
        reopen.stderr.contains("FullOps[H-03]")
            && reopen
                .stderr
                .contains("`fullops merge F-1 --completion msg_a1`"),
        "{}",
        reopen.stderr
    );
    assert!(!logs.join("F-1.dev.r0.md").exists());
    assert_eq!(read(&inbox), before);

    // 안내대로 먼저 병합하면 0차 기록은 dev의 것 그대로, 인박스는 설계 역할이 쓴 1차 지시서다
    steps(
        &mut d,
        "[[step]]
act = \"runs_fullops\"
args = [\"merge\", \"F-1\", \"--completion\", \"msg_a1\"]
expect = { verdict = \"pass\" }
",
    );
    assert_eq!(read(&logs.join("F-1.dev.r0.md")), record);
    let next = read(&inbox);
    assert!(
        next.contains("round: 1") && next.contains("설계가 새로 정한 경계 규칙"),
        "{next}"
    );
    let reopen = d.sb.cli(
        &lead,
        &[
            "task", "reopen", "--role", "dev", "--key", "F-1", "--goal", "수정",
        ],
    );
    assert!(
        reopen.stderr.contains("FullOps[H-03]")
            && reopen
                .stderr
                .contains("`fullops dispatch F-1 --role dev --purpose fix`"),
        "{}",
        reopen.stderr
    );
    steps(
        &mut d,
        "[[step]]
act = \"lead_dispatches\"
key = \"F-1\"
role = \"dev\"
purpose = \"fix\"
bare = true
expect = { verdict = \"pass\" }
",
    );
    let delivered = read(&d.sb.path("dev").join(".fullops-squad/handovers/to_dev.md"));
    assert!(
        delivered.contains("설계가 새로 정한 경계 규칙"),
        "{delivered}"
    );
}

/// C4-1: 역할이 그 차수를 받은 뒤 lead의 인박스가 바뀌었으면 reopen은 완료 기록으로 덮어쓰지 않고 거절한다.
/// 탈출구대로 그 차수의 지시서로 되돌리면 reopen이 통하고 기록은 역할 브랜치의 것과 같다.
#[test]
fn reopen_does_not_overwrite_a_changed_inbox_with_the_record() {
    let mut d = Driver::new(&Setup::default());
    steps(&mut d, &format!("{DISPATCH}{}", round(3, "msg_r0")));
    let lead = d.sb.path("lead");
    let record = read(
        &d.sb
            .path("dev")
            .join(".fullops-squad/handovers/logs/A-1.dev.r0.md"),
    );
    let inbox = ".fullops-squad/handovers/to_dev.md";
    let changed = format!(
        "{}
- [ ] lead가 나중에 덧붙인 지시
",
        read(&lead.join(inbox))
    );
    d.sb.write(&lead.join(inbox), &changed);
    d.sb.git_ok(&lead, &["commit", "-q", "-am", "지시 덧붙임"]);
    let reopen = |d: &Driver| {
        d.sb.cli(
            &lead,
            &[
                "task", "reopen", "--role", "dev", "--key", "A-1", "--goal", "수정",
            ],
        )
    };
    let refused = reopen(&d);
    assert_eq!(
        refused.code,
        Some(2),
        "{}{}",
        refused.stdout,
        refused.stderr
    );
    assert!(
        refused.stderr.contains("FullOps[reopen]") && refused.stderr.contains("git checkout "),
        "{}",
        refused.stderr
    );
    let logs = lead.join(".fullops-squad/handovers/logs");
    assert!(!logs.join("A-1.dev.r0.md").exists());
    assert_eq!(read(&lead.join(inbox)), changed);

    // 탈출구: 그 차수의 지시서로 되돌려 커밋한 뒤 다시 실행한다
    let fork =
        d.sb.git_ok(&lead, &["merge-base", "HEAD", "refs/heads/fullops/dev"]);
    d.sb.git_ok(&lead, &["checkout", fork.trim(), "--", inbox]);
    d.sb.git_ok(&lead, &["commit", "-q", "-m", "지시 되돌림"]);
    let passed = reopen(&d);
    assert_eq!(passed.code, Some(0), "{}{}", passed.stdout, passed.stderr);
    assert_eq!(read(&logs.join("A-1.dev.r0.md")), record);
}

/// C4-2: `done`이 완료 기록을 커밋한 뒤 push(R-07)에서 멈춰 `reported` 없이 실패 보고를 보낸 차수도
/// 재배정이 D-02로 막히지 않고 그 완료 기록을 지시서로 다시 보낸다(D-02 ↔ H-03 고리가 없다).
#[test]
fn failed_report_after_a_push_stop_redispatches_the_record() {
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
    steps(
        &mut d,
        "[[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nno_checks = \"시험\"\nexpect = { verdict = \"external\", exit = 4 }\n\
         [[step]]\nact = \"orca_sends\"\nid = \"msg_f\"\noutcome = \"failed\"\nbody = \"[실패] A-1 | 사유: 푸시 실패\"\n\
         [[step]]\nact = \"lead_waits\"\nexpect = { verdict = \"pass\" }\n",
    );
    assert!(state(&d.sb)["dispatches"][0]["reported"].is_null());
    d.sb.git_ok(&dev, &["remote", "set-url", "origin", &origin]);
    steps(
        &mut d,
        "[[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\nbare = true\nexpect = { verdict = \"pass\" }\n",
    );
    let starts = calls(&d.sb, "worker-start");
    assert_eq!(starts.len(), 2);
    let spec = starts.last().unwrap()["stdin"].as_str().unwrap();
    let handover = spec.split("Handover: ").nth(1).unwrap();
    assert!(
        handover
            .split(';')
            .next()
            .unwrap()
            .ends_with("logs/A-1.dev.r0.md"),
        "{spec}"
    );
}

/// C4-3: 같은 배정에 성공 보고 뒤 실패 보고가 와도 끝난 차수다. 재배정은 D-02이고 실패 과제로 안내하지 않는다.
#[test]
fn failure_after_success_of_the_same_dispatch_keeps_the_round_finished() {
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        &format!(
            "{DISPATCH}{}[[step]]\nact = \"orca_sends\"\nid = \"msg_f\"\noutcome = \"failed\"\nbody = \"[실패] A-1 | 사유: 늦게 온 실패\"\n\
             [[step]]\nact = \"lead_waits\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\nbare = true\n\
             expect = {{ verdict = \"deny\", rule = \"D-02\", escape = \"fullops task reopen --role dev --key A-1\" }}\n",
            round(3, "msg_r0")
        ),
    );
    assert_eq!(calls(&d.sb, "worker-start").len(), 1);
    let status = d.sb.cli(&d.sb.path("lead"), &["status"]);
    assert!(
        !status.stdout.contains("실패 보고 뒤 멈췄다"),
        "{}",
        status.stdout
    );
}

// ── 검증(C4 2바퀴)에서 찾은 결함의 회귀 시험 ─────────────────────────────

/// 설계 역할의 한 바퀴: 세션 시작 → 설계 문서(첫 바퀴면 dev 지시서도) → 커밋 → lint → done → 보고를 lead가 받는다.
fn design_round(session: &str, doc: &str, brief: bool, message: &str) -> String {
    let a =
        |rest: &str| format!("[[step]]\nwho = \"architecture\"\nsession = \"{session}\"\n{rest}");
    let task = a(
        "act = \"runs_fullops\"\nargs = [\"task\", \"new\", \"--role\", \"dev\", \"--key\", \"F-1\", \"--goal\", \"F-1 구현\"]\nexpect = { verdict = \"pass\" }\n",
    );
    format!(
        "{}{}{}{}[[step]]\nact = \"commits\"\nwho = \"architecture\"\n{}{}\
         [[step]]\nact = \"orca_sends\"\nid = \"{message}\"\n[[step]]\nact = \"lead_waits\"\nexpect = {{ verdict = \"pass\" }}\n",
        a("act = \"session_starts\"\n"),
        a("act = \"preamble_arrives\"\n"),
        a(&format!(
            "act = \"writes_file\"\npath = \".fullops-squad/docs/design-docs/f-1.md\"\ncontent = \"---\\ntitle: F-1 설계\\nstatus: draft\\nupdated: 2026-01-01\\nowner: architecture\\nsummary: F-1 설계\\n---\\n# F-1 설계\\n\\n{doc}\\n\"\nexpect = {{ verdict = \"pass\" }}\n"
        )),
        if brief { task } else { String::new() },
        a("act = \"runs_fullops\"\nargs = [\"lint\"]\nexpect = { verdict = \"pass\" }\n"),
        a("act = \"runs_fullops\"\nargs = [\"done\"]\nexpect = { verdict = \"pass\" }\n"),
    )
}

/// 설계 문서도 리뷰를 받는 설계 과제에서 설계 역할의 첫 보고(msg_a0)를 받은 데까지. 그 보고의 SHA를 돌려준다.
fn designed_report(d: &mut Driver) -> String {
    let lead = d.sb.path("lead");
    let config = lead.join(".fullops-squad/fullops.toml");
    let text = format!(
        "{}\n[review]\nalways = [\".fullops-squad/docs/design-docs/**\"]\n",
        read(&config)
    );
    d.sb.write(&config, &text);
    d.sb.git_ok(&lead, &["commit", "-q", "-am", "설계 문서도 리뷰"]);
    d.sb.git_ok(&lead, &["push", "-q", "origin", "main"]);
    steps(
        d,
        &format!(
            "[[step]]\nact = \"lead_routes\"\nkey = \"F-1\"\nrole = \"architecture\"\nclass = \"design\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"lead_dispatches\"\nkey = \"F-1\"\nrole = \"architecture\"\nbare = true\nexpect = {{ verdict = \"pass\" }}\n{}",
            design_round("a1", "첫 설계", true, "msg_a0")
        ),
    );
    head(&d.sb, "architecture")
}

/// 설계 역할 자신의 보고가 반려됐을 때의 수정 배정 안내: 반려 사유를 요청 글로 남겨 설계 역할에 다시 맡긴다.
const DESIGN_FIX: [&str; 2] = [
    "`fullops route F-1 --force --request \"<반려 사유를 담은 재작업 글>\" --class design --role architecture`",
    "`fullops dispatch F-1 --role architecture`",
];

/// 반려 뒤 새 설계 보고(msg_a1)를 받아 새 SHA로 리뷰하고 병합한다. `old`는 반려된 보고의 SHA다.
fn redesign_and_merge(d: &mut Driver, old: &str, request: &str, doc: &str) {
    let lead = d.sb.path("lead");
    let ok = |run: Run| assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    ok(d.sb.cli(
        &lead,
        &[
            "route",
            "F-1",
            "--force",
            "--request",
            request,
            "--class",
            "design",
            "--role",
            "architecture",
        ],
    ));
    steps(
        d,
        &format!(
            "[[step]]\nact = \"lead_dispatches\"\nkey = \"F-1\"\nrole = \"architecture\"\nbare = true\nexpect = {{ verdict = \"pass\" }}\n{}",
            design_round("a2", doc, false, "msg_a1")
        ),
    );
    // spec이 반려 사유를 담은 요청 글을 싣는다
    let starts = calls(&d.sb, "worker-start");
    let spec = starts.last().unwrap()["stdin"].as_str().unwrap();
    let file = ".fullops-squad/routes/F-1.request.md";
    assert!(spec.contains(&format!("Request: {file}")), "{spec}");
    assert_eq!(read(&d.sb.path("architecture").join(file)), request);
    let new = head(&d.sb, "architecture");
    d.sb.orca_rule(json!({ "when": ["terminal", "list"], "stdout": orca_shapes::terminals(&[]) }));
    for args in [
        vec!["review", "cleanup", "F-1", "--sha", old, "--finished"],
        vec!["review", "snapshot", "F-1", "--sha", new.as_str()],
    ] {
        ok(d.sb.cli(&lead, &args));
    }
    review_of(&d.sb, "F-1", &new, "accept");
    ok(d.sb.cli(&lead, &["review", "check", "F-1", "--sha", &new]));
    steps(
        d,
        "[[step]]\nact = \"runs_fullops\"\nargs = [\"merge\", \"F-1\", \"--completion\", \"msg_a1\"]\nexpect = { verdict = \"pass\" }\n",
    );
    let merged = read(&lead.join(".fullops-squad/docs/design-docs/f-1.md"));
    assert!(merged.contains(doc), "{merged}");
}

/// C4-4: 설계 역할의 보고가 리뷰 reject를 받으면 review check·M-04·RT-02가 모두 설계 역할 재배정을 안내하고
/// (`task reopen --role architecture`도, 먼저 병합하라는 안내도 아니다) 안내대로 하면 새 보고가 리뷰를 거쳐 병합된다.
#[test]
fn rejected_design_report_goes_back_to_the_design_role() {
    let mut d = Driver::new(&roles(&[("dev", "worker"), ("architecture", "design")]));
    let old = designed_report(&mut d);
    let lead = d.sb.path("lead");
    let cli = |args: &[&str]| d.sb.cli(&lead, args);
    let snapshot = cli(&["review", "snapshot", "F-1"]);
    assert_eq!(
        snapshot.code,
        Some(0),
        "{}{}",
        snapshot.stdout,
        snapshot.stderr
    );
    review_of(&d.sb, "F-1", &old, "reject");
    let guided = |text: &str| {
        DESIGN_FIX.iter().all(|f| text.contains(f))
            && !text.contains("reopen --role architecture")
            && !text.contains("--purpose fix")
    };
    let checked = cli(&["review", "check", "F-1", "--sha", &old]);
    assert_eq!(
        checked.code,
        Some(0),
        "{}{}",
        checked.stdout,
        checked.stderr
    );
    assert!(guided(&checked.stdout), "{}", checked.stdout);
    let merge = cli(&["merge", "F-1"]);
    assert!(
        merge.stderr.contains("FullOps[M-04]") && guided(&merge.stderr),
        "{}",
        merge.stderr
    );
    let rework = cli(&[
        "route",
        "F-1-FIX",
        "--request",
        "설계 지적 반영",
        "--class",
        "design",
        "--role",
        "architecture",
    ]);
    assert_eq!(rework.code, Some(2), "{}{}", rework.stdout, rework.stderr);
    assert!(
        rework.stderr.contains("FullOps[RT-02]")
            && guided(&rework.stderr)
            && !rework.stderr.contains("--completion msg_a0"),
        "{}",
        rework.stderr
    );
    // 안내대로 하면 새 보고가 병합된다
    redesign_and_merge(
        &mut d,
        &old,
        "리뷰 반려: 경계 규칙이 빠졌다",
        "경계 규칙을 더한 설계",
    );
}

/// C4-4: 설계 역할의 보고가 병합 뒤 lint에 걸려(M-07) 되돌려지면 안내는 설계 역할 재배정이고, 그 길이 통한다.
#[test]
fn design_report_failing_lint_after_merge_goes_back_to_the_design_role() {
    let mut d = Driver::new(&roles(&[("dev", "worker"), ("architecture", "design")]));
    let old = designed_report(&mut d);
    let lead = d.sb.path("lead");
    let ok = |run: Run| assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    ok(d.sb.cli(&lead, &["review", "snapshot", "F-1"]));
    review_of(&d.sb, "F-1", &old, "accept");
    ok(d.sb.cli(&lead, &["review", "check", "F-1", "--sha", &old]));
    // 보고 뒤 기본 브랜치에 설계 문서의 낱말을 막는 검사가 들어온다
    let rule = r#"{"schema_version":1,"commands":[],"exclude":[],"rules":[{"code":"OLD-001","description":"옛 설계","pattern":"첫 설계","severity":"ERROR"}]}"#;
    d.sb.write(&lead.join(".fullops-squad/lint/lint.json"), rule);
    d.sb.git_ok(&lead, &["add", "-A"]);
    d.sb.git_ok(&lead, &["commit", "-q", "-m", "옛 설계 검사"]);
    d.sb.git_ok(&lead, &["push", "-q", "origin", "main"]);
    let merge = d.sb.cli(&lead, &["merge", "F-1"]);
    assert!(
        merge.stderr.contains("FullOps[M-07]")
            && DESIGN_FIX.iter().all(|f| merge.stderr.contains(f))
            && !merge.stderr.contains("reopen --role architecture"),
        "{}",
        merge.stderr
    );
    redesign_and_merge(
        &mut d,
        &old,
        "병합 뒤 lint 실패: 옛 낱말을 걷어낸다",
        "새 설계",
    );
}

/// C4-5: 0차 배정의 성공 보고가 한도 재배정 뒤에 늦게 오고 재배정이 실패로 끝나도 그 차수는 끝났다.
/// 같은 차수의 재전송은 D-02이고 status는 실패 과제로 안내하지 않는다.
#[test]
fn failed_redispatch_after_the_rounds_success_keeps_it_finished() {
    let mut d = Driver::new(&Setup::default());
    steps(
        &mut d,
        &format!(
            "{DISPATCH}[[step]]\nact = \"worker_works\"\nrole = \"dev\"\nkey = \"A-1\"\n{}\
             [[step]]\nact = \"worker_lints\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"worker_finishes\"\nrole = \"dev\"\nexpect = {{ verdict = \"pass\" }}\n{}\
             [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\nbare = true\nreason = \"limit\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"orca_sends\"\nid = \"msg_r0\"\ndispatch = \"ctx_1\"\n\
             [[step]]\nact = \"lead_waits\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"orca_sends\"\nid = \"msg_f\"\noutcome = \"failed\"\nbody = \"[실패] A-1 | 사유: 도구가 없다\"\n\
             [[step]]\nact = \"lead_waits\"\nexpect = {{ verdict = \"pass\" }}\n\
             [[step]]\nact = \"lead_dispatches\"\nkey = \"A-1\"\nrole = \"dev\"\nbare = true\n\
             expect = {{ verdict = \"deny\", rule = \"D-02\", escape = \"fullops task reopen --role dev --key A-1\" }}\n",
            report("dev"),
            escalate("ctx_1")
        ),
    );
    assert_eq!(calls(&d.sb, "worker-start").len(), 2);
    let status = d.sb.cli(&d.sb.path("lead"), &["status"]);
    assert!(
        !status.stdout.contains("실패 보고 뒤 멈췄다"),
        "{}",
        status.stdout
    );
}

/// C4-6: RT-02의 수정 배정은 구현 역할(worker)을 적는다. 그 키를 나중에 보고한 tester가 아니다.
#[test]
fn rework_guidance_skips_the_tester() {
    let mut d = Driver::new(&roles(&[("dev", "worker"), ("qa", "tester")]));
    steps(&mut d, &format!("{DISPATCH}{}", round(3, "msg_r0")));
    // 같은 키의 tester 보고가 뒤에 왔다
    let mut st = state(&d.sb);
    let mut qa = st["completions"][0].clone();
    qa["id"] = json!("msg_qa");
    qa["dispatch"] = Value::Null;
    qa["role"] = json!("qa");
    qa["received_at"] = json!(qa["received_at"].as_i64().unwrap() + 1);
    st["completions"].as_array_mut().unwrap().push(qa);
    let path = d.sb.path("lead").join(".git/fullops/state.json");
    std::fs::write(&path, st.to_string()).unwrap();
    let rework = d.sb.cli(
        &d.sb.path("lead"),
        &[
            "route",
            "A-1-FIX",
            "--request",
            "수정",
            "--class",
            "direct",
            "--role",
            "dev",
        ],
    );
    assert_eq!(rework.code, Some(2), "{}{}", rework.stdout, rework.stderr);
    assert!(
        rework.stderr.contains("FullOps[RT-02]")
            && rework
                .stderr
                .contains("`fullops task reopen --role dev --key A-1")
            && !rework.stderr.contains("--role qa"),
        "{}",
        rework.stderr
    );
}
