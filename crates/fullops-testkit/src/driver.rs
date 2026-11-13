//! `rs` 구동기(설계 12.1절). 빌드한 `fullops`를 hook(stdin JSON)과 CLI로 실제 프로세스로 돌린다.
//! 동사 하나는 에이전트가 하는 일 하나다. hook 입력의 모양은 호스트 계약(미정-M2)의 가정을 따른다.

use crate::orca_shapes;
use crate::sandbox::{Run, Sandbox, Setup};
use crate::scenario::{Expect, Scenario, Step};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::PathBuf;

/// 동사 하나의 설명. `fullops-testkit`의 동사 목록이 곧 시나리오 어휘다.
pub struct Verb {
    pub name: &'static str,
    /// 인자. `?`가 붙은 것은 생략할 수 있다. `who`의 기본값은 `lead`다.
    pub args: &'static str,
    /// `rs` 구동기가 하는 일.
    pub rs: &'static str,
}

pub static VERBS: &[Verb] = &[
    // hook 이벤트
    Verb {
        name: "session_starts",
        args: "who?, session?, source?",
        rs: "SessionStart hook. source 기본값은 startup",
    },
    Verb {
        name: "user_prompts",
        args: "who?, session?, text",
        rs: "UserPromptSubmit hook",
    },
    Verb {
        name: "preamble_arrives",
        args: "who, session?, dispatch?, task?",
        rs: "Orca preamble을 실은 UserPromptSubmit hook. ID 기본값은 그 역할의 마지막 배정",
    },
    Verb {
        name: "runs_shell",
        args: "who?, session?, command, dialect?",
        rs: "셸 도구의 PreToolUse hook. 명령은 실행하지 않는다. dialect = \"powershell\"이면 Claude의 PowerShell 도구",
    },
    Verb {
        name: "writes_file",
        args: "who?, session?, path, content",
        rs: "Write(Codex는 apply_patch Add File)의 PreToolUse hook. 막히지 않으면 파일을 쓴다",
    },
    Verb {
        name: "edits_file",
        args: "who?, session?, path, old, new",
        rs: "Edit(Codex는 apply_patch Update File)의 PreToolUse hook. 막히지 않으면 파일을 고친다",
    },
    Verb {
        name: "deletes_file",
        args: "who?, session?, path",
        rs: "빈 내용의 Write(Codex는 apply_patch Delete File)의 PreToolUse hook. 막히지 않으면 파일을 지운다",
    },
    Verb {
        name: "spawns_subagent",
        args: "who?, session?, prompt",
        rs: "하위 에이전트 도구(Agent, Codex는 spawn_agent)의 PreToolUse hook",
    },
    Verb {
        name: "stops",
        args: "who?, session?, reason?",
        rs: "Stop hook",
    },
    Verb {
        name: "turn_fails",
        args: "who?, session?, error?",
        rs: "StopFailure hook",
    },
    // CLI
    Verb {
        name: "runs_fullops",
        args: "who?, session?, args, raw?",
        rs: "에이전트가 `fullops <args>`를 실행한다: 셸 도구의 PreToolUse hook 뒤에 CLI. raw = true면 hook 없이 CLI만(사람이 터미널에서)",
    },
    // 준비(판정 없음)
    Verb {
        name: "file",
        args: "who?, path, content",
        rs: "hook 없이 파일을 쓴다",
    },
    Verb {
        name: "git",
        args: "who?, args",
        rs: "hook 없이 git을 실행한다. 실패하면 시나리오가 실패한다",
    },
    Verb {
        name: "commits",
        args: "who?, message?",
        rs: "hook 없이 `git add -A`와 `git commit`",
    },
    Verb {
        name: "orca_replies",
        args: "when, stdout?, times?, exit?, sleep_ms?",
        rs: "fake-orca 각본에 규칙을 덧붙인다",
    },
    // 흐름(의도 수준). 1.24.0의 대응은 py 구동기가 정한다
    Verb {
        name: "lead_routes",
        args: "key, role?, class?",
        rs: "`fullops route <KEY> --request … --class <class> --role <R>`와 tuple 기록. class 기본값은 direct",
    },
    Verb {
        name: "lead_plans",
        args: "key, role, goal?, source?, status?, after?",
        rs: "`fullops plan add <KEY> --role <R> --goal … --source …`",
    },
    Verb {
        name: "lead_writes_task",
        args: "key, role, goal?",
        rs: "`fullops task new --role <R> --key <KEY> --goal …`",
    },
    Verb {
        name: "lead_dispatches",
        args: "key, role, purpose?, reason?, sha?, terminal?, run?, bare?",
        rs: "`fullops dispatch <KEY> --role <R>`. bare가 아니면 아직 안 한 route와 task new를 먼저 한다. fake-orca가 run·dispatch·task·터미널 ID를 준다",
    },
    Verb {
        name: "worker_works",
        args: "role, key, path?, lines?",
        rs: "배정받은 세션을 시작하고(SessionStart, preamble) 코드 파일을 고쳐 커밋한다",
    },
    Verb {
        name: "worker_lints",
        args: "role",
        rs: "`fullops lint`",
    },
    Verb {
        name: "worker_finishes",
        args: "role, failed?, followup?, no_checks?",
        rs: "`fullops done`",
    },
    Verb {
        name: "worker_reports",
        args: "role, key",
        rs: "배정(아직이면) → worker_works → worker_lints → worker_finishes → fake-orca가 worker_done을 전달 → lead_waits",
    },
    Verb {
        name: "lead_waits",
        args: "run?, ack?",
        rs: "`fullops wait --run <R> [--ack <delivery>]…`",
    },
    Verb {
        name: "lead_merges",
        args: "key",
        rs: "`fullops merge <KEY>`",
    },
];

