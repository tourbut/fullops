//! `fullops install`: 실행본 복사, 네 호스트의 플러그인 등록, 의존 플러그인·스킬 설치와 확인(설계 10.3절, 9.5절).
//! 호스트 CLI에는 계약에서 확인된 설치·목록 명령만 넘긴다. `--trust` 같은 신뢰 인자와 `--help`는 넘기지 않는다.
//! 호스트 쪽 폴더에 직접 쓰는 파일은 grok의 hook 등록 파일 하나뿐이다.

use crate::assets;
use crate::cli::{self, CliError, Ctx, EXIT_OK, EXIT_REFUSED};
use fullops_core::deps::{self, ALL_HOSTS, InstallStage, Step, StepKind, host_commands};
use fullops_core::messages::install as msg;
use fullops_core::model::{Deps, HostId, HostInstall, Known, path_text, sha256_hex};
use fullops_ext::hosts;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 설치 명령의 제한 시간(설계 9.5절).
const INSTALL_TIMEOUT: Duration = Duration::from_secs(600);

fn fullops_home() -> io::Result<PathBuf> {
    fullops_store::fullops_home()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "사용자 홈을 찾지 못했습니다"))
}

fn fixed_bin() -> io::Result<PathBuf> {
    fullops_store::fixed_bin()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "사용자 홈을 찾지 못했습니다"))
}

/// grok의 hook 등록 파일 `<GROK_HOME 또는 ~/.grok>/hooks/fullops.json`.
pub(super) fn grok_hook_file() -> Option<PathBuf> {
    std::env::var_os("GROK_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| fullops_store::home().map(|h| h.join(".grok")))
        .map(|dir| dir.join("hooks").join("fullops.json"))
}

/// 플러그인 루트의 `VERSION`, 없으면 이 실행본의 버전.
pub(super) fn package_version(root: Option<&Path>) -> String {
    root.and_then(|r| std::fs::read_to_string(r.join("VERSION")).ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

/// 플러그인 루트에서 이 OS·CPU에 맞는 바이너리(설계 10.4절의 배치).
fn platform_binary(root: &Path) -> Option<PathBuf> {
    let name = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", _) => "fullops.exe",
        ("macos", "aarch64") => "fullops-darwin-arm64",
        ("macos", "x86_64") => "fullops-darwin-x64",
        ("linux", "x86_64") => "fullops-linux-x64",
        _ => return None,
    };
    Some(root.join("bin").join(name)).filter(|p| p.is_file())
}

/// 옛 실행본의 이름 `fullops.old-<시각>[.exe]`.
fn old_bin_name(at: i64) -> String {
    format!("fullops.old-{at}{}", std::env::consts::EXE_SUFFIX)
}

/// 옛 실행본을 옮겨 둘 빈 자리. 같은 초에 두 번 바꿔도 실행 중일 수 있는 옛 파일을 덮지 않게 시각을 올려 찾는다.
fn old_bin_path(bin_dir: &Path) -> PathBuf {
    let now = fullops_store::now();
    (now..now + 60)
        .map(|at| bin_dir.join(old_bin_name(at)))
        .find(|path| !path.exists())
        .unwrap_or_else(|| bin_dir.join(old_bin_name(now)))
}

fn same_file(a: &Path, b: &Path) -> bool {
    matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(a), Ok(b)) if a == b)
}

/// 새 파일(또는 폴더)을 제자리로 바꿔 넣는다. 있던 것은 `old`로 옮기고, 실패하면 이름을 되돌린다.
fn swap_in(new: &Path, place: &Path, old: &Path) -> io::Result<()> {
    let had = place.exists();
    if had {
        std::fs::rename(place, old)?;
    }
    std::fs::rename(new, place).inspect_err(|_| {
        if had {
            let _ = std::fs::rename(old, place);
        }
    })
}

pub struct Runtime {
    pub bin: PathBuf,
    pub version: String,
    /// 실행본을 바꿔 넣었다(이미 고정 위치의 실행본으로 실행했으면 거짓).
    pub copied: bool,
}

/// 통합 패키지(`hosts/`가 있는 플러그인 루트)를 보관할 자리 `~/.fullops/package/<버전>/source`. 디렉터리판이면 `None`.
fn kept_source(root: &Path) -> io::Result<Option<PathBuf>> {
    if !root.join("hosts").is_dir() {
        return Ok(None);
    }
    let version = package_version(Some(root));
    Ok(Some(
        fullops_home()?.join("package").join(version).join("source"),
    ))
}

/// 통합 패키지를 보관 자리로 복사한다. npx 캐시처럼 지워질 수 있는 폴더를 `install.json`의 `source`로 남기지 않게 한다.
/// 이미 같은 내용이면 건너뛴다. 디렉터리판은 사람이 둔 폴더에서 그대로 돌므로 복사하지 않고 그 자리를 적는다.
fn keep_source(root: &Path) -> io::Result<PathBuf> {
    let Some(target) = kept_source(root)? else {
        return std::path::absolute(root);
    };
    if same_file(root, &target)
        || (target.is_dir() && folder_sha256(&target)? == folder_sha256(root)?)
    {
        return Ok(target);
    }
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::other("보관할 자리가 없습니다"))?;
    std::fs::create_dir_all(parent)?;
    let staging = parent.join(format!("source.new-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    let old = parent.join(format!("source.old-{}", fullops_store::now()));
    copy_tree(root, &staging)
        .and_then(|()| swap_in(&staging, &target, &old))
        .inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&staging);
        })?;
    Ok(target)
}

