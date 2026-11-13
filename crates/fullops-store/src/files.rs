//! 파일 읽기·쓰기의 공통 부분과 커밋되는 기록의 입출력(설계 6.1·6.5절).

use crate::repo::Repo;
use fullops_core::config::{PolicyError, PolicyKind, RawConfig, validate};
use fullops_core::frontmatter::{inbox_of, parse_front_matter};
use fullops_core::model::{
    ArchiveLine, Baseline, Config, Current, Deps, DepsState, HostId, Inbox, Known, LintPass, Plan,
    RoleId, Route, TaskKey,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 지금 시각(에폭 초).
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// 파일 글. 없으면 `Yes(None)`, 읽지 못하면 `Unknown("io")`.
pub fn read_text(path: &Path) -> Known<Option<String>> {
    // Windows에서 원자 교체와 겹치면 잠깐 열리지 않을 수 있어 몇 번 다시 읽는다
    for attempt in 0..3 {
        match std::fs::read_to_string(path) {
            Ok(text) => return Known::Yes(Some(text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Known::Yes(None),
            Err(_) if attempt < 2 => std::thread::sleep(Duration::from_millis(5)),
            Err(_) => {}
        }
    }
    Known::Unknown("io")
}

/// JSON 파일. 없으면 `Yes(None)`, 읽지 못하거나 형식이 다르면 `Unknown`.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Known<Option<T>> {
    match read_text(path) {
        Known::Yes(Some(text)) => match serde_json::from_str(&text) {
            Ok(value) => Known::Yes(Some(value)),
            Err(_) => Known::Unknown("format"),
        },
        Known::Yes(None) => Known::Yes(None),
        Known::Unknown(why) => Known::Unknown(why),
    }
}

/// `<경로>.tmp.<pid>`에 쓰고 `sync_all` 한 뒤 `rename`으로 바꾼다(설계 6.5절).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp.{}", std::process::id()));
    let tmp = Path::new(&tmp);
    let mut file = std::fs::File::create(tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    // Windows에서는 읽는 쪽이 파일을 잡고 있으면 rename이 실패할 수 있다
    let retries = if cfg!(windows) { 3 } else { 0 };
    let mut tried = 0;
    loop {
        match std::fs::rename(tmp, path) {
            Ok(()) => return Ok(()),
            Err(_) if tried < retries => {
                tried += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                let _ = std::fs::remove_file(tmp);
                return Err(e);
            }
        }
    }
}

/// 값을 JSON으로 원자 교체한다.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)
}

/// JSONL 파일 끝에 한 줄을 `write` 한 번으로 더한다. 잠그지 않는다.
pub fn append_line<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut line = serde_json::to_vec(value).map_err(io::Error::other)?;
    line.push(b'\n');
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(&line)
}

/// `fullops.toml` 글을 해석하고 검증한다. 기준 시점 blob에도 쓴다.
pub fn parse_config(text: &str) -> Result<Config, PolicyError> {
    let raw: RawConfig = toml::from_str(text).map_err(|e| PolicyError {
        kind: PolicyKind::Unreadable,
        detail: format!("fullops.toml을 읽을 수 없습니다: {}", e.message()),
    })?;
    validate(raw)
}

/// 기준 시점(merge-base 커밋)의 검사 기준(설계 6.1.1절). `toml_text`는 그 커밋의 `fullops.toml` blob이고 없으면 `None`이다.
/// 글이 없으면 내장 기본값, 읽히지 않거나 검증을 어기면 내장 기본값과 `fell_back = true`다.
pub fn baseline(toml_text: Option<&str>) -> Baseline {
    fullops_core::config::baseline(toml_text.map(parse_config))
}

/// 작업 트리의 `fullops.toml`을 읽는다. 실패는 모두 정책 오류다(설계 7.8절).
pub fn load_config(repo: &Repo) -> Result<Config, PolicyError> {
    let text = std::fs::read_to_string(repo.config_path()).map_err(|e| PolicyError {
        kind: PolicyKind::Unreadable,
        detail: format!("fullops.toml을 읽을 수 없습니다: {e}"),
    })?;
    parse_config(&text)
}

