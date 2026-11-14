//! 묶음 B(hook)의 통합 시험: 빌드한 바이너리를 실제 hook처럼 돌린다.
//! 시나리오 파일(`scenarios/*.toml`)이 주력이고, 동사로 적을 수 없는 것(계약 표본 그대로의 입력, 깨진 stdin, 병렬 경합,
//! 살아 있는 대기 표식, 무작위 이름의 리뷰 사본, 여러 경우를 한 상태에서 잇는 Stop 규칙)만 여기서 직접 본다.

use fullops_core::model::*;
use fullops_testkit::{Sandbox, Setup, orca_shapes, run_scenario, scenario};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn scenarios_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios")
}

/// 묶음 B가 맡은 시나리오 파일(설계 18.2절 표의 "시나리오" 칸). 이름은 `<ID 소문자>[-<경우>].toml`이다.
const OWNED: &[&str] = &[
    "a1", "a2", "a3", "a4", "b1", "b2", "b3", "b4", "b5", "b6", "c1", "c2", "c3", "c4", "c5", "d7",
    "e1", "e2", "e3", "e4", "e5", "f1", "f2", "g1", "g2", "g4", "g5", "h1", "h2", "h3", "i1", "i2",
    "i3", "i4", "j1", "j2", "j3", "k1", "k2", "k3", "m1", "m2", "n1", "o1", "o2", "o3", "o4", "o5",
    "q7", "q8", "r1", "r5", "r7",
];

fn owned_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(scenarios_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "toml"))
        .filter(|path| {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            let id = stem.split('-').next().unwrap();
            OWNED.contains(&id)
        })
        // 하나만 다시 돌릴 때: FULLOPS_SCENARIO=i2,j1
        .filter(|path| {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            std::env::var("FULLOPS_SCENARIO")
                .map_or(true, |only| only.split(',').any(|o| o == stem))
        })
        .collect();
    files.sort();
    files
}

#[test]
fn owned_scenarios_pass() {
    let files = owned_files();
    // 맡은 ID마다 파일이 하나 이상 있다
    for id in OWNED {
        let has = files.iter().any(|f| {
            let stem = f.file_stem().unwrap().to_string_lossy().into_owned();
            stem.split('-').next() == Some(id)
        });
        let filtered = std::env::var_os("FULLOPS_SCENARIO").is_some();
        assert!(has || filtered, "시나리오 {id}의 파일이 없다");
    }
    // 시나리오마다 임시 레포를 만들므로 여럿을 함께 돌린다
    let failures: Vec<String> = std::thread::scope(|scope| {
        let workers: Vec<_> = files
            .chunks(files.len().div_ceil(6).max(1))
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .filter_map(|path| {
                            let loaded = scenario::load(path).and_then(|s| run_scenario(&s));
                            loaded.err().map(|e| format!("{}: {e}", path.display()))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap())
            .collect()
    });
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// 규칙 ID마다 걸림 시나리오가 있다: 맡은 시나리오의 `expect.rule`과 여기 시험이 직접 보는 것을 합친다.
#[test]
fn every_rule_has_a_hit_scenario() {
    let mut expected: Vec<String> = Vec::new();
    for path in owned_files() {
        let scenario = scenario::load(&path).unwrap();
        for step in &scenario.steps {
            // 묶음을 합친 뒤에는 건너뛰는 단계가 없다: 걸림은 실제로 실행된 단계만 센다
            assert!(step.pending.is_none(), "{}: 보류 단계", path.display());
            expected.extend(step.expect.as_ref().and_then(|e| e.rule.clone()));
        }
    }
    // 안내·효과 규칙은 출력에 ID가 없다. 그 걸림은 아래 시험과 시나리오의 escape 단언이 본다
    let by_effect = [
        "S-00", "S-01", "S-02", "S-03", "S-04", "S-05", "S-06", "P-00", "P-01", "P-02", "P-03",
        "T-11", "T-12", "T-13", "F-01",
    ];
    for rule in fullops_core::rules::RULES {
        assert!(
            expected.iter().any(|id| id == rule.id) || by_effect.contains(&rule.id),
            "{}의 걸림 시나리오가 없다",
            rule.id
        );
    }
}

// ── 아래는 동사로 적을 수 없는 것을 직접 본다 ─────────────────────

fn fixture(host: &str, name: &str) -> Value {
    let dir = scenarios_dir().join("fixtures/host");
    let text = std::fs::read_to_string(dir.join(host).join(name))
        .unwrap_or_else(|e| panic!("{host}/{name}: {e}"));
    serde_json::from_str(&text).unwrap()
}

/// 표본의 자리 채움 경로를 시험 폴더로 바꾼다.
fn at(mut input: Value, cwd: &Path) -> Value {
    if input.get("cwd").is_some() {
        input["cwd"] = json!(cwd);
    }
    if input.get("workspacePaths").is_some() {
        input["workspacePaths"] = json!([cwd]);
    }
    input
}

/// 글자를 지운 JSON의 뼈대. 출력 표본과 필드 구성이 같은지 볼 때 쓴다.
fn shape(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            Value::Object(map.iter().map(|(k, v)| (k.clone(), shape(v))).collect())
        }
        Value::Array(items) => Value::Array(items.iter().map(shape).collect()),
        Value::String(_) => json!(""),
        other => other.clone(),
    }
}

struct Hooks {
    sb: Sandbox,
}

impl Hooks {
    fn new(setup: &Setup) -> Self {
        Self {
            sb: Sandbox::with(setup),
        }
    }

    /// `fullops hook <args>`에 입력을 넣고 stdout의 JSON을 돌려준다. 종료 코드는 항상 0이고 stderr는 비어 있다.
    fn call(&self, args: &[&str], env: &[(&str, &str)], input: &Value) -> Value {
        let args: Vec<&str> = std::iter::once("hook")
            .chain(args.iter().copied())
            .collect();
        let run = self
            .sb
            .fullops(&self.sb.home, &args, env, input.to_string().as_bytes());
        assert_eq!((run.code, run.stderr.as_str()), (Some(0), ""), "{args:?}");
        serde_json::from_str(&run.stdout)
            .unwrap_or_else(|e| panic!("{args:?}: {e}: {}", run.stdout))
    }

    /// 표본 하나를 그 호스트의 등록 인자로 넣는다.
    fn sample(&self, host: &str, name: &str, who: &str, edit: impl FnOnce(&mut Value)) -> Value {
        let mut input = at(fixture(host, name), &self.sb.path(who));
        edit(&mut input);
        let event = match name {
            n if n.starts_with("session_start") => "SessionStart",
            n if n.starts_with("pre_tool_use") => "PreToolUse",
            _ => "Stop",
        };
        let args: Vec<&str> = if host == "agy" {
            vec![host, "--event", event]
        } else {
            vec![host]
        };
        self.call(&args, &[], &input)
    }

    /// Claude Code 모양의 이벤트 하나.
    fn claude(
        &self,
        who: &str,
        sid: &str,
        event: &str,
        more: Value,
        env: &[(&str, &str)],
    ) -> Value {
        let mut input =
            json!({"hook_event_name": event, "session_id": sid, "cwd": self.sb.path(who)});
        input
            .as_object_mut()
            .unwrap()
            .extend(more.as_object().unwrap().clone());
        self.call(&["claude"], env, &input)
    }

    fn stop(&self, who: &str, sid: &str, env: &[(&str, &str)]) -> Value {
        self.claude(who, sid, "Stop", json!({}), env)
    }

    fn prompt(&self, who: &str, sid: &str, text: &str) -> Value {
        self.claude(who, sid, "UserPromptSubmit", json!({"prompt": text}), &[])
    }

    fn bash(&self, who: &str, sid: &str, command: &str) -> Value {
        let more = json!({"tool_name": "Bash", "tool_input": {"command": command}});
        self.claude(who, sid, "PreToolUse", more, &[])
    }

    fn repo(&self, who: &str) -> fullops_store::Repo {
        fullops_store::discover(&self.sb.path(who))
            .unwrap()
            .unwrap()
    }

    fn set_state(&self, state: Value) {
        let state: State = serde_json::from_value(state).expect("state 형식");
        fullops_store::update_state(&self.repo("lead"), fullops_store::CLI_WAIT, None, |s| {
            *s = state
        })
        .unwrap();
    }

    fn state(&self) -> State {
        fullops_store::read_state(&self.repo("lead")).yes().unwrap()
    }

    fn head(&self, who: &str) -> String {
        self.sb.git_ok(&self.sb.path(who), &["rev-parse", "HEAD"])
    }

    fn commit(&self, who: &str, path: &str, text: &str) -> String {
        let dir = self.sb.path(who);
        self.sb.write(&dir.join(path), text);
        self.sb.git_ok(&dir, &["add", "-A"]);
        self.sb.git_ok(&dir, &["commit", "-q", "-m", "시험 커밋"]);
        self.head(who)
    }

    fn orca(&self, when: &[&str], stdout: Value) {
        self.sb.orca_rule(json!({"when": when, "stdout": stdout}));
    }

    fn orca_calls(&self, word: &str) -> Vec<Value> {
        let has = |call: &Value| call["argv"].as_array().unwrap().iter().any(|a| a == word);
        self.sb.orca_calls().into_iter().filter(has).collect()
    }
}

