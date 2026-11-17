//! `rs` 구동기(설계 12.1절). 빌드한 `fullops`를 hook(stdin JSON)과 CLI로 실제 프로세스로 돌린다.
//! 동사 하나는 에이전트가 하는 일 하나다. 호스트별 입력 모양과 출력 읽기는 `host_shapes`가 맡는다.
//! 동결 뒤 이 파일은 묶음 A 담당만 고친다. 새 동사가 필요하면 A 담당에게 요청한다.

use crate::host_shapes::{Event, Tool, hook_call, verdict_of};
use crate::orca_shapes;
use crate::sandbox::{Run, Sandbox, Setup};
use crate::scenario::{Expect, Scenario, Step};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// 동사 하나의 설명. `fullops-testkit`의 동사 목록이 곧 시나리오 어휘다.
pub struct Verb {
    pub name: &'static str,
    /// 인자. `?`가 붙은 것은 생략할 수 있다. `who`의 기본값은 `lead`다.
    /// hook과 CLI를 부르는 동사는 모두 `env?`(환경 변수 표)를 더 받는다.
    /// hook을 부르는 동사는 `hook_args?`(등록 문자열에 더 붙는 인자. 예: `["--plugin-root", "../pkg"]`)도 받는다.
    pub args: &'static str,
    /// `rs` 구동기가 하는 일.
    pub rs: &'static str,
}

const fn verb(name: &'static str, args: &'static str, rs: &'static str) -> Verb {
    Verb { name, args, rs }
}

