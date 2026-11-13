//! `cargo xtask bench [--runs <횟수>] [--py-runs <횟수>] [--check]`: hook과 주요 명령의 p50·p95·최대, 프로세스 수,
//! 파일 쓰기 수를 재서 Markdown 표로 stdout에 낸다(설계 2.2·12.4절). 결과는 `docs/bench.md`에 옮겨 적는다.
//!
//! - 시간은 `fullops` 프로세스의 생성부터 종료까지의 벽시계다. 셸 없이 직접 띄운다.
//! - git 프로세스 수는 git의 추적(`GIT_TRACE2_EVENT`)으로, Orca 호출 수는 fake-orca의 기록으로, 쓰기 수는
//!   `fullops/` 상태 폴더의 바뀐 파일 수로 센다. 세는 실행은 시간을 재는 실행과 따로 한 번 한다.
//! - `FULLOPS_PY_BASELINE`이 있으면 py 구동기가 같은 입력으로 1.24.0의 hook 명령을 돌린 값을 나란히 적는다.
//! - `--check`는 p95가 2.2절 목표의 두 배를 넘으면 실패한다(CI용).

use crate::compare::{rerun_in_deps, workspace};
use fullops_core::model::{ArchiveLine, State, Turn};
use fullops_store::{Repo, discover};
use fullops_testkit::host_shapes::{Event, HookCall, Tool, hook_call, verdict_of};
use fullops_testkit::{Sandbox, Setup, bin, orca_shapes};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime};

/// 측정 하나의 결과. 시간은 밀리초다.
struct Stat {
    name: &'static str,
    /// 2.2절의 p95 목표(밀리초). 목표가 없는 입력은 `None`.
    target: Option<f64>,
    /// 세는 실행에서 본 것: hook은 판정, CLI는 종료 코드.
    seen: String,
    p50: f64,
    p95: f64,
    max: f64,
    git: usize,
    orca: usize,
    writes: usize,
}

struct Bench {
    sb: Sandbox,
    runs: usize,
    stats: Vec<Stat>,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// 폴더 아래 파일의 (크기, 수정 시각).
fn snapshot(dir: &Path, into: &mut BTreeMap<PathBuf, (u64, Option<SystemTime>)>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        match entry.metadata() {
            Ok(meta) if meta.is_dir() => snapshot(&path, into),
            Ok(meta) => drop(into.insert(path, (meta.len(), meta.modified().ok()))),
            Err(_) => {}
        }
    }
}

impl Bench {
    /// 프로세스 하나를 끝까지 돌린다. (밀리초, 종료 코드, stdout).
    fn spawn(
        &self,
        cwd: &Path,
        args: &[String],
        stdin: &[u8],
        trace: Option<&Path>,
    ) -> Result<(f64, Option<i32>, String), String> {
        let mut cmd = self.sb.command(bin("fullops"), cwd);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(trace) = trace {
            cmd.env("GIT_TRACE2_EVENT", trace);
        }
        let started = Instant::now();
        let mut child = cmd.spawn().map_err(|e| format!("fullops 실행 실패: {e}"))?;
        if let Some(mut pipe) = child.stdin.take() {
            let _ = pipe.write_all(stdin);
        }
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        Ok((
            ms,
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        ))
    }

    fn hook(
        &self,
        who: &str,
        call: &HookCall,
        trace: Option<&Path>,
    ) -> Result<(f64, String), String> {
        let args: Vec<String> = std::iter::once("hook".to_string())
            .chain(call.args.iter().cloned())
            .collect();
        let (ms, code, stdout) = self.spawn(
            &self.sb.path(who),
            &args,
            call.input.to_string().as_bytes(),
            trace,
        )?;
        if code != Some(0) {
            return Err(format!(
                "hook의 종료 코드가 0이 아닙니다({code:?}): {args:?}"
            ));
        }
        Ok((ms, stdout))
    }

    fn call(&self, who: &str, event: &Event) -> Result<HookCall, String> {
        hook_call(
            "claude",
            &format!("sid-bench-{who}"),
            &self.sb.path(who),
            None,
            event,
        )
        .ok_or_else(|| "이 호스트에 없는 이벤트입니다".to_string())
    }

