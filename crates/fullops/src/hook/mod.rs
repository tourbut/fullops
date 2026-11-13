//! hook 진입(설계 7.1절). 종료 코드는 항상 0이고 판정은 stdout JSON으로만 낸다.
//! 뼈대는 "어떤 입력에도 그 호스트의 통과 글을 낸다"까지 한다. 감시 스레드, 실행 표식, 분류, 효과, 추적은 묶음 B가 채운다.
//! `fullops hook`은 프로세스의 작업 디렉터리를 쓰지 않는다. 레포를 찾는 기준은 입력의 `cwd`다.

mod effects;
mod facts_real;
mod host_agy;
mod host_claude;
mod host_codex;
mod host_grok;

use fullops_core::model::{Caps, EventKind, HookInput, HostId, Verdict, caps_of};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

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

/// 판정 글을 돌려준다. `None`은 통과다.
fn handle(args: &HookArgs) -> Option<String> {
    let mut raw = Vec::new();
    std::io::stdin()
        .lock()
        .take(STDIN_LIMIT + 1)
        .read_to_end(&mut raw)
        .ok()?;
    if raw.len() as u64 > STDIN_LIMIT {
        return None;
    }
    let input: HookInput = args.host.decode(&raw, args.event.as_deref()).ok()?;
    let repo = fullops_store::discover(&input.cwd).ok()??;
    let mut facts = facts_real::RealFacts::new(repo, args.host.id());
    let outcome = fullops_core::decide(&input, &mut facts);
    let out = args.host.encode(input.event, &outcome.verdict);
    effects::run(&mut facts, &outcome.effects);
    Some(out)
}

fn answer(args: &[OsString]) -> String {
    let Some(args) = parse_args(args) else {
        return EMPTY.to_string();
    };
    // grok이 다른 호스트의 등록 파일을 읽어 실행한 경우다. grok의 판정은 `hook grok` 등록 하나만 낸다
    let other_host = matches!(args.host.id(), HostId::Claude | HostId::Codex);
    if other_host && std::env::var_os("GROK_HOOK_EVENT").is_some() {
        return EMPTY.to_string();
    }
    // stdin을 읽기 전에 정해지므로 해석 실패와 panic 처리가 같은 글을 쓴다
    let pass = args.host.pass(args.event.as_deref());
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle(&args)))
        .ok()
        .flatten()
        .unwrap_or_else(|| pass.to_string())
}

pub fn run(args: Vec<OsString>) -> ExitCode {
    // panic 글이 호스트 화면에 나가지 않게 한다. 오류는 추적으로만 남긴다
    std::panic::set_hook(Box::new(|_| {}));
    let out = answer(&args);
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(out.as_bytes());
    let _ = stdout.flush();
    ExitCode::SUCCESS
}
