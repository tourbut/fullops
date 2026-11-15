//! 호스트 CLI 조회와 스킬 설치 도구 호출(설계 9.5절). `fullops install`과 `doctor`만 부른다. 묶음 F의 파일이다.
//! `skill_roots`는 뼈대의 완성본이다(S-06과 `install --check`가 함께 쓴다).
//! 호스트 CLI의 어떤 하위 명령에도 `--help`를 붙여 부르지 않는다(agy의 `plugin` 하위 명령은 `--help`를 인자 값으로 받아 실제로 동작한다).
//! 부르는 명령은 `fullops_core::deps::HOST_COMMANDS`의 표와 `<실행 파일> --version`, 그리고 `run_argv`로 넘긴 grok 제거 명령뿐이다.

use crate::proc::{ProcResult, ProcStatus, run};
use fullops_core::deps::{Step, host_commands};
use fullops_core::model::{HostId, Known};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

/// 목록 조회의 제한 시간(설계 9.5절).
const LIST_TIMEOUT: Duration = Duration::from_secs(30);

pub use crate::proc::find_program;

/// 실행 파일을 PATH에서 찾아 그 전체 경로로 실행한다. 셸을 거치지 않는다.
fn run_found(argv: &[String], timeout: Duration) -> ProcResult {
    let missing = ProcResult {
        status: ProcStatus::NotFound,
        stdout: Vec::new(),
        stderr: Vec::new(),
    };
    let Some((program, args)) = argv.split_first() else {
        return missing;
    };
    let Some(found) = find_program(program) else {
        return missing;
    };
    let full: Vec<String> = std::iter::once(found.to_string_lossy().into_owned())
        .chain(args.iter().cloned())
        .collect();
    // 작업 디렉터리의 프로젝트 설정이 끼어들지 않게 임시 폴더에서 실행한다
    run(&full, &std::env::temp_dir(), timeout, &[], None)
}

fn host_json(host: HostId, args: &[&str]) -> Known<Value> {
    let cmds = host_commands(host);
    let argv: Vec<String> = std::iter::once(cmds.program)
        .chain(args.iter().copied())
        .map(str::to_string)
        .collect();
    let out = run_found(&argv, LIST_TIMEOUT);
    if !out.ok() {
        return Known::Unknown("host_cli");
    }
    match serde_json::from_slice(&out.stdout) {
        Ok(value) => Known::Yes(value),
        Err(_) => Known::Unknown("format"),
    }
}

/// PATH에 그 호스트의 실행 파일이 있는가.
pub fn host_available(host: HostId) -> bool {
    find_program(host_commands(host).program).is_some()
}

/// `<실행 파일> --version`의 첫 줄.
pub fn host_version(host: HostId) -> Known<String> {
    program_version(host_commands(host).program)
}

/// `<이름> --version`의 첫 줄. `doctor`가 git에도 쓴다.
pub fn program_version(name: &str) -> Known<String> {
    let out = run_found(&[name.to_string(), "--version".into()], LIST_TIMEOUT);
    if !out.ok() {
        return Known::Unknown("host_cli");
    }
    match out.stdout_text().lines().next().map(str::trim) {
        Some(line) if !line.is_empty() => Known::Yes(line.to_string()),
        _ => Known::Unknown("format"),
    }
}

fn text<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(value, |v, key| v.get(key))?
        .as_str()
        .filter(|s| !s.is_empty())
}

/// 등록된 마켓플레이스(이름 → 출처). 읽지 못하면 "이미 등록된 것을 알 수 없다"로 본다. agy는 마켓플레이스가 없다.
pub fn host_marketplaces(host: HostId) -> Known<BTreeMap<String, String>> {
    let Some(list) = host_commands(host).market_list else {
        return Known::Yes(BTreeMap::new());
    };
    let json = match host_json(host, list) {
        Known::Yes(json) => json,
        Known::Unknown(why) => return Known::Unknown(why),
    };
    // 목록에서 읽는 값(설계 9.5절의 표). 형태가 다른 항목이 하나라도 있으면 모른다고 본다
    let (items, sources): (Option<&Vec<Value>>, &[&[&str]]) = match host {
        HostId::Claude => (
            json.as_array(),
            &[&["repo"], &["url"], &["path"], &["installLocation"]],
        ),
        HostId::Codex => (
            json["marketplaces"].as_array(),
            &[&["marketplaceSource", "source"], &["root"]],
        ),
        _ => (json.as_array(), &[&["source", "url"]]),
    };
    let Some(items) = items else {
        return Known::Unknown("format");
    };
    let mut out = BTreeMap::new();
    for item in items {
        let source = sources.iter().find_map(|path| text(item, path));
        match (text(item, &["name"]), source) {
            (Some(name), Some(source)) => out.insert(name.to_string(), source.to_string()),
            _ => return Known::Unknown("format"),
        };
    }
    Known::Yes(out)
}

