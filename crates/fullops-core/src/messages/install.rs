//! 설치·이행 명령(`install`, `doctor`, `init`, `mode`, `upgrade`, `migrate`)의 문구. 묶음 F의 파일이다. 다른 묶음은 이 파일을 고치지 않는다(설계 18.2절).
//! 설계가 글자를 정한 알림은 여기 상수로 둔다. 값이 끼는 문장은 그 명령의 파일에서 조립한다.

use crate::model::HostId;

/// 규칙 ID의 사유 문구. 이 파일이 맡지 않는 ID면 `None`이다. `args`는 그 검사가 넘긴 값이다.
/// 설치·이행 명령의 거절은 규칙 표에 ID가 없고 사유를 부르는 쪽이 만든다.
pub fn reason(_id: &str, _args: &[String]) -> Option<String> {
    None
}

/// `FULLOPS_OFFLINE=1`에서 `install`이 네트워크 단계를 건너뛸 때(설계 10.3절).
pub const OFFLINE: &str = "폐쇄망 설정이라 플러그인 등록과 의존 설치를 건너뛰었다. 망이 열린 곳에서 `fullops install`을 다시 실행한다.";

/// pstack을 설치했을 때(설계 10.3절). `deps.toml`의 `notice = "pstack"`이 가리킨다.
pub const PSTACK: &str = "pstack의 '사람을 기다리지 않는다'·'다른 모델로 재시도' 규칙은 FullOps 규칙과 다르다. FullOps를 적용한 레포에서는 FullOps 규칙이 우선한다.";

/// 고정 위치 실행본 없이 grok의 hook 등록 파일을 쓰려 할 때.
pub const HOOK_NEEDS_RUNTIME: &str = "고정 위치 실행본이 없어 grok의 hook 등록 파일을 쓰지 않았다. `fullops install --only runtime`을 먼저 실행한다.";

/// 설치 뒤에 사람이 할 일(설계 10.3절). 신뢰는 사람이 한다.
pub fn after_install(host: HostId) -> Option<&'static str> {
    match host {
        HostId::Claude => None,
        HostId::Codex => Some(
            "Codex 세션에서 `/hooks`로 fullops hook을 한 번 신뢰해야 hook이 돈다(등록 문자열이 바뀐 갱신 뒤에도).",
        ),
        HostId::Grok => Some(
            "grok의 hook과 worker는 원본 레포의 폴더 신뢰가 필요하다(`/hooks-trust`). fullops 플러그인을 신뢰해야 Context7 MCP가 켜진다.",
        ),
        HostId::Agy => Some("agy를 다시 시작해야 플러그인이 보인다. agy의 hook은 CLI에서만 돈다."),
    }
}

/// 고른 호스트의 CLI가 PATH에 없을 때.
pub fn host_missing(program: &str) -> String {
    format!("`{program}` CLI가 PATH에 없어 건너뛴다.")
}

/// 플러그인 루트가 없어 grok·agy의 패키지를 조립하지 못할 때(설계 10.3절 "조립하는 패키지" 1번).
pub fn package_missing(host: HostId) -> String {
    format!(
        "통합 패키지 폴더를 찾지 못해 `{host}` 플러그인을 건너뛴다. 통합 패키지의 `bin/fullops install --hosts {host}`를 실행한다."
    )
}

/// 스킬 설치 도구가 PATH에 없을 때.
pub fn skill_cli_missing(program: &str) -> String {
    format!("`{program}`가 PATH에 없어 사용자 스킬 설치를 건너뛴다.")
}

/// `doctor`의 병렬 방침 행이 `ultra`일 때 내는 두 문장(설계 10.6절, 8.15절).
pub const ULTRA_NOTES: [&str; 2] = [
    "Claude Code worker를 울트라코드로 띄운다. 사용량이 빨리 준다.",
    "worker의 권한 모드가 auto나 bypass가 아니면 Workflow 실행마다 승인 창에서 멈춘다.",
];

/// `migrate`가 `subagent_level`이 없던 레포를 옮길 때(설계 13.2절, H12).
pub const PARALLEL_WAS_OFF: &str = "subagent_level이 없어 parallel = \"on\"으로 옮겼다. 1.24.0에서는 하위 위임이 꺼져 있었다. 그대로 두려면 `parallel = \"off\"`로 고친다.";

/// `migrate`가 `parallel`을 `on`으로 옮겼을 때 덧붙이는 문장.
pub const PARALLEL_HOW_TO_LIMIT: &str = "읽기 전용만 허용하려면 `read`, 끄려면 `off`로 고친다.";

/// `migrate`가 사람이 고친 `rules/delegation.md`를 그대로 둘 때.
pub const DELEGATION_KEPT: &str =
    "rules/delegation.md는 사람이 고친 흔적이 있어 두었다. 병렬 방침(8.15절)에 맞게 고친다.";
