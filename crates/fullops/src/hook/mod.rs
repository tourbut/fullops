//! hook 진입(설계 7.1절). 종료 코드는 항상 0이고 판정은 stdout JSON으로만 낸다. 죽거나 시간이 넘으면 통과가 된다.
//! `fullops hook`은 프로세스의 작업 디렉터리를 쓰지 않는다. 레포를 찾는 기준은 입력의 `cwd`다.

pub mod changes;
mod effects;
mod facts_real;
mod host_agy;
mod host_claude;
mod host_codex;
mod host_grok;

use fullops_core::messages;
use fullops_core::model::{
    Caps, Dialect, EditOp, EventKind, HookInput, HostId, Intent, SessionId, ToolCall, ToolKind,
    TraceLine, Verdict, caps_of,
};
use fullops_core::rules::{Outcome, Scene, decide_in, start_skill};
use fullops_store::Repo;
use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// stdin을 읽는 한도.
const STDIN_LIMIT: u64 = 8 * 1024 * 1024;

/// 모르는 호스트 인자와 grok 겹침 막기에서 내는 글. Claude Code·Codex·grok의 통과 글이기도 하다.
pub const EMPTY: &str = "{}";

/// 입력을 해석하지 못한 사유. 추적에만 남는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError(pub &'static str);

/// 호스트 어댑터(설계 7.2절). 구현은 넷이다: `host_claude`, `host_codex`, `host_grok`, `host_agy`.
/// 호스트 계약(미정-M2·M10·M11)이 바뀌면 그 호스트의 구현 파일만 고친다.
pub trait Host {
    fn id(&self) -> HostId;
    /// stdin의 JSON을 `HookInput`으로 옮긴다(7.2절의 입력 대응·도구 대응 표). `event_arg`는 `--event`의 값이다.
    fn decode(&self, raw: &[u8], event_arg: Option<&str>) -> Result<HookInput, DecodeError>;
    /// 판정을 그 호스트의 출력 글로 옮긴다(7.2절의 판정 출력 표).
    fn encode(&self, event: EventKind, v: &Verdict) -> String;
    /// 통과의 글(7.1절). 호스트 인자와 `--event`만으로 정해지고 stdin과 무관하다.
    fn pass(&self, event_arg: Option<&str>) -> &'static str;
    fn caps(&self) -> Caps {
        caps_of(self.id())
    }
}

/// 호스트 인자로 어댑터를 고른다. 인자만 본다(입력 모양으로 가리지 않는다). 그 밖의 값은 `None`이다.
pub fn host_for(arg: &str) -> Option<Box<dyn Host>> {
    Some(match HostId::parse(arg)? {
        HostId::Claude => Box::new(host_claude::Claude),
        HostId::Codex => Box::new(host_codex::Codex),
        HostId::Grok => Box::new(host_grok::Grok),
        HostId::Agy => Box::new(host_agy::Agy),
    })
}

// ── 어댑터가 함께 쓰는 것 ─────────────────────────────────────

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_string)
}

/// 오류 글의 앞 300자.
fn clip(text: String) -> String {
    text.chars().take(300).collect()
}

/// 방언을 도구 이름으로 알 수 없는 호스트는 OS로 정한다(Windows는 PowerShell. 미정-M2·M10·M11).
fn os_dialect() -> Dialect {
    if cfg!(windows) {
        Dialect::PowerShell
    } else {
        Dialect::Bash
    }
}

fn tool(name: &str, kind: ToolKind) -> ToolCall {
    ToolCall {
        name: name.to_string(),
        kind,
        command: None,
        file_path: None,
        content: None,
        edits: Vec::new(),
        text: None,
        truncated: false,
    }
}

/// 편집 한 곳. 두 글 가운데 하나라도 없으면 `None`이다(바뀐 뒤 글은 모름이 된다).
fn edit_op(args: &Value, old: &str, new: &str) -> Option<EditOp> {
    Some(EditOp {
        old: text(args, old)?,
        new: text(args, new)?,
        replace_all: args.get("replace_all").and_then(Value::as_bool) == Some(true),
    })
}