/// 이 체크아웃의 역할 인박스. 파일이 없으면 `Yes(None)`, front matter가 문법에 안 맞으면 `Unknown("inbox_syntax")`.
pub fn read_inbox(repo: &Repo, role: &RoleId) -> Known<Option<Inbox>> {
    match read_text(&repo.inbox_path(role)) {
        Known::Yes(Some(text)) => match parse_front_matter(&text)
            .ok()
            .and_then(|(fm, _)| inbox_of(&fm))
        {
            Some(inbox) => Known::Yes(Some(inbox)),
            None => Known::Unknown("inbox_syntax"),
        },
        Known::Yes(None) => Known::Yes(None),
        Known::Unknown(why) => Known::Unknown(why),
    }
}

/// 이 체크아웃의 `PLANS.md`. 파일이 없으면 빈 계획이다. `template`은 내장 `PLANS.md` 템플릿이다.
pub fn read_plan(repo: &Repo, template: &str) -> Known<Plan> {
    read_text(&repo.plan_path())
        .map(|text| fullops_core::parse_plan(&text.unwrap_or_default(), template))
}

pub fn read_route(repo: &Repo, key: &TaskKey) -> Known<Option<Route>> {
    read_json(&repo.route_path(key))
}

/// `routes/<KEY>.json`을 쓴다. 커밋하지 않는다.
pub fn write_route(repo: &Repo, route: &Route) -> io::Result<()> {
    write_json(&repo.route_path(&route.key), route)
}

/// 이 워크트리의 `lint/pass.json`.
pub fn read_lint_pass(repo: &Repo) -> Known<Option<LintPass>> {
    read_json(&repo.lint_dir().join("pass.json"))
}

/// `archive.jsonl`의 모든 줄. 해석하지 못한 줄은 건너뛴다. 파일이 영구히 자라므로 잠금 안과 hook에서는 부르지 않는다
/// (`fullops status`와 `merge`가 지표를 계산할 때 잠금 밖에서 부른다).
pub fn read_archive(repo: &Repo) -> Known<Vec<ArchiveLine>> {
    read_text(&repo.archive_path()).map(|text| {
        text.unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    })
}

/// `stage_of`에 넣는 키 집합: `landed_at`이 있는 과제 줄의 키. 줄마다 두 필드만 뽑는다(줄 전체를 타입으로 풀지 않는다).
pub fn archive_keys(repo: &Repo) -> Known<BTreeSet<TaskKey>> {
    #[derive(serde::Deserialize)]
    struct Line {
        key: TaskKey,
        landed_at: Option<i64>,
    }
    read_text(&repo.archive_path()).map(|text| {
        text.unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<Line>(line).ok())
            .filter(|line| line.landed_at.is_some())
            .map(|line| line.key)
            .collect()
    })
}

pub fn append_archive(repo: &Repo, line: &ArchiveLine) -> io::Result<()> {
    append_line(&repo.archive_path(), line)
}

/// `current.json`. CLI가 자기 세션을 찾는 데 쓴다(설계 8.1절).
pub fn read_current(repo: &Repo) -> Known<Option<Current>> {
    read_json(&repo.current_path())
}

pub fn write_current(repo: &Repo, current: &Current) -> io::Result<()> {
    write_json(&repo.current_path(), current)
}

/// 사용자 홈. Windows는 `USERPROFILE`, 그 밖은 `HOME`이다(설계 6.4절).
pub fn home() -> Option<PathBuf> {
    let name = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// `~/.fullops/`. 홈을 모르면 `None`.
pub fn fullops_home() -> Option<PathBuf> {
    home().map(|h| h.join(".fullops"))
}

/// 고정 위치 실행본 `~/.fullops/bin/fullops[.exe]`(설계 10.3절). 홈을 모르면 `None`.
pub fn fixed_bin() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "fullops.exe"
    } else {
        "fullops"
    };
    fullops_home().map(|home| home.join("bin").join(name))
}

/// `deps.toml`을 읽지 못한 사유(설계 10.2절).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepsError {
    /// TOML 오류이거나 필수 필드가 없다.
    Parse(String),
    /// `schema != 1`. 내장 사본으로 넘어가지 않는다.
    Schema(u32),
    /// 10.7절의 필드 규칙을 어겼다.
    Invalid(String),
}

