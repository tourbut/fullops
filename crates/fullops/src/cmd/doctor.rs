//! `fullops doctor`: 설치와 레포 상태 점검(설계 10.6절의 표). 네트워크 없이 본다. 활성 레포가 아니어도 설치 항목은 본다.
//! 호스트 CLI에는 `--version`과 목록 조회만 부르고 호스트의 설정 파일은 읽기만 한다.

use super::install;
use crate::cli::{self, CliError, Ctx, EXIT_OK};
use fullops_core::deps::ALL_HOSTS;
use fullops_core::model::{HostId, Known, Parallel, TraceLine, same_path};
use fullops_ext::{Git, GitCli, hosts};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

const DAY: i64 = 24 * 60 * 60;

/// 글에서 처음 나오는 `a.b.c` 버전.
fn version_of(text: &str) -> Option<(u64, u64, u64)> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .find_map(|word| {
            let mut parts = word.split('.').filter(|p| !p.is_empty());
            let major = parts.next()?.parse().ok()?;
            let minor = parts.next()?.parse().ok()?;
            Some((
                major,
                minor,
                parts.next().and_then(|p| p.parse().ok()).unwrap_or(0),
            ))
        })
}

/// 버전이 `min`보다 낮은가. 읽지 못한 버전은 낮다고 하지 않는다.
fn below(text: &str, min: (u64, u64, u64)) -> bool {
    version_of(text).is_some_and(|v| v < min)
}

struct Report {
    items: Vec<Value>,
    lines: Vec<String>,
}

impl Report {
    /// 항목 하나. `notes`는 알릴 것이고 비어 있으면 문제없음이다.
    fn item(&mut self, name: &str, detail: String, notes: Vec<String>) {
        let mark = if notes.is_empty() { "정상" } else { "알림" };
        self.lines.push(format!("[{mark}] {name}: {detail}"));
        self.lines.extend(notes.iter().map(|n| format!("    {n}")));
        self.items.push(
            json!({ "item": name, "ok": notes.is_empty(), "detail": detail, "notes": notes }),
        );
    }
}

fn runtime(report: &mut Report, root: Option<&Path>) {
    let mut notes = Vec::new();
    let fixed = fullops_store::fixed_bin();
    let record: Option<Value> = fullops_store::fullops_home().and_then(|home| {
        fullops_store::read_json(&home.join("install.json"))
            .yes()
            .flatten()
    });
    let installed = record.as_ref().and_then(|r| r["version"].as_str());
    match &fixed {
        Some(path) if path.is_file() => {}
        _ => notes.push("고정 위치 실행본이 없다. `fullops install --only runtime`".to_string()),
    }
    if let (Some(root), Some(installed)) = (root, installed) {
        let package = install::package_version(Some(root));
        if version_of(installed) < version_of(&package) {
            notes.push(format!(
                "고정 위치 실행본({installed})이 플러그인({package})보다 낮다. `fullops install --only runtime`"
            ));
        }
    }
    let detail = format!(
        "이 실행본 {} / 고정 위치 {} / 플러그인 루트 {}",
        env!("CARGO_PKG_VERSION"),
        installed.unwrap_or("없음"),
        root.map_or("없음".to_string(), |r| r.display().to_string()),
    );
    report.item("실행본", detail, notes);
}