/// 하위 에이전트의 지시 글: 있는 인자를 줄바꿈으로 잇는다.
fn joined(args: &Value, keys: &[&str]) -> Option<String> {
    let parts: Vec<String> = keys.iter().filter_map(|key| text(args, key)).collect();
    (!parts.is_empty()).then(|| parts.join("\n"))
}

/// `claude`·`codex`·`grok`이 함께 쓰는 snake_case 입력 읽기. 도구 대응은 호스트마다 준다.
fn decode_snake(
    raw: &[u8],
    event_arg: Option<&str>,
    tool_of: fn(&str, &Value) -> ToolCall,
) -> Result<(HookInput, Value), DecodeError> {
    let v: Value = serde_json::from_slice(raw).map_err(|_| DecodeError("json"))?;
    let event = text(&v, "hook_event_name")
        .as_deref()
        .or(event_arg)
        .and_then(EventKind::parse)
        .ok_or(DecodeError("event"))?;
    let sid = text(&v, "session_id")
        .and_then(|s| SessionId::from_host(&s))
        .ok_or(DecodeError("session"))?;
    let cwd = text(&v, "cwd")
        .filter(|c| !c.is_empty())
        .ok_or(DecodeError("cwd"))?;
    let is = |kind: EventKind| event == kind;
    let call = text(&v, "tool_name")
        .filter(|_| is(EventKind::PreToolUse))
        .map(|name| tool_of(&name, v.get("tool_input").unwrap_or(&Value::Null)));
    let failure = |key: &str| text(&v, key).filter(|_| is(EventKind::StopFailure));
    let input = HookInput {
        event,
        sid,
        cwd: PathBuf::from(cwd),
        source: text(&v, "source").filter(|_| is(EventKind::SessionStart)),
        prompt: text(&v, "prompt"),
        tool: call,
        stop_reason: None,
        error: failure("error"),
        error_details: failure("error_details")
            .or_else(|| failure("last_assistant_message"))
            .map(clip),
        agent_id: text(&v, "agent_id"),
    };
    Ok((input, v))
}

/// Claude Code 형식의 출력(`claude`·`codex`·`grok`). `quiet`는 사용자 경고와 문맥이 SessionStart 밖에서 닿지 않는 호스트(grok)다.
fn encode_claude(event: EventKind, v: &Verdict, quiet: bool) -> String {
    let start = event == EventKind::SessionStart;
    let out = match v {
        // additionalContext는 SessionStart와 UserPromptSubmit에서만 쓴다(Stop에서는 대화를 이어 가게 만든다)
        Verdict::Context(t) if !quiet && (start || event == EventKind::UserPromptSubmit) => {
            json!({"hookSpecificOutput": {"hookEventName": event.as_str(), "additionalContext": t}})
        }
        Verdict::Warn(t) if !quiet || start => json!({"systemMessage": messages::warn(t)}),
        Verdict::Deny { rule, reason } => json!({"hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": messages::refusal(rule, reason, ""),
        }}),
        Verdict::Block { rule, reason, also } => {
            json!({"decision": "block", "reason": messages::block(rule, reason, "", also)})
        }
        _ => return EMPTY.to_string(),
    };
    out.to_string()
}

// ── 진입 ─────────────────────────────────────────────────────

/// `fullops hook <claude|codex|grok|agy> [--event <이름>] [--plugin-root <경로>] [--in-place]`의 인자.
pub struct HookArgs {
    pub host: Box<dyn Host>,
    /// agy의 등록 문자열만 준다. stdin에 `hook_event_name`이 없을 때 쓴다.
    pub event: Option<String>,
    pub plugin_root: Option<PathBuf>,
    pub in_place: bool,
}

fn parse_args(args: &[OsString]) -> Option<HookArgs> {
    let mut args = args.iter();
    let host = host_for(args.next()?.to_str()?)?;
    let (mut event, mut plugin_root, mut in_place) = (None, None, false);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--event") => event = args.next().and_then(|v| v.to_str()).map(str::to_string),
            Some("--plugin-root") => plugin_root = args.next().map(PathBuf::from),
            Some("--in-place") => in_place = true,
            _ => {}
        }
    }
    Some(HookArgs {
        host,
        event,
        plugin_root,
        in_place,
    })
}

