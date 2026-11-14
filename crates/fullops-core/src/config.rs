//! `fullops.toml`의 형태와 검증(설계 6.1.1절). TOML 해석은 `fullops-store`가 한다.

use crate::model::{
    Baseline, CodeDef, Config, Mode, ModeTuple, Parallel, ReviewPolicy, Role, RoleId, RoleKind,
    Routing, TestLevel, WaitConfig,
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
    pub parallel: Parallel,
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

/// git 원격 이름으로만 읽히는 값인가. 영문·숫자로 시작하고 영문·숫자·`._-/`만 쓴다.
/// `-`로 시작하면 git이 옵션으로, `:`·`.`·`/`로 시작하거나 `..`이 있으면 URL·경로·`ext::`로 읽는다.
pub fn valid_remote(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-/".contains(c))
        && !name.contains("..")
        && !name.contains("//")
        && !name.ends_with('/')
}

/// `git check-ref-format --branch`가 받는 브랜치 이름인가. `-`로 시작하면 git이 옵션으로 읽는다.
pub fn valid_branch(name: &str) -> bool {
    let piece = |p: &str| !p.is_empty() && !p.starts_with('.') && !p.ends_with(".lock");
    !name.starts_with('-')
        && name != "@"
        && !name.ends_with('.')
        && !name.contains("..")
        && !name.contains("@{")
        && !name
            .chars()
            .any(|c| c.is_control() || " ~^:?*[\\".contains(c))
        && name.split('/').all(piece)
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
        if !valid_branch(&role.branch) {
            return Err(invalid(format!(
                "역할 {id}의 브랜치 `{}`는 git 브랜치 이름이 아닙니다.",
                role.branch
            )));
        }
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
    // 이 값들은 git의 인자로 간다. 커밋되는 파일이라 worker가 고칠 수 있다
    for (name, branch) in [
        ("[git].base", Some(&raw.git.base)),
        ("primary_branch", raw.primary_branch.as_ref()),
    ] {
        if let Some(branch) = branch.filter(|b| !valid_branch(b)) {
            return Err(invalid(format!(
                "{name} `{branch}`는 git 브랜치 이름이 아닙니다."
            )));
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
            Some(role) if !raw.roles.contains_key(role) => {
                return Err(invalid(format!(
                    "tech_plan_role {role}은 등록된 역할이 아닙니다."
                )));
            }
            Some(role) if Some(role) == design => {
                return Err(invalid(format!(
                    "tech_plan_role {role}은 design 역할과 달라야 합니다."
                )));
            }
            Some(_) => {}
        }
    }
    if raw.git.remote.as_deref() == Some("") {
        return Err(invalid(
            "[git].remote가 빈 문자열입니다. 로컬 전용이면 키를 지웁니다.".into(),
        ));
    }
    if let Some(remote) = raw.git.remote.as_ref().filter(|r| !valid_remote(r)) {
        return Err(invalid(format!(
            "[git].remote `{remote}`는 원격 이름이 아닙니다. 영문·숫자로 시작하고 영문·숫자·`._-/`만 씁니다(URL과 경로는 쓸 수 없습니다)."
        )));
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
        parallel: raw.parallel,
        base: raw.git.base,
        remote: raw.git.remote,
        roles: raw.roles,
        review: raw.review,
        code: raw.code,
        wait: raw.wait,
    })
}

/// 새 레포의 `test_level` 기본값(설계 10.6절). 기준 시점에 설정이 없을 때도 이 값을 쓴다.
pub const DEFAULT_TEST_LEVEL: TestLevel = TestLevel::Lite;

/// 기준 시점(merge-base 커밋)의 검사 기준을 고른다(설계 6.1.1절). `parsed`는 그 커밋의 `fullops.toml`을 해석한 결과이고
/// 파일이 없으면 `None`이다. 없으면 내장 기본값, 읽히지 않거나 검증을 어기면 내장 기본값과 `fell_back = true`다.
/// TOML 글을 받는 `baseline(toml_text)`는 `fullops-store`에 있다(core는 `toml`을 쓰지 않는다).
pub fn baseline(parsed: Option<Result<Config, PolicyError>>) -> Baseline {
    match parsed {
        Some(Ok(config)) => Baseline {
            review: config.review,
            code: config.code,
            test_level: config.test_level,
            fell_back: false,
        },
        other => Baseline {
            review: ReviewPolicy::default(),
            code: CodeDef::default(),
            test_level: DEFAULT_TEST_LEVEL,
            fell_back: other.is_some(),
        },
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    // R2-1: fullops.toml의 원격과 브랜치는 git의 인자로 간다. 옵션·URL·경로로 읽힐 값은 받지 않는다
    #[test]
    fn remote_and_branch_names_cannot_be_options_urls_or_paths() {
        for name in ["origin", "up-stream", "team/fork", "a.b_c"] {
            assert!(valid_remote(name), "{name}");
        }
        for name in [
            "",
            "-u",
            "--upload-pack=touch x",
            "ext::sh -c touch% x",
            "https://example.com/a.git",
            "git@example.com:a.git",
            "../planted.git",
            "./planted.git",
            "/abs/planted.git",
            "a//b",
            "a/",
            "C:\\planted.git",
            "원격",
        ] {
            assert!(!valid_remote(name), "{name}");
        }
        for name in ["main", "fullops/dev", "release-1.2", "기능/결제", "+main"] {
            assert!(valid_branch(name), "{name}");
        }
        for name in [
            "",
            "-main",
            "--upload-pack=touch x",
            "a..b",
            "a b",
            "a:b",
            "a~1",
            "a^",
            "a?",
            "a*",
            "a[b",
            "a\\b",
            "/main",
            "main/",
            "a//b",
            ".hidden",
            "a/.b",
            "a.lock",
            "main.",
            "@",
            "a@{1}",
            "a\nb",
        ] {
            assert!(!valid_branch(name), "{name:?}");
        }
    }
}