impl std::fmt::Display for DepsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DepsError::Parse(why) => write!(f, "deps.toml을 읽을 수 없습니다: {why}"),
            DepsError::Schema(n) => write!(f, "deps.toml의 schema가 {n}입니다. 1이어야 합니다."),
            DepsError::Invalid(why) => write!(f, "deps.toml이 규칙에 맞지 않습니다: {why}"),
        }
    }
}

impl std::error::Error for DepsError {}

/// `deps.toml` 글을 해석하고 10.7절의 필드 규칙을 검증한다.
pub fn parse_deps(text: &str) -> Result<Deps, DepsError> {
    let table: toml::Table =
        toml::from_str(text).map_err(|e| DepsError::Parse(e.message().into()))?;
    match table.get("schema").and_then(toml::Value::as_integer) {
        Some(1) => {}
        Some(other) => return Err(DepsError::Schema(u32::try_from(other).unwrap_or(u32::MAX))),
        None => return Err(DepsError::Parse("schema가 없습니다".into())),
    }
    let deps: Deps = table
        .try_into()
        .map_err(|e: toml::de::Error| DepsError::Parse(e.message().into()))?;
    let invalid = |why: String| Err(DepsError::Invalid(why));
    if deps.skill_cli.is_empty() {
        return invalid("skill_cli가 비어 있습니다".into());
    }
    for plugin in &deps.plugins {
        let known = plugin.id.split_once('@').is_some_and(|(name, market)| {
            !name.is_empty()
                && (deps.marketplaces.contains_key(market) || market == deps.own.marketplace)
        });
        if !known {
            return invalid(format!(
                "[[plugins]] {}의 마켓플레이스가 [marketplaces]에 없습니다",
                plugin.id
            ));
        }
        if plugin.hosts.contains(&HostId::Agy) {
            return invalid(format!(
                "[[plugins]] {}의 hosts에 agy는 쓸 수 없습니다",
                plugin.id
            ));
        }
    }
    if let Some(mcp) = deps.mcp.iter().find(|m| !m.url.starts_with("https://")) {
        return invalid(format!(
            "[[mcp]] {}의 url이 https://로 시작하지 않습니다",
            mcp.name
        ));
    }
    if let Some(item) = deps
        .optional
        .iter()
        .find(|o| !matches!(o.kind.as_str(), "tool" | "skill"))
    {
        return invalid(format!(
            "[[optional]] {}의 kind는 tool 또는 skill입니다",
            item.name
        ));
    }
    Ok(deps)
}

/// 의존 목록을 10.7절의 순서로 찾아 읽는다: (1) 플러그인 루트의 `deps.toml` (2) `~/.fullops/deps.toml` (3) 내장 사본.
/// `builtin`은 바이너리에 내장한 빌드 시점 사본의 글이다(store는 바이너리의 자산을 모른다). 내장 사본을 썼으면 경로는 비어 있다.
/// 찾은 파일이 규칙에 맞지 않으면 다음 자리로 넘어가지 않고 오류를 돌려준다.
pub fn load_deps(plugin_root: Option<&Path>, builtin: &str) -> Result<(Deps, PathBuf), DepsError> {
    let places = [
        plugin_root.map(|root| root.join("deps.toml")),
        fullops_home().map(|home| home.join("deps.toml")),
    ];
    for path in places.into_iter().flatten() {
        match std::fs::read_to_string(&path) {
            Ok(text) => return parse_deps(&text).map(|deps| (deps, path)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(DepsError::Parse(format!("{}: {e}", path.display()))),
        }
    }
    parse_deps(builtin).map(|deps| (deps, PathBuf::new()))
}

fn deps_state_path() -> Option<PathBuf> {
    fullops_home().map(|home| home.join("deps-state.json"))
}

/// `~/.fullops/deps-state.json`. 파일이 없으면 빈 기록이다.
pub fn read_deps_state() -> Known<DepsState> {
    let Some(path) = deps_state_path() else {
        return Known::Unknown("no_home");
    };
    read_json(&path).map(|state| {
        state.unwrap_or(DepsState {
            v: 1,
            manifest_sha256: String::new(),
            hosts: Default::default(),
        })
    })
}

pub fn write_deps_state(state: &DepsState) -> io::Result<()> {
    let path = deps_state_path()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "사용자 홈을 찾지 못했습니다"))?;
    write_json(&path, state)
}