/// 출력은 한 번만 쓴다. 감시 스레드와 본 흐름 가운데 먼저 쓴 쪽이 이긴다.
static WRITTEN: Mutex<bool> = Mutex::new(false);

fn emit(text: &str) {
    let mut written = WRITTEN.lock().unwrap_or_else(|e| e.into_inner());
    if !*written {
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(text.as_bytes());
        let _ = stdout.flush();
        *written = true;
    }
}

/// 내부 기한(밀리초). 이벤트를 알기 전에는 가장 긴 값이다. `--event`가 있으면 stdin을 읽기 전에 그 이벤트의 값으로 낮춘다.
static DEADLINE_MS: AtomicU64 = AtomicU64::new(12_000);

fn deadline_ms(event: EventKind) -> u64 {
    match event {
        EventKind::SessionStart | EventKind::PreToolUse => 4_000,
        EventKind::UserPromptSubmit => 1_500,
        EventKind::Stop => 12_000,
        EventKind::StopFailure => 8_000,
    }
}

/// 오류와 시간 초과를 추적에 남길 때 필요한 것. 레포를 찾은 뒤에 채운다.
struct Running {
    repo: Repo,
    sid: SessionId,
    host: HostId,
    event: EventKind,
    started: Instant,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

fn trace_line(host: HostId, event: EventKind, started: Instant, err: Option<&str>) -> TraceLine {
    TraceLine {
        t: fullops_store::now(),
        v: 1,
        bin: env!("CARGO_PKG_VERSION").to_string(),
        host,
        ev: event,
        actor: None,
        intents: Vec::new(),
        rules: Vec::new(),
        git: 0,
        orca: 0,
        ms: started.elapsed().as_millis() as u64,
        err: err.map(str::to_string),
    }
}

fn trace_error(kind: &str) {
    let running = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(run) = running.as_ref() {
        let line = trace_line(run.host, run.event, run.started, Some(kind));
        fullops_store::append_trace(&run.repo, &run.sid, &line);
    }
}

/// 감시 스레드: 내부 기한을 넘으면 아직 출력이 없을 때 통과를 쓰고 끝낸다.
fn watch(pass: &'static str, started: Instant) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(25));
            if started.elapsed().as_millis() as u64 >= DEADLINE_MS.load(Ordering::Relaxed) {
                emit(pass);
                trace_error("timeout");
                std::process::exit(0);
            }
        }
    });
}