fn reason(out: &Value) -> &str {
    out["reason"]
        .as_str()
        .or(out["hookSpecificOutput"]["permissionDecisionReason"].as_str())
        .or(out["systemMessage"].as_str())
        .unwrap_or_default()
}

fn dispatch(id: &str, run: &str, terminal: &str, worktree: &Path, more: Value) -> Value {
    let mut d = json!({
        "id": id, "request": "req_0000000000000001", "task_id": format!("task_{id}"), "run": run,
        "key": "A-1", "role": "dev", "purpose": "build",
        "tuple": {"agent": "claude", "model": "m", "effort": "medium"},
        "terminal": terminal, "worktree": path_text(worktree), "session_mode": "fresh", "started_at": 10,
    });
    d.as_object_mut()
        .unwrap()
        .extend(more.as_object().unwrap().clone());
    d
}

fn run_of(id: &str, owner: &str, coordinator: Option<&str>) -> Value {
    json!({"id": id, "owner": owner, "coordinator": coordinator, "opened_at": 5, "closed_at": null})
}

const PREAMBLE: &str = "You are a dispatched worker.\n  orca orchestration send --type worker_done --task-id task_9 --dispatch-id ctx_9 --outcome succeeded\n=== TASK ===\nTask key: A-1";

#[test]
fn sample_inputs_follow_the_output_table() {
    let h = Hooks::new(&Setup::default());
    let out_dir = scenarios_dir().join("fixtures/host");
    let expected = |path: &str| -> Value {
        serde_json::from_str(&std::fs::read_to_string(out_dir.join(path)).unwrap()).unwrap()
    };
    let same_shape =
        |out: &Value, path: &str| assert_eq!(shape(out), shape(&expected(path)), "{path}: {out}");
    let pass = json!({});
    let inbox = ".fullops-squad/handovers/to_dev.md";
    let live = orca_shapes::worker(
        "run_1",
        "ctx_2a3b4c5d6e7f",
        "task_1a2b3c4d5e6f",
        "term_1",
        false,
        "live",
    );
    h.orca(
        &["orchestration", "worker-show"],
        json!({"ok": true, "result": live}),
    );
    h.orca(
        &["orchestration", "send"],
        json!({"ok": true, "result": {}}),
    );
    let worker_start = |input: &mut Value| {
        input["tool_input"]["command"] = json!("orca orchestration worker-start --spec x");
    };

    // Claude Code
    let out = h.sample("claude", "session_start.startup.json", "dev", |_| {});
    same_shape(&out, "output/context.session_start.json");
    let text = out["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        text.contains("worker `dev`") && text.contains("/pstack:*") && !text.contains("caveman")
    );
    assert!(text.chars().count() <= 9000);
    assert_eq!(out["hookSpecificOutput"]["hookEventName"], "SessionStart");
    same_shape(
        &h.sample("claude", "session_start.fork.json", "dev", |_| {}),
        "output/context.session_start.json",
    );
    assert_eq!(
        h.sample("claude", "session_start.resume.json", "dev", |_| {}),
        pass
    );
    let out = h.sample(
        "claude",
        "pre_tool_use.bash_worker_start.json",
        "dev",
        |_| {},
    );
    same_shape(&out, "output/deny.pre_tool_use.json");
    assert!(
        reason(&out).starts_with("FullOps[T-01]: ")
            && reason(&out).contains(" 다음: `fullops dispatch <KEY>`")
    );
    for name in [
        "pre_tool_use.bash.json",
        "pre_tool_use.edit.json",
        "pre_tool_use.write.json",
        "pre_tool_use.agent.json",
        "stop.json",
        "stop.continued.json",
    ] {
        assert_eq!(h.sample("claude", name, "dev", |_| {}), pass, "{name}");
    }
    // PowerShell 도구: 지시서 경로를 실은 terminal send
    let out = h.sample("claude", "pre_tool_use.powershell.json", "dev", |_| {});
    assert!(reason(&out).starts_with("FullOps[T-02]: "), "{out}");
    // Windows의 file_path는 역슬래시 절대 경로다
    let out = h.sample(
        "claude",
        "pre_tool_use.write_windows.json",
        "dev",
        |input| {
            let path = h.sb.path("dev").join(inbox);
            input["tool_input"]["file_path"] = json!(
                path.to_string_lossy()
                    .replace('/', std::path::MAIN_SEPARATOR_STR)
            );
        },
    );
    assert!(
        reason(&out).starts_with("FullOps[T-05]: ") && reason(&out).contains("fullops task new"),
        "{out}"
    );
    // 서브에이전트 안의 호출(agent_id)도 같은 규칙을 탄다
    let out = h.sample(
        "claude",
        "pre_tool_use.bash_in_subagent.json",
        "dev",
        worker_start,
    );
    assert!(reason(&out).starts_with("FullOps[T-01]: "));
    // 배정 프롬프트: dispatch가 묶이고 보고 없는 종료가 막힌다
    assert_eq!(
        h.sample(
            "claude",
            "user_prompt_submit.orca_dispatch.json",
            "dev",
            |_| {}
        ),
        pass
    );
    let sid = SessionId::new("11111111-2222-4333-8444-555555555555").unwrap();
    let bound = fullops_store::read_session(&h.repo("dev"), &sid)
        .yes()
        .flatten()
        .unwrap();
    let bound = bound.dispatch.unwrap();
    assert_eq!(
        (bound.id.as_str(), bound.task.as_deref(), bound.bound_by),
        (
            "ctx_2a3b4c5d6e7f",
            Some("task_1a2b3c4d5e6f"),
            BoundBy::Preamble
        )
    );
    let out = h.sample("claude", "stop.json", "dev", |_| {});
    same_shape(&out, "output/block.stop.json");
    assert!(reason(&out).starts_with("FullOps[X-03]: ") && reason(&out).contains("fullops done"));
    same_shape(
        &h.sample("claude", "stop.json", "dev", |_| {}),
        "output/warn.json",
    );
    // StopFailure: escalation은 dispatch당 한 번이고 본문에 종류와 글이 있다
    for _ in 0..2 {
        assert_eq!(
            h.sample("claude", "stop_failure.rate_limit.json", "dev", |_| {}),
            pass
        );
    }
    let sent = h.orca_calls("send");
    assert_eq!(sent.len(), 1, "{sent:?}");
    let argv: Vec<&str> = sent[0]["argv"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let after = |flag: &str| argv[argv.iter().position(|a| *a == flag).unwrap() + 1];
    assert_eq!(after("--type"), "escalation");
    assert!(after("--subject").starts_with("[fullops:limit]"));
    assert_eq!(
        after("--body"),
        "rate_limit: 429 Too Many Requests. Task task_1a2b3c4d5e6f, Dispatch ctx_2a3b4c5d6e7f."
    );
    assert_eq!(
        (after("--dispatch-id"), after("--task-id")),
        ("ctx_2a3b4c5d6e7f", "task_1a2b3c4d5e6f")
    );

    // Codex
    let out = h.sample("codex", "session_start.startup.json", "dev", |_| {});
    same_shape(&out, "output/context.session_start.json");
    let text = out["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(text.contains("(`spawn_agent`)") && !text.contains("pstack"));
    same_shape(
        &h.sample("codex", "pre_tool_use.bash.json", "dev", |_| {}),
        "output/deny.pre_tool_use.json",
    );
    let out = h.sample("codex", "pre_tool_use.apply_patch_add.json", "dev", |_| {});
    assert!(reason(&out).starts_with("FullOps[T-05]: "), "{out}");
    for name in [
        "pre_tool_use.apply_patch_update.json",
        "pre_tool_use.spawn_agent.json",
        "stop.json",
        "user_prompt_submit.human.json",
    ] {
        assert_eq!(h.sample("codex", name, "dev", |_| {}), pass, "{name}");
    }

    // grok: 닿지 않는 안내 대신 사용자에게 한 줄
    let out = h.sample("grok", "session_start.new.json", "dev", |_| {});
    same_shape(&out, "grok/output/warn.json");
    assert!(
        out["systemMessage"]
            .as_str()
            .unwrap()
            .contains("fullops status")
    );
    same_shape(
        &h.sample(
            "grok",
            "pre_tool_use.run_terminal_command_worker_start.json",
            "dev",
            |_| {},
        ),
        "grok/output/deny.pre_tool_use.json",
    );
    let to_inbox = |input: &mut Value| {
        let path = json!(h.sb.path("dev").join(inbox));
        input["tool_input"]["file_path"] = path.clone();
        input["toolInput"]["file_path"] = path;
    };
    let out = h.sample("grok", "pre_tool_use.write.json", "dev", to_inbox);
    assert!(reason(&out).starts_with("FullOps[T-05]: "), "{out}");
    // 잘린 입력으로는 판정하지 않는다
    let out = h.sample("grok", "pre_tool_use.write.json", "dev", |input| {
        to_inbox(input);
        input["toolInputTruncated"] = json!(true);
    });
    assert_eq!(out, pass);
    for name in [
        "pre_tool_use.run_terminal_command.json",
        "pre_tool_use.spawn_subagent.json",
        "pre_tool_use.search_replace.json",
        "pre_tool_use.run_terminal_command_in_subagent.json",
        "stop.session_end.json",
        "user_prompt_submit.human.json",
        "user_prompt_submit.orca_dispatch.json",
    ] {
        assert_eq!(h.sample("grok", name, "dev", |_| {}), pass, "{name}");
    }
    same_shape(
        &h.sample("grok", "stop.json", "dev", |_| {}),
        "grok/output/block.stop.json",
    );
    // 한 번 막은 뒤의 통과는 조용하다
    assert_eq!(h.sample("grok", "stop.json", "dev", |_| {}), pass);
    assert_eq!(
        h.sample("grok", "stop_failure.rate_limit.json", "dev", |_| {}),
        pass
    );
    assert_eq!(h.orca_calls("send").len(), 2);

    // agy: 통과에도 decision이 있다
    let ask = json!({"decision": "ask"});
    let out = h.sample("agy", "session_start.json", "dev", |_| {});
    same_shape(&out, "agy/output/context.session_start.json");
    let text = out["injectSteps"][0]["systemMessage"]["systemMessage"]
        .as_str()
        .unwrap();
    assert!(
        text.contains("worker `dev`")
            && text.contains("(`invoke_subagent`)")
            && !text.contains("pstack")
    );
    for name in [
        "pre_tool_use.run_command.json",
        "pre_tool_use.invoke_subagent.json",
        "pre_tool_use.replace_file_content.json",
    ] {
        assert_eq!(h.sample("agy", name, "dev", |_| {}), ask, "{name}");
    }
    let out = h.sample("agy", "pre_tool_use.write_to_file.json", "dev", |input| {
        input["toolCall"]["args"]["TargetFile"] = json!(h.sb.path("dev").join(inbox));
    });
    same_shape(&out, "agy/output/deny.pre_tool_use.json");
    assert!(reason(&out).starts_with("FullOps[T-05]: "));
    let out = h.sample("agy", "pre_tool_use.run_command.json", "dev", |input| {
        input["toolCall"]["args"]["CommandLine"] =
            json!("orca orchestration worker-start --spec x");
    });
    assert_eq!(out["decision"], "deny");
    assert_eq!(
        h.sample("agy", "stop.user_canceled.json", "dev", |_| {}),
        json!({"decision": ""})
    );
    assert_eq!(
        h.sample("agy", "stop.json", "dev", |_| {}),
        json!({"decision": ""})
    );
    // preamble이 든 lastUserInput: 그 호출에서 dispatch가 묶이고 지문을 쓴다
    assert_eq!(
        h.sample(
            "agy",
            "pre_tool_use.run_command_orca_dispatch.json",
            "dev",
            |_| {}
        ),
        ask
    );
    let agy = SessionId::new("ec33ebf9-0cba-4100-8142-c61503f6c587").unwrap();
    let session_file = h.repo("dev").session_path(&agy);
    let session = fullops_store::read_session(&h.repo("dev"), &agy)
        .yes()
        .flatten()
        .unwrap();
    assert_eq!(
        session.dispatch.as_ref().unwrap().id.as_str(),
        "ctx_2a3b4c5d6e7f"
    );
    assert_eq!(
        (session.prompt_fp.as_ref().map(String::len), session.host),
        (Some(16), HostId::Agy)
    );
    // 같은 입력으로 더 불러도 세션 파일을 다시 쓰지 않는다
    let before = std::fs::read(&session_file).unwrap();
    for _ in 0..2 {
        assert_eq!(
            h.sample(
                "agy",
                "pre_tool_use.run_command_orca_dispatch.json",
                "dev",
                |_| {}
            ),
            ask
        );
    }
    assert_eq!(std::fs::read(&session_file).unwrap(), before);
    let keep = |input: &mut Value| input["lastUserInput"] = session_input();
    fn session_input() -> Value {
        fixture("agy", "pre_tool_use.run_command_orca_dispatch.json")["lastUserInput"].clone()
    }
    let out = h.sample("agy", "stop.json", "dev", keep);
    same_shape(&out, "agy/output/block.stop.json");
    assert!(out["decision"] == "continue" && reason(&out).starts_with("FullOps[X-03]: "));
    assert_eq!(
        h.sample("agy", "stop.json", "dev", keep),
        json!({"decision": ""})
    );
    // 오류로 끝난 Stop은 F-01을 돌리고 Stop의 통과 글을 낸다
    assert_eq!(
        h.sample("agy", "stop.error.json", "dev", keep),
        json!({"decision": ""})
    );
    let sent = h.orca_calls("send");
    assert_eq!(sent.len(), 3);
    assert!(sent[2]["argv"].as_array().unwrap().iter().any(|a| {
        a.as_str()
            .is_some_and(|a| a.starts_with("error: RESOURCE_EXHAUSTED: quota exceeded. Task "))
    }));

    // 본문(프롬프트, 명령, 오류 글)은 어떤 상태 파일에도 없다
    let mut stack = vec![h.sb.path("lead").join(".git")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.components().any(|c| c.as_os_str() == "fullops") {
                let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
                for body in [
                    "dispatched worker",
                    "Too Many Requests",
                    "worker-start",
                    "RESOURCE_EXHAUSTED",
                ] {
                    assert!(!text.contains(body), "{}에 본문이 있다", path.display());
                }
            }
        }
    }
}

#[test]
fn lead_stop_rules_follow_state() {
    let h = Hooks::new(&Setup::default());
    let dev = h.sb.path("dev");
    let handle = [("ORCA_AGENT_SESSION_ID", "term_lead")];
    h.claude(
        "lead",
        "owner",
        "SessionStart",
        json!({"source": "startup"}),
        &handle,
    );
    h.claude(
        "lead",
        "other",
        "SessionStart",
        json!({"source": "startup"}),
        &[],
    );

    // I2: worker 실행 중. Run을 연 세션(세션 핸들이 coordinator)은 통과, 그 밖은 X-05 (b)
    h.set_state(json!({"v": 1,
        "runs": [run_of("run_1", "lead", Some("term_lead"))],
        "dispatches": [dispatch("ctx_1", "run_1", "term_1", &dev, json!({}))]}));
    assert_eq!(h.stop("lead", "owner", &handle), json!({}));
    let out = h.stop("lead", "other", &[]);
    assert!(
        reason(&out).starts_with("FullOps[X-05]: ")
            && reason(&out).contains("fullops wait --run run_1"),
        "{out}"
    );
    assert!(reason(&h.stop("lead", "other", &[])).starts_with("FullOps: [X-05]"));
    // 백그라운드 대기가 살아 있으면 통과한다. 백그라운드 명령이 세션을 깨우지 않는 호스트(Codex)는 그래도 막는다
    let waiter = Waiter {
        pid: 1,
        sid: None,
        heartbeat: fullops_store::now(),
    };
    let run_1 = RunId::new("run_1").unwrap();
    fullops_store::write_json(&h.repo("lead").waiter_path(&run_1), &waiter).unwrap();
    h.prompt("lead", "other", "다음");
    assert_eq!(h.stop("lead", "other", &[]), json!({}));
    let codex =
        json!({"hook_event_name": "Stop", "session_id": "codex-lead", "cwd": h.sb.path("lead")});
    assert!(reason(&h.call(&["codex"], &[], &codex)).starts_with("FullOps[X-05]: "));

    // T-11: lead의 날것 check 앞에서 미리 본다
    assert_eq!(
        h.bash("lead", "other", "orca orchestration check --run run_1"),
        json!({})
    );
    let peeks = h.orca_calls("check");
    assert!(
        peeks.len() == 1
            && peeks[0]["argv"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a == "--peek")
    );
    // T-12: fullops를 부르면 current.json을 쓴다
    assert_eq!(h.bash("lead", "other", "fullops status"), json!({}));
    let current = fullops_store::read_current(&h.repo("lead"))
        .yes()
        .flatten()
        .unwrap();
    assert_eq!(current.sid.as_str(), "other");

    // I4·J1: 구현 보고와 리뷰 보고가 미처리로 남았다
    let sha = h.commit("dev", "src/a.py", "print(1)\n");
    let done = json!({"how": "done", "outcome": "succeeded", "message": "msg_1", "at": 20});
    let message = |id: &str, dispatch: &str| {
        json!({"id": id, "delivery": null, "run": "run_1", "type": "worker_done",
        "dispatch": dispatch, "received_at": 20, "acked_at": null})
    };
    let completion = json!({"id": "msg_1", "dispatch": "ctx_1", "key": "A-1", "role": "dev", "branch": "fullops/dev",
        "sha": sha, "sha_source": "done", "outcome": "succeeded", "received_at": 20});
    h.set_state(json!({"v": 1,
        "runs": [run_of("run_1", "lead", Some("term_lead"))],
        "dispatches": [
            dispatch("ctx_1", "run_1", "term_1", &dev, json!({"settled": done})),
            dispatch("ctx_2", "run_1", "term_2", &dev, json!({"purpose": "review", "settled": done})),
        ],
        "completions": [completion],
        "messages": [message("msg_1", "ctx_1"), message("msg_2", "ctx_2")]}));
    let out = h.stop("lead", "owner", &handle);
    let text = reason(&out);
    assert!(
        text.starts_with("FullOps[X-05]: ") && text.contains("A-1(msg_1)"),
        "{text}"
    );
    assert!(
        text.contains("fullops wait --run run_1 --ack msg_2 --no-wait")
            && text.contains("fullops merge")
    );
    assert!(
        text.contains("함께 남은 것: X-06 미통합 완료 보고"),
        "{text}"
    );
    // 처리 표시 뒤에는 X-06만 남고 끝낼 때마다 막는다(J3). Run은 닫힌다
    let ack_all = |h: &Hooks| {
        fullops_store::update_state(&h.repo("lead"), fullops_store::CLI_WAIT, None, |s| {
            s.messages.iter_mut().for_each(|m| m.acked_at = Some(30));
        })
        .unwrap()
    };
    ack_all(&h);
    for _ in 0..3 {
        let out = h.stop("lead", "owner", &handle);
        assert!(
            reason(&out).starts_with("FullOps[X-06]: ") && reason(&out).contains("병합 남음"),
            "{out}"
        );
        assert!(reason(&out).contains("fullops merge <KEY>"));
    }
    assert!(h.state().runs[0].closed_at.is_some());
    let status: Status = fullops_store::read_json(&h.repo("lead").status_path())
        .yes()
        .flatten()
        .unwrap();
    assert_eq!(status.pending[0].landing, Some(Landing::MergeLeft));
    // 로컬 기본 브랜치에만 있으면 push가 남았다. 날것 git merge로 들어온 커밋은 lead 세션이 만든 변경이
    // 아니므로 X-01에 걸리지 않고 X-06이 알린다
    h.sb.git_ok(
        &h.sb.path("lead"),
        &["merge", "-q", "--no-edit", "fullops/dev"],
    );
    let out = h.stop("lead", "owner", &handle);
    assert!(
        reason(&out).starts_with("FullOps[X-06]: ") && reason(&out).contains("push 남음"),
        "{out}"
    );
    // 병합 뒤에 lead가 직접 고친 코드는 여전히 X-01이 본다
    h.sb.write(&h.sb.path("lead").join("src/lead.py"), "print(3)\n");
    let out = h.stop("lead", "owner", &handle);
    assert!(
        reason(&out).starts_with("FullOps[X-01]: ") && reason(&out).contains("src/lead.py"),
        "{out}"
    );
    assert!(!reason(&out).contains("src/a.py"), "{out}");
    std::fs::remove_file(h.sb.path("lead").join("src/lead.py")).unwrap();
    h.claude(
        "lead",
        "late",
        "SessionStart",
        json!({"source": "startup"}),
        &handle,
    );
    assert!(reason(&h.stop("lead", "late", &handle)).contains("push 남음"));
    // 날것 push로 통합한 뒤: 통과하고 통합 시각과 처리 표시가 남는다
    h.sb.git_ok(&h.sb.path("lead"), &["push", "-q", "origin", "main"]);
    assert_eq!(h.stop("lead", "owner", &handle), json!({}));
    assert!(h.state().completions[0].landed_at.is_some());

    // J2: 로컬 기본 브랜치가 뒤처져도 원격에 들어가 있으면 막지 않는다
    let sha = h.commit("dev", "src/b.py", "print(2)\n");
    h.sb.git_ok(&dev, &["push", "-q", "origin", "HEAD:main"]);
    h.sb.git_ok(&h.sb.path("lead"), &["fetch", "-q", "origin"]);
    let mut second = completion_with(&sha);
    fn completion_with(sha: &str) -> Value {
        json!({"id": "msg_9", "dispatch": null, "key": "B-2", "role": "dev", "branch": "fullops/dev",
            "sha": sha, "sha_source": "body", "outcome": "succeeded", "received_at": 40})
    }
    h.set_state(json!({"v": 1, "completions": [second.clone()]}));
    assert_eq!(h.stop("lead", "owner", &handle), json!({}));
    // SHA가 없는 보고는 대기로 세고, 보류한 보고는 세지 않는다
    second["sha"] = Value::Null;
    h.set_state(json!({"v": 1, "completions": [second.clone()]}));
    assert!(reason(&h.stop("lead", "owner", &handle)).contains("B-2(SHA 없음)"));
    second["hold"] = json!({"reason": "배포 창", "owner": "lead", "resume": "금요일", "at": 50});
    h.set_state(json!({"v": 1, "completions": [second]}));
    assert_eq!(h.stop("lead", "owner", &handle), json!({}));

    // I1: 분류만 하고 배정하지 않은 키. PLANS.md에 적으면 통과한다
    let routed = TaskKey::new("C-3").unwrap();
    let owner = SessionId::new("owner").unwrap();
    fullops_store::update_session(&h.repo("lead"), &owner, fullops_store::CLI_WAIT, |s| {
        s.add_routed(&routed)
    })
    .unwrap();
    let out = h.stop("lead", "owner", &handle);
    assert!(
        reason(&out).starts_with("FullOps[X-04]: ") && reason(&out).contains("C-3"),
        "{out}"
    );
    assert!(reason(&out).contains("fullops task new"));
    let plan = "# 현재 작업\n\n| 과제 키 | 목표 | 담당 | 상태 | 지시서·원천 링크 |\n|---|---|---|---|---|\n| C-3 | 정리 | dev | 보류(다음 분기) | 사용자 지시 |\n| D-4 | 배정됨 | dev | 대기 | 사용자 지시 |\n";
    h.sb.write(&h.sb.path("lead").join(".fullops-squad/PLANS.md"), plan);
    // K2: 이미 배정된 키의 행은 준비된 행이 아니다
    h.set_state(json!({"v": 1,
        "runs": [run_of("run_2", "lead", Some("term_lead"))],
        "dispatches": [dispatch("ctx_4", "run_2", "term_4", &dev, json!({"key": "D-4"}))]}));
    h.prompt("lead", "owner", "다음");
    assert_eq!(h.stop("lead", "owner", &handle), json!({}));

    // S-03: 경로와 Git 등록이 모두 사라진 snapshot을 닫는다
    h.set_state(json!({"v": 1, "snapshots": [{"review": "A-1@aaaaaaaaaaaa", "path": path_text(&h.sb.root.join("없는-사본")),
        "head": sha, "base": sha, "state": "active", "implementer_session": null, "reviewer_dispatch": null,
        "owner": "lead", "created_at": 1, "hold": null}]}));
    h.claude(
        "lead",
        "fresh",
        "SessionStart",
        json!({"source": "startup"}),
        &[],
    );
    assert_eq!(h.state().snapshots[0].state, SnapshotState::Missing);

    // why: 거부를 사후에 확인한다
    assert!(
        reason(&h.bash("lead", "other", "orca orchestration worker-start"))
            .starts_with("FullOps[T-01]")
    );
    let why = h.sb.cli(
        &h.sb.path("lead"),
        &["why", "--session", "other", "--rule", "T-01", "--json"],
    );
    let why: Value = serde_json::from_str(&why.stdout).unwrap();
    assert_eq!(
        (&why["events"]["PreToolUse"], &why["events"]["StopFailure"]),
        (&json!("실행됨"), &json!("기록 없음"))
    );
    let line = why["trace"].as_array().unwrap().last().unwrap();
    assert_eq!(
        (
            &line["rules"][0]["id"],
            &line["rules"][0]["verdict"],
            &line["actor"]
        ),
        (&json!("T-01"), &json!("deny"), &json!("lead"))
    );
    assert_eq!(line["intents"], json!(["OrcaWorkerStart"]));
}

#[test]
fn worker_stop_rules_follow_state() {
    let h = Hooks::new(&Setup::default());
    let dev = h.sb.path("dev");
    let (parent, sub) = (
        [("ORCA_TERMINAL_HANDLE", "term_p")],
        [("ORCA_TERMINAL_HANDLE", "term_s")],
    );
    let live = |id: &str| orca_shapes::worker("run_1", id, "task", "term", false, "live");
    h.sb.orca_rule(
        json!({"when": ["worker-show", "ctx_p"], "stdout": {"ok": true, "result": live("ctx_p")}}),
    );
    h.sb.orca_rule(
        json!({"when": ["worker-show", "ctx_s"], "stdout": {"ok": true, "result": live("ctx_s")}}),
    );
    // 부모 worker가 하위 worker를 띄웠다
    let with_sub = |settled: Value, acked: Value| {
        json!({"v": 1,
        "runs": [run_of("run_1", "lead", Some("term_lead")), run_of("run_sub", "dispatch:ctx_p", Some("term_p"))],
        "dispatches": [
            dispatch("ctx_p", "run_1", "term_p", &dev, json!({})),
            dispatch("ctx_s", "run_sub", "term_s", &dev, json!({"sub_of": "ctx_p", "brief": "0123456789abcdef", "settled": settled})),
        ],
        "messages": [{"id": "msg_s", "delivery": null, "run": "run_sub", "type": "worker_done", "dispatch": "ctx_s",
                      "received_at": 20, "acked_at": acked}]})
    };
    let unsettled = |h: &Hooks| {
        let mut state = with_sub(Value::Null, Value::Null);
        state["messages"] = json!([]);
        h.set_state(state);
    };
    unsettled(&h);
    // 세션 핸들로 dispatch가 묶인다(7.3절 2번)
    let out = h.claude(
        "dev",
        "sub",
        "SessionStart",
        json!({"source": "startup"}),
        &sub,
    );
    assert!(
        out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("하위 worker다"),
        "{out}"
    );
    h.claude(
        "dev",
        "parent",
        "SessionStart",
        json!({"source": "startup"}),
        &parent,
    );
    h.sb.write(&dev.join("src/sub.py"), "print(1)\n");

    // R5: 하위 worker에게는 X-01을 적용하지 않고 X-03은 적용한다
    let out = h.stop("dev", "sub", &sub);
    assert!(
        reason(&out).starts_with("FullOps[X-03]: ") && !reason(&out).contains("X-01"),
        "{out}"
    );
    assert!(reason(&out).contains("하위 worker는 `fullops done` 없이"));
    // R4: 하위 작업이 남은 부모는 X-01·X-03 없이 X-05만 본다. Orca가 깨우는 세션이면 통과한다
    assert_eq!(h.stop("dev", "parent", &parent), json!({}));
    assert_eq!(
        h.prompt("dev", "bare", PREAMBLE.replace("ctx_9", "ctx_p").as_str()),
        json!({})
    );
    let out = h.stop("dev", "bare", &[]);
    assert!(
        reason(&out).starts_with("FullOps[X-05]: ") && !reason(&out).contains("함께 남은 것"),
        "{out}"
    );
    assert!(reason(&out).contains("fullops wait --run run_sub"));
    // P-01이 묶을 때 state.json의 bound_session을 쓴다
    assert_eq!(
        h.state().dispatches[0]
            .bound_session
            .as_ref()
            .map(|s| s.as_str()),
        Some("bare")
    );
    // 하위 보고를 받았고 처리 표시 전이다: X-05 (a). 하위 보고는 --ack로 표시한다
    let done = json!({"how": "done", "outcome": "succeeded", "message": "msg_s", "at": 20});
    h.set_state(with_sub(done.clone(), Value::Null));
    let out = h.stop("dev", "parent", &parent);
    assert!(
        reason(&out).starts_with("FullOps[X-05]: ") && reason(&out).contains("--ack msg_s"),
        "{out}"
    );
    assert!(!reason(&out).contains("함께 남은 것"));
    // 처리 표시 뒤: 하위 Run을 닫고 X-01과 X-03이 평소대로 걸린다
    h.set_state(with_sub(done, json!(30)));
    let out = h.stop("dev", "parent", &parent);
    assert!(
        reason(&out).starts_with("FullOps[X-01]: ") && reason(&out).contains("함께 남은 것: X-03"),
        "{out}"
    );
    assert!(h.state().runs[1].closed_at.is_some());

    // H2: Orca가 정산을 보고하면 state.json에 정산을 쓰고 통과한다
    let settled = orca_shapes::worker("run_1", "ctx_p", "task", "term_p", true, "live");
    h.sb.git_ok(&dev, &["add", "-A"]);
    h.sb.git_ok(&dev, &["commit", "-q", "-m", "구현"]);
    let head = Sha::new(h.head("dev")).unwrap();
    let base = Sha::new(h.head("lead")).unwrap();
    // G2: lint 통과 기록이 HEAD를 덮으면 X-01이 걸리지 않는다
    let pass = LintPass {
        v: 1,
        base,
        head: head.clone(),
        tree: head.clone(),
        config_sha256: "x".into(),
        test_level: TestLevel::Lite,
        at: 1,
    };
    fullops_store::write_json(&h.repo("dev").lint_dir().join("pass.json"), &pass).unwrap();
    let script = h.sb.orca_dir.join("script.json");
    std::fs::remove_file(&script).unwrap();
    h.sb.orca_rule(json!({"when": ["worker-show"], "stdout": {"ok": true, "result": settled}}));
    h.prompt("dev", "parent", "다음");
    assert_eq!(h.stop("dev", "parent", &parent), json!({}));
    let state = h.state();
    assert_eq!(
        state.dispatches[0].settled.as_ref().map(|s| s.how),
        Some(SettledHow::Done)
    );
    // 통과 뒤의 기록 전용 커밋은 통과 기록을 무효로 만들지 않고, 코드 커밋은 만든다
    h.commit(
        "dev",
        ".fullops-squad/handovers/logs/A-1.dev.r0.md",
        "# 기록\n",
    );
    assert_eq!(h.stop("dev", "parent", &parent), json!({}));
    h.commit("dev", "src/sub.py", "print(2)\n");
    assert!(reason(&h.stop("dev", "parent", &parent)).starts_with("FullOps[X-01]: "));
}

#[test]
fn direct_dev_needs_review_evidence() {
    let setup = Setup {
        mode: "dev".into(),
        primary_role: Some("dev".into()),
        ..Setup::default()
    };
    let h = Hooks::new(&setup);
    h.claude(
        "lead",
        "s",
        "SessionStart",
        json!({"source": "startup"}),
        &[],
    );
    let base = Sha::new(h.head("lead")).unwrap();
    let head = Sha::new(h.commit("lead", "src/big.py", &"print(1)\n".repeat(400))).unwrap();
    assert!(reason(&h.stop("lead", "s", &[])).starts_with("FullOps[X-01]: "));
    let pass = LintPass {
        v: 1,
        base: base.clone(),
        head: head.clone(),
        tree: head.clone(),
        config_sha256: "x".into(),
        test_level: TestLevel::Lite,
        at: 1,
    };
    fullops_store::write_json(&h.repo("lead").lint_dir().join("pass.json"), &pass).unwrap();
    // G5: lint를 통과해도 리뷰가 필요한 변경은 증거가 있어야 끝난다(끝낼 때마다 막는다)
    for _ in 0..2 {
        let out = h.stop("lead", "s", &[]);
        assert!(
            reason(&out).starts_with("FullOps[X-02]: ")
                && reason(&out).contains("fullops review waive"),
            "{out}"
        );
    }
    // 면제 기록(기록 전용 커밋)이 있으면 통과한다
    let key = TaskKey::new("DIRECT").unwrap();
    let need = fullops_core::review_need(
        &ReviewPolicy::default(),
        &[],
        &CodeDef::default(),
        &globset::GlobSet::empty(),
        base.clone(),
        head.clone(),
        base.clone(),
    );
    let waiver = Waiver {
        v: 1,
        key: key.clone(),
        base: base.clone(),
        head: head.clone(),
        reason: "긴급".into(),
        decision: DecisionId::from_number(1),
        decided_by: "user".into(),
        via: None,
        human_turn: true,
        need,
        at: 2,
    };
    let dir = h.repo("lead").review_dir(&key, &head);
    fullops_store::write_json(&dir.join("waiver.json"), &waiver).unwrap();
    h.sb.git_ok(&h.sb.path("lead"), &["add", "-A"]);
    h.sb.git_ok(&h.sb.path("lead"), &["commit", "-q", "-m", "면제 기록"]);
    assert_eq!(h.stop("lead", "s", &[]), json!({}));

    // 코드를 더 바꾸면 앞의 면제는 그 HEAD의 증거가 아니다. 수락 리뷰는 두 해시가 지금 파일과 같아야 유효하다
    let head = Sha::new(h.commit("lead", "src/big.py", &"print(2)\n".repeat(400))).unwrap();
    let pass = LintPass {
        head: head.clone(),
        tree: head.clone(),
        ..pass
    };
    fullops_store::write_json(&h.repo("lead").lint_dir().join("pass.json"), &pass).unwrap();
    assert!(reason(&h.stop("lead", "s", &[])).starts_with("FullOps[X-02]: "));
    let dir = h.repo("lead").review_dir(&key, &head);
    h.sb.write(&dir.join("result.json"), "{}");
    h.sb.write(&dir.join("lint.json"), "{}");
    let accepted = |result_sha256: String| Accepted {
        v: 1,
        key: key.clone(),
        base: base.clone(),
        head: head.clone(),
        result_sha256,
        lint_sha256: sha256_hex(b"{}"),
        reviewer_session: "rv".into(),
        implementer_session: Some("s".into()),
        at: 3,
    };
    let path = dir.join("accepted.json");
    fullops_store::write_json(&path, &accepted("0".repeat(64))).unwrap();
    assert!(reason(&h.stop("lead", "s", &[])).starts_with("FullOps[X-02]: "));
    fullops_store::write_json(&path, &accepted(sha256_hex(b"{}"))).unwrap();
    assert_eq!(h.stop("lead", "s", &[]), json!({}));
}

#[test]
fn turns_are_counted_without_bodies() {
    let h = Hooks::new(&Setup::default());
    h.claude(
        "lead",
        "s",
        "SessionStart",
        json!({"source": "startup"}),
        &[],
    );
    for text in [
        "결제 재시도 버튼을 추가해 줘",
        "\nYou have 1 orchestration message. Run `orca orchestration check --run run_1`.\n",
        "──── From: DISPATCH:CTX_1 (dispatch:ctx_1) (worker_done) ────\nSubject: A-1 완료\n",
        PREAMBLE,
    ] {
        assert_eq!(h.prompt("lead", "s", text), json!({}));
    }
    // worker의 입력은 사람 턴으로 세지 않고, 서브에이전트와 Workflow 호출은 누구든 센다
    h.prompt("dev", "w", "계속해 줘");
    let agent = json!({"tool_name": "Agent", "tool_input": {"prompt": "src/a.py를 맡는다"}});
    h.claude("dev", "w", "PreToolUse", agent, &[]);
    let workflow = json!({"tool_name": "Workflow", "tool_input": {"script": "x"}});
    h.claude("dev", "w", "PreToolUse", workflow, &[]);
    let text = std::fs::read_to_string(h.repo("lead").turns_path()).unwrap();
    let kinds: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let names: Vec<&str> = kinds.iter().map(|k| k["kind"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "human",
            "machine:orca-message",
            "machine:orca-message",
            "subagent",
            "workflow"
        ]
    );
    for line in &kinds {
        let keys: Vec<&String> = line.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["at", "kind", "sid"], "{line}");
    }
    assert!(text.lines().all(|l| l.len() <= 200) && !text.contains("결제"));
    // 병렬 방침 off에서 거부된 호출은 세지 않는다
    let config = h.sb.path("lead").join(".fullops-squad/fullops.toml");
    let off = std::fs::read_to_string(&config)
        .unwrap()
        .replace("parallel = \"on\"", "parallel = \"off\"");
    h.sb.write(&config, &off);
    let agent = json!({"tool_name": "Agent", "tool_input": {"prompt": "x"}});
    assert!(reason(&h.claude("lead", "s2", "PreToolUse", agent, &[])).starts_with("FullOps[T-08]"));
    assert_eq!(
        std::fs::read_to_string(h.repo("lead").turns_path()).unwrap(),
        text
    );
    let session = fullops_store::read_session(&h.repo("lead"), &SessionId::new("s").unwrap())
        .yes()
        .flatten()
        .unwrap();
    assert_eq!(
        (session.turn, session.dispatch.map(|d| d.id.to_string())),
        (4, Some("ctx_9".to_string()))
    );
}