/// 낮은 버전으로 바꾸려는 실행본 단계의 거절(설계 10.3절). 고정 위치 실행본이 없거나 버전을 읽지 못하면 거절하지 않는다.
fn downgrade(root: Option<&Path>) -> io::Result<Option<fullops_core::model::Refusal>> {
    let fixed = fixed_bin()?;
    // 플러그인 루트 없이 고정 위치 실행본으로 돌리면 기록의 버전을 잇는다(바꾸지 않는다)
    if !fixed.exists() || (root.is_none() && same_file(&std::env::current_exe()?, &fixed)) {
        return Ok(None);
    }
    let record: Value = fullops_store::read_json(&fullops_home()?.join("install.json"))
        .yes()
        .flatten()
        .unwrap_or(Value::Null);
    let Some(installed) = record["version"].as_str() else {
        return Ok(None);
    };
    let incoming = package_version(root);
    let lower = matches!(
        (crate::hook::version(&incoming), crate::hook::version(installed)),
        (Some(new), Some(old)) if new < old
    );
    Ok(lower.then(|| fullops_core::model::Refusal {
        rule: "install",
        reason: format!(
            "설치된 실행본({installed})보다 낮은 버전({incoming})으로 바꾸지 않는다."
        ),
        escape: "새 버전의 배포본으로 다시 설치한다. 일부러 낮추려면 `fullops install --allow-downgrade`, 바로 앞 실행본으로 되돌리려면 `fullops install --rollback`".into(),
        exit: EXIT_REFUSED,
    }))
}

/// 실행본 단계(설계 10.3절): 실행본과 `deps.toml`을 고정 위치에 두고 `install.json`을 쓴다.
/// `install --only runtime`과 S-04의 자동 설치가 함께 쓴다. 실행 중인 hook은 이름이 바뀐 옛 파일로 끝까지 돈다.
pub fn runtime_stage(plugin_root: Option<&Path>) -> io::Result<Runtime> {
    let home = fullops_home()?;
    let fixed = fixed_bin()?;
    let source = match plugin_root.and_then(platform_binary) {
        Some(binary) => binary,
        None => std::env::current_exe()?,
    };
    let record_path = home.join("install.json");
    let before: Value = fullops_store::read_json(&record_path)
        .yes()
        .flatten()
        .unwrap_or(Value::Null);
    // 실행본을 바꾸기 전에 보관한다: 보관에 실패하면 실행본과 기록이 어긋나지 않게 아무것도 바꾸지 않는다
    let source_text = match plugin_root {
        Some(root) => json!(path_text(&keep_source(root)?)),
        None => before["source"].clone(),
    };
    let copied = !same_file(&source, &fixed);
    if copied {
        let bin_dir = fixed.parent().unwrap_or(&home);
        std::fs::create_dir_all(bin_dir)?;
        let new = bin_dir.join(format!("fullops.new-{}", std::process::id()));
        std::fs::copy(&source, &new)?;
        let old = old_bin_path(bin_dir);
        swap_in(&new, &fixed, &old).inspect_err(|_| {
            let _ = std::fs::remove_file(&new);
        })?;
    }
    if let Some(deps) = plugin_root
        .map(|r| r.join("deps.toml"))
        .filter(|p| p.is_file())
    {
        fullops_store::write_atomic(&home.join("deps.toml"), &std::fs::read(deps)?)?;
    }
    let version = match plugin_root {
        Some(root) => package_version(Some(root)),
        // 플러그인 루트 없이 고정 위치의 실행본으로 실행했으면 기록의 버전을 잇는다
        None if !copied => before["version"]
            .as_str()
            .map_or_else(|| package_version(None), str::to_string),
        None => package_version(None),
    };
    let previous = if copied {
        before["version"].clone()
    } else {
        before["previous"].clone()
    };
    fullops_store::write_json(
        &record_path,
        &json!({
            "version": version,
            "sha256": sha256_hex(&std::fs::read(&fixed)?),
            "source": source_text,
            "at": fullops_store::now(),
            "previous": previous,
        }),
    )?;
    Ok(Runtime {
        bin: fixed,
        version,
        copied,
    })
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", from.display())))
}

/// grok·agy 패키지를 통합 패키지의 재료로 조립한다. 새 폴더에 복사한 뒤 제자리로 이름을 바꾼다.
fn assemble(root: &Path, host: HostId, target: &Path) -> io::Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::other("조립할 자리가 없습니다"))?;
    std::fs::create_dir_all(parent)?;
    let staging = parent.join(format!("{host}.new-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    let built = deps::package_items(host)
        .iter()
        .try_for_each(|(from, to)| copy_tree(&root.join(from), &staging.join(to)))
        .and_then(|()| {
            let old = parent.join(format!("{host}.old-{}", fullops_store::now()));
            swap_in(&staging, target, &old)
        });
    if built.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    built
}

fn files_under(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            files_under(&path, base, out)?;
        } else if let Ok(rel) = path.strip_prefix(base) {
            out.push((path_text(rel), path));
        }
    }
    Ok(())
}

/// 폴더 내용의 sha256(상대 경로와 파일 바이트를 경로 순으로 이은 것).
fn folder_sha256(dir: &Path) -> io::Result<String> {
    let mut files = Vec::new();
    files_under(dir, dir, &mut files)?;
    files.sort();
    let mut all = Vec::new();
    for (rel, path) in files {
        all.extend(rel.as_bytes());
        all.push(0);
        all.extend(std::fs::read(path)?);
        all.push(0);
    }
    Ok(sha256_hex(&all))
}

/// 스킬 명령의 `--skill` 뒤 이름들. `*`도 그대로 돌려준다.
fn skill_names(step: &Step) -> Vec<&str> {
    step.argv
        .iter()
        .skip_while(|a| *a != "--skill")
        .skip(1)
        .take_while(|a| !a.starts_with("--"))
        .map(String::as_str)
        .collect()
}

