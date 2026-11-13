//! hook 진입(설계 7.1절). 종료 코드는 항상 0이고 판정은 stdout JSON으로만 낸다.
//! 뼈대는 1~9단계 가운데 "어떤 입력에도 `{}`를 낸다"까지만 한다. 감시 스레드, 실행 표식, 분류, 효과, 추적은 묶음 B가 채운다.

mod effects;
mod facts_real;
mod host_claude;
mod host_codex;

use fullops_core::model::{Caps, EventKind, HookInput, HostKind, ToolKind, Verdict};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

/// stdin을 읽는 한도.
const STDIN_LIMIT: u64 = 8 * 1024 * 1024;

/// 입력을 해석하지 못한 사유. 추적에만 남는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError(pub &'static str);

/// 호스트 어댑터(설계 7.2절). 호스트 계약(미정-M2)이 바뀌면 구현 파일만 고친다.
pub trait Host {
    fn kind(&self) -> HostKind;
    fn decode(&self, raw: &[u8]) -> Result<HookInput, DecodeError>;
    fn encode(&self, event: EventKind, v: &Verdict) -> String;
    fn caps(&self) -> Caps;
    fn tool_kind(&self, name: &str) -> ToolKind;
}

/// `fullops hook <claude|codex> [--plugin-root <경로>] [--in-place]`의 인자.
pub struct HookArgs {
    pub host: Box<dyn Host>,
    pub plugin_root: Option<PathBuf>,
    pub in_place: bool,
}

fn parse_args(args: &[OsString]) -> Option<HookArgs> {
    let mut args = args.iter();
    let host: Box<dyn Host> = match args.next()?.to_str()? {
        "claude" => Box::new(host_claude::Claude),
        "codex" => Box::new(host_codex::Codex),
        _ => return None,
    };
    let (mut plugin_root, mut in_place) = (None, false);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--plugin-root") => plugin_root = args.next().map(PathBuf::from),
            Some("--in-place") => in_place = true,
            _ => {}
        }
    }
    Some(HookArgs {
        host,
        plugin_root,
        in_place,
    })
}

/// 판정 JSON을 돌려준다. `None`은 빈 판정이다.
fn handle(args: &[OsString]) -> Option<String> {
    let args = parse_args(args)?;
    let mut raw = Vec::new();
    std::io::stdin()
        .lock()
        .take(STDIN_LIMIT + 1)
        .read_to_end(&mut raw)
        .ok()?;
    if raw.len() as u64 > STDIN_LIMIT {
        return None;
    }
    let input = args.host.decode(&raw).ok()?;
    let repo = fullops_store::discover(&input.cwd).ok()??;
    let mut facts = facts_real::RealFacts::new(repo, args.host.caps());
    let outcome = fullops_core::decide(&input, &mut facts);
    let out = args.host.encode(input.event, &outcome.verdict);
    effects::run(&mut facts, &outcome.effects);
    Some(out)
}

pub fn run(args: Vec<OsString>) -> ExitCode {
    // panic 글이 호스트 화면에 나가지 않게 한다. 오류는 추적으로만 남긴다
    std::panic::set_hook(Box::new(|_| {}));
    let out = std::panic::catch_unwind(|| handle(&args))
        .ok()
        .flatten()
        .unwrap_or_else(|| "{}".to_string());
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(out.as_bytes());
    let _ = stdout.flush();
    ExitCode::SUCCESS
}