fn tools(report: &mut Report) {
    let mut notes = Vec::new();
    let mut parts = Vec::new();
    match hosts::program_version("git") {
        Known::Yes(version) => {
            if below(&version, (2, 41, 0)) {
                notes.push(format!("git이 낮다({version}). 2.41 이상이 필요하다."));
            }
            parts.push(version);
        }
        Known::Unknown(_) => notes.push("git을 찾지 못했다(2.41 이상이 필요하다).".to_string()),
    }
    let orca = fullops_ext::orca_program();
    let out = hosts::find_program(&orca).map(|found| {
        fullops_ext::run(
            &[found.to_string_lossy().into_owned(), "--version".into()],
            &std::env::temp_dir(),
            Duration::from_secs(30),
            &[],
            None,
        )
    });
    let version = out.as_ref().map(|o| o.stdout_text()).unwrap_or_default();
    if out.is_some_and(|o| o.ok()) && version_of(&version).is_some() {
        if below(&version, (1, 4, 256)) {
            notes.push(format!(
                "orca가 낮다({}). 1.4.256 이상이 필요하다.",
                version.trim()
            ));
        }
        parts.push(format!("orca {}", version.trim()));
    } else {
        notes.push(format!(
            "orca(`{orca}`)의 버전을 읽지 못했다(1.4.256 이상이 필요하다)."
        ));
    }
    if hosts::find_program("curl").is_some() {
        parts.push("curl 있음".into());
    } else {
        notes.push("curl이 없다. Jev 분류와 탐색이 로컬 후보로만 돈다.".to_string());
    }
    report.item("바깥 도구", parts.join(", "), notes);
}

/// PATH에 있는 호스트를 돌려준다.
fn host_row(report: &mut Report) -> Vec<HostId> {
    let mut notes = Vec::new();
    let mut parts = Vec::new();
    let mut present = Vec::new();
    for host in ALL_HOSTS {
        if !hosts::host_available(host) {
            parts.push(format!("{host} 없음"));
            continue;
        }
        present.push(host);
        // 폐쇄망에서는 호스트 CLI를 부르지 않고 PATH에 있는지만 본다
        let version = (!install::offline())
            .then(|| hosts::host_version(host).yes())
            .flatten()
            .unwrap_or_else(|| "버전 모름".into());
        if host == HostId::Claude && below(&version, (2, 1, 295)) {
            notes.push(format!(
                "Claude Code가 낮다({version}). hook 등록의 onFailure 때문에 2.1.295 이상이 필요하다."
            ));
        }
        parts.push(format!("{host} {version}"));
    }
    report.item("호스트", parts.join(", "), notes);
    present
}

fn deps_row(report: &mut Report, root: Option<&Path>, present: &[HostId]) {
    match install::load_deps(root) {
        Ok((deps, sha)) => {
            let found = install::check(&deps, &sha, present);
            let mut detail = format!("확인한 호스트 {}개", present.len());
            if !found.optional.is_empty() {
                detail.push_str(&format!("\n    {}", found.optional.join("\n    ")));
            }
            report.item("의존", detail, found.missing);
        }
        Err(CliError::Internal(why)) => report.item("의존", "deps.toml".into(), vec![why]),
        Err(_) => report.item("의존", "deps.toml".into(), vec!["읽지 못했다.".to_string()]),
    }
}