/// `a.b.c` 꼴의 버전. 읽지 못하면 `None`이다.
fn version(text: &str) -> Option<(u64, u64, u64)> {
    let core = text.trim().split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

/// 플러그인 루트(7.1절): `--plugin-root`가 없거나 풀리지 않은 `${`가 남아 있으면 자기 실행 파일의 자리로 추론한다.
fn plugin_root(args: &HookArgs) -> Option<PathBuf> {
    args.plugin_root
        .clone()
        .filter(|p| !p.as_os_str().is_empty() && !p.to_string_lossy().contains("${"))
        .or_else(crate::cli::plugin_root_of_exe)
}

/// S-04의 조건: 고정 위치 실행본이 없거나 플러그인 루트의 `VERSION`이 `install.json`의 `version`보다 높다.
fn self_install(args: &HookArgs, root: Option<&Path>) -> bool {
    let (false, Some(root), Some(fixed)) = (args.in_place, root, fullops_store::fixed_bin()) else {
        return false;
    };
    let Some(shipped) = std::fs::read_to_string(root.join("VERSION"))
        .ok()
        .and_then(|v| version(&v))
    else {
        return false;
    };
    if !fixed.exists() {
        return true;
    }
    let record = fullops_store::fullops_home().and_then(|home| {
        fullops_store::read_json::<Value>(&home.join("install.json"))
            .yes()
            .flatten()
    });
    // `install --rollback`이 고정한 실행본은 사람이 `fullops install`을 다시 할 때까지 올리지 않는다
    if record.as_ref().is_some_and(|r| r["pinned"] == true) {
        return false;
    }
    let installed = record.and_then(|record| version(record["version"].as_str()?));
    installed.is_none_or(|installed| shipped > installed)
}

fn scene(args: &HookArgs, repo: &Repo, event: EventKind, root: Option<&Path>) -> Scene {
    let start = event == EventKind::SessionStart;
    let orca = fullops_ext::orca_program();
    let mut missing = Vec::new();
    if start {
        for (label, program) in [("git", "git"), ("orca", orca.as_str())] {
            if fullops_ext::hosts::find_program(program).is_none() {
                missing.push(label.to_string());
            }
        }
    }
    Scene {
        root: repo.root.clone(),
        orca_names: std::env::var("ORCA_CLI_COMMAND")
            .ok()
            .filter(|v| !v.is_empty())
            .into_iter()
            .collect(),
        self_install: start && self_install(args, root),
        missing_tools: missing,
    }
}

fn intent_names(input: &HookInput, scene: &Scene) -> Vec<String> {
    let Some((ToolKind::Shell(dialect), Some(command))) =
        input.tool.as_ref().map(|t| (t.kind, &t.command))
    else {
        return Vec::new();
    };
    fullops_core::shell::classify_shell(command, dialect, &scene.orca_names)
        .iter()
        .map(|intent| {
            match intent {
                Intent::OrcaWorkerStart => "OrcaWorkerStart",
                Intent::OrcaTaskCreate { .. } => "OrcaTaskCreate",
                Intent::OrcaInject => "OrcaInject",
                Intent::OrcaTerminalSend { .. } => "OrcaTerminalSend",
                Intent::OrcaCheck { .. } => "OrcaCheck",
                Intent::GitCommit { .. } => "GitCommit",
                Intent::GitAdd { .. } => "GitAdd",
                Intent::Fullops { .. } => "Fullops",
                Intent::Other => "Other",
                Intent::Unparsed => "Unparsed",
            }
            .to_string()
        })
        .collect()
}

/// 판정을 내고 효과를 실행한다. `Ok`인데 아무것도 쓰지 않았으면 부르는 쪽이 통과를 쓴다.
fn handle(args: &HookArgs, started: Instant) -> Result<(), &'static str> {
    let mut raw = Vec::new();
    std::io::stdin()
        .lock()
        .take(STDIN_LIMIT + 1)
        .read_to_end(&mut raw)
        .map_err(|_| "stdin")?;
    if raw.len() as u64 > STDIN_LIMIT {
        return Err("stdin_limit");
    }
    let input = args
        .host
        .decode(&raw, args.event.as_deref())
        .map_err(|e| e.0)?;
    // agy의 오류로 끝난 Stop은 StopFailure로 옮겨지지만 내부 기한은 등록한 이벤트(Stop)의 것이다
    let registered = args.event.as_deref().and_then(EventKind::parse);
    let timed = registered.unwrap_or(input.event);
    DEADLINE_MS.store(deadline_ms(timed), Ordering::Relaxed);
    let host = args.host.id();
    // 활성 표식이 없으면 통과하고 끝낸다(추적도 쓰지 않는다). 새 세션의 caveman 주입(S-06)만은 레포를 가리지 않는다
    let Some(repo) = fullops_store::discover(&input.cwd).map_err(|_| "io")? else {
        if input.event == EventKind::SessionStart {
            let read = |name: &str| facts_real::RealFacts::skill_of(host, name);
            if let Some(skill) = start_skill(&input, args.host.caps(), read) {
                let text = messages::hook::session_context(&Default::default(), Some(&skill));
                emit(&args.host.encode(input.event, &Verdict::Context(text)));
            }
        }
        return Ok(());
    };
    *RUNNING.lock().unwrap_or_else(|e| e.into_inner()) = Some(Running {
        repo: repo.clone(),
        sid: input.sid.clone(),
        host,
        event: input.event,
        started,
    });
    // 실행 근거: 그 세션에서 이 이벤트가 처음 돌았다는 표식
    let seen = repo.seen_path(&input.sid, input.event.as_str());
    if !seen.exists() {
        if let Some(dir) = seen.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::File::create(&seen);
    }

    let root = plugin_root(args);
    let scene = scene(args, &repo, input.event, root.as_deref());
    let mut facts = facts_real::RealFacts::new(repo, host, &input, root);
    let Outcome {
        verdict,
        effects,
        trace,
    } = decide_in(&scene, &input, &mut facts);
    let trace_all = std::env::var("FULLOPS_TRACE").is_ok_and(|v| v == "1");
    if verdict == Verdict::Pass && effects.is_empty() && !facts.must_bind() && !trace_all {
        return Ok(());
    }

    // 판정이 기대는 효과(세션 파일의 blocks 기록)는 출력 전에 쓴다
    let mut errors: Vec<&'static str> = Vec::new();
    let unrecorded = effects::before_output(&mut facts, &effects, &mut errors);
    let verdict = match verdict {
        // blocks를 쓰지 못하면 그 호출의 한 번 막기는 경고로 낸다
        Verdict::Block { rule, reason, .. } if unrecorded.contains(&rule) => {
            Verdict::Warn(messages::refusal(rule, &reason, ""))
        }
        other => other,
    };
    emit(&args.host.encode(input.event, &verdict));
    effects::after_output(&mut facts, &effects, &mut errors);

    if verdict != Verdict::Pass || !errors.is_empty() || trace_all {
        let (git, orca) = facts.calls();
        let line = TraceLine {
            actor: facts.actor_label(),
            intents: intent_names(&input, &scene),
            rules: trace,
            git,
            orca,
            err: errors.first().map(|e| e.to_string()),
            ..trace_line(host, input.event, started, None)
        };
        fullops_store::append_trace(&facts.repo, &input.sid, &line);
    }
    Ok(())
}