#[rustfmt::skip]
pub static VERBS: &[Verb] = &[
    // hook 이벤트
    verb("session_starts", "who?, session?, source?", "SessionStart hook. source 기본값은 startup(agy는 source가 없다)"),
    verb("user_prompts", "who?, session?, text", "UserPromptSubmit hook. agy는 이벤트가 없어 다음 hook 입력의 lastUserInput으로만 싣는다"),
    verb("preamble_arrives", "who, session?, dispatch?, task?", "Orca preamble을 실은 사용자 입력. ID 기본값은 그 역할의 마지막 배정"),
    verb("runs_shell", "who?, session?, command, dialect?", "셸 도구의 PreToolUse hook. 명령은 실행하지 않는다. dialect = \"powershell\"이면 Claude Code의 PowerShell 도구"),
    verb("writes_file", "who?, session?, path, content", "쓰기 도구(Codex는 apply_patch Add File)의 PreToolUse hook. 막히지 않으면 파일을 쓴다"),
    verb("edits_file", "who?, session?, path, old, new", "편집 도구(Codex는 apply_patch Update File)의 PreToolUse hook. 막히지 않으면 파일을 고친다"),
    verb("deletes_file", "who?, session?, path", "빈 내용의 쓰기(Codex는 apply_patch Delete File)의 PreToolUse hook. 막히지 않으면 파일을 지운다"),
    verb("spawns_subagent", "who?, session?, prompt", "하위 에이전트 도구(Agent·spawn_agent·spawn_subagent·invoke_subagent)의 PreToolUse hook"),
    verb("runs_workflow", "who?, session?, script?", "Workflow 도구의 PreToolUse hook. Claude Code에만 있다"),
    verb("stops", "who?, session?, reason?", "Stop hook. reason은 호스트의 종료 이유 값(grok·agy)"),
    verb("turn_fails", "who?, session?, error?", "StopFailure hook(agy는 오류로 끝난 Stop). Codex에는 없다"),
    // CLI
    verb("runs_fullops", "who?, session?, args, raw?", "에이전트가 `fullops <args>`를 실행한다: 셸 도구의 PreToolUse hook 뒤에 CLI. raw = true면 hook 없이 CLI만(사람이 터미널에서)"),
    // 준비(판정 없음)
    verb("file", "who?, path, content", "hook 없이 파일을 쓴다"),
    verb("git", "who?, args", "hook 없이 git을 실행한다. 실패하면 시나리오가 실패한다"),
    verb("commits", "who?, message?", "hook 없이 `git add -A`와 `git commit`"),
    verb("orca_replies", "when, stdout?, times?, exit?, sleep_ms?, stderr?", "fake-orca 각본에 규칙을 덧붙인다"),
    verb("orca_sends", "run?, id?, type?, dispatch?, task?, subject?, body?, outcome?", "fake-orca의 우편함에 메시지를 넣는다. run 기본값은 lead의 Run, type 기본값은 worker_done"),
    verb("host_replies", "when, stdout?, times?, exit?, sleep_ms?, stderr?", "fake-host 각본에 규칙을 덧붙인다. when의 첫 낱말은 실행 파일 이름"),
    verb("curl_replies", "when, stdout?, times?, exit?, sleep_ms?, stderr?", "fake-curl 각본에 규칙을 덧붙인다"),
    verb("fakes", "names", "가짜 실행 파일(claude·codex·grok·agy·npx·curl)을 PATH 앞에 놓는다"),
    // 흐름(의도 수준). 아직 하지 않은 앞 단계를 스스로 먼저 한다. 1.24.0의 대응은 py 구동기가 정한다
    verb("lead_routes", "key, role?, class?", "`fullops route <KEY> --request … --class <class> --role <R>`와 tuple 기록. class 기본값은 direct"),
    verb("lead_plans", "key, role, goal?, source?, status?, after?", "`fullops plan add <KEY> --role <R> --goal … --source …`"),
    verb("lead_writes_task", "key, role, goal?", "`fullops task new --role <R> --key <KEY> --goal …`"),
    verb("lead_dispatches", "key, role, purpose?, reason?, sha?, terminal?, run?, agent?, model?, effort?, bare?, chat?", "`fullops dispatch <KEY> --role <R>`. bare가 아니면 아직 안 한 route와 task new를 먼저 한다. fake-orca가 run·dispatch·task·터미널 ID를 준다(chat = true면 터미널 없음)"),
    verb("worker_subdispatches", "role, key, brief, agent?, model?, effort?", "worker가 `fullops dispatch <KEY> --brief …`로 같은 워크트리에 하위 worker를 띄운다"),
    verb("worker_works", "role, key, path?, lines?", "배정받은 세션을 시작하고(SessionStart, preamble) 코드 파일을 고쳐 커밋한다"),
    verb("worker_lints", "role", "`fullops lint`"),
    verb("worker_finishes", "role, failed?, followup?, no_checks?", "`fullops done`"),
    verb("worker_reports", "role, key", "배정(아직이면) → worker_works → 인박스의 완료 보고 절 채우기 → worker_lints → worker_finishes → fake-orca의 우편함에 worker_done → lead_waits. env는 lead 쪽 호출(배정과 wait)에 준다"),
    verb("lead_waits", "run?, ack?, no_wait?", "`fullops wait --run <R> [--ack <메시지 ID>]… [--no-wait]`"),
    verb("lead_merges", "key", "`fullops merge <KEY>`"),
];

