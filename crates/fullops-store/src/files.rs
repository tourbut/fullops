//! 파일 읽기·쓰기의 공통 부분과 커밋되는 기록의 입출력(설계 6.1·6.5절).

use crate::repo::Repo;
use fullops_core::config::{PolicyError, PolicyKind, RawConfig, validate};
use fullops_core::frontmatter::{inbox_of, parse_front_matter};
use fullops_core::model::{
    ArchiveLine, Config, Current, Inbox, Known, LintPass, Plan, RoleId, Route, TaskKey,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::Path;
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

/// `archive.jsonl`의 모든 줄. 해석하지 못한 줄은 건너뛴다.
pub fn read_archive(repo: &Repo) -> Known<Vec<ArchiveLine>> {
    read_text(&repo.archive_path()).map(|text| {
        text.unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    })
}

/// `stage_of`에 넣는 키 집합: `landed_at`이 있는 과제 줄의 키.
pub fn archive_keys(lines: &[ArchiveLine]) -> BTreeSet<TaskKey> {
    lines
        .iter()
        .filter_map(|line| match line {
            ArchiveLine::Task(task) if task.landed_at.is_some() => Some(task.key.clone()),
            _ => None,
        })
        .collect()
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