/// 스킬 폴더 `root` 아래에서 이 명령이 덮어쓸 수 있는 폴더들.
fn skills_at_risk(root: &Path, names: &[&str]) -> Vec<PathBuf> {
    if !names.contains(&"*") {
        return names.iter().map(|name| root.join(name)).collect();
    }
    // `*` 묶음은 어떤 스킬을 덮어쓸지 알 수 없어 있는 스킬을 모두 남긴다(1.24.0과 같다)
    std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|dir| dir.join("SKILL.md").is_file())
        .collect()
}

/// 스킬 백업 폴더 안에 원래 자리를 적는 파일. 되살릴 때는 복사하지 않는다.
const ORIGIN_FILE: &str = ".fullops-origin.json";

fn hash12(hash: &str) -> &str {
    hash.get(..12).unwrap_or(hash)
}

/// 스킬 폴더 하나를 `<backups>/<호스트>/<이름>/<앞 12자>/`에 한 번만 복사하고 원래 자리를 `ORIGIN_FILE`에 더한다.
fn backup_skill(backups: &Path, host: &str, dir: &Path) -> io::Result<()> {
    let Some(name) = dir.file_name() else {
        return Ok(());
    };
    let hash = folder_sha256(dir)?;
    let target = backups.join(host).join(name).join(hash12(&hash));
    let origin = path_text(dir);
    if target.exists() {
        let meta = target.join(ORIGIN_FILE);
        // 원래 자리 기록이 없는 옛 백업에는 덧붙이지 않는다(그때의 자리를 모른다)
        let Some(mut record) = fullops_store::read_json::<Value>(&meta).yes().flatten() else {
            return Ok(());
        };
        if let Some(list) = record["origins"].as_array_mut()
            && !list.iter().any(|o| o.as_str() == Some(origin.as_str()))
        {
            list.push(json!(origin));
            fullops_store::write_json(&meta, &record)?;
        }
        return Ok(());
    }
    // 반쯤 복사된 폴더가 온전한 백업으로 보이지 않게 새 폴더에 복사한 뒤 이름을 바꾼다
    let staging = target.with_extension(format!("new-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    copy_tree(dir, &staging)
        .and_then(|()| {
            fullops_store::write_json(
                &staging.join(ORIGIN_FILE),
                &json!({"origins": [origin], "at": fullops_store::now()}),
            )
        })
        .and_then(|()| std::fs::rename(&staging, &target))
        .inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&staging);
        })
}

/// 다시 설치하기 전에 있던 스킬 폴더를 `~/.fullops/skill-backups/<호스트>/<이름>/<앞 12자>/`에 한 번만 복사한다.
fn backup_skills(step: &Step) -> io::Result<()> {
    let backups = fullops_home()?.join("skill-backups");
    let names = skill_names(step);
    for &host in &step.hosts {
        for root in hosts::skill_roots(host) {
            for dir in skills_at_risk(&root, &names)
                .into_iter()
                .filter(|d| d.is_dir())
            {
                backup_skill(&backups, host.as_str(), &dir)?;
            }
        }
    }
    Ok(())
}

/// `--restore-skills`(설계 10.3절): 원래 자리가 적힌 백업 가운데 가장 먼저 남긴 것을 그 자리로 되돌린다.
/// 기본은 계획만 낸다. `--apply`면 지금 있는 폴더를 먼저 백업한 뒤 바꾼다(되돌림도 되돌릴 수 있다).
fn restore_skills(ctx: &Ctx, apply: bool) -> Result<u8, CliError> {
    let backups = fullops_home()?.join("skill-backups");
    let dirs = |dir: &Path| -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        out.sort();
        out
    };
    // 원래 자리 → (백업 시각, 호스트, 백업 폴더). 같은 자리에 여럿이면 가장 먼저 남긴 것
    let mut chosen: BTreeMap<String, (i64, String, PathBuf)> = BTreeMap::new();
    let mut unknown = Vec::new();
    for host_dir in dirs(&backups) {
        let host = host_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        for copy in dirs(&host_dir).iter().flat_map(|name| dirs(name)) {
            if copy.to_string_lossy().contains(".new-") {
                continue;
            }
            let record = fullops_store::read_json::<Value>(&copy.join(ORIGIN_FILE))
                .yes()
                .flatten();
            let Some(record) = record.filter(|r| r["origins"].is_array()) else {
                unknown.push(path_text(&copy));
                continue;
            };
            let at = record["at"].as_i64().unwrap_or(0);
            for origin in record["origins"].as_array().into_iter().flatten() {
                let Some(origin) = origin.as_str() else {
                    continue;
                };
                if chosen.get(origin).is_none_or(|(old, _, _)| at < *old) {
                    chosen.insert(origin.to_string(), (at, host.clone(), copy.clone()));
                }
            }
        }
    }
    let mut lines = Vec::new();
    let mut planned = Vec::new();
    for (origin, (_, host, copy)) in &chosen {
        let place = Path::new(origin);
        let same = place.is_dir()
            && folder_sha256(place).is_ok_and(|h| copy.file_name() == Some(hash12(&h).as_ref()));
        if same {
            lines.push(format!("이미 같다: {origin}"));
            continue;
        }
        lines.push(format!("되돌림: {} → {origin}", copy.display()));
        planned.push(json!({"from": path_text(copy), "to": origin}));
        if apply {
            put_back(&backups, host, copy, place).map_err(|e| {
                CliError::Internal(format!("스킬을 되돌리지 못했습니다({origin}): {e}"))
            })?;
        }
    }
    for copy in &unknown {
        lines.push(format!("원래 자리 모름(되돌리지 않는다): {copy}"));
    }
    if !apply && !planned.is_empty() {
        lines.push("계획만 냈다. 실행하려면 `fullops install --restore-skills --apply`".into());
    }
    if lines.is_empty() {
        lines.push(format!("되돌릴 스킬 백업이 없다: {}", backups.display()));
    }
    ctx.print(
        &lines.join("\n"),
        json!({"apply": apply, "restore": planned, "unknown": unknown}),
    );
    Ok(EXIT_OK)
}

