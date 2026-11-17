//! `fullops migrate --layout [--dry-run]`: 옛 자리 `.fullops-squad/`를 `.fullops/`로 옮긴다(설계 13.6절).
//! 추적 파일은 표(`layout::moved`)대로 옮기고, 지울 것은 추적에서만 빼고 작업 트리의 파일을 `.fullops/local/legacy/`에 둔다.
//! 커밋 하나(`COMMIT`)를 만든다. push하지 않는다. `migrate --from-python`이 전환 끝에 같은 함수를 부른다.

use super::init;
use crate::assets;
use crate::cli::CliError;
use fullops_core::layout::{self, DIR, LEGACY_DIR};
use fullops_core::messages::install as msg;
use fullops_core::model::{HeadRef, Known, Refusal};
use fullops_ext::{Git, GitCli};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

pub(super) const COMMIT: &str = "fullops: .fullops 자리로 옮김";
/// 추적에서 뺀 파일과 자리가 없는 미추적 파일을 두는 곳(`.fullops/` 기준). 옛 상대 경로를 지킨다.
const KEPT: &str = "local/legacy";
/// lint·리뷰의 대상이 아닌 폴더. `config/lint.json`·`config/review.json`의 `exclude`에 없으면 더한다.
const EXCLUDES: [&str; 2] = [".fullops/tasks/**", ".fullops/archive/**"];

/// git을 한 번 실행한다. 큰 레포의 인덱스 갱신과 커밋이 있어 제한 시간이 길다.
fn git(root: &Path, args: &[&str], stdin: Option<&[u8]>) -> Result<Vec<u8>, CliError> {
    let argv: Vec<String> = std::iter::once("git")
        .chain(args.iter().copied())
        .map(String::from)
        .collect();
    let env = [("GIT_TERMINAL_PROMPT", "0")];
    let out = fullops_ext::run(&argv, root, Duration::from_secs(600), &env, stdin);
    if out.ok() {
        Ok(out.stdout)
    } else {
        Err(CliError::External(format!(
            "git {} 실패: {}",
            args.first().copied().unwrap_or(""),
            out.stderr_text().trim()
        )))
    }
}

/// NUL로 끝나는 레코드들.
fn records(bytes: &[u8]) -> impl Iterator<Item = String> + '_ {
    bytes
        .split(|b| *b == 0)
        .filter(|rec| !rec.is_empty())
        .map(|rec| String::from_utf8_lossy(rec).into_owned())
}

/// 폴더 아래의 파일(심볼릭 링크 포함)을 그 폴더 기준 슬래시 경로로 모은다. 링크가 가리키는 폴더로는 들어가지 않는다.
fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let rel = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            walk(&entry.path(), &format!("{rel}/"), out);
        } else {
            out.push(rel);
        }
    }
}

/// 빈 폴더만 아래에서부터 지운다. 파일이 남은 폴더는 둔다.
fn remove_empty_dirs(dir: &Path) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            remove_empty_dirs(&entry.path());
        }
    }
    let _ = std::fs::remove_dir(dir);
}

/// 새 자리의 `.gitignore` 템플릿(`.env*`, `!.env.example`, `local/`, `**/raw/`)이 무시하는 경로인가.
/// 그런 파일을 추적한 채 옮기면 '무시 목록에 있으면서 추적되는 파일'이 된다.
fn ignored_at_new(new: &str) -> bool {
    let (dirs, name) = new.rsplit_once('/').unwrap_or(("", new));
    dirs.split('/').any(|dir| dir == "raw" || dir == "local")
        || (name.starts_with(".env") && name != ".env.example")
}