    /// 입력 하나를 잰다. `once`는 한 번 실행하고 (밀리초, 본 것)을 돌려준다.
    fn measure(
        &mut self,
        name: &'static str,
        target: Option<f64>,
        who: &str,
        before_each: &dyn Fn(&Bench) -> Result<(), String>,
        once: &dyn Fn(&Bench, Option<&Path>) -> Result<(f64, String), String>,
    ) -> Result<(), String> {
        eprintln!("{name}");
        // 세는 실행: git 프로세스, Orca 호출, 상태 폴더의 바뀐 파일
        let dirs: Vec<PathBuf> = discover(&self.sb.path(who))
            .ok()
            .flatten()
            .map(|repo| vec![repo.shared(), repo.local()])
            .unwrap_or_default();
        let files = || {
            let mut found = BTreeMap::new();
            dirs.iter().for_each(|dir| snapshot(dir, &mut found));
            found
        };
        before_each(self)?;
        let trace = self.sb.root.join(format!("trace-{name}.jsonl"));
        let (before, calls) = (files(), self.sb.orca_calls().len());
        let (_, seen) = once(self, Some(&trace))?;
        let after = files();
        let writes = after
            .iter()
            .filter(|(path, meta)| before.get(*path) != Some(meta))
            .count();
        // git이 스스로 띄운 자식 git은 sid에 `/`가 있다. fullops가 띄운 것만 센다
        let git = std::fs::read_to_string(&trace)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|e| e["event"] == "start" && !e["sid"].as_str().unwrap_or("/").contains('/'))
            .count();
        let orca = self.sb.orca_calls().len() - calls;

        let mut times = Vec::with_capacity(self.runs);
        for _ in 0..self.runs {
            before_each(self)?;
            times.push(once(self, None)?.0);
        }
        // 처음 것들은 파일 캐시가 차기 전이라 버린다(300회면 20회)
        let mut times = times.split_off((self.runs / 15).min(self.runs - 1));
        times.sort_by(f64::total_cmp);
        let at = |q: f64| times[((times.len() as f64 * q) as usize).min(times.len() - 1)];
        self.stats.push(Stat {
            name,
            target,
            seen,
            p50: at(0.5),
            p95: at(0.95),
            max: at(1.0),
            git,
            orca,
            writes,
        });
        Ok(())
    }

    fn hook_case(
        &mut self,
        name: &'static str,
        target: Option<f64>,
        who: &str,
        event: &Event,
    ) -> Result<(), String> {
        let call = self.call(who, event)?;
        // Stop은 한 번 막기가 매번 처음처럼 걸리게 사용자 입력으로 턴을 넘긴 뒤에 잰다
        let turn = matches!(event, Event::Stop { .. })
            .then(|| self.call(who, &Event::UserPromptSubmit { prompt: "측정" }))
            .transpose()?;
        self.measure(
            name,
            target,
            who,
            &|b| {
                turn.as_ref()
                    .map_or(Ok(()), |turn| b.hook(who, turn, None).map(drop))
            },
            &|b, trace| {
                let (ms, stdout) = b.hook(who, &call, trace)?;
                Ok((ms, verdict_of("claude", &stdout).to_string()))
            },
        )
    }

    fn cli_case(&mut self, name: &'static str, args: &[&str]) -> Result<(), String> {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        self.measure(name, None, "lead", &|_| Ok(()), &|b, trace| {
            let (ms, code, _) = b.spawn(&b.sb.path("lead"), &args, b"", trace)?;
            Ok((ms, format!("종료 코드 {}", code.unwrap_or(-1))))
        })
    }
}

/// 형식(`docs/formats.md` 4.1절)과 맞는지 타입으로 확인하고 `state.json`을 쓴다. `None`이면 지운다.
fn put_state(repo: &Repo, state: Option<Value>) -> Result<(), String> {
    let Some(state) = state else {
        let _ = std::fs::remove_file(repo.state_path());
        return Ok(());
    };
    let typed: State = serde_json::from_value(state)
        .map_err(|e| format!("측정용 state.json이 형식과 다릅니다: {e}"))?;
    let text = serde_json::to_string(&typed).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(repo.shared()).map_err(|e| e.to_string())?;
    std::fs::write(repo.state_path(), text).map_err(|e| e.to_string())
}