/// 단계 하나의 결과. 비교 항목은 판정 종류와 흐름이다(문구와 파일 바이트는 비교하지 않는다).
#[derive(Debug, Clone)]
pub struct StepResult {
    pub act: String,
    /// hook: `pass`·`context`·`warn`·`deny`·`block`. CLI: `pass`·`deny`·`human`·`external`·`error`.
    pub verdict: &'static str,
    /// 출력에 있는 `FullOps[<ID>]`의 ID들.
    pub rules: Vec<String>,
    pub exit: Option<i32>,
    pub ms: u128,
    /// stdout과 stderr.
    pub output: String,
}

impl StepResult {
    /// `continues`(통과·안내·경고) 또는 `stopped`.
    pub fn flow(&self) -> &'static str {
        if matches!(self.verdict, "pass" | "context" | "warn") {
            "continues"
        } else {
            "stopped"
        }
    }

    fn fixture(act: &str) -> Self {
        Self {
            act: act.into(),
            verdict: "pass",
            rules: Vec::new(),
            exit: None,
            ms: 0,
            output: String::new(),
        }
    }

    fn of(act: &str, run: &Run, verdict: &'static str) -> Self {
        let output = format!("{}{}", run.stdout, run.stderr);
        let rules = output
            .split("FullOps[")
            .skip(1)
            .filter_map(|s| s.split_once(']'))
            .map(|(id, _)| id.to_string());
        Self {
            act: act.into(),
            verdict,
            rules: rules.collect(),
            exit: run.code,
            ms: run.ms,
            output,
        }
    }

    fn hook(act: &str, run: &Run) -> Self {
        let out: Value = serde_json::from_str(&run.stdout).unwrap_or(Value::Null);
        let verdict = if run.code != Some(0) || !out.is_object() {
            "error"
        } else if out["hookSpecificOutput"]["permissionDecision"] == "deny" {
            "deny"
        } else if out["decision"] == "block" {
            "block"
        } else if out["systemMessage"].is_string() {
            "warn"
        } else if out["hookSpecificOutput"]["additionalContext"].is_string() {
            "context"
        } else {
            "pass"
        };
        Self::of(act, run, verdict)
    }

    fn cli(act: &str, run: &Run) -> Self {
        let verdict = match run.code {
            Some(0) => "pass",
            Some(2) => "deny",
            Some(3) => "human",
            Some(4) => "external",
            _ => "error",
        };
        Self::of(act, run, verdict)
    }
}