/// 같은 오류 종류는 세션당 한 번 사용자에게 알린다(낼 수 있는 호스트와 이벤트에서만. 설계 7.8절).
fn warn_once(args: &HookArgs, kind: &str) {
    let running = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    let Some(run) = running.as_ref() else {
        return;
    };
    let key = format!("hook-error:{kind}");
    let fresh = fullops_store::update_session(&run.repo, &run.sid, fullops_store::HOOK_WAIT, |s| {
        s.notice(&key)
    });
    if fresh.is_ok_and(|fresh| fresh) {
        let text = format!("hook 오류({kind})로 이번 판정을 건너뛰었다. `fullops why`로 확인한다");
        emit(&args.host.encode(run.event, &Verdict::Warn(text)));
    }
}

fn answer(args: &[OsString], started: Instant) {
    let Some(args) = parse_args(args) else {
        return emit(EMPTY);
    };
    // grok이 다른 호스트의 등록 파일을 읽어 실행한 경우다. grok의 판정은 `hook grok` 등록 하나만 낸다
    let other_host = matches!(args.host.id(), HostId::Claude | HostId::Codex);
    if other_host && std::env::var_os("GROK_HOOK_EVENT").is_some() {
        return emit(EMPTY);
    }
    // stdin을 읽기 전에 정해지므로 감시 스레드, 해석 실패, panic 처리가 같은 글을 쓴다
    let pass = args.host.pass(args.event.as_deref());
    // 등록 문자열이 이벤트를 알려 주면(agy) stdin이 멈춰도 등록 타임아웃보다 먼저 통과를 낸다
    if let Some(event) = args.event.as_deref().and_then(EventKind::parse) {
        DEADLINE_MS.store(deadline_ms(event), Ordering::Relaxed);
    }
    watch(pass, started);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle(&args, started)));
    let failed = match result {
        Ok(Ok(())) => None,
        Ok(Err(kind)) => Some(kind),
        Err(_) => Some("panic"),
    };
    if let Some(kind) = failed {
        trace_error(kind);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| warn_once(&args, kind)));
    }
    emit(pass);
}

pub fn run(args: Vec<OsString>) -> ExitCode {
    // panic 글이 호스트 화면에 나가지 않게 한다. 오류는 추적으로만 남긴다
    std::panic::set_hook(Box::new(|_| {}));
    answer(&args, Instant::now());
    ExitCode::SUCCESS
}