/// `"exclude": […]` 배열에 없는 항목을 더한 글. 형식을 지키려고 글에 끼워 넣는다. 배열을 찾지 못하면 `None`이다.
fn add_excludes(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    let list = value.get("exclude")?.as_array()?;
    let missing: Vec<&str> = EXCLUDES
        .into_iter()
        .filter(|glob| !list.iter().any(|have| have.as_str() == Some(glob)))
        .collect();
    if missing.is_empty() {
        return Some(text.to_string());
    }
    let open = text.find("\"exclude\"")? + text[text.find("\"exclude\"")?..].find('[')?;
    // 문자열 밖의 첫 `]`가 배열의 끝이다
    let (mut quoted, mut escaped, mut close) = (false, false, None);
    for (at, c) in text[open..].char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ']' if !quoted => {
                close = Some(open + at);
                break;
            }
            _ => {}
        }
    }
    let close = close?;
    let body = text[open + 1..close].trim_end();
    // 마지막 항목의 들여쓰기를 따른다
    let indent = match body.rfind('\n') {
        Some(at) => {
            let line = &body[at + 1..];
            format!("\n{}", &line[..line.len() - line.trim_start().len()])
        }
        None => " ".to_string(),
    };
    let comma = if list.is_empty() { "" } else { "," };
    let added: Vec<String> = missing.iter().map(|glob| format!("\"{glob}\"")).collect();
    let tail = &text[open + 1 + body.len()..];
    Some(format!(
        "{}{body}{comma}{indent}{}{tail}",
        &text[..open + 1],
        added.join(&format!(",{indent}"))
    ))
}

fn triple(version: &str) -> (u64, u64, u64) {
    let mut parts = version
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|p| p.parse().ok());
    let mut next = || parts.next().unwrap_or(0);
    (next(), next(), next())
}

/// 옮길 것의 계획. 경로는 모두 레포 폴더 기준이다(옛 것은 `.fullops-squad/`, 새 것은 `.fullops/`).
#[derive(Default)]
struct Plan {
    /// 추적을 유지하며 옮기는 파일 `(옛 경로, 새 경로, 인덱스의 "<mode> <sha>")`.
    moves: Vec<(String, String, String)>,
    /// 추적에서 빼는 파일. 작업 트리의 파일은 `local/legacy/<옛 경로>`로 간다.
    drops: Vec<String>,
    /// 추적되지 않는 파일 `(옛 경로, 새 경로)`.
    loose: Vec<(String, String)>,
    /// 옮긴 뒤 글을 바꿀 파일 `(레포 상대 경로, 새 글)`.
    edits: Vec<(String, String)>,
    /// 새 자리에서 같은 경로로 겹치는 것.
    clashes: Vec<String>,
    notes: Vec<String>,
    /// 글은 고치지만 커밋에 넣지 않는 파일(추적되지 않는 엔트리 문서).
    unstaged: Vec<String>,
}