/// 구동기가 기억하는 배정 하나. ID는 fake-orca가 실제로 낸 응답에서 읽는다.
struct Dispatched {
    key: String,
    role: String,
    run: String,
    dispatch: String,
    task: String,
    terminal: String,
    session_started: bool,
}

pub struct Driver {
    pub sb: Sandbox,
    host: String,
    routed: BTreeSet<String>,
    tasked: BTreeSet<String>,
    dispatched: Vec<Dispatched>,
    run: Option<String>,
    seq: u32,
}

fn quote(arg: &str) -> String {
    if !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:".contains(c))
    {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

impl Driver {
    pub fn new(setup: &Setup) -> Self {
        Self {
            sb: Sandbox::with(setup),
            host: setup.host.clone(),
            routed: BTreeSet::new(),
            tasked: BTreeSet::new(),
            dispatched: Vec::new(),
            run: None,
            seq: 0,
        }
    }

    fn dir(&self, who: &str) -> PathBuf {
        self.sb.path(who)
    }

    /// 그 역할의 마지막 배정이 준 터미널 핸들. worker 프로세스의 `ORCA_TERMINAL_HANDLE`이 된다.
    fn env_of(&self, who: &str) -> Vec<(&str, &str)> {
        let terminal = self
            .dispatched
            .iter()
            .rev()
            .find(|d| d.role == who)
            .map(|d| d.terminal.as_str());
        terminal
            .map(|t| ("ORCA_TERMINAL_HANDLE", t))
            .into_iter()
            .collect()
    }

    fn hook(
        &self,
        act: &str,
        who: &str,
        session: Option<&str>,
        event: &str,
        extra: Value,
    ) -> StepResult {
        let sid = format!(
            "sid-{who}{}",
            session.map(|s| format!("-{s}")).unwrap_or_default()
        );
        let mut input =
            json!({ "hook_event_name": event, "session_id": sid, "cwd": self.dir(who) });
        if let (Some(input), Some(extra)) = (input.as_object_mut(), extra.as_object()) {
            input.extend(extra.clone());
        }
        let run = self.sb.fullops(
            &self.dir(who),
            &["hook", &self.host],
            &self.env_of(who),
            input.to_string().as_bytes(),
        );
        StepResult::hook(act, &run)
    }

    fn tool(
        &self,
        act: &str,
        who: &str,
        session: Option<&str>,
        tool: &str,
        input: Value,
    ) -> StepResult {
        self.hook(
            act,
            who,
            session,
            "PreToolUse",
            json!({ "tool_name": tool, "tool_input": input }),
        )
    }

    fn shell_tool(&self, dialect: Option<&str>) -> &'static str {
        match (self.host.as_str(), dialect) {
            ("codex", _) => "shell",
            (_, Some("powershell")) => "PowerShell",
            _ => "Bash",
        }
    }

    /// 에이전트가 `fullops`를 실행한다: 셸 도구의 PreToolUse hook(막히면 거기서 끝) 뒤에 CLI.
    fn agent_cli(
        &self,
        act: &str,
        who: &str,
        session: Option<&str>,
        args: &[String],
    ) -> StepResult {
        let command = format!(
            "fullops {}",
            args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ")
        );
        let gate = self.tool(
            act,
            who,
            session,
            self.shell_tool(None),
            json!({ "command": command }),
        );
        if gate.flow() == "stopped" {
            return gate;
        }
        let mut result = self.cli(act, who, args);
        result.ms += gate.ms;
        result
    }

    fn cli(&self, act: &str, who: &str, args: &[String]) -> StepResult {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        StepResult::cli(
            act,
            &self
                .sb
                .fullops(&self.dir(who), &args, &self.env_of(who), b""),
        )
    }

    /// 편집 도구의 hook을 부르고, 막히지 않으면 디스크에 반영한다. `after`가 `None`이면 삭제다.
    fn edit(&self, step: &Step, who: &str, session: Option<&str>) -> Result<StepResult, String> {
        let path = step.req("path")?;
        let file = self.dir(who).join(path);
        let before = std::fs::read_to_string(&file).unwrap_or_default();
        let (after, claude, patch) = match step.act.as_str() {
            "writes_file" => {
                let content = step.req("content")?;
                let added: String = content.lines().map(|l| format!("+{l}\n")).collect();
                (
                    Some(content.to_string()),
                    ("Write", json!({ "file_path": file, "content": content })),
                    format!("*** Add File: {path}\n{added}"),
                )
            }
            "edits_file" => {
                let (old, new) = (step.req("old")?, step.req("new")?);
                let hunk: String = old
                    .lines()
                    .map(|l| format!("-{l}\n"))
                    .chain(new.lines().map(|l| format!("+{l}\n")))
                    .collect();
                (
                    Some(before.replacen(old, new, 1)),
                    (
                        "Edit",
                        json!({ "file_path": file, "old_string": old, "new_string": new }),
                    ),
                    format!("*** Update File: {path}\n@@\n{hunk}"),
                )
            }
            _ => (
                None,
                ("Write", json!({ "file_path": file, "content": "" })),
                format!("*** Delete File: {path}\n"),
            ),
        };
        let result = if self.host == "codex" {
            let body = format!("*** Begin Patch\n{patch}*** End Patch\n");
            self.tool(
                &step.act,
                who,
                session,
                "apply_patch",
                json!({ "command": body }),
            )
        } else {
            self.tool(&step.act, who, session, claude.0, claude.1)
        };
        if result.flow() == "continues" {
            match after {
                Some(text) => self.sb.write(&file, &text),
                None => drop(std::fs::remove_file(&file)),
            }
        }
        Ok(result)
    }

    fn lead_routes(&mut self, act: &str, key: &str, role: Option<&str>, class: &str) -> StepResult {
        let request = format!("{key} 요청");
        let mut args = strings(&["route", key, "--request", &request, "--class", class]);
        if let Some(role) = role {
            args.extend(strings(&[
                "--role",
                role,
                "--agent",
                "claude",
                "--model",
                "test-model",
                "--effort",
                "medium",
            ]));
        }
        let result = self.agent_cli(act, "lead", None, &args);
        if result.flow() == "continues" {
            self.routed.insert(key.to_string());
        }
        result
    }

    fn lead_writes_task(
        &mut self,
        act: &str,
        key: &str,
        role: &str,
        goal: Option<&str>,
    ) -> StepResult {
        let goal = goal
            .map(str::to_string)
            .unwrap_or_else(|| format!("{key} 구현"));
        let result = self.agent_cli(
            act,
            "lead",
            None,
            &strings(&["task", "new", "--role", role, "--key", key, "--goal", &goal]),
        );
        if result.flow() == "continues" {
            self.tasked.insert(format!("{key}/{role}"));
        }
        result
    }

    fn lead_dispatches(&mut self, step: &Step, key: &str, role: &str) -> StepResult {
        let act = step.act.as_str();
        let purpose = step.str("purpose").unwrap_or("build");
        if step.args.get("bare").and_then(|v| v.as_bool()) != Some(true) {
            if !self.routed.contains(key) {
                let routed = self.lead_routes(act, key, Some(role), "direct");
                if routed.flow() == "stopped" {
                    return routed;
                }
            }
            if matches!(purpose, "build" | "fix") && !self.tasked.contains(&format!("{key}/{role}"))
            {
                let tasked = self.lead_writes_task(act, key, role, None);
                if tasked.flow() == "stopped" {
                    return tasked;
                }
            }
        }
        self.seq += 1;
        let n = self.seq;
        let run = step
            .str("run")
            .map(str::to_string)
            .or_else(|| self.run.clone())
            .unwrap_or_else(|| format!("run_{n}"));
        if self.run.is_none() && step.str("run").is_none() {
            self.sb.orca_rule(json!({ "when": ["orchestration", "run-create"], "times": 1, "stdout": orca_shapes::run_created(&run) }));
        }
        let started = orca_shapes::worker_started(
            &run,
            &format!("dsp_{n}"),
            &format!("task_{n}"),
            &format!("term_{n}"),
        );
        self.sb.orca_rule(
            json!({ "when": ["orchestration", "worker-start"], "times": 1, "stdout": started }),
        );

        let mut args = strings(&["dispatch", key, "--role", role, "--purpose", purpose]);
        for flag in [
            "reason", "sha", "terminal", "run", "agent", "model", "effort",
        ] {
            if let Some(value) = step.str(flag) {
                args.extend([format!("--{flag}"), value.to_string()]);
            }
        }
        let calls_before = self.sb.orca_calls().len();
        let result = self.agent_cli(act, "lead", None, &args);
        // fake-orca가 실제로 낸 응답에서 ID를 읽는다(쓰이지 않은 각본 규칙은 다음 배정이 쓴다)
        let reply = self
            .sb
            .orca_calls()
            .into_iter()
            .skip(calls_before)
            .find(|call| {
                call["argv"]
                    .as_array()
                    .is_some_and(|argv| argv.iter().any(|a| a == "worker-start"))
            });
        if let (Some(reply), "continues") = (reply, result.flow()) {
            let field = |name: &str| {
                reply["stdout"]["result"][name]
                    .as_str()
                    .unwrap_or_default()
                    .to_string()
            };
            self.run = Some(field("runId"));
            self.dispatched.push(Dispatched {
                key: key.to_string(),
                role: role.to_string(),
                run: field("runId"),
                dispatch: field("dispatchId"),
                task: field("taskId"),
                terminal: field("terminal"),
                session_started: false,
            });
        }
        result
    }

    fn worker_works(
        &mut self,
        role: &str,
        key: &str,
        path: Option<&str>,
        lines: usize,
    ) -> StepResult {
        let fresh = self
            .dispatched
            .iter_mut()
            .rev()
            .find(|d| d.role == role && !d.session_started)
            .map(|d| {
                d.session_started = true;
                orca_shapes::preamble(&d.dispatch, &d.task)
            });
        if let Some(prompt) = fresh {
            self.hook(
                "worker_works",
                role,
                None,
                "SessionStart",
                json!({ "source": "startup" }),
            );
            self.hook(
                "worker_works",
                role,
                None,
                "UserPromptSubmit",
                json!({ "prompt": prompt }),
            );
        }
        let path = path
            .map(str::to_string)
            .unwrap_or_else(|| format!("src/{}.py", key.to_lowercase()));
        let body: String = (0..lines).map(|i| format!("print({i})\n")).collect();
        self.sb.write(
            &self.dir(role).join(path),
            &format!("\"\"\"{key} 구현.\"\"\"\n{body}"),
        );
        self.sb.git_ok(&self.dir(role), &["add", "-A"]);
        self.sb.git_ok(
            &self.dir(role),
            &["commit", "-q", "-m", &format!("{key} 구현")],
        );
        StepResult::fixture("worker_works")
    }

    fn lead_waits(&self, act: &str, run: Option<&str>, acks: &[String]) -> StepResult {
        let mut args = strings(&["wait"]);
        if let Some(run) = run.or(self.run.as_deref()) {
            args.extend(strings(&["--run", run]));
        }
        for ack in acks {
            args.extend(strings(&["--ack", ack]));
        }
        self.agent_cli(act, "lead", None, &args)
    }

    fn worker_reports(&mut self, step: &Step, role: &str, key: &str) -> StepResult {
        if !self
            .dispatched
            .iter()
            .any(|d| d.key == key && d.role == role)
        {
            let dispatched = self.lead_dispatches(step, key, role);
            if dispatched.flow() == "stopped" {
                return dispatched;
            }
        }
        self.worker_works(role, key, None, 3);
        for args in [strings(&["lint"]), strings(&["done"])] {
            let result = self.agent_cli(&step.act, role, None, &args);
            if result.flow() == "stopped" {
                return result;
            }
            if args[0] == "done" {
                self.seq += 1;
                let Some(d) = self
                    .dispatched
                    .iter()
                    .rev()
                    .find(|d| d.key == key && d.role == role)
                else {
                    return result;
                };
                let (id, delivery) = (format!("msg_{}", self.seq), format!("dlv_{}", self.seq));
                let message = orca_shapes::message(
                    &id,
                    &delivery,
                    &d.run,
                    "worker_done",
                    &d.dispatch,
                    &d.task,
                    result.output.trim(),
                );
                self.sb.orca_rule(json!({ "when": ["orchestration", "check"], "times": 1, "stdout": orca_shapes::checked(vec![message]) }));
            }
        }
        self.lead_waits(&step.act, None, &[])
    }

    /// 단계 하나를 실행한다. `Err`는 시나리오 자체의 오류(모르는 동사, 빠진 인자)다.
    pub fn step(&mut self, step: &Step) -> Result<StepResult, String> {
        let act = step.act.as_str();
        let who = step.str("who").unwrap_or("lead");
        let session = step.str("session");
        Ok(match act {
            "session_starts" => self.hook(
                act,
                who,
                session,
                "SessionStart",
                json!({ "source": step.str("source").unwrap_or("startup") }),
            ),
            "user_prompts" => self.hook(
                act,
                who,
                session,
                "UserPromptSubmit",
                json!({ "prompt": step.req("text")? }),
            ),
            "preamble_arrives" => {
                let last = self.dispatched.iter().rev().find(|d| d.role == who);
                let dispatch = step
                    .str("dispatch")
                    .or(last.map(|d| d.dispatch.as_str()))
                    .ok_or("preamble_arrives에 dispatch가 필요합니다")?;
                let task = step
                    .str("task")
                    .or(last.map(|d| d.task.as_str()))
                    .unwrap_or("task_0");
                self.hook(
                    act,
                    who,
                    session,
                    "UserPromptSubmit",
                    json!({ "prompt": orca_shapes::preamble(dispatch, task) }),
                )
            }
            "runs_shell" => {
                let tool = self.shell_tool(step.str("dialect"));
                self.tool(
                    act,
                    who,
                    session,
                    tool,
                    json!({ "command": step.req("command")? }),
                )
            }
            "writes_file" | "edits_file" | "deletes_file" => self.edit(step, who, session)?,
            "spawns_subagent" => {
                let tool = if self.host == "codex" {
                    "spawn_agent"
                } else {
                    "Agent"
                };
                self.tool(
                    act,
                    who,
                    session,
                    tool,
                    json!({ "prompt": step.req("prompt")? }),
                )
            }
            "stops" => self.hook(
                act,
                who,
                session,
                "Stop",
                step.str("reason")
                    .map_or(json!({}), |r| json!({ "reason": r })),
            ),
            "turn_fails" => {
                let error = step
                    .str("error")
                    .unwrap_or("API Error: usage limit reached");
                self.hook(act, who, session, "StopFailure", json!({ "error": error }))
            }
            "runs_fullops" => {
                let args = step.list("args");
                if step.args.get("raw").and_then(|v| v.as_bool()) == Some(true) {
                    self.cli(act, who, &args)
                } else {
                    self.agent_cli(act, who, session, &args)
                }
            }
            "file" => {
                self.sb
                    .write(&self.dir(who).join(step.req("path")?), step.req("content")?);
                StepResult::fixture(act)
            }
            "git" => {
                let args = step.list("args");
                if args.is_empty() {
                    return Err("동사 git에 `args` 인자가 필요합니다".into());
                }
                self.sb.git_ok(
                    &self.dir(who),
                    &args.iter().map(String::as_str).collect::<Vec<_>>(),
                );
                StepResult::fixture(act)
            }
            "commits" => {
                self.sb.git_ok(&self.dir(who), &["add", "-A"]);
                let message = step.str("message").unwrap_or("시나리오 커밋");
                self.sb.git_ok(
                    &self.dir(who),
                    &["commit", "-q", "--allow-empty", "-m", message],
                );
                StepResult::fixture(act)
            }
            "orca_replies" => {
                let mut rule = serde_json::to_value(&step.args).map_err(|e| e.to_string())?;
                rule["when"] = json!(step.list("when"));
                self.sb.orca_rule(rule);
                StepResult::fixture(act)
            }
            "lead_routes" => self.lead_routes(
                act,
                step.req("key")?,
                step.str("role"),
                step.str("class").unwrap_or("direct"),
            ),
            "lead_plans" => {
                let key = step.req("key")?;
                let goal = step
                    .str("goal")
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{key} 구현"));
                let mut args = strings(&[
                    "plan",
                    "add",
                    key,
                    "--role",
                    step.req("role")?,
                    "--goal",
                    &goal,
                ]);
                args.extend(strings(&[
                    "--source",
                    step.str("source").unwrap_or("시나리오 지시"),
                ]));
                for flag in ["status", "after"] {
                    if let Some(value) = step.str(flag) {
                        args.extend([format!("--{flag}"), value.to_string()]);
                    }
                }
                self.agent_cli(act, "lead", None, &args)
            }
            "lead_writes_task" => {
                self.lead_writes_task(act, step.req("key")?, step.req("role")?, step.str("goal"))
            }
            "lead_dispatches" => self.lead_dispatches(step, step.req("key")?, step.req("role")?),
            "worker_works" => {
                let lines = step
                    .args
                    .get("lines")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(3) as usize;
                self.worker_works(step.req("role")?, step.req("key")?, step.str("path"), lines)
            }
            "worker_lints" => self.agent_cli(act, step.req("role")?, None, &strings(&["lint"])),
            "worker_finishes" => {
                let mut args = strings(&["done"]);
                for (name, flag) in [
                    ("failed", "--failed"),
                    ("followup", "--followup"),
                    ("no_checks", "--no-checks"),
                ] {
                    if let Some(value) = step.str(name) {
                        args.extend(strings(&[flag, value]));
                    }
                }
                self.agent_cli(act, step.req("role")?, None, &args)
            }
            "worker_reports" => self.worker_reports(step, step.req("role")?, step.req("key")?),
            "lead_waits" => self.lead_waits(act, step.str("run"), &step.list("ack")),
            "lead_merges" => {
                self.agent_cli(act, "lead", None, &strings(&["merge", step.req("key")?]))
            }
            other => return Err(format!("모르는 동사입니다: {other}")),
        })
    }
}

/// 기대와 다른 점을 글로 돌려준다. 비어 있으면 기대대로다.
pub fn mismatches(expect: &Expect, result: &StepResult) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(verdict) = &expect.verdict
        && verdict != result.verdict
    {
        out.push(format!("판정이 {verdict}가 아니라 {}", result.verdict));
    }
    if let Some(rule) = &expect.rule
        && !result.rules.contains(rule)
    {
        out.push(format!(
            "규칙 {rule}이 출력에 없음(있는 것: {:?})",
            result.rules
        ));
    }
    if let Some(flow) = &expect.flow
        && flow != result.flow()
    {
        out.push(format!("흐름이 {flow}가 아니라 {}", result.flow()));
    }
    if let Some(exit) = expect.exit
        && Some(exit) != result.exit
    {
        out.push(format!("종료 코드가 {exit}가 아니라 {:?}", result.exit));
    }
    if let Some(escape) = &expect.escape
        && !result.output.contains(escape.as_str())
    {
        out.push(format!("출력에 탈출구 `{escape}`가 없음"));
    }
    out
}

/// 시나리오를 끝까지 돌린다. 기대와 다르면 그 단계에서 멈추고 `Err`로 알린다.
pub fn run_scenario(scenario: &Scenario) -> Result<Vec<StepResult>, String> {
    let mut driver = Driver::new(&scenario.setup);
    let mut results = Vec::new();
    for (index, step) in scenario.steps.iter().enumerate() {
        let result = driver
            .step(step)
            .map_err(|e| format!("{} {}단계: {e}", scenario.name, index + 1))?;
        if let Some(expect) = &step.expect {
            let diff = mismatches(expect, &result);
            if !diff.is_empty() {
                return Err(format!(
                    "{} {}단계({}): {}\n출력: {}",
                    scenario.name,
                    index + 1,
                    step.act,
                    diff.join(", "),
                    result.output
                ));
            }
        }
        results.push(result);
    }
    Ok(results)
}