#[test]
fn hook_survives_bad_input_and_contention() {
    let h = Hooks::new(&Setup::default());
    let lead = h.sb.path("lead");
    // A2: 모르는 도구 이름과 깨진 입력은 통과다
    let unknown = json!({"tool_name": "Mystery", "tool_input": {"command": "orca orchestration worker-start"}});
    assert_eq!(
        h.claude("lead", "s", "PreToolUse", unknown.clone(), &[]),
        json!({})
    );
    for junk in [&b"{ \"hook_event_name\": "[..], b"[1,2]", b"\xff\xfe", b""] {
        let run = h.sb.fullops(&h.sb.home, &["hook", "claude"], &[], junk);
        assert_eq!(
            (run.stdout.as_str(), run.code, run.stderr.as_str()),
            ("{}", Some(0), "")
        );
    }

    // N1: 병렬 PreToolUse 8개(세션 파일과 current.json을 함께 쓴다). 모두 판정을 내고 파일이 온전하다
    h.claude(
        "lead",
        "s",
        "SessionStart",
        json!({"source": "startup"}),
        &[],
    );
    let outs: Vec<Value> = std::thread::scope(|scope| {
        let calls: Vec<_> = (0..8)
            .map(|i| {
                let h = &h;
                scope.spawn(move || match i % 2 {
                    0 => h.bash("lead", "s", "fullops status"),
                    _ => h.prompt("lead", "s", PREAMBLE),
                })
            })
            .collect();
        calls.into_iter().map(|c| c.join().unwrap()).collect()
    });
    assert!(outs.iter().all(|o| *o == json!({})));
    let sid = SessionId::new("s").unwrap();
    let session = fullops_store::read_session(&h.repo("lead"), &sid)
        .yes()
        .flatten()
        .unwrap();
    assert_eq!(session.dispatch.unwrap().id.as_str(), "ctx_9");
    assert!(
        fullops_store::read_current(&h.repo("lead"))
            .yes()
            .flatten()
            .is_some()
    );

    // hook 중 강제 종료: stdin을 기다리는 hook을 죽여도 다음 호출이 판정을 낸다
    let mut cmd = h.sb.command(fullops_testkit::bin("fullops"), &lead);
    cmd.args(["hook", "claude"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(200));
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(
        reason(&h.bash("lead", "s2", "orca orchestration worker-start"))
            .starts_with("FullOps[T-01]")
    );

    // state.json 파손: 통과하고 다른 판정은 이어진다
    h.sb.write(&h.repo("lead").state_path(), "{ 깨진 글");
    assert_eq!(h.stop("lead", "s2", &[])["decision"], Value::Null);
    assert!(
        reason(&h.bash("lead", "s2", "orca orchestration worker-start"))
            .starts_with("FullOps[T-01]")
    );
    // 정책 오류에서도 모르는 도구는 통과다. 아는 도구는 거부한다
    h.sb.write(&lead.join(".fullops-squad/fullops.toml"), "schema = = 2");
    assert_eq!(
        h.claude("lead", "s2", "PreToolUse", unknown, &[]),
        json!({})
    );
    assert!(reason(&h.bash("lead", "s2", "ls")).starts_with("FullOps[T-00]"));
    assert!(reason(&h.stop("lead", "s2", &[])).starts_with("FullOps[X-00]"));
}

/// S-04: 플러그인 루트의 실행본이 고정 위치보다 새로우면 실행본 단계를 효과로 돌린다.
/// `--in-place`이거나 플러그인 루트를 모르면 돌리지 않는다.
#[test]
fn self_install_follows_the_plugin_root() {
    let h = Hooks::new(&Setup::default());
    let root = h.sb.root.join("plugin");
    let exe = format!("fullops{}", std::env::consts::EXE_SUFFIX);
    std::fs::create_dir_all(root.join("bin")).unwrap();
    std::fs::copy(fullops_testkit::bin("fullops"), root.join("bin").join(&exe)).unwrap();
    h.sb.write(
        &root.join("VERSION"),
        "2.0.0
",
    );
    let installs = |sid: &str, args: &[&str]| {
        let input = json!({"hook_event_name": "SessionStart", "session_id": sid,
            "cwd": h.sb.path("lead"), "source": "startup"});
        h.call(args, &[], &input);
        let why =
            h.sb.cli(&h.sb.path("lead"), &["why", "--session", sid, "--json"]);
        let why: Value = serde_json::from_str(&why.stdout).unwrap();
        let rules = why["trace"][0]["rules"].as_array().unwrap().clone();
        rules
            .iter()
            .any(|r| r["id"] == "S-04" && r["verdict"] == "effect")
    };
    let root = root.to_string_lossy().into_owned();
    assert!(installs("a", &["claude", "--plugin-root", &root]));
    // 효과가 실행본 단계를 실제로 했다: 고정 위치의 실행본과 install.json
    let fixed = h.sb.home.join(".fullops");
    assert!(fixed.join("bin").join(&exe).is_file());
    let record: Value =
        serde_json::from_str(&std::fs::read_to_string(fixed.join("install.json")).unwrap())
            .unwrap();
    assert_eq!(record["version"], "2.0.0");
    // 같은 버전이 설치된 뒤에는 다시 걸리지 않는다
    assert!(!installs("a2", &["claude", "--plugin-root", &root]));
    assert!(!installs(
        "b",
        &["claude", "--plugin-root", &root, "--in-place"]
    ));
    // 치환되지 않은 변수가 남은 경로는 플러그인 루트로 쓰지 않는다
    assert!(!installs(
        "c",
        &["claude", "--plugin-root", "${CLAUDE_PLUGIN_ROOT}"]
    ));
    assert!(!installs("d", &["claude"]));
}

/// B4·S-03: 실제 `review snapshot`이 만든 사본 안의 세션은 `Reviewer`이고(사본 폴더 이름이 무작위라 시나리오로 적지 못한다),
/// 사본이 사라지면 lead의 SessionStart가 그 기록을 닫는다.
/// 세션 시작 안내의 역할 글(설계 7.7절): 이 글만 보고 다음 명령을 알 수 있다(1.24.0 BRIEF·PRODUCT_BRIEF의 항목).
#[test]
fn session_start_role_guides_carry_the_next_command() {
    let roles = |list: &[(&str, &str)]| {
        list.iter()
            .map(|(id, kind)| (id.to_string(), kind.to_string()))
            .collect()
    };
    let guide = |h: &Hooks, who: &str| {
        let out = h.claude(who, who, "SessionStart", json!({"source": "startup"}), &[]);
        let text = out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.chars().count() <= 9000, "{who}: 한도를 넘는다");
        text
    };
    let has = |text: &str, needles: &[&str]| {
        for needle in needles {
            assert!(text.contains(needle), "`{needle}`가 없다:\n{text}");
        }
    };

    let h = Hooks::new(&Setup {
        roles: roles(&[("dev", "worker"), ("qa", "tester"), ("arch", "design")]),
        ..Setup::default()
    });
    has(
        &guide(&h, "lead"),
        &[
            "coordinator(lead)",
            "`fullops route <KEY> --request",
            "`fullops dispatch <KEY> --role <설계 역할>`",
            "`orca orchestration reply --id <메시지 ID> --body",
            "설계·범위·공유 계약 질문은 직접 답하지 않고 설계 역할에 넘긴다",
            "`docs/planning/intents/<묶음 키>.md`",
            "보안·데이터 손실 방지와 프로젝트 필수 검사는 모든 레벨에서 유지한다",
            "보고용 캡처를 만들지 않는다",
        ],
    );
    // 안내가 시키는 답하기 명령은 hook에 걸리지 않는다(긴 `terminal send`는 T-03이 거부한다)
    let reply = "orca orchestration reply --id msg_3 --body \"공개 API는 바꾸지 않는다. 내부 함수만 고친다\"";
    assert_eq!(h.bash("lead", "lead", reply), json!({}));
    let worker = guide(&h, "dev");
    has(
        &worker,
        &[
            "worker `dev`",
            "묻지 않고 추천안으로 진행하고 완료 보고에 선택과 되돌리는 방법을 적는다",
            "추측하지 않고 preamble의 `ask`로 묻는다",
            "`fullops done`이 출력한 본문으로 preamble의 `worker_done`을 한 번 보낸다",
            "보내지 않고 끝내면 종료가 한 번 막힌다",
        ],
    );
    has(
        &guide(&h, "qa"),
        &[
            "tester `qa`",
            "제품 코드는 고치지 않고",
            "실패는 재현 절차와 증거로 보고하고 직접 고치지 않는다",
            "`worker_done`을 한 번 보낸다",
        ],
    );
    has(
        &guide(&h, "arch"),
        &[
            "설계 역할 `arch`",
            "코드 파일은 고치지 않는다",
            "`fullops task new --role <구현 역할> --key <KEY> --goal",
            "사용자 결정으로 올리지 않고 직접 정해",
            "`[설계] <KEY> | … | 지시서: <만든 인박스 경로>`",
            "`worker_done`을 한 번 보낸다",
        ],
    );

    // split: 설계 좌석은 제품 기획자이고, worker는 제품 규칙·범위만 묻는다
    let h = Hooks::new(&Setup {
        roles: roles(&[("dev", "worker"), ("plan", "design")]),
        routing: "split".into(),
        tech_plan_role: Some("dev".into()),
        ..Setup::default()
    });
    let planner = guide(&h, "plan");
    has(
        &planner,
        &[
            "제품 기획자 `plan`",
            "기술 계획, 구조·API, 버그 수정 방법은 구현 역할의 책임이다",
        ],
    );
    assert!(!planner.contains("설계 역할 `plan`"));
    has(
        &guide(&h, "dev"),
        &["제품 규칙 변경·범위 확대·공유 제품 기준이 불명확한 것만"],
    );
    has(
        &guide(&h, "lead"),
        &["`product`(제품 규칙·범위·공유 제품 기준)만 제품 기획자에게 배정하고"],
    );

    // dev 모드의 주 담당
    let h = Hooks::new(&Setup {
        mode: "dev".into(),
        primary_role: Some("dev".into()),
        ..Setup::default()
    });
    has(
        &guide(&h, "lead"),
        &[
            "dev 모드의 주 담당",
            "제품 범위를 바꾸는 일은 사용자에게 확인한다",
            "`fullops review need`",
        ],
    );
}

