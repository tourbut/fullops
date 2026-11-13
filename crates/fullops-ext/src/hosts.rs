//! 호스트 CLI 조회와 스킬 설치 도구 호출(설계 9.5절). `fullops install`과 `doctor`만 부른다. 묶음 F가 채운다.
//! `skill_roots`는 뼈대의 완성본이다(S-06과 `install --check`가 함께 쓴다). 나머지는 임시 구현이고 프로세스를 띄우지 않는다.
//! 호스트 CLI의 어떤 하위 명령에도 `--help`를 붙여 부르지 않는다(agy의 `plugin` 하위 명령은 `--help`를 인자 값으로 받아 실제로 동작한다).

use crate::proc::{ProcResult, ProcStatus};
use fullops_core::deps::Step;
use fullops_core::model::{HostId, Known};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

const TODO: &str = "unimplemented";

/// PATH에 그 호스트의 실행 파일이 있는가. 임시 구현은 없다고 답한다(설치 대상에서 빠진다).
pub fn host_available(_host: HostId) -> bool {
    false
}

/// `<실행 파일> --version`의 글.
pub fn host_version(_host: HostId) -> Known<String> {
    Known::Unknown(TODO)
}

/// 등록된 마켓플레이스(이름 → 출처). 읽지 못하면 "이미 등록된 것을 알 수 없다"로 본다.
pub fn host_marketplaces(_host: HostId) -> Known<BTreeMap<String, String>> {
    Known::Unknown(TODO)
}

/// 설치돼 켜져 있는 플러그인 id(`<이름>@<마켓플레이스>`).
pub fn host_plugins(_host: HostId) -> Known<Vec<String>> {
    Known::Unknown(TODO)
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

/// 계획의 명령 하나를 실행한다(`Assemble`·`HookFile`은 넘기지 않는다). 임시 구현은 띄우지 않는다.
pub fn run_step(_step: &Step, _timeout: Duration) -> ProcResult {
    ProcResult {
        status: ProcStatus::NotFound,
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
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
