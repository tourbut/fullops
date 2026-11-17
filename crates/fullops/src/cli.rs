//! CLI 공통 규약(설계 8.1절): 종료 코드, 출력 자리, `--repo`·`--json`, 거절 출력, 호출자 판정.
//! 결과는 stdout에, 거절·오류·알림 문구는 stderr에 쓴다. `--json`이면 stdout에는 JSON 객체 하나만 쓴다.

use crate::cmd;
use fullops_core::config::PolicyError;
use fullops_core::facts::{ActorInput, resolve_actor};
use fullops_core::messages;
use fullops_core::model::{
    Actor, Config, DecisionId, Derived, Refusal, Session, SessionId, State, path_text,
};
use fullops_store::{Repo, StoreError};
use serde_json::json;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

pub const EXIT_OK: u8 = 0;
/// 내부 오류·사용법 오류.
pub const EXIT_ERROR: u8 = 1;
/// 규칙에 의한 거절. 상태를 바꾸지 않는다. 정책 오류와 활성 표식 없음도 이 값이다.
pub const EXIT_REFUSED: u8 = 2;
/// 사람 손이 필요함(결정을 열었거나 병합 충돌).
pub const EXIT_HUMAN: u8 = 3;
/// 바깥 도구 실패. 다시 실행해도 안전하다.
pub const EXIT_EXTERNAL: u8 = 4;

/// `current.json`을 믿는 시간(초).
const CURRENT_TTL: i64 = 600;

#[derive(Debug)]
pub enum CliError {
    Usage(String),
    Internal(String),
    /// 활성 표식이 없다.
    Inactive,
    /// 정책 오류. 복구 명령만 받는다(설계 7.8절).
    Policy(PolicyError),
    /// 규칙에 의한 거절. 종료 코드는 걸린 `Refusal.exit`의 최댓값이다.
    Refused(Vec<Refusal>),
    /// 사람 손이 필요한 종료(코드 3). `decision`은 이 명령이 연 결정의 ID다.
    Human {
        decision: Option<DecisionId>,
        message: String,
    },
    External(String),
}

impl CliError {
    pub fn unimplemented(command: &str) -> Self {
        CliError::Internal(format!(
            "`fullops {command}`는 {}.",
            messages::UNIMPLEMENTED
        ))
    }

    pub fn exit(&self) -> u8 {
        match self {
            CliError::Usage(_) | CliError::Internal(_) => EXIT_ERROR,
            CliError::Inactive | CliError::Policy(_) => EXIT_REFUSED,
            CliError::Refused(list) => list.iter().map(|r| r.exit).max().unwrap_or(EXIT_REFUSED),
            CliError::Human { .. } => EXIT_HUMAN,
            CliError::External(_) => EXIT_EXTERNAL,
        }
    }