/// 활성 표식이 없는 레포에서도 새 세션의 caveman 주입(S-06)만은 돈다(1.24.0의 caveman 주입은 레포를 가리지 않았다).
/// 그 밖의 이벤트와 이어 가는 세션은 아무것도 만들지 않고 통과만 낸다.
#[test]
fn inactive_repo_gets_only_the_caveman_skill() {
    let h = Hooks {
        sb: Sandbox::empty(),
    };
    let repo = h.sb.init_repo("plain");
    let event = |host: &str, name: &str, more: Value| {
        let mut input = json!({"hook_event_name": name, "session_id": "s1", "cwd": repo});
        input
            .as_object_mut()
            .unwrap()
            .extend(more.as_object().unwrap().clone());
        h.call(&[host], &[], &input)
    };
    let start = |host: &str, source: &str| event(host, "SessionStart", json!({"source": source}));
    // 스킬이 없으면 아무 말도 하지 않는다
    assert_eq!(start("claude", "startup"), json!({}));
    h.sb.write(
        &h.sb.home.join(".agents/skills/caveman/SKILL.md"),
        "---\nname: caveman\n---\n군말 없이 답부터 말한다.\n",
    );
    for (host, source) in [
        ("claude", "startup"),
        ("claude", "clear"),
        ("codex", "startup"),
    ] {
        let out = start(host, source);
        let text = out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or_else(|| panic!("{host} {source}: {out}"));
        assert!(text.starts_with("설치된 caveman 스킬을 이 새 세션에 full 강도로 적용한다"));
        assert!(text.ends_with("군말 없이 답부터 말한다."));
        // 역할 안내(S-01)는 붙지 않는다
        assert!(!text.contains("FullOps"), "{text}");
    }
    assert_eq!(start("claude", "resume"), json!({}));
    // SessionStart의 Context가 모델에게 가지 않는 호스트
    assert_eq!(start("grok", "startup"), json!({}));
    let bash = json!({"tool_name": "Bash", "tool_input": {"command": "ls"}});
    for (name, more) in [
        ("UserPromptSubmit", json!({"prompt": "안녕"})),
        ("PreToolUse", bash),
        ("Stop", json!({})),
    ] {
        assert_eq!(event("claude", name, more), json!({}), "{name}");
    }
    // 표식이 없는 레포에는 세션 파일도 추적도 남기지 않는다
    assert!(!repo.join(".git/fullops").exists());
}