fn plan(root: &Path) -> Result<Plan, CliError> {
    let prefix = format!("{LEGACY_DIR}/");
    let inside = |path: String| path.strip_prefix(&prefix).map(str::to_string);
    let listed = git(root, &["ls-files", "-s", "-z", "--", LEGACY_DIR], None)?;
    let tracked: Vec<(String, String)> = records(&listed)
        .filter_map(|rec| {
            let (meta, path) = rec.split_once('\t')?;
            let (entry, _stage) = meta.rsplit_once(' ')?;
            Some((inside(path.to_string())?, entry.to_string()))
        })
        .collect();
    // 무시 목록에 있으면서 추적되는 파일은 옮기지 않고 추적에서 뺀다
    let ignore_args = [
        "ls-files",
        "-c",
        "-i",
        "--exclude-standard",
        "-z",
        "--",
        LEGACY_DIR,
    ];
    let ignored: BTreeSet<String> = records(&git(root, &ignore_args, None)?)
        .filter_map(inside)
        .collect();
    // 과제 키: 끝난 지시서·분류 기록·리뷰 기록이 있는 키. 그 키의 QA 폴더가 `tasks/<KEY>/qa/`로 간다
    let keys: BTreeSet<String> = tracked
        .iter()
        .filter_map(|(old, _)| {
            let new = layout::moved(old, &|_| false)?;
            let (key, _) = new.strip_prefix("tasks/")?.split_once('/')?;
            Some(key.to_string())
        })
        .collect();
    let has_task = |key: &str| keys.contains(key);

    let mut plan = Plan::default();
    let mut taken: BTreeMap<String, String> = BTreeMap::new();
    let mut claim = |new: &str, old: &str, clashes: &mut Vec<String>| {
        if let Some(first) = taken.insert(new.to_string(), old.to_string()) {
            clashes.push(format!("{first}, {old} → {new}"));
        }
    };
    let old_dir = root.join(LEGACY_DIR);
    let mut ignore_file = false;
    for (old, entry) in &tracked {
        let target = layout::moved(old, &has_task)
            .filter(|new| !ignored.contains(old) && !ignored_at_new(new));
        let Some(new) = target else {
            claim(&format!("{KEPT}/{old}"), old, &mut plan.clashes);
            plan.drops.push(old.clone());
            continue;
        };
        claim(&new, old, &mut plan.clashes);
        // 커밋되는 글 안의 경로(13.6절 4번). 과제 기록과 보관은 고치지 않는다
        let prose = ["guide/", "templates/"].iter().any(|d| new.starts_with(d))
            || ["PLANS.md", "docs/deliverables/README.md"].contains(&new.as_str());
        let config = ["config/lint.json", "config/review.json"].contains(&new.as_str());
        let special = [".gitignore", "fullops.toml"].contains(&new.as_str());
        if (prose || config || special)
            && let Ok(before) = std::fs::read_to_string(old_dir.join(old))
        {
            let mut after = before.clone();
            if prose || config {
                after = layout::rewrite(&after, &has_task);
            }
            if config {
                match add_excludes(&after) {
                    Some(added) => after = added,
                    None => plan.notes.push(format!(
                        "`{}`에서 `exclude` 배열을 찾지 못해 `{}`를 더하지 않았다.",
                        layout::at(&new),
                        EXCLUDES.join("`, `")
                    )),
                }
            }
            if new == ".gitignore" {
                ignore_file = true;
                for line in assets::ignore_lines() {
                    if !after.lines().any(|have| have.trim() == line) {
                        if !after.is_empty() && !after.ends_with('\n') {
                            after.push('\n');
                        }
                        after.push_str(&format!("{line}\n"));
                    }
                }
            }
            if new == "fullops.toml" {
                let current = env!("CARGO_PKG_VERSION");
                let applied = toml::from_str::<toml::Table>(&after)
                    .ok()
                    .and_then(|t| t.get("applied_version")?.as_str().map(str::to_string))
                    .unwrap_or_default();
                if triple(current) > triple(&applied) {
                    after = init::set_top_key(&after, "applied_version", current);
                }
            }
            if after != before {
                plan.edits.push((layout::at(&new), after));
            }
        }
        plan.moves.push((old.clone(), new, entry.clone()));
    }
    if !ignore_file {
        let template: String = assets::REPO_FILES
            .iter()
            .find(|(rel, _)| *rel == ".gitignore")
            .map_or_else(String::new, |(_, text)| text.to_string());
        plan.edits.push((layout::at(".gitignore"), template));
    }
    // 추적되지 않는 파일: `.env*`와 `local/`은 같은 이름으로, 그 밖은 `local/legacy/` 아래로
    let known: BTreeSet<&String> = tracked.iter().map(|(old, _)| old).collect();
    let mut on_disk = Vec::new();
    walk(&old_dir, "", &mut on_disk);
    on_disk.sort();
    for old in on_disk.into_iter().filter(|rel| !known.contains(rel)) {
        let env = !old.contains('/') && old.starts_with(".env");
        let new = if env || old.starts_with("local/") {
            old.clone()
        } else {
            format!("{KEPT}/{old}")
        };
        claim(&new, &old, &mut plan.clashes);
        plan.loose.push((old, new));
    }
    // 엔트리 문서의 관리 블록
    let entry_args: Vec<&str> = ["ls-files", "-z", "--"]
        .into_iter()
        .chain(assets::ENTRY_DOCS)
        .collect();
    let entry_tracked: Vec<String> = records(&git(root, &entry_args, None)?).collect();
    for name in assets::ENTRY_DOCS {
        let path = root.join(name);
        let Ok(before) = std::fs::read_to_string(&path) else {
            continue;
        };
        if !before.contains(assets::BLOCK_START) || init::is_symlink(&path) {
            continue;
        }
        match init::with_block(&before) {
            Ok(after) if after != before => {
                // 무시 목록에 둔 엔트리 문서는 글만 고친다(`git add`가 거절한다)
                if !entry_tracked.iter().any(|tracked| tracked == name) {
                    plan.unstaged.push(name.to_string());
                }
                plan.edits.push((name.to_string(), after));
            }
            Ok(_) => {}
            Err(_) => plan.notes.push(format!("{name}: {}", msg::BLOCK_BROKEN)),
        }
    }
    Ok(plan)
}