/// 백업 하나를 원래 자리에 넣는다. 지금 있는 폴더는 백업한 뒤 비운다(내용은 백업에 남는다).
fn put_back(backups: &Path, host: &str, copy: &Path, place: &Path) -> io::Result<()> {
    let staging = backups.join(format!(".restore-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    copy_tree(copy, &staging)?;
    let _ = std::fs::remove_file(staging.join(ORIGIN_FILE));
    let result = (|| {
        if place.is_dir() {
            backup_skill(backups, host, place)?;
        }
        if std::fs::symlink_metadata(place).is_ok() {
            // 링크면 링크만 지운다(가리키던 폴더는 그대로다)
            std::fs::remove_dir_all(place).or_else(|_| std::fs::remove_file(place))?;
        }
        if let Some(parent) = place.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&staging, place)
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

#[derive(Default)]
struct Args {
    hosts: Option<Vec<HostId>>,
    only: Vec<String>,
    dry_run: bool,
    check: bool,
    prune: bool,
    rollback: bool,
    remove_hooks: bool,
    allow_downgrade: bool,
    restore_skills: bool,
    /// `--restore-skills`·`--retire-python`의 실행. 없으면 계획만 낸다.
    apply: bool,
    retire: super::retire::Args,
}

fn parse_args(mut parser: lexopt::Parser) -> Result<Args, CliError> {
    use lexopt::prelude::*;
    let mut args = Args::default();
    while let Some(arg) = parser.next()? {
        match arg {
            Long("hosts") => {
                let value = parser.value()?.string()?;
                let mut list = Vec::new();
                for name in value.split(',').map(str::trim).filter(|n| !n.is_empty()) {
                    if name == "all" {
                        list.extend(ALL_HOSTS);
                    } else {
                        list.push(HostId::parse(name).ok_or_else(|| {
                            CliError::Usage(format!(
                                "모르는 호스트입니다: {name}. claude, codex, grok, agy, all 가운데 고릅니다."
                            ))
                        })?);
                    }
                }
                list.sort();
                list.dedup();
                args.hosts = Some(list);
            }
            Long("only") => {
                let value = parser.value()?.string()?;
                if !matches!(value.as_str(), "runtime" | "plugin" | "deps") {
                    return Err(CliError::Usage(format!(
                        "--only의 값은 runtime, plugin, deps 가운데 하나입니다: {value}"
                    )));
                }
                args.only.push(value);
            }
            Long("dry-run") => args.dry_run = true,
            Long("check") => args.check = true,
            Long("prune") => args.prune = true,
            Long("rollback") => args.rollback = true,
            Long("remove-hooks") => args.remove_hooks = true,
            Long("allow-downgrade") => args.allow_downgrade = true,
            Long("restore-skills") => args.restore_skills = true,
            Long("apply") => args.apply = true,
            Long("retire-python") => args.retire.on = true,
            Long("remove-meter") => args.retire.remove_meter = true,
            Long("restore") => args.retire.restore = Some(parser.value()?.string()?),
            Long("repos") => {
                let value = parser.value()?.string()?;
                args.retire.repos.extend(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|p| !p.is_empty())
                        .map(PathBuf::from),
                );
            }
            other => return Err(other.unexpected().into()),
        }
    }
    Ok(args)
}

/// 폐쇄망 설정(`FULLOPS_OFFLINE=1`)인가. `install`은 네트워크 단계를, `--check`와 `doctor`는 호스트 CLI 조회를 건너뛴다.
pub(super) fn offline() -> bool {
    std::env::var("FULLOPS_OFFLINE").is_ok_and(|v| v == "1")
}

/// `deps.toml`과 그 글의 sha256. 형식이 맞지 않으면 내장 사본으로 넘어가지 않고 종료 코드 1이다.
pub(super) fn load_deps(root: Option<&Path>) -> Result<(Deps, String), CliError> {
    let (deps, path) = fullops_store::load_deps(root, assets::DEPS_TOML)
        .map_err(|e| CliError::Internal(e.to_string()))?;
    let text = if path.as_os_str().is_empty() {
        assets::DEPS_TOML.to_string()
    } else {
        std::fs::read_to_string(&path)?
    };
    Ok((deps, sha256_hex(text.as_bytes())))
}

/// 고른 호스트 가운데 PATH에 실행 파일이 있는 것. 고르지 않았으면 넷 가운데 있는 것 전부다.
/// 고른 호스트가 없으면 건너뛴다고 알린다(실패가 아니다).
fn targets(ctx: &Ctx, chosen: Option<&[HostId]>) -> Vec<HostId> {
    let mut out = Vec::new();
    for &host in chosen.unwrap_or(&ALL_HOSTS) {
        if hosts::host_available(host) {
            out.push(host);
        } else if chosen.is_some() {
            ctx.note(&msg::host_missing(host_commands(host).program));
        }
    }
    out
}

fn stage_name(stage: InstallStage) -> &'static str {
    match stage {
        InstallStage::Plugin => "plugin",
        InstallStage::Deps => "deps",
    }
}

fn tail3(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(3)..].join("\n")
}

/// 호스트 하나의 진행 기록.
#[derive(Default)]
struct HostRun {
    failed: Vec<String>,
    /// 실패한 명령의 stderr 끝 3줄.
    detail: Vec<String>,
    /// 건너뛴 것(실패가 아니다). 있으면 `complete`가 아니다.
    skipped: Vec<String>,
    stopped: bool,
    /// 이 호스트에 fullops 플러그인이나 hook 등록 파일을 넣었다(끝의 알림을 정한다).
    self_installed: bool,
}

struct Runner<'a> {
    ctx: &'a Ctx,
    root: Option<&'a Path>,
    runs: BTreeMap<HostId, HostRun>,
    notices: BTreeSet<&'static str>,
    deps: &'a Deps,
}