#[test]
fn snapshot_session_is_a_reviewer() {
    let h = Hooks::new(&Setup::default());
    let lead = h.sb.path("lead");
    let sha = h.commit("dev", "src/a.py", "print(1)\n");
    h.set_state(
        json!({"v": 1, "completions": [{"id": "msg_1", "dispatch": null, "key": "A-1",
        "role": "dev", "branch": "fullops/dev", "sha": sha, "sha_source": "done",
        "outcome": "succeeded", "received_at": 20}]}),
    );
    // 보고에 묶인 dispatch가 없어 구현자 세션을 직접 준다
    let args = ["review", "snapshot", "A-1", "--implementer-session", "w"];
    let made = h.sb.cli(&lead, &args);
    assert_eq!(made.code, Some(0), "{}", made.stderr);
    let snapshot = h.state().snapshots[0].clone();
    assert_eq!(snapshot.state, SnapshotState::Active);

    let event = |name: &str| json!({"hook_event_name": name, "session_id": "rv", "cwd": snapshot.path, "source": "startup"});
    let out = h.call(&["claude"], &[], &event("SessionStart"));
    let text = out["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(text.contains("읽기 전용 리뷰 사본"), "{text}");
    // 리뷰어는 lead의 규칙(미통합 완료 보고 X-06)에 걸리지 않는다
    assert_eq!(h.call(&["claude"], &[], &event("Stop")), json!({}));
    assert!(reason(&h.stop("lead", "l", &[])).starts_with("FullOps[X-06]: "));

    std::fs::remove_dir_all(&snapshot.path).unwrap();
    h.sb.git_ok(&lead, &["worktree", "prune"]);
    h.claude(
        "lead",
        "l2",
        "SessionStart",
        json!({"source": "startup"}),
        &[],
    );
    assert_eq!(h.state().snapshots[0].state, SnapshotState::Missing);
}

/// 키도 SHA도 없는 완료 보고: X-06의 문구가 메시지 ID와 그 건을 실제로 푸는 명령을 준다(BlockAlways의 탈출구).
#[test]
fn keyless_completion_has_a_way_out() {
    let h = Hooks::new(&Setup::default());
    let lead = h.sb.path("lead");
    h.claude(
        "lead",
        "s",
        "SessionStart",
        json!({"source": "startup"}),
        &[],
    );
    let keyless = json!({"v": 1,
        "runs": [run_of("run_1", "lead", None)],
        "completions": [{"id": "msg_1", "dispatch": null, "key": null, "role": null, "branch": null,
            "sha": null, "sha_source": null, "outcome": "succeeded", "received_at": 20}],
        "messages": [{"id": "msg_1", "delivery": null, "run": "run_1", "type": "worker_done",
            "dispatch": null, "received_at": 20, "acked_at": 21}]});
    h.set_state(keyless.clone());
    for _ in 0..2 {
        let out = h.stop("lead", "s", &[]);
        let text = reason(&out);
        assert!(text.starts_with("FullOps[X-06]: "), "{out}");
        for part in [
            "키 없음(메시지 msg_1, SHA 없음)",
            "fullops completion set-sha <메시지 ID> <SHA> --key <KEY>",
            "fullops hold --completion <메시지 ID>",
        ] {
            assert!(text.contains(part), "{part}: {out}");
        }
    }
    // 길 하나: 메시지 ID만으로 보류한다
    let hold = [
        "hold",
        "--completion",
        "msg_1",
        "--reason",
        "r",
        "--owner",
        "o",
        "--resume",
        "x",
    ];
    let run = h.sb.cli(&lead, &hold);
    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert!(h.state().completions[0].hold.is_some());
    assert_eq!(h.stop("lead", "s", &[]), json!({}));
    // 길 둘: 키와 SHA를 적으면 키로 가리키는 보통의 탈출구(`merge`·`hold <KEY>`)가 통한다
    h.set_state(keyless);
    let head = h.head("lead");
    let run = h.sb.cli(
        &lead,
        &["completion", "set-sha", "msg_1", &head, "--key", "A-1"],
    );
    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    let run = h.sb.cli(
        &lead,
        &[
            "hold", "A-1", "--reason", "r", "--owner", "o", "--resume", "x",
        ],
    );
    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert_eq!(h.stop("lead", "s", &[]), json!({}));
    // 키 없이 부르려면 메시지 ID가 있어야 한다
    let run = h.sb.cli(
        &lead,
        &["hold", "--reason", "r", "--owner", "o", "--resume", "x"],
    );
    assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
}

/// 링크를 만든다(Windows는 junction). 만들지 못하면 거짓이다.
fn link_dir(link: &Path, target: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let line = format!(
            "/c mklink /J \"{}\" \"{}\"",
            link.display(),
            target.display()
        );
        let made = std::process::Command::new("cmd").raw_arg(line).output();
        made.is_ok_and(|o| o.status.success())
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
}

/// 경로 규칙은 실제 경로로 판정한다: 링크(junction)를 거친 경로와 Windows에서 대소문자만 다른 경로도 같은 파일이다.
#[test]
fn path_rules_see_the_real_path() {
    let h = Hooks::new(&Setup::default());
    let lead = h.sb.path("lead");
    let write = |path: &str| {
        let more = json!({"tool_name": "Write", "tool_input": {"file_path": path, "content": "# A-1 — x\n"}});
        h.claude("lead", "s", "PreToolUse", more, &[])
    };
    let inbox = lead.join(".fullops-squad/handovers");
    std::fs::create_dir_all(&inbox).unwrap();
    let plain = write(&inbox.join("A-1.md").to_string_lossy());
    assert!(reason(&plain).starts_with("FullOps[T-05]"), "{plain}");

    let link = lead.join("hv");
    assert!(link_dir(&link, &inbox), "링크를 만들지 못했다");
    let linked = write(&link.join("A-1.md").to_string_lossy());
    // 문구의 경로는 실제 자리다
    assert!(
        reason(&linked).starts_with("FullOps[T-05]: `.fullops-squad/handovers/A-1.md`"),
        "{linked}"
    );
    // 링크 안의 다른 폴더는 그대로 통과한다
    let other = lead.join("docs-link");
    std::fs::create_dir_all(lead.join("docs")).unwrap();
    assert!(link_dir(&other, &lead.join("docs")));
    assert_eq!(write(&other.join("A-1.md").to_string_lossy()), json!({}));
    std::fs::remove_dir(&link)
        .or_else(|_| std::fs::remove_file(&link))
        .unwrap();
    std::fs::remove_dir(&other)
        .or_else(|_| std::fs::remove_file(&other))
        .unwrap();

    if cfg!(windows) {
        let upper = format!(r"{}\.FULLOPS-SQUAD\Handovers\A-1.md", lead.display());
        assert!(
            reason(&write(&upper)).starts_with("FullOps[T-05]"),
            "{upper}"
        );
        // 아직 없는 폴더도 이름만으로 같은 자리다(T-10의 차수 폴더)
        let qa = lead.join(".fullops-squad/docs/evaluations/qa-reports");
        std::fs::create_dir_all(qa.join("A-1")).unwrap();
        let round = format!(
            r"{}\.fullops-squad\DOCS\Evaluations\QA-Reports\A-1-R2\report.md",
            lead.display()
        );
        assert!(
            reason(&write(&round)).starts_with("FullOps[T-10]"),
            "{round}"
        );
    }
}

/// T-09: hook의 cwd가 하위 폴더여도 레포 전체를 담는 add(`-A`, `-C ..`, 변수 경로)는 링크 없는 QA 캡처를 본다.
#[test]
fn commit_gate_sees_the_whole_repo_from_a_subfolder() {
    let h = Hooks::new(&Setup::default());
    let dev = h.sb.path("dev");
    let qa = dev.join(".fullops-squad/docs/evaluations/qa-reports/A-1-test");
    h.commit("dev", "src/lib.rs", "fn a() {}\n");
    h.sb.write(&qa.join("report.md"), "# A-1 test report\n");
    h.sb.write(&qa.join("shot.png"), "PNG");
    let run = |cwd: &Path, tool: &str, command: &str| {
        let input = json!({"hook_event_name": "PreToolUse", "session_id": "s", "cwd": cwd,
            "tool_name": tool, "tool_input": {"command": command}});
        h.call(&["claude"], &[], &input)
    };
    let src = dev.join("src");
    for (cwd, tool, command) in [
        (&src, "Bash", "git add -A && git commit -m qa"),
        (&src, "Bash", "git add --all && git commit -m qa"),
        (&src, "Bash", "git -C .. add . && git commit -m qa"),
        (&src, "Bash", "git stage -A && git commit -m qa"),
        (&src, "PowerShell", "git add -A; git commit -m qa"),
        (
            &dev,
            "Bash",
            "D=.fullops-squad; git add $D && git commit -m qa",
        ),
    ] {
        let out = run(cwd, tool, command);
        assert!(
            reason(&out).starts_with("FullOps[T-09]"),
            "{command}: {out}"
        );
    }
    // 범위를 좁힌 add와 추적 파일만 담는 commit -am은 통과한다
    for command in ["git add . && git commit -m src", "git commit -am src"] {
        assert_eq!(run(&src, "Bash", command), json!({}), "{command}");
    }
}

/// 감시 스레드: stdin이 끝나지 않으면 내부 기한 뒤에 그 호스트의 통과 글을 내고 종료 코드 0으로 끝난다.
/// `--event`로 이벤트를 아는 agy는 그 이벤트의 내부 기한(PreToolUse 4초)을 쓴다: 등록 타임아웃(8초)보다 먼저 답한다.
#[test]
fn watchdog_passes_when_the_deadline_is_over() {
    let sb = Sandbox::with(&Setup::default());
    let mut cmd = sb.command(fullops_testkit::bin("fullops"), &sb.path("lead"));
    cmd.args(["hook", "agy", "--event", "PreToolUse"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    let started = std::time::Instant::now();
    let child = cmd.spawn().unwrap();
    // stdin을 닫지 않고 기다린다
    let out = child.wait_with_output_keeping_stdin();
    assert_eq!(
        (
            String::from_utf8_lossy(&out.stdout).as_ref(),
            out.status.code()
        ),
        (r#"{"decision":"ask"}"#, Some(0))
    );
    let elapsed = started.elapsed().as_millis();
    let registered = fullops_core::deps::HOOK_TIMEOUTS
        .iter()
        .find(|(event, _)| *event == "PreToolUse")
        .map(|(_, seconds)| u128::from(*seconds) * 1000)
        .unwrap();
    assert!((3_900..registered).contains(&elapsed), "{elapsed}ms");
}

trait KeepStdin {
    fn wait_with_output_keeping_stdin(self) -> std::process::Output;
}

impl KeepStdin for std::process::Child {
    fn wait_with_output_keeping_stdin(mut self) -> std::process::Output {
        use std::io::Read;
        // wait_with_output은 stdin을 닫는다. 닫지 않은 채로 stdout을 끝까지 읽는다
        let stdin = self.stdin.take();
        let mut stdout = Vec::new();
        self.stdout
            .take()
            .unwrap()
            .read_to_end(&mut stdout)
            .unwrap();
        let status = self.wait().unwrap();
        drop(stdin);
        std::process::Output {
            status,
            stdout,
            stderr: Vec::new(),
        }
    }
}

/// 벽시계 측정(설계 2.2절). 기본 실행에서는 돌지 않는다:
/// `FULLOPS_PERF_BIN=target/release/fullops.exe cargo test -p fullops --test hook wall_clock -- --ignored --nocapture`
#[test]
#[ignore]
fn wall_clock_report() {
    let h = Hooks::new(&Setup::default());
    let lead = h.sb.path("lead");
    let bin = std::env::var_os("FULLOPS_PERF_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| fullops_testkit::bin("fullops"));
    let input = json!({"hook_event_name": "PreToolUse", "session_id": "perf", "cwd": lead,
        "tool_name": "Bash", "tool_input": {"command": "ls -la"}})
    .to_string();
    // 같은 조건의 기준값으로 git의 프로세스 시작 시간을 함께 잰다
    let git = PathBuf::from("git");
    let cases: [(&str, &PathBuf, &[&str], &str); 3] = [
        ("git --version", &git, &["--version"], ""),
        ("fullops --version", &bin, &["--version"], ""),
        ("무관한 PreToolUse", &bin, &["hook", "claude"], &input),
    ];
    for (name, program, args, stdin) in cases {
        let mut ms: Vec<u128> = (0..31)
            .map(|_| {
                let mut cmd = h.sb.command(program, &lead);
                cmd.args(args);
                let run = h.sb.run(cmd, stdin.as_bytes());
                assert_eq!(run.code, Some(0), "{}", run.stderr);
                run.ms
            })
            .skip(1) // 첫 실행은 파일 캐시를 데운다
            .collect();
        ms.sort();
        println!(
            "{name}: 30회 p50 {}ms, p95 {}ms, 최소 {}ms, 최대 {}ms",
            ms[15], ms[28], ms[0], ms[29]
        );
    }
}
