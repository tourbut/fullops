//! CLI 공통 규약(설계 8.1절): 종료 코드, `--repo`·`--json`, 거절 출력, 호출자 판정.

use crate::cmd;
use fullops_core::config::PolicyError;
use fullops_core::facts::{ActorInput, resolve_actor};
use fullops_core::messages;
use fullops_core::model::{Actor, Config, Plan, Refusal, Session, SessionId, State, path_text};
use fullops_store::{Repo, StoreError};
use serde_json::json;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

pub const EXIT_OK: u8 = 0;
/// 내부 오류·사용법 오류.
pub const EXIT_ERROR: u8 = 1;
/// 규칙에 의한 거절. 상태를 바꾸지 않는다.
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
    Refused(Vec<Refusal>),
    External(String),
}

impl CliError {
    pub fn unimplemented(command: &str) -> Self {
        CliError::Internal(format!("`fullops {command}`는 아직 구현되지 않았습니다."))
    }

    pub fn exit(&self) -> u8 {
        match self {
            CliError::Usage(_) | CliError::Internal(_) => EXIT_ERROR,
            CliError::Inactive | CliError::Policy(_) => EXIT_REFUSED,
            CliError::Refused(list) => list.iter().map(|r| r.exit).max().unwrap_or(EXIT_REFUSED),
            CliError::External(_) => EXIT_EXTERNAL,
        }
    }

    fn message(&self) -> String {
        match self {
            CliError::Usage(m) | CliError::Internal(m) | CliError::External(m) => m.clone(),
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

    /// 호출자 판정(설계 7.3절과 같다). 세션을 못 찾으면 브랜치와 터미널 핸들로만 정한다.
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
        let terminal = std::env::var("ORCA_TERMINAL_HANDLE").ok();
        let resolved = resolve_actor(&ActorInput {
            config,
            head: &head,
            worktree: &path_text(&repo.root),
            session_dispatch: session
                .as_ref()
                .and_then(|s| s.dispatch.as_ref())
                .map(|d| &d.id),
            terminal: terminal.as_deref(),
            state: state.as_ref(),
        });
        Ok(Caller {
            actor: resolved.actor,
            sid: current.map(|c| c.sid),
            session,
        })
    }

    /// CLI의 `state.json` 쓰기. 파손이면 옮겨 두고 빈 상태로 시작한 뒤 종료 코드 1로 알린다(설계 6.5절).
    pub fn update_state<T>(
        &self,
        lead_plan: Option<&Plan>,
        f: impl FnOnce(&mut State) -> T,
    ) -> Result<T, CliError> {
        let repo = self.repo()?;
        match fullops_store::update_state(repo, fullops_store::CLI_WAIT, lead_plan, f) {
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

    /// 성공 출력. `--json`이면 JSON 한 줄, 아니면 사람용 글이다.
    pub fn print(&self, human: &str, json: &serde_json::Value) {
        if self.json {
            println!("{json}")
        } else {
            println!("{human}")
        }
    }
}

fn report(e: &CliError, as_json: bool) {
    if !as_json {
        eprintln!("{}", e.message());
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
        other => json!({"ok": false, "error": other.message()}),
    };
    println!("{out}");
}

/// 모든 명령이 받는 `--repo <경로>`와 `--json`을 떼어 낸다.
/// shortcut: 다른 옵션의 값이 정확히 `--json`·`--repo`인 경우를 가리지 못한다, 그런 값이 필요해지면 명령별 파서로 옮긴다.
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
    out.push_str("  hook <claude|codex>  호스트 hook 진입점\n");
    for c in cmd::COMMANDS {
        out.push_str(&format!("  {:<20} {}\n", c.name, c.about));
    }
    out
}

fn dispatch(name: &str, args: Vec<OsString>, as_json: &mut bool) -> Result<u8, CliError> {
    let command = cmd::COMMANDS
        .iter()
        .find(|c| c.name == name)
        .ok_or_else(|| {
            CliError::Usage(format!(
                "모르는 명령입니다: {name}. `fullops help`로 목록을 봅니다."
            ))
        })?;
    let (rest, json_flag, repo_arg) = split_globals(args)?;
    *as_json = json_flag;
    let start = match repo_arg {
        Some(path) => path,
        None => std::env::current_dir()?,
    };
    let repo = match fullops_store::discover(&start) {
        Ok(repo) => repo,
        Err(e) if command.needs_repo => return Err(e.into()),
        Err(_) => None,
    };
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
        "" => {
            eprint!("{}", usage());
            return ExitCode::from(EXIT_ERROR);
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