/// 단계 하나의 결과. 비교 항목은 판정 종류와 흐름이다(문구와 파일 바이트는 비교하지 않는다).
#[derive(Debug, Clone)]
pub struct StepResult {
    pub act: String,
    /// hook: `pass`·`context`·`warn`·`deny`·`block`. CLI: `pass`·`deny`·`human`·`external`·`error`. 건너뛴 단계: `pending`.
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
        if matches!(self.verdict, "pass" | "context" | "warn" | "pending") {
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

    fn hook(act: &str, host: &str, run: &Run) -> Self {
        let verdict = if run.code == Some(0) {
            verdict_of(host, &run.stdout)
        } else {
            "error"
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
    /// 배정받은 역할(하위 배정은 부모의 역할).
    role: String,
    run: String,
    dispatch: String,
    task: String,
    terminal: Option<String>,
    spec: String,
    session_started: bool,
    sub: bool,
}

pub struct Driver {
    pub sb: Sandbox,
    host: String,
    routed: BTreeSet<String>,
    tasked: BTreeSet<String>,
    dispatched: Vec<Dispatched>,
    /// 배정한 쪽(`lead` 또는 역할 ID) → 그쪽이 소유한 Run.
    runs: BTreeMap<String, String>,
    /// 세션 ID → 마지막 사용자 입력. agy가 모든 이벤트에 싣는다.
    prompts: BTreeMap<String, String>,
    /// 지금 단계의 `hook_args`.
    hook_args: Vec<String>,
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

fn sid_of(who: &str, session: Option<&str>) -> String {
    format!(
        "sid-{who}{}",
        session.map(|s| format!("-{s}")).unwrap_or_default()
    )
}

/// 단계의 `env` 표.
fn env_of_step(step: &Step) -> Vec<(String, String)> {
    step.args
        .get("env")
        .and_then(|v| v.as_table())
        .map(|table| {
            table
                .iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

impl Driver {
    pub fn new(setup: &Setup) -> Self {
        Self {
            sb: Sandbox::with(setup),
            host: setup.host.clone(),
            routed: BTreeSet::new(),
            tasked: BTreeSet::new(),
            dispatched: Vec::new(),
            runs: BTreeMap::new(),
            prompts: BTreeMap::new(),
            hook_args: Vec::new(),
            seq: 0,
        }
    }

    fn dir(&self, who: &str) -> PathBuf {
        self.sb.path(who)
    }

    /// 그 역할의 마지막 배정이 준 터미널 핸들(worker 프로세스의 `ORCA_TERMINAL_HANDLE`)과 단계의 `env`.
    fn env(&self, who: &str, extra: &[(String, String)]) -> Vec<(String, String)> {
        let terminal = self
            .dispatched
            .iter()
            .rev()
            .find(|d| d.role == who && !d.sub)
            .and_then(|d| d.terminal.clone());
        terminal
            .map(|t| ("ORCA_TERMINAL_HANDLE".to_string(), t))
            .into_iter()
            .chain(extra.iter().cloned())
            .collect()
    }

    fn fullops(&self, who: &str, args: &[String], env: &[(String, String)], stdin: &[u8]) -> Run {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let env = self.env(who, env);
        let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        self.sb.fullops(&self.dir(who), &args, &env, stdin)
    }

    fn hook(
        &self,
        act: &str,
        who: &str,
        session: Option<&str>,
        event: &Event,
        env: &[(String, String)],
    ) -> StepResult {
        let sid = sid_of(who, session);
        let last_prompt = self.prompts.get(&sid).map(String::as_str);
        let Some(call) = hook_call(&self.host, &sid, &self.dir(who), last_prompt, event) else {
            // 이 호스트에는 이 이벤트(또는 도구)가 없다. hook이 불리지 않으므로 아무것도 막지 않는다
            return StepResult::fixture(act);
        };
        let args: Vec<String> = std::iter::once("hook".to_string())
            .chain(call.args)
            .chain(self.hook_args.iter().cloned())
            .collect();
        let run = self.fullops(who, &args, env, call.input.to_string().as_bytes());
        StepResult::hook(act, &self.host, &run)
    }

    /// 사용자 입력 하나. agy는 프롬프트 이벤트가 없어 다음 hook 호출의 `lastUserInput`에 실린다.
    fn prompt(
        &mut self,
        act: &str,
        who: &str,
        session: Option<&str>,
        text: &str,
        env: &[(String, String)],
    ) -> StepResult {
        self.prompts.insert(sid_of(who, session), text.to_string());
        self.hook(
            act,
            who,
            session,
            &Event::UserPromptSubmit { prompt: text },
            env,
        )
    }

    /// 에이전트가 `fullops`를 실행한다: 셸 도구의 PreToolUse hook(막히면 거기서 끝) 뒤에 CLI.
    fn agent_cli(
        &self,
        act: &str,
        who: &str,
        session: Option<&str>,
        args: &[String],
        env: &[(String, String)],
    ) -> StepResult {
        let command = format!(
            "fullops {}",
            args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" ")
        );
        let shell = Tool::Shell {
            command: &command,
            powershell: false,
        };
        let gate = self.hook(act, who, session, &Event::PreToolUse(shell), env);
        if gate.flow() == "stopped" {
            return gate;
        }
        let mut result = self.cli(act, who, args, env);
        result.ms += gate.ms;
        result
    }

    fn cli(&self, act: &str, who: &str, args: &[String], env: &[(String, String)]) -> StepResult {
        StepResult::cli(act, &self.fullops(who, args, env, b""))
    }

    /// 편집 도구의 hook을 부르고, 막히지 않으면 디스크에 반영한다. `after`가 `None`이면 삭제다.
    fn edit(
        &self,
        step: &Step,
        who: &str,
        session: Option<&str>,
        env: &[(String, String)],
    ) -> Result<StepResult, String> {
        let path = step.req("path")?;
        let file = self.dir(who).join(path);
        let before = std::fs::read_to_string(&file).unwrap_or_default();
        let (after, tool, patch) = match step.act.as_str() {
            "writes_file" => {
                let content = step.req("content")?;
                let added: String = content.lines().map(|l| format!("+{l}\n")).collect();
                (
                    Some(content.to_string()),
                    Tool::Write {
                        path: &file,
                        content,
                    },
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
                    Tool::Edit {
                        path: &file,
                        old,
                        new,
                    },
                    format!("*** Update File: {path}\n@@\n{hunk}"),
                )
            }
            _ => (
                None,
                Tool::Write {
                    path: &file,
                    content: "",
                },
                format!("*** Delete File: {path}\n"),
            ),
        };
        let tool = if self.host == "codex" {
            Tool::Raw {
                name: "apply_patch",
                input: json!({ "command": format!("*** Begin Patch\n{patch}*** End Patch\n") }),
            }
        } else {
            tool
        };
        let result = self.hook(&step.act, who, session, &Event::PreToolUse(tool), env);
        if result.flow() == "continues" {
            match after {
                Some(text) => self.sb.write(&file, &text),
                None => drop(std::fs::remove_file(&file)),
            }
        }
        Ok(result)
    }

    fn lead_routes(
        &mut self,
        act: &str,
        key: &str,
        role: Option<&str>,
        class: &str,
        env: &[(String, String)],
    ) -> StepResult {
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
        let result = self.agent_cli(act, "lead", None, &args, env);
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
        env: &[(String, String)],
    ) -> StepResult {
        let goal = goal
            .map(str::to_string)
            .unwrap_or_else(|| format!("{key} 구현"));
        let result = self.agent_cli(
            act,
            "lead",
            None,
            &strings(&["task", "new", "--role", role, "--key", key, "--goal", &goal]),
            env,
        );
        if result.flow() == "continues" {
            self.tasked.insert(format!("{key}/{role}"));
        }
        result
    }

    /// fake-orca에 응답을 걸고 `who`가 `fullops dispatch <args>`를 실행한다. 성공하면 fake-orca가 실제로 낸 ID를 기억한다.
    fn dispatch(
        &mut self,
        act: &str,
        who: &str,
        key: &str,
        role: &str,
        args: Vec<String>,
        run: Option<&str>,
        chat: bool,
        env: &[(String, String)],
    ) -> StepResult {
        self.seq += 1;
        let n = self.seq;
        // 기억한 Run이 닫혔으면(Stop의 X-05) 다음 배정이 새 Run을 만든다
        let state: Value =
            std::fs::read_to_string(self.dir("lead").join(".git/fullops/state.json"))
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or_default();
        let open = |run: &String| {
            state["runs"].as_array().is_some_and(|runs| {
                runs.iter()
                    .any(|r| r["id"] == **run && r["closed_at"].is_null())
            })
        };
        let known = run
            .map(str::to_string)
            .or_else(|| self.runs.get(who).filter(|run| open(run)).cloned());
        let run = known.clone().unwrap_or_else(|| format!("run_{n}"));
        if known.is_none() {
            self.sb.orca_rule(json!({ "when": ["orchestration", "run-create"], "times": 1, "stdout": orca_shapes::run_created(&run) }));
        }
        let terminal = format!("term_{n}");
        let started = orca_shapes::worker_started(
            &run,
            &format!("ctx_{n}"),
            &format!("task_{n}"),
            (!chat).then_some(terminal.as_str()),
        );
        self.sb.orca_rule(
            json!({ "when": ["orchestration", "worker-start"], "times": 1, "stdout": started }),
        );

        let calls_before = self.sb.orca_calls().len();
        let result = self.agent_cli(act, who, None, &args, env);
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
            let field = |name: &str| reply["stdout"]["result"][name].as_str().map(str::to_string);
            let run = field("runId").unwrap_or_default();
            self.runs.insert(who.to_string(), run.clone());
            self.dispatched.push(Dispatched {
                key: key.to_string(),
                role: role.to_string(),
                run,
                dispatch: field("dispatchId").unwrap_or_default(),
                task: field("taskId").unwrap_or_default(),
                terminal: field("terminal"),
                spec: reply["stdin"].as_str().unwrap_or_default().to_string(),
                session_started: false,
                sub: who != "lead",
            });
        }
        result
    }

    fn lead_dispatches(&mut self, step: &Step, key: &str, role: &str) -> StepResult {
        let act = step.act.as_str();
        let env = env_of_step(step);
        let purpose = step.str("purpose").unwrap_or("build");
        let flag = |name: &str| step.args.get(name).and_then(|v| v.as_bool()) == Some(true);
        if !flag("bare") {
            if !self.routed.contains(key) {
                let routed = self.lead_routes(act, key, Some(role), "direct", &env);
                if routed.flow() == "stopped" {
                    return routed;
                }
            }
            if matches!(purpose, "build" | "fix") && !self.tasked.contains(&format!("{key}/{role}"))
            {
                let tasked = self.lead_writes_task(act, key, role, None, &env);
                if tasked.flow() == "stopped" {
                    return tasked;
                }
            }
        }
        let mut args = strings(&["dispatch", key, "--role", role, "--purpose", purpose]);
        for name in [
            "reason", "sha", "terminal", "run", "agent", "model", "effort",
        ] {
            if let Some(value) = step.str(name) {
                args.extend([format!("--{name}"), value.to_string()]);
            }
        }
        self.dispatch(
            act,
            "lead",
            key,
            role,
            args,
            step.str("run"),
            flag("chat"),
            &env,
        )
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
            .find(|d| d.role == role && !d.sub && !d.session_started)
            .map(|d| {
                d.session_started = true;
                orca_shapes::preamble(&d.dispatch, &d.task, &d.spec)
            });
        if let Some(prompt) = fresh {
            self.hook(
                "worker_works",
                role,
                None,
                &Event::SessionStart { source: "startup" },
                &[],
            );
            self.prompt("worker_works", role, None, &prompt, &[]);
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

    fn lead_waits(
        &self,
        act: &str,
        run: Option<&str>,
        acks: &[String],
        no_wait: bool,
        env: &[(String, String)],
    ) -> StepResult {
        let mut args = strings(&["wait"]);
        if let Some(run) = run.or(self.runs.get("lead").map(String::as_str)) {
            args.extend(strings(&["--run", run]));
        }
        for ack in acks {
            args.extend(strings(&["--ack", ack]));
        }
        if no_wait {
            args.push("--no-wait".into());
        }
        self.agent_cli(act, "lead", None, &args, env)
    }

    fn worker_reports(&mut self, step: &Step, role: &str, key: &str) -> StepResult {
        if !self
            .dispatched
            .iter()
            .any(|d| d.key == key && d.role == role && !d.sub)
        {
            let dispatched = self.lead_dispatches(step, key, role);
            if dispatched.flow() == "stopped" {
                return dispatched;
            }
        }
        self.worker_works(role, key, None, 3);
        // 완료 보고 절이 템플릿 그대로면 done이 거절한다(R-04). worker가 채워 커밋한다
        let inbox = self.dir(role).join(format!(".fullops/inbox/to_{role}.md"));
        if let Ok(text) = std::fs::read_to_string(&inbox) {
            let filled = text.replacen(
                "## 완료 보고\n",
                &format!("## 완료 보고\n\n{key} 구현과 확인을 마쳤다.\n"),
                1,
            );
            self.sb.write(&inbox, &filled);
            self.sb.git_ok(&self.dir(role), &["add", "-A"]);
            self.sb.git_ok(
                &self.dir(role),
                &["commit", "-q", "-m", &format!("{key} 완료 보고")],
            );
        }
        for args in [strings(&["lint"]), strings(&["done"])] {
            let result = self.agent_cli(&step.act, role, None, &args, &[]);
            if result.flow() == "stopped" {
                return result;
            }
            if args[0] == "done" {
                self.seq += 1;
                let Some(d) = self
                    .dispatched
                    .iter()
                    .rev()
                    .find(|d| d.key == key && d.role == role && !d.sub)
                else {
                    return result;
                };
                let message = orca_shapes::message(
                    &format!("msg_{}", self.seq),
                    "worker_done",
                    &d.dispatch,
                    &d.task,
                    &format!("{key} 완료"),
                    result.output.trim(),
                    Some("succeeded"),
                );
                self.sb.orca_mail(&d.run, message);
            }
        }
        self.lead_waits(&step.act, None, &[], false, &env_of_step(step))
    }

    /// 단계 하나를 실행한다. `Err`는 시나리오 자체의 오류(모르는 동사, 빠진 인자)다.
    pub fn step(&mut self, step: &Step) -> Result<StepResult, String> {
        let act = step.act.as_str();
        let who = step.str("who").unwrap_or("lead");
        let session = step.str("session");
        let env = env_of_step(step);
        self.hook_args = step.list("hook_args");
        let flag = |name: &str| step.args.get(name).and_then(|v| v.as_bool()) == Some(true);
        // 각본 규칙: 단계의 인자를 그대로 규칙으로 쓴다
        let rule = || -> Result<Value, String> {
            let mut rule = serde_json::to_value(&step.args).map_err(|e| e.to_string())?;
            rule["when"] = json!(step.list("when"));
            Ok(rule)
        };
        Ok(match act {
            "session_starts" => {
                let source = step.str("source").unwrap_or("startup");
                self.hook(act, who, session, &Event::SessionStart { source }, &env)
            }
            "user_prompts" => self.prompt(act, who, session, step.req("text")?, &env),
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
                let prompt =
                    orca_shapes::preamble(dispatch, task, last.map_or("", |d| d.spec.as_str()));
                self.prompt(act, who, session, &prompt, &env)
            }
            "runs_shell" => {
                let shell = Tool::Shell {
                    command: step.req("command")?,
                    powershell: step.str("dialect") == Some("powershell"),
                };
                self.hook(act, who, session, &Event::PreToolUse(shell), &env)
            }
            "writes_file" | "edits_file" | "deletes_file" => self.edit(step, who, session, &env)?,
            "spawns_subagent" => {
                let tool = Tool::SubAgent {
                    prompt: step.req("prompt")?,
                };
                self.hook(act, who, session, &Event::PreToolUse(tool), &env)
            }
            "runs_workflow" => {
                let tool = Tool::Workflow {
                    script: step.str("script").unwrap_or("시나리오 워크플로"),
                };
                self.hook(act, who, session, &Event::PreToolUse(tool), &env)
            }
            "stops" => {
                let reason = step.str("reason");
                self.hook(act, who, session, &Event::Stop { reason }, &env)
            }
            "turn_fails" => {
                let error = step
                    .str("error")
                    .unwrap_or("API Error: usage limit reached");
                self.hook(act, who, session, &Event::StopFailure { error }, &env)
            }
            "runs_fullops" => {
                let args = step.list("args");
                if flag("raw") {
                    self.cli(act, who, &args, &env)
                } else {
                    self.agent_cli(act, who, session, &args, &env)
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
                self.sb.orca_rule(rule()?);
                StepResult::fixture(act)
            }
            "orca_sends" => {
                self.seq += 1;
                let last = self.dispatched.last();
                let text =
                    |name: &str, default: &str| step.str(name).unwrap_or(default).to_string();
                let run = step
                    .str("run")
                    .or(self.runs.get("lead").map(String::as_str))
                    .ok_or("orca_sends에 run이 필요합니다")?;
                let kind = text("type", "worker_done");
                let outcome = step
                    .str("outcome")
                    .or((kind == "worker_done").then_some("succeeded"));
                let message = orca_shapes::message(
                    &text("id", &format!("msg_{}", self.seq)),
                    &kind,
                    &text("dispatch", last.map_or("ctx_0", |d| d.dispatch.as_str())),
                    &text("task", last.map_or("task_0", |d| d.task.as_str())),
                    &text("subject", "시나리오 메시지"),
                    &text("body", ""),
                    outcome,
                );
                self.sb.orca_mail(run, message);
                StepResult::fixture(act)
            }
            "host_replies" => {
                self.sb.host_rule(rule()?);
                StepResult::fixture(act)
            }
            "curl_replies" => {
                self.sb.curl_rule(rule()?);
                StepResult::fixture(act)
            }
            "fakes" => {
                let names = step.list("names");
                self.sb
                    .install_fakes(&names.iter().map(String::as_str).collect::<Vec<_>>());
                StepResult::fixture(act)
            }
            "lead_routes" => self.lead_routes(
                act,
                step.req("key")?,
                step.str("role"),
                step.str("class").unwrap_or("direct"),
                &env,
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
                for name in ["status", "after"] {
                    if let Some(value) = step.str(name) {
                        args.extend([format!("--{name}"), value.to_string()]);
                    }
                }
                self.agent_cli(act, "lead", None, &args, &env)
            }
            "lead_writes_task" => self.lead_writes_task(
                act,
                step.req("key")?,
                step.req("role")?,
                step.str("goal"),
                &env,
            ),
            "lead_dispatches" => self.lead_dispatches(step, step.req("key")?, step.req("role")?),
            "worker_subdispatches" => {
                let (role, key) = (step.req("role")?, step.req("key")?);
                let mut args = strings(&["dispatch", key, "--brief", step.req("brief")?]);
                for name in ["agent", "model", "effort"] {
                    if let Some(value) = step.str(name) {
                        args.extend([format!("--{name}"), value.to_string()]);
                    }
                }
                self.dispatch(act, role, key, role, args, None, false, &env)
            }
            "worker_works" => {
                let lines = step
                    .args
                    .get("lines")
                    .and_then(|v| v.as_integer())
                    .unwrap_or(3) as usize;
                self.worker_works(step.req("role")?, step.req("key")?, step.str("path"), lines)
            }
            "worker_lints" => {
                self.agent_cli(act, step.req("role")?, None, &strings(&["lint"]), &env)
            }
            "worker_finishes" => {
                let mut args = strings(&["done"]);
                for (name, option) in [
                    ("failed", "--failed"),
                    ("followup", "--followup"),
                    ("no_checks", "--no-checks"),
                ] {
                    if let Some(value) = step.str(name) {
                        args.extend(strings(&[option, value]));
                    }
                }
                self.agent_cli(act, step.req("role")?, None, &args, &env)
            }
            "worker_reports" => self.worker_reports(step, step.req("role")?, step.req("key")?),
            "lead_waits" => self.lead_waits(
                act,
                step.str("run"),
                &step.list("ack"),
                flag("no_wait"),
                &env,
            ),
            "lead_merges" => self.agent_cli(
                act,
                "lead",
                None,
                &strings(&["merge", step.req("key")?]),
                &env,
            ),
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
/// `pending`이 적힌 단계는 실행하지 않고 판정 `pending`("보류")으로 적는다.
pub fn run_scenario(scenario: &Scenario) -> Result<Vec<StepResult>, String> {
    let mut driver = Driver::new(&scenario.setup);
    let mut results = Vec::new();
    for (index, step) in scenario.steps.iter().enumerate() {
        if let Some(bundle) = &step.pending {
            let mut skipped = StepResult::fixture(&step.act);
            skipped.verdict = "pending";
            skipped.output = format!("보류: 묶음 {bundle}");
            results.push(skipped);
            continue;
        }
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