/// 설치된 플러그인 `(id, 켜짐)`. id는 `<이름>@<마켓플레이스>`, 마켓플레이스 없이 설치한 것은 이름만.
pub fn host_plugins(host: HostId) -> Known<Vec<(String, bool)>> {
    let json = match host_json(host, host_commands(host).plugin_list) {
        Known::Yes(json) => json,
        Known::Unknown(why) => return Known::Unknown(why),
    };
    let items = match host {
        HostId::Codex => json["installed"].as_array(),
        // shortcut: agy의 목록 형태는 미정-M12다, 배열이나 `plugins` 배열의 `name`만 읽고 그 밖은 모른다고 본다
        HostId::Agy => json.as_array().or(json["plugins"].as_array()),
        _ => json.as_array(),
    };
    let Some(items) = items else {
        return Known::Unknown("format");
    };
    let mut out = Vec::new();
    for item in items {
        // 꺼진 것도 돌려준다: doctor가 꺼진 Python판 플러그인도 잔재로 알린다
        let enabled = item["enabled"] != Value::Bool(false);
        let with_market = |name: &str, market: Option<&str>| match market {
            Some(market) => format!("{name}@{market}"),
            None => name.to_string(),
        };
        let id = match host {
            HostId::Claude => text(item, &["id"]).map(str::to_string),
            HostId::Codex => {
                text(item, &["name"]).map(|n| with_market(n, text(item, &["marketplaceName"])))
            }
            HostId::Grok => {
                text(item, &["name"]).map(|n| with_market(n, text(item, &["marketplace"])))
            }
            HostId::Agy => text(item, &["name"]).map(str::to_string),
        };
        match id {
            Some(id) => out.push((id, enabled)),
            None => return Known::Unknown("format"),
        }
    }
    Known::Yes(out)
}

fn home() -> Option<PathBuf> {
    let name = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// 사용자 스킬 폴더(설계 9.5절 표의 "사용자 스킬 폴더" 행, 그 순서). 이 목록의 정의는 여기 하나다.
/// 홈을 모르면 환경 변수로 정해지는 폴더만 돌려준다.
pub fn skill_roots(host: HostId) -> Vec<PathBuf> {
    let home = home();
    let under_home = |parts: &[&str]| {
        home.as_ref()
            .map(|h| parts.iter().fold(h.clone(), |p, part| p.join(part)))
    };
    // 환경 변수가 가리키는 설정 폴더, 없으면 홈 아래의 기본 폴더
    let config = |var: &str, dir: &str| {
        std::env::var_os(var)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| under_home(&[dir]))
            .map(|p| p.join("skills"))
    };
    let agents = under_home(&[".agents", "skills"]);
    let roots = match host {
        HostId::Claude => vec![config("CLAUDE_CONFIG_DIR", ".claude")],
        HostId::Codex => vec![agents, config("CODEX_HOME", ".codex")],
        HostId::Grok => vec![agents, config("GROK_HOME", ".grok")],
        HostId::Agy => vec![
            under_home(&[".gemini", "config", "skills"]),
            under_home(&[".gemini", "antigravity-cli", "skills"]),
            under_home(&[".gemini", "skills"]),
        ],
    };
    roots.into_iter().flatten().collect()
}

/// 계획의 명령 하나를 실행한다(`Assemble`·`HookFile`은 넘기지 않는다). 실행 파일은 PATH에서 찾은 전체 경로로 띄운다.
pub fn run_step(step: &Step, timeout: Duration) -> ProcResult {
    run_found(&step.argv, timeout)
}

/// 표 밖의 호스트 명령 하나. `install --retire-python`이 계약 문서에서 확인한 grok 제거 명령에만 쓴다.
pub fn run_argv(argv: &[String], timeout: Duration) -> ProcResult {
    run_found(argv, timeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_roots_follow_design_table() {
        // 환경 변수를 바꾸지 않고(다른 시험과 겹친다) 지금 환경에서 순서와 끝 폴더만 본다
        let tail = |host| -> Vec<String> {
            skill_roots(host)
                .iter()
                .map(|p| {
                    let parts: Vec<_> = p
                        .components()
                        .rev()
                        .take(2)
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect();
                    format!("{}/{}", parts[1], parts[0])
                })
                .collect()
        };
        if home().is_none() {
            return;
        }
        assert_eq!(skill_roots(HostId::Claude).len(), 1);
        assert_eq!(tail(HostId::Codex)[0], ".agents/skills");
        assert_eq!(tail(HostId::Grok)[0], ".agents/skills");
        assert_eq!(
            tail(HostId::Agy),
            ["config/skills", "antigravity-cli/skills", ".gemini/skills"]
        );
        assert!(
            skill_roots(HostId::Claude)
                .iter()
                .all(|p| p.ends_with("skills"))
        );
    }
}