/// 거절 조건(13.6절 1번). `base`는 설정의 기준 브랜치다. `chained`는 `--from-python`이 전환 끝에 부르는 경우로,
/// 그쪽이 이미 본 것(깨끗한 작업 트리, 빈 인박스)과 허용하는 것(가져온 보류)은 보지 않는다.
pub(super) fn stops(root: &Path, base: &str, chained: bool) -> Vec<String> {
    let mut out = Vec::new();
    let old = root.join(LEGACY_DIR);
    if std::fs::symlink_metadata(root.join(DIR)).is_ok() {
        out.push("`.fullops/`가 이미 있다. 옮길 자리가 비어 있어야 한다".into());
    }
    if init::is_home(root) {
        out.push(
            "레포 루트가 사용자 홈이다. 레포 폴더 `.fullops/`가 `~/.fullops/`와 겹친다".into(),
        );
    }
    if init::is_symlink(&old) {
        out.push("`.fullops-squad`가 심볼릭 링크다. 링크 대상은 옮기지 않는다".into());
    }
    let Some((git_dir, common_dir)) = init::git_dirs(root) else {
        out.push("Git 디렉터리를 찾지 못했다".into());
        return out;
    };
    let repo = fullops_store::Repo {
        root: root.to_path_buf(),
        git_dir,
        common_dir,
    };
    match fullops_store::head(&repo) {
        Ok(HeadRef::Branch(name)) if name == base => {}
        Ok(head) => out.push(format!(
            "기준 브랜치(`{base}`)를 체크아웃한 lead 체크아웃이 아니다(지금 {})",
            match head {
                HeadRef::Branch(name) => format!("`{name}`"),
                HeadRef::Detached(_) => "detached HEAD".to_string(),
            }
        )),
        Err(_) => out.push("HEAD를 읽지 못했다".into()),
    }
    if chained {
        return out;
    }
    match GitCli::cli(root.to_path_buf()).status(None) {
        Known::Yes(entries) if entries.is_empty() => {}
        Known::Yes(_) => out.push("작업 트리가 깨끗하지 않다. 커밋하거나 치운다".into()),
        Known::Unknown(_) => out.push("git 상태를 읽지 못했다".into()),
    }
    match fullops_store::read_state(&repo) {
        Known::Yes(state) => {
            let open = state
                .dispatches
                .iter()
                .filter(|d| d.settled.is_none() && !d.is_reservation())
                .count();
            if open > 0 {
                out.push(format!(
                    "열린 dispatch가 {open}건 있다. `fullops wait`로 보고를 받고 `fullops merge`로 정리한다"
                ));
            }
            let held = state
                .completions
                .iter()
                .filter(|c| c.hold.is_some() && c.landed_at.is_none())
                .count();
            if held > 0 {
                out.push(format!(
                    "보류가 {held}건 있다. `fullops resume`으로 풀고 병합한다"
                ));
            }
        }
        Known::Unknown(_) => {
            out.push("state.json을 읽지 못했다. `fullops doctor`로 확인한다".into())
        }
    }
    for entry in std::fs::read_dir(old.join("handovers"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let busy = std::fs::read_to_string(entry.path()).is_ok_and(|t| !t.trim().is_empty());
        if name.starts_with("to_") && name.ends_with(".md") && busy {
            out.push(format!(
                "인박스에 진행 중 과제가 있다: handovers/{name}. 끝내고(`fullops done`·`fullops merge`) 옮긴다"
            ));
        }
    }
    out
}

fn refuse(reasons: &[String]) -> CliError {
    CliError::Refused(vec![Refusal {
        rule: "migrate",
        reason: reasons.join(" / "),
        escape: "남은 것을 정리한 뒤 lead 체크아웃에서 `fullops migrate --layout`".into(),
        exit: 2,
    }])
}

/// 계획대로 옮긴다: 파일 → 인덱스(한 번에) → 글 → 커밋.
fn apply(root: &Path, plan: &Plan) -> Result<(), CliError> {
    let (old_dir, new_dir) = (root.join(LEGACY_DIR), root.join(DIR));
    let rename = |old: &str, new: &str| -> std::io::Result<()> {
        let to = new_dir.join(new);
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::rename(old_dir.join(old), to)
    };
    let moves = plan
        .moves
        .iter()
        .map(|(old, new, _)| (old.clone(), new.clone()));
    let drops = plan
        .drops
        .iter()
        .map(|old| (old.clone(), format!("{KEPT}/{old}")));
    for (old, new) in moves.chain(drops).chain(plan.loose.iter().cloned()) {
        rename(&old, &new)?;
    }
    remove_empty_dirs(&old_dir);

    // 인덱스: 옛 경로를 지우고 같은 blob을 새 경로에 넣는다. 파일을 다시 읽지 않는다
    let mut index = Vec::new();
    // 지우는 줄의 해시는 그 레포의 해시 길이만큼의 0이다(SHA-1 40자, SHA-256 64자)
    let width = plan
        .moves
        .first()
        .and_then(|(_, _, entry)| entry.split(' ').nth(1))
        .map_or(40, str::len);
    let zero = "0".repeat(width);
    let gone = plan.moves.iter().map(|(old, _, _)| old).chain(&plan.drops);
    for old in gone {
        index.extend(format!("0 {zero}\t{LEGACY_DIR}/{old}\0").into_bytes());
    }
    for (_, new, entry) in &plan.moves {
        index.extend(format!("{entry} 0\t{DIR}/{new}\0").into_bytes());
    }
    git(root, &["update-index", "-z", "--index-info"], Some(&index))?;

    let mut edited: Vec<&str> = Vec::new();
    for (rel, text) in &plan.edits {
        fullops_store::write_atomic(&root.join(rel), text.as_bytes())?;
        if !plan.unstaged.contains(rel) {
            edited.push(rel);
        }
    }
    if !edited.is_empty() {
        let mut args = vec!["add", "--"];
        args.extend(edited);
        git(root, &args, None)?;
    }
    git(root, &["commit", "--quiet", "-m", COMMIT], None)?;
    Ok(())
}

/// `migrate --layout`의 본체. 사람용 글과 `--json` 필드를 돌려준다.
pub(super) fn run_at(
    root: &Path,
    dry_run: bool,
    chained: bool,
) -> Result<(String, Value), CliError> {
    let old = root.join(LEGACY_DIR);
    let text = std::fs::read_to_string(old.join("fullops.toml")).map_err(|_| {
        let reason = if root.join(layout::config_toml()).is_file() {
            "이미 `.fullops/` 자리다. 옮길 것이 없다"
        } else if old.join("fullops.json").exists() {
            "Python판(`.fullops-squad/fullops.json`) 레포다. `fullops migrate --from-python`이 전환과 자리 옮김을 함께 한다"
        } else {
            "옛 자리 표식 `.fullops-squad/fullops.toml`이 없다"
        };
        refuse(&[reason.to_string()])
    })?;
    let config = fullops_store::parse_config(&text).map_err(|e| {
        refuse(&[format!(
            "옛 자리의 fullops.toml을 읽을 수 없다: {}",
            e.detail
        )])
    })?;
    let mut stops = stops(root, &config.base, chained);
    if !stops.is_empty() {
        return Err(refuse(&stops));
    }
    let plan = plan(root)?;
    stops.extend(plan.clashes.iter().take(5).map(|clash| {
        format!("새 자리에서 경로가 겹친다: {clash}. 옛 자리에서 한쪽 이름을 바꿔 커밋한 뒤 다시 실행한다")
    }));
    if !stops.is_empty() {
        return Err(refuse(&stops));
    }
    let edits: Vec<&str> = plan.edits.iter().map(|(rel, _)| rel.as_str()).collect();
    let mut lines = vec![format!(
        "{}: 옮기는 추적 파일 {}개, 추적에서 빼는 파일 {}개(작업 트리의 파일은 `.fullops/{KEPT}/`로), 추적되지 않는 파일 {}개.",
        if dry_run {
            "계획(--dry-run. 아무것도 바꾸지 않았다)"
        } else {
            "`.fullops/` 자리로 옮겼다"
        },
        plan.moves.len(),
        plan.drops.len(),
        plan.loose.len()
    )];
    if !edits.is_empty() {
        lines.push(format!("글을 고치는 파일:\n  {}", edits.join("\n  ")));
    }
    lines.extend(plan.notes.iter().cloned());
    if !dry_run {
        apply(root, &plan).map_err(|e| {
            CliError::Internal(format!(
                "자리를 옮기다 멈췄다: {}\n되돌리기: `git reset --hard`가 추적 파일을 옛 자리에 되살린다. 그 뒤 `.fullops/`에 남은 추적되지 않는 파일(`.env*`, `local/`)을 `.fullops-squad/`로 옮기고 `.fullops/`를 지운다.",
                match e {
                    CliError::External(m) | CliError::Internal(m) => m,
                    other => format!("{other:?}"),
                }
            ))
        })?;
        lines.push(format!(
            "커밋 `{COMMIT}`을 만들었다. push하지 않았다. 다른 워크트리는 `fullops sync`나 다음 `fullops merge`의 동기화로 따라온다(그 워크트리에 남는 `.fullops-squad/`의 추적되지 않는 파일은 손으로 옮기거나 지운다). 되돌리기는 그 커밋의 `git revert`다."
        ));
    }
    let fields = json!({
        "dry_run": dry_run, "moved": plan.moves.len(), "untracked": plan.drops.len(),
        "loose": plan.loose.len(), "edited": edits, "notes": plan.notes,
    });
    Ok((lines.join("\n"), fields))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_are_added_in_place() {
        let text = "{\n  \"include\": [\"**/*.md\"],\n  \"exclude\": [\n    \"**/*.bak\",\n    \"a]b/**\"\n  ],\n  \"rules\": []\n}\n";
        let added = add_excludes(text).unwrap();
        assert_eq!(
            added,
            "{\n  \"include\": [\"**/*.md\"],\n  \"exclude\": [\n    \"**/*.bak\",\n    \"a]b/**\",\n    \".fullops/tasks/**\",\n    \".fullops/archive/**\"\n  ],\n  \"rules\": []\n}\n"
        );
        // 다시 해도 같고, 한 줄 배열과 빈 배열에도 들어간다
        assert_eq!(add_excludes(&added).unwrap(), added);
        assert_eq!(
            add_excludes(r#"{"exclude": [".fullops/tasks/**"]}"#).unwrap(),
            r#"{"exclude": [".fullops/tasks/**", ".fullops/archive/**"]}"#
        );
        let empty: Value =
            serde_json::from_str(&add_excludes(r#"{"exclude": []}"#).unwrap()).unwrap();
        assert_eq!(empty["exclude"], json!(EXCLUDES));
        assert_eq!(add_excludes(r#"{"rules": []}"#), None);
    }

    #[test]
    fn new_ignore_rules_match_the_template() {
        // `ignored_at_new`는 템플릿의 줄을 손으로 옮긴 것이다. 템플릿이 바뀌면 함께 고친다
        let lines: Vec<&str> = assets::ignore_lines().collect();
        assert_eq!(lines, [".env*", "!.env.example", "local/", "**/raw/"]);
        for path in [
            ".env",
            ".env.local",
            "local/tool.log",
            "tasks/A-1/qa/raw/shot.png",
        ] {
            assert!(ignored_at_new(path), "{path}");
        }
        for path in [".env.example", "tasks/A-1/qa/shot.png", "guide/rules/a.md"] {
            assert!(!ignored_at_new(path), "{path}");
        }
    }
}