    /// `--json` 오류 형태의 `error.kind`. 거절과 사람 손이 필요한 종료는 자기 형태가 따로 있다.
    fn kind(&self) -> &'static str {
        match self {
            CliError::Usage(_) => "usage",
            CliError::Internal(_) | CliError::Refused(_) | CliError::Human { .. } => "internal",
            CliError::Inactive => "inactive",
            CliError::Policy(_) => "policy",
            CliError::External(_) => "external",
        }
    }

    fn message(&self) -> String {
        match self {
            CliError::Usage(m) | CliError::Internal(m) | CliError::External(m) => m.clone(),
            CliError::Human { message, .. } => message.clone(),
            CliError::Inactive => format!(
                "{}. `fullops init`으로 시작합니다.",
                messages::INACTIVE_REPO
            ),
            CliError::Policy(e) => format!("정책 오류: {e} 다음: 설정을 고치거나 `fullops doctor`"),
            CliError::Refused(list) => list
                .iter()
                .map(|r| messages::refusal(r.rule, &r.reason, &r.escape))
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

impl From<lexopt::Error> for CliError {
    fn from(e: lexopt::Error) -> Self {
        CliError::Usage(format!("인자를 읽을 수 없습니다: {e}"))
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Internal(format!("입출력 오류: {e}"))
    }
}

impl From<StoreError> for CliError {
    fn from(e: StoreError) -> Self {
        CliError::Internal(e.to_string())
    }
}

impl From<fullops_ext::ExtError> for CliError {
    fn from(e: fullops_ext::ExtError) -> Self {
        CliError::External(e.to_string())
    }
}

/// 세션 핸들(설계 4절): `ORCA_AGENT_SESSION_ID`, 없으면 `ORCA_TERMINAL_HANDLE`. hook과 CLI가 함께 쓴다.
pub fn session_handle() -> Option<String> {
    ["ORCA_AGENT_SESSION_ID", "ORCA_TERMINAL_HANDLE"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
}

/// 자기 실행 파일이 `<X>/bin/` 아래에 있고 `<X>/VERSION`이 있으면 X다(설계 7.1절). hook의 플러그인 루트 추론이 쓴다.
pub fn plugin_root_of_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bin = exe.parent()?;
    let root = bin.parent()?;
    (bin.file_name()? == "bin" && root.join("VERSION").is_file()).then(|| root.to_path_buf())
}

/// CLI의 플러그인 루트(설계 8.1절). `install`·`doctor`·`upgrade`가 쓴다.
/// (1) 자기 실행 파일의 `<X>/bin/` 규칙 (2) `install.json`의 `source` 폴더에 `VERSION`이 있으면 그 폴더 (3) 없음.
pub fn plugin_root() -> Option<PathBuf> {
    plugin_root_of_exe().or_else(|| {
        let install = fullops_store::fullops_home()?.join("install.json");
        let record: serde_json::Value = fullops_store::read_json(&install).yes().flatten()?;
        let source = PathBuf::from(record["source"].as_str()?);
        source.join("VERSION").is_file().then_some(source)
    })
}

/// CLI를 부른 쪽(설계 8.1절). 세션은 `current.json`(10분 이내)의 `sid`로 찾는다.
pub struct Caller {
    pub actor: Actor,
    pub sid: Option<SessionId>,
    pub session: Option<Session>,
}

/// 명령 하나의 실행 문맥.
pub struct Ctx {
    /// 실행한 명령 이름. 한 파일이 여러 명령을 맡을 때 가른다.
    pub command: &'static str,
    pub json: bool,
    /// `--repo` 값. 기본은 cwd다.
    pub start: PathBuf,
    repo: Option<Repo>,
    config: Option<Result<Config, PolicyError>>,
}

impl Ctx {
    pub fn repo(&self) -> Result<&Repo, CliError> {
        self.repo.as_ref().ok_or(CliError::Inactive)
    }

    pub fn config(&self) -> Result<&Config, CliError> {
        match &self.config {
            Some(Ok(config)) => Ok(config),
            Some(Err(e)) => Err(CliError::Policy(e.clone())),
            None => Err(CliError::Inactive),
        }
    }

    /// 호출자 판정(설계 7.3절과 같다). 세션을 못 찾으면 브랜치만으로 좌석을 정하고 dispatch는 세션 핸들로만 묶는다.
    /// hook이 돌지 않은 세션은 항상 이 뒤쪽 길을 탄다.
    pub fn caller(&self) -> Result<Caller, CliError> {
        let (repo, config) = (self.repo()?, self.config()?);
        let current = fullops_store::read_current(repo)
            .yes()
            .flatten()
            .filter(|c| fullops_store::now() - c.at <= CURRENT_TTL);
        let session = current
            .as_ref()
            .and_then(|c| fullops_store::read_session(repo, &c.sid).yes().flatten());
        let head = fullops_store::head(repo)?;
        let state = fullops_store::read_state(repo).yes();
        let handle = session_handle();
        let resolved = resolve_actor(&ActorInput {
            config,
            head: &head,
            worktree: &path_text(&repo.root),
            session_dispatch: session
                .as_ref()
                .and_then(|s| s.dispatch.as_ref())
                .map(|d| &d.id),
            terminal: handle.as_deref(),
            state: state.as_ref(),
        });
        Ok(Caller {
            actor: resolved.actor,
            sid: current.map(|c| c.sid),
            session,
        })
    }

    /// CLI의 `state.json` 쓰기. `derived`는 이 명령이 잠금 밖에서 계산한 유도값이다(설계 6.2.3절).
    /// 파손이면 옮겨 두고 빈 상태로 시작한 뒤 종료 코드 1로 알린다(설계 6.5절).
    pub fn update_state<T>(
        &self,
        derived: Option<&Derived>,
        f: impl FnOnce(&mut State) -> T,
    ) -> Result<T, CliError> {
        let repo = self.repo()?;
        match fullops_store::update_state(repo, fullops_store::CLI_WAIT, derived, f) {
            Err(StoreError::Corrupt("state_broken")) => {
                let moved = fullops_store::quarantine_state(repo, fullops_store::CLI_WAIT)?;
                Err(CliError::Internal(format!(
                    "state.json을 읽을 수 없어 {}로 옮기고 빈 상태로 시작했습니다. 열린 결정과 보류는 status.json의 마지막 값을 보고 다시 넣습니다.",
                    moved.display()
                )))
            }
            Err(StoreError::Corrupt("state_newer")) => Err(CliError::Internal(
                "state.json이 이 실행본보다 새 형식입니다. 설치본이 낡았습니다. `fullops install`로 갱신합니다.".into(),
            )),
            other => Ok(other?),
        }
    }

    /// 성공 출력(stdout). `--json`이면 `{"ok":true, …fields}` 한 줄, 아니면 사람용 글이다. `fields`는 JSON 객체다.
    pub fn print(&self, human: &str, fields: serde_json::Value) {
        if !self.json {
            println!("{human}");
            return;
        }
        let mut out = json!({ "ok": true });
        if let (Some(out), Some(fields)) = (out.as_object_mut(), fields.as_object()) {
            out.extend(fields.clone());
        }
        println!("{out}");
    }

    /// 알림 한 줄(stderr). 결과가 아닌 문구는 모두 여기로 낸다.
    pub fn note(&self, text: &str) {
        eprintln!("{text}");
    }
}

/// 오류를 낸다. 사람용 글은 항상 stderr에, `--json`이면 stdout에 JSON 객체 하나를 더 쓴다.
fn report(e: &CliError, as_json: bool) {
    eprintln!("{}", e.message());
    if !as_json {
        return;
    }
    let out = match e {
        CliError::Refused(list) => {
            let refused: Vec<_> = list
                .iter()
                .map(|r| json!({"rule": r.rule, "reason": r.reason, "escape": r.escape}))
                .collect();
            json!({"ok": false, "refused": refused})
        }
        CliError::Human { decision, message } => {
            json!({"ok": false, "human": {"decision": decision, "message": message}})
        }
        other => json!({"ok": false, "error": {"kind": other.kind(), "message": other.message()}}),
    };
    println!("{out}");
}

/// 모든 명령이 받는 `--repo <경로>`와 `--json`을 떼어 낸다. 두 옵션은 하위 명령 이름 뒤 어디에나 올 수 있다.
/// 다른 옵션의 값이 `--json`·`--repo`와 같은 글이면 `--옵션=값` 꼴로 준다(설계 8.1절).
fn split_globals(args: Vec<OsString>) -> Result<(Vec<OsString>, bool, Option<PathBuf>), CliError> {
    let (mut rest, mut as_json, mut repo) = (Vec::new(), false, None);
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--") => {
                rest.push(arg);
                rest.extend(args.by_ref());
            }
            Some("--json") => as_json = true,
            Some("--repo") => {
                repo = Some(PathBuf::from(args.next().ok_or_else(|| {
                    CliError::Usage("--repo에 경로가 없습니다.".into())
                })?));
            }
            Some(text) if text.starts_with("--repo=") => {
                repo = Some(PathBuf::from(&text["--repo=".len()..]))
            }
            _ => rest.push(arg),
        }
    }
    Ok((rest, as_json, repo))
}

