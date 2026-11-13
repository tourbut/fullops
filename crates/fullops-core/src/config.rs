//! `fullops.toml`의 형태와 검증(설계 6.1.1절). TOML 해석은 `fullops-store`가 한다.

use crate::model::{
    CodeDef, Config, Mode, ModeTuple, ReviewPolicy, Role, RoleId, RoleKind, Routing, SubagentLevel,
    TestLevel, WaitConfig,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 이 바이너리가 받는 `schema` 값.
pub const SCHEMA: u32 = 2;

/// 내장 코드 확장자 목록(설계 8.11절 5번).
pub const CODE_EXTENSIONS: &[&str] = &[
    ".py",
    ".sh",
    ".bash",
    ".rb",
    ".gd",
    ".pl",
    ".r",
    ".js",
    ".jsx",
    ".ts",
    ".tsx",
    ".mjs",
    ".cjs",
    ".svelte",
    ".vue",
    ".go",
    ".rs",
    ".java",
    ".kt",
    ".kts",
    ".scala",
    ".c",
    ".h",
    ".cc",
    ".cpp",
    ".hpp",
    ".cs",
    ".swift",
    ".php",
    ".dart",
    ".css",
    ".scss",
    ".less",
    ".html",
    ".gdshader",
    ".shader",
];

impl Default for CodeDef {
    fn default() -> Self {
        Self {
            extensions: CODE_EXTENSIONS.iter().map(|e| e.to_string()).collect(),
        }
    }
}

/// 파일에 적힌 그대로의 형태. 모르는 키는 무시한다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawConfig {
    pub schema: u32,
    pub applied_version: String,
    pub mode: Mode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_role: Option<RoleId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_branch: Option<String>,
    pub routing: Routing,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tech_plan_role: Option<RoleId>,
    pub test_level: TestLevel,
    pub subagent_level: SubagentLevel,
    pub git: RawGit,
    #[serde(default)]
    pub roles: BTreeMap<RoleId, Role>,
    #[serde(default)]
    pub review: ReviewPolicy,
    #[serde(default)]
    pub code: CodeDef,
    #[serde(default)]
    pub wait: WaitConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawGit {
    pub base: String,
    /// 없으면 로컬 전용.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
}

/// 정책 오류의 종류. X-00의 지문과 `noticed` 키에 쓴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyKind {
    /// 파일을 읽지 못했거나 TOML로 읽히지 않는다.
    Unreadable,
    /// 6.1.1절의 검증 규칙을 어겼다.
    Invalid,
    /// 세션의 `mode_at_start`가 현재 값과 다르다.
    ModeChanged,
}

impl PolicyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PolicyKind::Unreadable => "config_unreadable",
            PolicyKind::Invalid => "config_invalid",
            PolicyKind::ModeChanged => "mode_changed",
        }
    }
}

/// 정책 오류(설계 7.8절). hook이 막는 유일한 예외다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyError {
    pub kind: PolicyKind,
    /// 사람에게 보여 줄 사유.
    pub detail: String,
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for PolicyError {}

fn invalid(detail: String) -> PolicyError {
    PolicyError {
        kind: PolicyKind::Invalid,
        detail,
    }
}

/// 6.1.1절의 검증 규칙을 적용해 `Config`로 바꾼다.
pub fn validate(raw: RawConfig) -> Result<Config, PolicyError> {
    if raw.schema != SCHEMA {
        return Err(invalid(format!(
            "schema가 {}입니다. {SCHEMA}여야 합니다.",
            raw.schema
        )));
    }
    let mut branches = BTreeSet::new();
    for (id, role) in &raw.roles {
        if !branches.insert(role.branch.as_str()) {
            return Err(invalid(format!(
                "역할 {id}의 브랜치 {}가 다른 역할과 겹칩니다.",
                role.branch
            )));
        }
    }
    for kind in [RoleKind::Design, RoleKind::Coordinator, RoleKind::Tester] {
        if raw.roles.values().filter(|r| r.kind == kind).count() > 1 {
            return Err(invalid(format!(
                "kind = \"{kind}\"인 역할이 둘 이상입니다."
            )));
        }
    }
    if raw.mode == Mode::Dev {
        match &raw.primary_role {
            None => return Err(invalid("dev 모드에는 primary_role이 필요합니다.".into())),
            Some(role) if !raw.roles.contains_key(role) => {
                return Err(invalid(format!(
                    "primary_role {role}은 등록된 역할이 아닙니다."
                )));
            }
            Some(_) => {}
        }
        if raw.primary_branch.as_deref().is_none_or(str::is_empty) {
            return Err(invalid("dev 모드에는 primary_branch가 필요합니다.".into()));
        }
    }
    if raw.routing == Routing::Split {
        let design = raw
            .roles
            .iter()
            .find(|(_, r)| r.kind == RoleKind::Design)
            .map(|(id, _)| id);
        match &raw.tech_plan_role {
            None => {
                return Err(invalid(
                    "routing = \"split\"에는 tech_plan_role이 필요합니다.".into(),
                ));
            }
            Some(role) if Some(role) == design => {
                return Err(invalid(format!(
                    "tech_plan_role {role}은 design 역할과 달라야 합니다."
                )));
            }
            Some(_) => {}
        }
    }
    Ok(Config {
        schema: raw.schema,
        applied_version: raw.applied_version,
        mode: raw.mode,
        primary_role: raw.primary_role,
        primary_branch: raw.primary_branch,
        routing: raw.routing,
        tech_plan_role: raw.tech_plan_role,
        test_level: raw.test_level,
        subagent_level: raw.subagent_level,
        base: raw.git.base,
        remote: raw.git.remote,
        roles: raw.roles,
        review: raw.review,
        code: raw.code,
        wait: raw.wait,
    })
}

/// 세션이 시작한 뒤 모드가 바뀌었으면 정책 오류다(설계 7.8절).
pub fn mode_changed(config: &Config, at_start: &ModeTuple) -> Option<PolicyError> {
    (config.mode_tuple() != *at_start).then(|| PolicyError {
        kind: PolicyKind::ModeChanged,
        detail: "세션이 시작한 뒤 fullops.toml의 mode·primary_role·primary_branch가 바뀌었습니다."
            .into(),
    })
}

/// 코드 파일인가(설계 8.11절 5번). 확장자가 목록에 있고 `exclude`에 안 맞는다.
pub fn is_code(path: &str, code: &CodeDef, exclude: &globset::GlobSet) -> bool {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let Some(dot) = name.rfind('.') else {
        return false;
    };
    let ext = &name[dot..];
    code.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)) && !exclude.is_match(path)
}