impl Runner<'_> {
    fn run_mut(&mut self, host: HostId) -> &mut HostRun {
        self.runs.entry(host).or_default()
    }

    fn fail(&mut self, host: HostId, what: &str, detail: &str) {
        let run = self.run_mut(host);
        run.failed.push(what.to_string());
        if !detail.is_empty() {
            run.detail.push(format!("{what}\n{detail}"));
        }
        run.stopped = true;
    }

    /// 호스트별 명령 하나. 실패하면 그 호스트의 남은 호스트별 명령을 건너뛴다.
    fn host_step(&mut self, host: HostId, step: &Step) {
        if self.run_mut(host).stopped {
            return;
        }
        self.ctx.note(&format!("[{host}] {}", step.what));
        match step.kind {
            StepKind::Assemble => {
                let (Some(root), Some(target)) = (self.root, step.argv.get(1)) else {
                    return;
                };
                if !root.join("hosts").join(host.as_str()).is_dir() {
                    self.skip_package(host);
                    return;
                }
                if let Err(e) = assemble(root, host, Path::new(target)) {
                    self.fail(host, &step.what, &e.to_string());
                }
            }
            StepKind::HookFile => {
                let (Some(file), Some(bin)) = (step.argv.first(), step.argv.get(1)) else {
                    return;
                };
                if !Path::new(bin).is_file() {
                    self.ctx.note(msg::HOOK_NEEDS_RUNTIME);
                    self.run_mut(host).skipped.push(step.what.clone());
                    return;
                }
                let text = deps::grok_hook_json(Path::new(bin));
                match fullops_store::write_atomic(Path::new(file), text.as_bytes()) {
                    Ok(()) => self.run_mut(host).self_installed = true,
                    Err(e) => self.fail(host, &step.what, &e.to_string()),
                }
            }
            _ => {
                // 조립을 건너뛴 호스트는 그 패키지의 설치 명령도 건너뛴다
                if step.kind == StepKind::SelfInstall
                    && host_commands(host).local_package
                    && !step.argv.last().is_some_and(|p| Path::new(p).is_dir())
                {
                    return;
                }
                let out = hosts::run_step(step, INSTALL_TIMEOUT);
                if !out.ok() {
                    self.fail(host, &step.what, &tail3(&out.stderr_text()));
                    return;
                }
                if step.kind == StepKind::SelfInstall {
                    self.run_mut(host).self_installed = true;
                }
                let pstack =
                    self.deps.plugins.iter().any(|p| {
                        p.notice.as_deref() == Some("pstack") && step.argv.contains(&p.id)
                    });
                if pstack {
                    self.notices.insert(msg::PSTACK);
                }
            }
        }
    }

    fn skip_package(&mut self, host: HostId) {
        self.ctx.note(&msg::package_missing(host));
        self.run_mut(host).skipped.push(format!("{host} 플러그인"));
    }
}

fn step_json(host: Option<HostId>, step: &Step) -> Value {
    json!({
        "host": host,
        "stage": stage_name(step.stage),
        "kind": format!("{:?}", step.kind),
        "what": step.what,
        "argv": step.argv,
        "hosts": step.hosts,
    })
}