fn usage() -> String {
    let mut out = format!(
        "fullops {}\n\n사용법: fullops <명령> [인자] [--repo <경로>] [--json]\n\n명령:\n",
        env!("CARGO_PKG_VERSION")
    );
    out.push_str(&format!(
        "  {:<12} {}\n",
        "hook", "호스트 hook 진입점: hook <claude|codex|grok|agy> [--event <이름>]"
    ));
    for c in cmd::COMMANDS {
        out.push_str(&format!("  {:<12} {}\n", c.name, c.about));
    }
    out
}

fn dispatch(name: &str, args: Vec<OsString>, as_json: &mut bool) -> Result<u8, CliError> {
    // 명령을 찾기 전에 떼어 내야 모르는 명령의 오류도 --json을 따른다
    let (rest, json_flag, repo_arg) = split_globals(args)?;
    *as_json = json_flag;
    let command = cmd::COMMANDS
        .iter()
        .find(|c| c.name == name)
        .ok_or_else(|| {
            CliError::Usage(format!(
                "모르는 명령입니다: {name}. `fullops help`로 목록을 봅니다."
            ))
        })?;
    let start = match repo_arg {
        Some(path) => path,
        None => std::env::current_dir()?,
    };
    let repo = match fullops_store::discover(&start) {
        Ok(repo) => repo,
        Err(e) if command.needs_repo => return Err(e.into()),
        Err(_) => None,
    };
    // 옛 자리 레포(설계 13.6절): 옮기는 명령과 설치·점검만 돈다. 요약과 upgrade는 안내만 내고 끝난다
    if repo.is_none() && fullops_store::legacy_root(&start).is_some() {
        match name {
            "migrate" | "doctor" | "install" | "rules" => {}
            "status" | "upgrade" => {
                if json_flag {
                    println!(
                        "{}",
                        json!({"ok": true, "legacy": true, "message": messages::LEGACY_REPO})
                    );
                } else {
                    println!("{}", messages::warn(messages::LEGACY_REPO));
                }
                return Ok(EXIT_OK);
            }
            _ => {
                return Err(CliError::Refused(vec![Refusal {
                    rule: "layout",
                    reason: messages::LEGACY_REPO.to_string(),
                    escape: String::new(),
                    exit: EXIT_REFUSED,
                }]));
            }
        }
    }
    if command.needs_repo && repo.is_none() {
        return Err(CliError::Inactive);
    }
    let config = repo.as_ref().map(fullops_store::load_config);
    if let (Some(Err(e)), false) = (&config, command.recovery) {
        return Err(CliError::Policy(e.clone()));
    }
    let ctx = Ctx {
        command: command.name,
        json: json_flag,
        start,
        repo,
        config,
    };
    (command.run)(&ctx, lexopt::Parser::from_args(rest))
}

pub fn run(first: Option<OsString>, rest: Vec<OsString>) -> ExitCode {
    let name = first.as_deref().and_then(|a| a.to_str()).unwrap_or("");
    match name {
        // 등록 파일의 args를 이해하지 못하는 호스트가 실행 파일만 불러도 빈 출력과 종료 코드 0(통과)이 되게 한다
        "" => {
            eprint!("{}", usage());
            return ExitCode::SUCCESS;
        }
        "help" | "--help" | "-h" => {
            print!("{}", usage());
            return ExitCode::SUCCESS;
        }
        "--version" | "-V" => {
            println!("fullops {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    let mut as_json = false;
    match dispatch(name, rest, &mut as_json) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            report(&e, as_json);
            ExitCode::from(e.exit())
        }
    }
}
