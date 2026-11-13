//! 의존 설치의 계획(설계 10.3절, 9.5절). 순수 함수다. 묶음 F가 채운다.
//! `Deps` 타입은 `model`에 있고 파일 읽기·TOML 해석·필드 검증은 `fullops-store::load_deps`가 한다.
//! 호스트 CLI의 명령 문법 표(`HOST_COMMANDS`, 9.5절)도 F가 이 파일에 둔다.

use crate::model::{Deps, HostId, Known, Refusal};
use std::collections::BTreeMap;
use std::path::Path;

/// 설치 계획. `--dry-run`은 이 계획의 명령 줄만 낸다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// 호스트별 `plugin` 단계와 `deps` 단계의 플러그인 명령. 마켓플레이스 출처 충돌이면 그 호스트만 `Err`다.
    pub per_host: BTreeMap<HostId, Result<Vec<Step>, Refusal>>,
    /// 스킬 묶음마다 하나. 호스트를 가로지른다(`Step.hosts`가 대상 호스트).
    pub skills: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub stage: InstallStage,
    pub kind: StepKind,
    pub argv: Vec<String>,
    /// 사람에게 보여 주는 한 줄. 실패하면 설치 기록의 `failed`에 들어간다.
    pub what: String,
    /// `per_host`의 Step은 그 호스트 하나, `skills`의 Step은 `--agent`에 실은 호스트 전부.
    pub hosts: Vec<HostId>,
}

/// `--only plugin|deps`가 이 값으로 거른다. `SkillInstall`은 항상 `Deps`다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStage {
    Plugin,
    Deps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    MarketAdd,
    MarketUpdate,
    PluginInstall,
    SelfInstall,
    SkillInstall,
    /// grok·agy 패키지 조립. `argv = [통합 패키지 폴더, 대상 폴더]`. 프로세스를 띄우지 않는다.
    Assemble,
    /// grok의 hook 등록 파일. `argv = [대상 경로, 실행본 경로]`. 프로세스를 띄우지 않는다.
    HookFile,
}

/// 대상 호스트 전체의 계획을 한 번에 만든다. `registered`는 호스트 → 등록된 마켓플레이스(이름 → 출처)이고
/// 키가 없거나 `Unknown`이면 "모른다"다. `package_dir`은 조립한 패키지의 자리 `~/.fullops/package/<버전>`이고
/// `None`이면 grok·agy의 `Assemble`과 `SelfInstall`을 넣지 않는다(grok의 `HookFile`은 넣는다).
/// 뼈대의 임시 구현은 빈 계획이다.
pub fn install_plan(
    _deps: &Deps,
    _hosts: &[HostId],
    _registered: &BTreeMap<HostId, Known<BTreeMap<String, String>>>,
    _package_dir: Option<&Path>,
) -> Plan {
    Plan::default()
}

/// grok의 hook 등록 파일 `<GROK_HOME 또는 ~/.grok>/hooks/fullops.json`의 내용(설계 10.4절). 임시 구현은 빈 객체다.
pub fn grok_hook_json(_fullops_bin: &Path) -> String {
    "{}".to_string()
}