/// 이 레포의 모든 워크트리의 세션 폴더.
fn session_dirs(ctx: &Ctx) -> Vec<PathBuf> {
    let Ok(repo) = ctx.repo() else {
        return Vec::new();
    };
    let mut dirs = vec![repo.local().join("sessions")];
    for tree in GitCli::cli(repo.root.clone())
        .worktrees()
        .yes()
        .unwrap_or_default()
    {
        if let Ok(Some(other)) = fullops_store::discover(&tree.path) {
            dirs.push(other.local().join("sessions"));
        }
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

fn session_files(dirs: &[PathBuf]) -> Vec<(PathBuf, Value)> {
    dirs.iter()
        .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .filter_map(|path| {
            let value = fullops_store::read_json(&path).yes().flatten()?;
            Some((path, value))
        })
        .collect()
}

fn codex_trust(report: &mut Report, sessions: &[(PathBuf, Value)]) {
    let installed = fullops_store::read_deps_state()
        .yes()
        .is_some_and(|s| s.hosts.contains_key(&HostId::Codex));
    if !installed {
        return;
    }
    let ran = sessions.iter().any(|(_, s)| s["host"] == "codex");
    let notes = if ran {
        Vec::new()
    } else {
        vec![
            "Codex에서 fullops hook이 돈 기록이 없다. Codex를 이 레포에서 쓴다면 `/hooks`에서 fullops hook을 신뢰했는지 확인한다."
                .to_string(),
        ]
    };
    report.item(
        "Codex hook 신뢰",
        format!("세션 기록 {}개", sessions.len()),
        notes,
    );
}

fn grok_home() -> Option<PathBuf> {
    install::grok_hook_file()?
        .parent()?
        .parent()
        .map(Path::to_path_buf)
}

/// `trusted_folders.toml`에서 `trusted = true`인 폴더. 읽기만 한다.
fn grok_trusted() -> Vec<String> {
    let text = grok_home()
        .and_then(|home| std::fs::read_to_string(home.join("trusted_folders.toml")).ok())
        .unwrap_or_default();
    let table: toml::Table = toml::from_str(&text).unwrap_or_default();
    table
        .get("folders")
        .and_then(|f| f.as_table())
        .map(|folders| {
            folders
                .iter()
                .filter(|(_, v)| v.get("trusted").and_then(|t| t.as_bool()) == Some(true))
                .map(|(path, _)| path.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn grok_rows(report: &mut Report, ctx: &Ctx) {
    // 폴더 신뢰: 워크트리는 원본 체크아웃 경로로 본다
    if let Ok(repo) = ctx.repo() {
        let origin = repo.common_dir.parent().unwrap_or(&repo.root).to_path_buf();
        let trusted = grok_trusted();
        let covered = origin
            .ancestors()
            .any(|dir| trusted.iter().any(|t| same_path(t, &dir.to_string_lossy())));
        let notes = if covered {
            Vec::new()
        } else {
            vec!["grok worker는 폴더 신뢰가 필요하다. 사용자가 grok에서 승인한다.".to_string()]
        };
        report.item("grok 폴더 신뢰", origin.display().to_string(), notes);
    }
    let file = install::grok_hook_file();
    let shown = file
        .as_ref()
        .map_or("없음".to_string(), |f| f.display().to_string());
    let notes = match install::grok_hook_command() {
        None => vec!["grok의 hook 등록 파일이 없다. `fullops install --hosts grok`".to_string()],
        Some(bin) if !bin.is_file() => vec![format!(
            "hook 등록 파일이 가리키는 실행본({})이 없다. grok에서 hook이 돌지 않는다. `fullops install --hosts grok`",
            bin.display()
        )],
        Some(_) => Vec::new(),
    };
    report.item("grok hook 등록", shown, notes);
}

fn agy_row(report: &mut Report, active: bool) {
    let Some(record) = fullops_store::read_deps_state()
        .yes()
        .and_then(|s| s.hosts.get(&HostId::Agy).cloned())
    else {
        return;
    };
    let mut notes = Vec::new();
    if version_of(&record.self_version) < version_of(env!("CARGO_PKG_VERSION")) {
        notes.push(
            "agy의 fullops 플러그인과 그 hook 실행본이 낡았다. `fullops install --hosts agy`"
                .to_string(),
        );
    }
    let mut detail = format!("설치한 버전 {}", record.self_version);
    if active {
        detail.push_str(". agy의 hook은 CLI에서만 돈다");
    }
    report.item("agy 패키지", detail, notes);
}

fn repo_rows(report: &mut Report, ctx: &Ctx) {
    let Ok(repo) = ctx.repo() else {
        report.item(
            "레포",
            "활성 레포가 아니다(설치 항목만 봤다)".into(),
            Vec::new(),
        );
        return;
    };
    let mut notes = Vec::new();
    match ctx.config() {
        Ok(config) => {
            let parallel = config.parallel;
            let hint = if parallel == Parallel::Ultra {
                fullops_core::messages::install::ULTRA_NOTES
                    .map(String::from)
                    .to_vec()
            } else {
                Vec::new()
            };
            report.item("병렬 방침", format!("parallel = \"{parallel}\""), hint);
        }
        Err(CliError::Policy(e)) => notes.push(format!("fullops.toml: {e}")),
        Err(_) => notes.push("fullops.toml을 읽지 못했다.".to_string()),
    }
    if let Known::Unknown(why) = fullops_store::read_state(repo) {
        notes.push(format!("state.json을 읽지 못했다({why})."));
    }
    let shared = repo.common_dir.to_string_lossy().to_lowercase();
    let synced = [
        "onedrive",
        "dropbox",
        "google drive",
        "googledrive",
        "icloud",
    ]
    .iter()
    .any(|name| shared.contains(name));
    if synced || shared.starts_with("\\\\") || shared.starts_with("//") {
        notes.push(
            "Git 공용 디렉터리가 네트워크·동기화 폴더에 있다. 잠금과 원자 교체가 깨질 수 있다."
                .to_string(),
        );
    }
    report.item("레포", repo.root.display().to_string(), notes);
}

fn hook_row(report: &mut Report, dirs: &[PathBuf], sessions: &[(PathBuf, Value)]) {
    let since = fullops_store::now() - DAY;
    let mut errors = 0;
    for dir in dirs {
        let trace = dir.parent().map(|local| local.join("trace"));
        for entry in trace
            .into_iter()
            .flat_map(std::fs::read_dir)
            .flatten()
            .flatten()
        {
            let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
            errors += text
                .lines()
                .filter_map(|line| serde_json::from_str::<TraceLine>(line).ok())
                .filter(|line| line.t >= since && line.err.is_some())
                .count();
        }
    }
    // 세션 파일은 있는데 실행 표식도 추적도 없는 최근 세션
    let silent = sessions
        .iter()
        .filter(|(_, s)| s["last_seen"].as_i64().is_some_and(|t| t >= since))
        .filter(|(path, _)| {
            let seen = path.with_extension("seen");
            let trace = path
                .parent()
                .and_then(Path::parent)
                .zip(path.file_stem())
                .map(|(local, sid)| local.join("trace").join(sid).with_extension("jsonl"));
            !seen.exists() && !trace.is_some_and(|t| t.exists())
        })
        .count();
    let mut notes = Vec::new();
    if errors > 0 {
        notes.push(format!(
            "최근 24시간 hook 추적에 오류·시간 초과가 {errors}건 있다. `fullops why`로 본다."
        ));
    }
    if silent > 0 {
        notes.push(format!(
            "추적과 실행 표식이 없는 최근 세션이 {silent}개 있다."
        ));
    }
    report.item(
        "hook 실행",
        format!("오류 {errors}건, 표식 없는 세션 {silent}개"),
        notes,
    );
}

pub fn run(ctx: &Ctx, mut parser: lexopt::Parser) -> Result<u8, CliError> {
    if let Some(arg) = parser.next()? {
        return Err(arg.unexpected().into());
    }
    let root = cli::plugin_root();
    let root = root.as_deref();
    let mut report = Report {
        items: Vec::new(),
        lines: Vec::new(),
    };
    runtime(&mut report, root);
    tools(&mut report);
    let present = host_row(&mut report);
    deps_row(&mut report, root, &present);
    let active = ctx.repo().is_ok();
    let dirs = session_dirs(ctx);
    let sessions = session_files(&dirs);
    if active {
        codex_trust(&mut report, &sessions);
    }
    if present.contains(&HostId::Grok) {
        grok_rows(&mut report, ctx);
    }
    agy_row(&mut report, active);
    // Git Bash 항목은 Claude Code 등록이 shell form일 때만 본다. 지금 등록은 exec form이라 보지 않는다(설계 10.4절)
    repo_rows(&mut report, ctx);
    if active {
        hook_row(&mut report, &dirs, &sessions);
    }
    let offline = install::offline();
    let note = if offline {
        vec![
            "Jev와 설치의 네트워크 단계를 건너뛴다. 호스트 CLI의 버전과 플러그인 목록도 조회하지 않았다."
                .to_string(),
        ]
    } else {
        Vec::new()
    };
    report.item(
        "폐쇄망",
        format!("FULLOPS_OFFLINE {}", if offline { "켜짐" } else { "꺼짐" }),
        note,
    );
    ctx.print(&report.lines.join("\n"), json!({ "items": report.items }));
    Ok(EXIT_OK)
}