fn install(ctx: &Ctx, args: &Args) -> Result<u8, CliError> {
    let root = cli::plugin_root();
    let root = root.as_deref();
    let (deps, manifest_sha) = load_deps(root)?;
    let wants = |stage: &str| args.only.is_empty() || args.only.iter().any(|o| o == stage);
    let offline = offline();
    let network = !offline && (wants("plugin") || wants("deps"));
    let version = package_version(root);
    let mut lines = Vec::new();
    let mut plan_json = Vec::new();

    // 1. 실행본 단계
    let mut runtime_json = Value::Null;
    if wants("runtime")
        && !args.allow_downgrade
        && let Some(refusal) = downgrade(root)?
    {
        return Err(CliError::Refused(vec![refusal]));
    }
    if wants("runtime") {
        let fixed = fixed_bin()?;
        if args.dry_run {
            let home = fullops_home()?;
            lines.push(format!(
                "[runtime] 실행본과 deps.toml 복사 → {}, {}",
                fixed.display(),
                home.join("deps.toml").display()
            ));
            if let Some(kept) = root.map(kept_source).transpose()?.flatten() {
                lines.push(format!("[runtime] 배포본 보관 → {}", kept.display()));
            }
            lines.push(format!(
                "[runtime] 쓰기: {}",
                home.join("install.json").display()
            ));
        } else {
            let done = runtime_stage(root)
                .map_err(|e| CliError::Internal(format!("실행본을 설치하지 못했습니다: {e}")))?;
            lines.push(format!("실행본: {} ({})", done.bin.display(), done.version));
            runtime_json = json!({"bin": path_text(&done.bin), "version": done.version, "copied": done.copied});
        }
    }
    if offline && (wants("plugin") || wants("deps")) {
        ctx.note(msg::OFFLINE);
    }

    // 2. 계획. --dry-run은 조회 명령도 부르지 않는다(등록 여부는 "모른다"로 계획한다)
    let hosts = if network {
        targets(ctx, args.hosts.as_deref())
    } else {
        Vec::new()
    };
    if network && hosts.is_empty() {
        ctx.note("PATH에서 호스트 CLI(claude, codex, grok, agy)를 찾지 못해 플러그인 등록과 의존 설치를 하지 않았다.");
    }
    let registered: BTreeMap<HostId, Known<BTreeMap<String, String>>> = if args.dry_run {
        BTreeMap::new()
    } else {
        hosts
            .iter()
            .map(|&h| (h, hosts::host_marketplaces(h)))
            .collect()
    };
    let package_dir = root
        .filter(|r| r.join("hosts").is_dir())
        .map(|_| fullops_home().map(|h| h.join("package").join(&version)))
        .transpose()?;
    let mut plan = deps::install_plan(&deps, &hosts, &registered, package_dir.as_deref());
    if let Some(hook_file) = grok_hook_file() {
        plan.bind(root, &hook_file, &fixed_bin()?);
    }
    let keep = |step: &Step| wants(stage_name(step.stage));
    let skill_cli = deps.skill_cli.first().map(String::as_str).unwrap_or("npx");
    let skills: Vec<&Step> = plan.skills.iter().filter(|s| keep(s)).collect();
    let have_skill_cli = hosts::find_program(skill_cli).is_some();

    let mut runner = Runner {
        ctx,
        root,
        runs: hosts.iter().map(|&h| (h, HostRun::default())).collect(),
        notices: BTreeSet::new(),
        deps: &deps,
    };

    if args.dry_run {
        for (&host, steps) in &plan.per_host {
            match steps {
                Ok(steps) => {
                    if package_dir.is_none() && host_commands(host).local_package && wants("plugin")
                    {
                        runner.skip_package(host);
                    }
                    for step in steps.iter().filter(|s| keep(s)) {
                        lines.push(format!("[{host}] {}", step.what));
                        plan_json.push(step_json(Some(host), step));
                    }
                }
                Err(refusal) => lines.push(format!("[{host}] 멈춤: {}", refusal.reason)),
            }
        }
        for step in &skills {
            lines.push(format!("[skills] {}", step.what));
            plan_json.push(step_json(None, step));
        }
        if !skills.is_empty() && !have_skill_cli {
            ctx.note(&msg::skill_cli_missing(skill_cli));
        }
        // 사용자 홈에 쓰는 나머지 자리도 계획에 낸다(있던 스킬의 백업과 설치 기록)
        if network {
            let home = fullops_home()?;
            if !skills.is_empty() && have_skill_cli {
                lines.push(format!(
                    "[skills] 있던 스킬 폴더의 백업 → {}",
                    home.join("skill-backups").display()
                ));
            }
            lines.push(format!(
                "[기록] 쓰기: {}",
                home.join("deps-state.json").display()
            ));
        }
        ctx.print(
            &format!("계획(실행하지 않는다):\n{}", lines.join("\n")),
            json!({ "dry_run": true, "plan": plan_json }),
        );
        return Ok(EXIT_OK);
    }

    // 3. 호스트마다 plugin 단계, 이어서 호스트마다 deps 단계의 플러그인
    for stage in [InstallStage::Plugin, InstallStage::Deps] {
        if !wants(stage_name(stage)) {
            continue;
        }
        for (&host, steps) in &plan.per_host {
            match steps {
                Ok(steps) => {
                    if stage == InstallStage::Plugin
                        && package_dir.is_none()
                        && host_commands(host).local_package
                    {
                        runner.skip_package(host);
                    }
                    for step in steps.iter().filter(|s| s.stage == stage) {
                        runner.host_step(host, step);
                    }
                }
                // 출처 충돌로 멈춘 호스트는 명령을 하나도 부르지 않고 실패로 적는다
                Err(refusal) if stage == InstallStage::Plugin || !wants("plugin") => {
                    ctx.note(&format!("[{host}] {}", refusal.reason));
                    runner.fail(host, &refusal.reason, &refusal.escape);
                }
                Err(_) => {}
            }
        }
    }

    // 4. 스킬 묶음마다 스킬 설치 도구 한 번. 호스트별 명령의 실패와 무관하게 계획대로 부른다
    if !skills.is_empty() && !have_skill_cli {
        ctx.note(&msg::skill_cli_missing(skill_cli));
        for step in &skills {
            for &host in &step.hosts {
                runner.run_mut(host).skipped.push(step.what.clone());
            }
        }
    } else {
        for step in &skills {
            ctx.note(&format!("[skills] {}", step.what));
            // 백업하지 못한 스킬은 덮어쓰지 않는다(사용자가 고친 스킬을 잃지 않게 한다)
            let detail = match backup_skills(step).map(|()| hosts::run_step(step, INSTALL_TIMEOUT))
            {
                Ok(out) if out.ok() => continue,
                Ok(out) => tail3(&out.stderr_text()),
                Err(e) => format!("있던 스킬을 백업하지 못해 설치하지 않았다: {e}"),
            };
            for &host in &step.hosts {
                let run = runner.run_mut(host);
                run.failed.push(step.what.clone());
                if !detail.is_empty() {
                    run.detail.push(format!("{}\n{detail}", step.what));
                }
            }
        }
    }

    // 5. 설치 기록과 결과
    let both = wants("plugin") && wants("deps");
    let mut hosts_json = serde_json::Map::new();
    let mut any_failed = false;
    if network {
        let mut state = fullops_store::read_deps_state()
            .yes()
            .ok_or_else(|| CliError::Internal("설치 기록을 읽지 못했습니다".into()))?;
        if state.manifest_sha256 != manifest_sha {
            // 목록이 바뀌었으면 이번에 설치하지 않은 호스트의 기록은 낡은 것이다
            state.hosts.values_mut().for_each(|h| h.complete = false);
            state.manifest_sha256 = manifest_sha;
        }
        for (&host, run) in &runner.runs {
            let was_complete = state.hosts.get(&host).is_some_and(|h| h.complete);
            let complete =
                run.failed.is_empty() && run.skipped.is_empty() && (both || was_complete);
            state.hosts.insert(
                host,
                HostInstall {
                    at: fullops_store::now(),
                    self_version: version.clone(),
                    complete,
                    failed: run.failed.clone(),
                },
            );
            let summary = if !run.failed.is_empty() {
                any_failed = true;
                format!("실패\n  {}", run.detail.join("\n").replace('\n', "\n  "))
            } else if !run.skipped.is_empty() {
                format!("일부 건너뜀({})", run.skipped.join(", "))
            } else {
                "성공".to_string()
            };
            lines.push(format!("{host}: {summary}"));
            hosts_json.insert(
                host.to_string(),
                json!({"complete": complete, "failed": run.failed, "skipped": run.skipped}),
            );
            if run.self_installed
                && let Some(notice) = msg::after_install(host)
            {
                runner.notices.insert(notice);
            }
        }
        fullops_store::write_deps_state(&state)?;
    }
    for notice in &runner.notices {
        ctx.note(notice);
    }
    if any_failed {
        ctx.note(&lines.join("\n"));
        return Err(CliError::External(
            "설치 명령 일부가 실패했습니다. 원인을 고친 뒤 `fullops install`을 다시 실행합니다(다시 실행해도 안전합니다).".into(),
        ));
    }
    ctx.print(
        &lines.join("\n"),
        json!({ "runtime": runtime_json, "hosts": hosts_json, "offline": offline }),
    );
    Ok(EXIT_OK)
}