/// worker 하나의 dispatch 기록. `message`가 있으면 그 메시지로 정산됐다.
fn dispatch(id: &str, key: &str, run: &str, worktree: &Path, message: Option<&str>) -> Value {
    let at = now();
    json!({
        "id": id, "request": "req_0000000000000000", "task_id": format!("task_{id}"), "run": run, "key": key, "role": "dev",
        "purpose": "build", "tuple": { "agent": "claude", "model": "test-model", "effort": "medium" },
        "terminal": format!("term_{id}"), "worktree": worktree, "session_mode": "fresh", "reason": null, "started_at": at,
        "bound_session": null, "sub_of": null, "brief": null, "reported": null, "limit_escalated": false,
        "settled": message.map(|m| json!({ "how": "done", "outcome": "succeeded", "message": m, "at": at })),
    })
}

fn open_run(id: &str) -> Value {
    json!({ "id": id, "owner": "lead", "coordinator": null, "opened_at": now(), "closed_at": null })
}

/// `PLANS.md` 과제 표의 행 하나를 `status` 상태로 둔다.
fn plan_row(repo: &Repo, status: &str) -> Result<(), String> {
    let path = repo.plan_path();
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with("| R-1 "))
        .collect();
    let row = format!("| R-1 | R-1 구현 | dev | {status} | 측정 |");
    let mut out = String::new();
    for line in kept {
        out.push_str(line);
        out.push('\n');
        if line.starts_with("|---") {
            out.push_str(&row);
            out.push('\n');
        }
    }
    std::fs::write(&path, out).map_err(|e| e.to_string())
}