/// `install --check`의 결과. `doctor`의 의존 항목이 같은 확인을 쓴다.
pub(super) struct Check {
    /// `<호스트>: plugin <id>`·`<호스트>: skill <이름>`·`<호스트>: 설치 기록이 낡았다`·`grok: hook 등록`.
    pub missing: Vec<String>,
    /// 선택형 항목. 안내만 한다.
    pub optional: Vec<String>,
}

/// 아무것도 설치하지 않고 빠진 것을 찾는다(설계 10.3절 `--check`). 호스트 CLI에는 목록 조회만 부른다.
pub(super) fn check(deps: &Deps, manifest_sha: &str, hosts: &[HostId]) -> Check {
    let mut missing = Vec::new();
    let state = fullops_store::read_deps_state().yes();
    for &host in hosts {
        // 폐쇄망에서는 호스트 CLI를 부르지 않는다(호스트 CLI가 실행될 때 네트워크를 쓸 수 있다)
        let installed = if offline() {
            None
        } else {
            hosts::host_plugins(host).yes()
        };
        // 목록을 읽지 못했으면 모른다(빠졌다고 하지 않는다)
        let has = |id: &str| {
            installed.as_ref().is_none_or(|list| {
                let name = id.split('@').next().unwrap_or(id);
                list.iter()
                    .any(|have| have == id || (host_commands(host).local_package && have == name))
            })
        };
        let own = format!("{}@{}", deps.own.plugin, deps.own.marketplace);
        if deps.own.hosts.contains(&host) && !has(&own) {
            missing.push(format!("{host}: plugin {own}"));
        }
        for plugin in deps.plugins.iter().filter(|p| p.hosts.contains(&host)) {
            if !has(&plugin.id) {
                missing.push(format!("{host}: plugin {}", plugin.id));
            }
        }
        let roots = hosts::skill_roots(host);
        for bundle in &deps.skills {
            if !bundle
                .hosts
                .as_deref()
                .unwrap_or(&ALL_HOSTS)
                .contains(&host)
            {
                continue;
            }
            // 이름이 `*`인 묶음은 개별 이름을 알 수 없어 설치 기록으로만 확인한다
            for name in bundle.names.iter().filter(|n| *n != "*") {
                if !roots
                    .iter()
                    .any(|r| r.join(name).join("SKILL.md").is_file())
                {
                    missing.push(format!("{host}: skill {name}"));
                }
            }
        }
        let fresh = state.as_ref().is_some_and(|s| {
            s.manifest_sha256 == manifest_sha && s.hosts.get(&host).is_some_and(|h| h.complete)
        });
        if !fresh {
            missing.push(format!("{host}: 설치 기록이 낡았다"));
        }
        if host == HostId::Grok && !grok_hook_ok() {
            missing.push("grok: hook 등록".to_string());
        }
    }
    let optional = deps
        .optional
        .iter()
        .map(|o| {
            format!(
                "선택(설치하지 않음): {} {} — {} ({})",
                o.kind, o.name, o.note, o.source
            )
        })
        .collect();
    Check { missing, optional }
}

/// grok의 hook 등록 파일이 가리키는 실행본. 파일이 없거나 읽지 못하면 `None`.
pub(super) fn grok_hook_command() -> Option<PathBuf> {
    let file: Value = fullops_store::read_json(&grok_hook_file()?)
        .yes()
        .flatten()?;
    let command = file["hooks"]
        .as_object()?
        .values()
        .find_map(|entries| entries[0]["hooks"][0]["command"].as_str())?;
    Some(PathBuf::from(command))
}

fn grok_hook_ok() -> bool {
    grok_hook_command().is_some_and(|bin| bin.is_file())
}

fn run_check(ctx: &Ctx, args: &Args) -> Result<u8, CliError> {
    let root = cli::plugin_root();
    let (deps, manifest_sha) = load_deps(root.as_deref())?;
    let hosts = targets(ctx, args.hosts.as_deref());
    let found = check(&deps, &manifest_sha, &hosts);
    let mut lines = found.missing.clone();
    lines.extend(found.optional.iter().cloned());
    if found.missing.is_empty() {
        lines.insert(0, "빠진 것이 없다.".to_string());
    }
    ctx.print(
        &lines.join("\n"),
        json!({ "ok": found.missing.is_empty(), "missing": found.missing, "optional": found.optional }),
    );
    Ok(if found.missing.is_empty() {
        EXIT_OK
    } else {
        EXIT_REFUSED
    })
}

/// 옛 실행본들 `(시각, 경로)`. 최근 것이 뒤에 온다.
fn old_bins(bin_dir: &Path) -> Vec<(i64, PathBuf)> {
    let mut out: Vec<(i64, PathBuf)> = std::fs::read_dir(bin_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let at = name
                .strip_prefix("fullops.old-")?
                .trim_end_matches(std::env::consts::EXE_SUFFIX)
                .parse()
                .ok()?;
            Some((at, entry.path()))
        })
        .collect();
    out.sort();
    out
}

fn prune(ctx: &Ctx) -> Result<u8, CliError> {
    let home = fullops_home()?;
    let record: Value = fullops_store::read_json(&home.join("install.json"))
        .yes()
        .flatten()
        .unwrap_or(Value::Null);
    let current = record["version"].as_str();
    // 되돌린(--rollback) 뒤에는 기록의 source가 다른 버전 폴더에 있을 수 있다. 플러그인 루트를 잃지 않게 남긴다
    let source = record["source"].as_str().map(PathBuf::from);
    let mut doomed: Vec<PathBuf> = old_bins(&home.join("bin"))
        .into_iter()
        .map(|(_, p)| p)
        .collect();
    for version in std::fs::read_dir(home.join("package"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let holds_source = source
            .as_ref()
            .is_some_and(|s| s.starts_with(version.path()));
        if Some(version.file_name().to_string_lossy().as_ref()) != current && !holds_source {
            doomed.push(version.path());
            continue;
        }
        for entry in std::fs::read_dir(version.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            if entry.file_name().to_string_lossy().contains(".old-") {
                doomed.push(entry.path());
            }
        }
    }
    let mut removed = Vec::new();
    for path in doomed.into_iter().filter(|p| p.exists()) {
        // 잠긴 것(실행 중인 옛 실행본)은 건너뛴다
        let gone = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        if gone.is_ok() {
            removed.push(path_text(&path));
        }
    }
    ctx.print(
        &format!("지운 것 {}개\n{}", removed.len(), removed.join("\n")),
        json!({ "removed": removed }),
    );
    Ok(EXIT_OK)
}

fn rollback(ctx: &Ctx) -> Result<u8, CliError> {
    let home = fullops_home()?;
    let fixed = fixed_bin()?;
    let bin_dir = home.join("bin");
    let Some((_, old)) = old_bins(&bin_dir).pop() else {
        return Err(CliError::Internal("되돌릴 옛 실행본이 없습니다.".into()));
    };
    // 지금 실행본을 옛 파일 자리로 옮기고 가장 최근 옛 파일을 제자리에 둔다
    swap_in(&old, &fixed, &old_bin_path(&bin_dir))?;
    let record_path = home.join("install.json");
    let mut record: Value = fullops_store::read_json(&record_path)
        .yes()
        .flatten()
        .unwrap_or_else(|| json!({}));
    let (version, previous) = (record["version"].clone(), record["previous"].clone());
    record["version"] = previous;
    record["previous"] = version;
    record["sha256"] = json!(sha256_hex(&std::fs::read(&fixed)?));
    record["at"] = json!(fullops_store::now());
    // 고정: S-04가 플러그인의 새 버전으로 다시 올리지 않게 한다. 실행본 단계가 기록을 새로 쓰면서 풀린다
    record["pinned"] = json!(true);
    fullops_store::write_json(&record_path, &record)?;
    ctx.print(
        &format!(
            "실행본을 되돌렸다: {}\n{}",
            fixed.display(),
            msg::ROLLBACK_PINNED
        ),
        json!({ "bin": path_text(&fixed), "version": record["version"], "pinned": true }),
    );
    Ok(EXIT_OK)
}

/// fullops가 직접 쓴 hook 등록 파일만 지운다. 호스트 CLI의 `plugin uninstall`은 부르지 않는다.
fn remove_hooks(ctx: &Ctx) -> Result<u8, CliError> {
    let file =
        grok_hook_file().ok_or_else(|| CliError::Internal("사용자 홈을 찾지 못했습니다".into()))?;
    let removed = match std::fs::remove_file(&file) {
        Ok(()) => vec![path_text(&file)],
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    let human = match removed.first() {
        Some(path) => format!("지웠다: {path}"),
        None => format!("지울 hook 등록 파일이 없다: {}", file.display()),
    };
    ctx.print(&human, json!({ "removed": removed }));
    Ok(EXIT_OK)
}

pub fn run(ctx: &Ctx, parser: lexopt::Parser) -> Result<u8, CliError> {
    let args = parse_args(parser)?;
    // 지우거나 되돌리는 인자는 --dry-run·--check를 보지 않으므로 함께 주면 거절한다
    if (args.remove_hooks || args.prune || args.rollback || args.restore_skills || args.retire.on)
        && (args.dry_run || args.check)
    {
        return Err(CliError::Usage(
            "--prune·--rollback·--remove-hooks·--restore-skills·--retire-python은 --dry-run·--check와 함께 쓰지 않습니다(--restore-skills·--retire-python은 --apply가 없으면 계획만 냅니다).".into(),
        ));
    }
    let retire_only =
        args.retire.restore.is_some() || !args.retire.repos.is_empty() || args.retire.remove_meter;
    if (retire_only && !args.retire.on) || (args.apply && !args.retire.on && !args.restore_skills) {
        return Err(CliError::Usage(
            "--apply는 --restore-skills·--retire-python과, --repos·--restore·--remove-meter는 --retire-python과 함께 씁니다.".into(),
        ));
    }
    if args.retire.on {
        super::retire::run(ctx, &args.retire, args.apply)
    } else if args.restore_skills {
        restore_skills(ctx, args.apply)
    } else if args.remove_hooks {
        remove_hooks(ctx)
    } else if args.prune {
        prune(ctx)
    } else if args.rollback {
        rollback(ctx)
    } else if args.check {
        run_check(ctx, &args)
    } else {
        install(ctx, &args)
    }
}