/// py 구동기가 같은 입력으로 잰 값. 이름 → {p50, p95, max, procs, verdict, git, orca}.
fn py_baseline(runs: usize) -> Result<Option<Value>, String> {
    let Some(baseline) = std::env::var("FULLOPS_PY_BASELINE")
        .ok()
        .filter(|b| !b.is_empty())
    else {
        return Ok(None);
    };
    eprintln!("py 기준선 측정({runs}회씩)");
    let fakes = bin("fake-orca");
    for name in ["fake-curl", "fake-host"] {
        bin(name);
    }
    let out = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .args(["-X", "utf8", "-B"])
        .arg(workspace().join("scenarios/drivers/py_driver.py"))
        .args(["--bench", &runs.to_string()])
        .env("FULLOPS_PY_BASELINE", baseline)
        .env("FULLOPS_FAKES", fakes.parent().unwrap_or(Path::new(".")))
        .output()
        .map_err(|e| format!("python을 실행하지 못했습니다: {e}"))?;
    let parsed: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    if !parsed["cases"].is_object() {
        return Err(format!(
            "py 구동기의 측정이 실패했습니다: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(Some(parsed["cases"].clone()))
}

pub fn run_bench(runs: usize, py_runs: usize, check: bool) -> Result<(), String> {
    let roles = [("dev", "worker"), ("architecture", "design")];
    let setup = Setup {
        roles: roles
            .map(|(id, kind)| (id.to_string(), kind.to_string()))
            .into(),
        ..Setup::default()
    };
    let mut b = Bench {
        sb: Sandbox::with(&setup),
        runs,
        stats: Vec::new(),
    };
    let (lead, dev) = (b.sb.path("lead"), b.sb.path("dev"));
    b.sb.init_repo("plain");
    let repo = discover(&lead)
        .map_err(|e| e.to_string())?
        .ok_or("측정용 레포가 활성 레포가 아닙니다")?;
    let head = b.sb.git_ok(&lead, &["rev-parse", "HEAD"]);

    // 영구히 자라는 파일이 lead의 Stop에 주는 영향을 재려고 넣어 둔다(12.4절): archive.jsonl 5,000줄, turns.jsonl 50,000줄
    std::fs::create_dir_all(repo.shared()).map_err(|e| e.to_string())?;
    let lines = |count: usize, line: &dyn Fn(usize) -> Value| -> String {
        (0..count).map(|n| format!("{}\n", line(n))).collect()
    };
    let archived = |n: usize| {
        json!({ "key": format!("ARC-{n}"), "role": "dev", "sha": head, "received_at": now(), "landed_at": now(),
        "rounds": 0, "dispatches": [{ "purpose": "build", "started_at": now(), "reason": null, "sub": false }], "direct": false })
    };
    let turn =
        |n: usize| json!({ "at": now() - n as i64, "sid": "sid-bench-lead", "kind": "human" });
    serde_json::from_value::<ArchiveLine>(archived(0))
        .map_err(|e| format!("측정용 archive.jsonl 줄이 형식과 다릅니다: {e}"))?;
    serde_json::from_value::<Turn>(turn(0))
        .map_err(|e| format!("측정용 turns.jsonl 줄이 형식과 다릅니다: {e}"))?;
    std::fs::write(repo.archive_path(), lines(5_000, &archived)).map_err(|e| e.to_string())?;
    std::fs::write(repo.turns_path(), lines(50_000, &turn)).map_err(|e| e.to_string())?;

    for who in ["lead", "dev"] {
        b.hook(
            who,
            &b.call(who, &Event::SessionStart { source: "startup" })?,
            None,
        )?;
    }
    let shell = |command: &'static str| {
        Event::PreToolUse(Tool::Shell {
            command,
            powershell: false,
        })
    };
    let inbox = lead.join(".fullops-squad/handovers/to_dev.md");
    let code = dev.join("src/app.py");
    let stop = Event::Stop { reason: None };

    b.hook_case("inactive", Some(15.0), "plain", &shell("ls -la"))?;
    b.hook_case("pre_bash", Some(30.0), "lead", &shell("ls -la"))?;
    b.hook_case(
        "pre_edit",
        Some(30.0),
        "dev",
        &Event::PreToolUse(Tool::Edit {
            path: &code,
            old: "a",
            new: "b",
        }),
    )?;
    b.hook_case(
        "pre_inbox_edit",
        Some(80.0),
        "lead",
        &Event::PreToolUse(Tool::Edit {
            path: &inbox,
            old: "a",
            new: "b",
        }),
    )?;
    b.hook_case(
        "pre_worker_start",
        Some(80.0),
        "lead",
        &shell("orca orchestration worker-start --spec x"),
    )?;
    b.hook_case(
        "pre_git_commit",
        Some(80.0),
        "dev",
        &shell("git commit -m x"),
    )?;
    b.hook_case("stop_lead_clean", Some(150.0), "lead", &stop)?;

    // 통합 대기 10건: 병합하지 않은 커밋 열 개의 완료 보고를 받은 상태
    let mut state = json!({ "v": 1, "runs": [open_run("run_bench")], "dispatches": [], "completions": [], "messages": [] });
    for n in 0..10 {
        b.sb.git_ok(
            &dev,
            &[
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                &format!("B-{n} 구현"),
            ],
        );
        let sha = b.sb.git_ok(&dev, &["rev-parse", "HEAD"]);
        let (id, message, key) = (format!("ctx_b{n}"), format!("msg_b{n}"), format!("B-{n}"));
        let push = |list: &str, value: Value, state: &mut Value| {
            if let Some(list) = state[list].as_array_mut() {
                list.push(value);
            }
        };
        push(
            "dispatches",
            dispatch(&id, &key, "run_bench", &dev, Some(&message)),
            &mut state,
        );
        push(
            "messages",
            json!({ "id": message, "delivery": null, "run": "run_bench", "type": "worker_done",
            "dispatch": id, "received_at": now(), "acked_at": null }),
            &mut state,
        );
        push(
            "completions",
            json!({ "id": message, "dispatch": id, "key": key, "role": "dev", "branch": "fullops/dev",
            "sha": sha, "sha_source": "body", "direct": false, "outcome": "succeeded", "received_at": now(),
            "hold": null, "released_hold": null, "landed_at": null, "drift": null }),
            &mut state,
        );
    }
    put_state(&repo, Some(state))?;
    b.hook_case("stop_lead_pending10", None, "lead", &stop)?;

    put_state(&repo, None)?;
    plan_row(&repo, "대기")?;
    b.hook_case("stop_lead_ready", None, "lead", &stop)?;

    plan_row(&repo, "보류(측정)")?;
    put_state(
        &repo,
        Some(json!({ "v": 1, "runs": [open_run("run_open")],
        "dispatches": [dispatch("ctx_open", "O-1", "run_open", &dev, None)] })),
    )?;
    b.hook_case("stop_lead_open_run", None, "lead", &stop)?;

    // dispatch가 묶이고 코드 커밋이 있는 worker 세션. Orca는 아직 정산되지 않았다고 답한다
    b.sb.orca_rule(json!({ "when": ["orchestration", "worker-show"],
        "stdout": { "ok": true, "result": orca_shapes::worker("run_open", "ctx_open", "task_ctx_open", "term_ctx_open", false, "live") } }));
    let preamble = orca_shapes::preamble("ctx_open", "task_ctx_open", "");
    b.hook(
        "dev",
        &b.call("dev", &Event::UserPromptSubmit { prompt: &preamble })?,
        None,
    )?;
    b.sb.write(&dev.join("src/w1.py"), "print(1)\n");
    b.sb.git_ok(&dev, &["add", "-A"]);
    b.sb.git_ok(&dev, &["commit", "-q", "-m", "W-1 구현"]);
    b.hook_case("stop_worker", None, "dev", &stop)?;

    b.hook_case(
        "session_start_lead",
        Some(250.0),
        "lead",
        &Event::SessionStart { source: "startup" },
    )?;
    b.hook_case(
        "session_start_worker",
        Some(250.0),
        "dev",
        &Event::SessionStart { source: "startup" },
    )?;
    b.cli_case("cli_rules", &["rules", "--json"])?;
    b.cli_case("cli_status", &["status", "--json"])?;

    let py = py_baseline(py_runs)?;
    let profile = if bin("fullops")
        .parent()
        .is_some_and(|p| p.ends_with("release"))
    {
        "release"
    } else {
        "debug"
    };
    println!(
        "측정 환경: {} {}, 논리 코어 {}개, 빌드 {profile}, fullops {}회씩(처음 {}회 버림), py {}.\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism().map_or(0, |n| n.get()),
        runs,
        (runs / 15).min(runs - 1),
        if py.is_some() {
            format!("{py_runs}회씩")
        } else {
            "건너뜀(FULLOPS_PY_BASELINE 없음)".to_string()
        },
    );
    println!(
        "| 입력 | 본 것 | p50 | p95 | 최대 | 목표 p95 | git | orca | 쓰기 | py 본 것 | py p50 | py p95 | py 프로세스 | py git | py orca |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    let mut over = Vec::new();
    for s in &b.stats {
        let old = py
            .as_ref()
            .map(|py| &py[s.name])
            .filter(|old| old.is_object());
        let cell = |field: &str, unit: &str| {
            old.map_or("-".to_string(), |old| format!("{}{unit}", old[field]))
        };
        println!(
            "| {} | {} | {:.1}ms | {:.1}ms | {:.1}ms | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            s.name,
            s.seen,
            s.p50,
            s.p95,
            s.max,
            s.target.map_or("-".to_string(), |t| format!("{t}ms")),
            s.git,
            s.orca,
            s.writes,
            old.and_then(|old| old["verdict"].as_str()).unwrap_or("-"),
            cell("p50", "ms"),
            cell("p95", "ms"),
            cell("procs", ""),
            cell("git", ""),
            cell("orca", ""),
        );
        if s.target.is_some_and(|t| s.p95 > t * 2.0) {
            over.push(s.name);
        }
    }
    if check && !over.is_empty() {
        return Err(format!(
            "p95가 목표의 두 배를 넘었습니다: {}",
            over.join(", ")
        ));
    }
    Ok(())
}

pub fn run(args: &[String]) -> Result<(), String> {
    if rerun_in_deps("bench", args)? {
        return Ok(());
    }
    let (mut runs, mut py_runs, mut check) = (300, 30, false);
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut number = || {
            rest.next()
                .and_then(|n| n.parse().ok())
                .filter(|n| *n > 0)
                .ok_or(format!("{arg}에 횟수가 필요합니다"))
        };
        match arg.as_str() {
            "--runs" => runs = number()?,
            "--py-runs" => py_runs = number()?,
            "--check" => check = true,
            other => return Err(format!("모르는 인자입니다: {other}")),
        }
    }
    run_bench(runs, py_runs, check)
}
