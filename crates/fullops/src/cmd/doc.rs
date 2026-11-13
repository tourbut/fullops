//! `fullops doc`: 산출물 문서 검사와 front matter 갱신(설계 8.12절).

use crate::cli::{CliError, Ctx, EXIT_OK, EXIT_REFUSED};
use fullops_core::frontmatter::{FmValue, FrontMatter, FrontMatterError, parse_front_matter};
use fullops_core::lint_rules::{self as rules, DOC_STATUSES, SQUAD};
use fullops_core::model::{DeliverableRow, path_text};
use fullops_store::Repo;
use lexopt::prelude::*;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const INDEX: &str = ".fullops-squad/docs/deliverables/README.md";

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    match args.next()? {
        Some(Value(sub)) if sub == "check" => check(ctx, args),
        Some(Value(sub)) if sub == "stamp" => stamp(ctx, args),
        _ => Err(CliError::Usage(
            "사용법: fullops doc check [--id <Dnn>] [--strict] | fullops doc stamp --path <경로> [--owner <담당>] [--summary <요약>] [--task <KEY>] [--status draft|review|approved]".into(),
        )),
    }
}

fn read_lossy(path: &Path) -> std::io::Result<String> {
    Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
}

fn index_rows(repo: &Repo) -> Option<Vec<DeliverableRow>> {
    let text = read_lossy(&repo.root.join(INDEX)).ok()?;
    Some(rules::parse_deliverable_index(&text))
}

fn markdown_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|e| e.path()) {
        if path.is_dir() {
            markdown_under(&path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

/// 인덱스의 원천 경로가 파일이면 그 파일, 폴더면 그 안의 Markdown 파일들이다.
fn source_files(repo: &Repo, source: &str) -> Vec<PathBuf> {
    let path = repo.root.join(source);
    let mut files = Vec::new();
    if path.is_dir() {
        markdown_under(&path, &mut files);
        files.sort();
    } else if path.is_file() {
        files.push(path);
    }
    files
}

/// Markdown 링크 `](대상)`의 대상들.
fn link_targets(text: &str) -> impl Iterator<Item = &str> {
    text.split("](")
        .skip(1)
        .filter_map(|rest| rest.split_once(')'))
        .map(|(target, _)| target)
        .filter(|target| !target.is_empty())
}

fn is_url(path: &str) -> bool {
    path.split_once("://").is_some_and(|(scheme, _)| {
        !scheme.is_empty() && scheme.bytes().all(|b| b.is_ascii_lowercase())
    })
}

/// 인덱스의 D01–D13 행마다 원천 문서의 존재, 조립 문서의 로컬 링크, front matter, 상태의 일치를 본다.
fn check(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let (mut selected, mut strict) = (None, false);
    while let Some(arg) = args.next()? {
        match arg {
            Long("id") => selected = Some(args.value()?.string()?),
            Long("strict") => strict = true,
            other => return Err(other.unexpected().into()),
        }
    }
    let repo = ctx.repo()?;
    let rows = index_rows(repo)
        .ok_or_else(|| CliError::Internal(format!("산출물 인덱스가 없습니다: {INDEX}")))?;
    let mut ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    ids.sort_unstable();
    if !ids.iter().copied().eq((1..=13).map(|n| format!("D{n:02}"))) {
        return Err(CliError::Internal(
            "산출물 인덱스의 D01–D13 행을 확인하세요.".into(),
        ));
    }
    if let Some(id) = &selected
        && !ids.contains(&id.as_str())
    {
        return Err(CliError::Usage(format!("없는 산출물 ID입니다: {id}")));
    }
    let relative = |path: &Path| {
        let text = path_text(path.strip_prefix(&repo.root).unwrap_or(path));
        text.strip_prefix(SQUAD).unwrap_or(&text).to_string()
    };
    let (mut errors, mut warnings, mut unwritten, mut lines) =
        (Vec::new(), Vec::new(), 0, Vec::new());
    for row in rows
        .iter()
        .filter(|r| selected.as_ref().is_none_or(|id| *id == r.id))
    {
        let sources: Vec<&str> = row
            .sources
            .iter()
            .map(|s| s.strip_prefix(SQUAD).unwrap_or(s))
            .collect();
        if selected.is_some() {
            lines.push(format!(
                "{}: {} / 원천: {}",
                row.id,
                row.status,
                sources.join(", ")
            ));
        }
        let files: Vec<Vec<PathBuf>> = row.sources.iter().map(|s| source_files(repo, s)).collect();
        let missing: Vec<&str> = sources
            .iter()
            .zip(&files)
            .filter(|(_, found)| found.is_empty())
            .map(|(source, _)| *source)
            .collect();
        let written = !["미작성", "범위 밖"].contains(&row.status.as_str());
        if row.status == "미작성" {
            unwritten += 1;
        } else if written && !missing.is_empty() {
            errors.push(format!("{} 원천 없음: {}", row.id, missing.join(", ")));
        }
        for path in files.iter().flatten().filter(|_| written) {
            for problem in rules::doc_problems(&read_lossy(path)?, Some(&row.id), Some(&row.status))
            {
                let message = format!("{} {}: {problem}", row.id, relative(path));
                if strict { &mut errors } else { &mut warnings }.push(message);
            }
        }
        // 조립 문서 `<ID>_*.md`의 로컬 링크
        let folder = repo.root.join(INDEX).with_file_name("");
        let mut documents: Vec<PathBuf> = std::fs::read_dir(&folder)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().unwrap_or_default().to_string_lossy();
                name.starts_with(&format!("{}_", row.id)) && name.ends_with(".md")
            })
            .collect();
        documents.sort();
        for document in documents {
            let text = read_lossy(&document)?;
            for target in link_targets(&text) {
                let path = target.split('#').next().unwrap_or("");
                if !path.is_empty() && !is_url(path) && !folder.join(path).exists() {
                    let name = document.file_name().unwrap_or_default().to_string_lossy();
                    errors.push(format!("{name} 링크 없음: {target}"));
                }
            }
        }
    }
    lines.extend(errors.iter().cloned());
    lines.extend(warnings.iter().map(|w| format!("경고: {w}")));
    let checked = selected.as_ref().map_or(rows.len(), |_| 1);
    lines.push(format!(
        "검사: {} / 미작성: {unwritten} / 문제: {} / 경고: {}",
        selected.clone().unwrap_or_else(|| rows.len().to_string()),
        errors.len(),
        warnings.len()
    ));
    if ctx.json {
        println!(
            "{}",
            json!({"ok": errors.is_empty(), "checked": checked, "unwritten": unwritten,
                   "errors": errors, "warnings": warnings})
        );
    } else {
        println!("{}", lines.join("\n"));
    }
    Ok(if errors.is_empty() {
        EXIT_OK
    } else {
        EXIT_REFUSED
    })
}

/// 에폭 날수를 `YYYY-MM-DD`로 바꾼다.
fn civil(days: i64) -> String {
    let z = days + 719_468;
    let (era, doe) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// front matter를 정해진 순서로 다시 쓰고 `updated`를 오늘로, `--task`를 `tasks`에 더한다. 본문은 건드리지 않는다.
fn stamp(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let (mut path, mut owner, mut summary, mut task, mut status) = (None, None, None, None, None);
    while let Some(arg) = args.next()? {
        let slot = match arg {
            Long("path") => &mut path,
            Long("owner") => &mut owner,
            Long("summary") => &mut summary,
            Long("task") => &mut task,
            Long("status") => &mut status,
            other => return Err(other.unexpected().into()),
        };
        *slot = Some(args.value()?.string()?);
    }
    if let Some(status) = &status
        && !DOC_STATUSES.contains(&status.as_str())
    {
        return Err(CliError::Usage(format!(
            "--status는 draft·review·approved 가운데 하나입니다: {status}"
        )));
    }
    let path = path.ok_or_else(|| CliError::Usage("--path가 필요합니다.".into()))?;
    // 경로는 `.fullops-squad` 기준이다. 레포 루트 기준으로 적은 것(`.fullops-squad/…`)도 받는다
    let path = path.replace('\\', "/");
    let relative = path.trim_start_matches("./");
    let relative = relative.strip_prefix(SQUAD).unwrap_or(relative);
    if !relative.ends_with(".md")
        || relative.starts_with('/')
        || relative.contains(':')
        || relative.split('/').any(|part| part == "..")
    {
        return Err(CliError::Usage(format!(
            "--path는 .fullops-squad 안의 Markdown 문서여야 합니다: {path}"
        )));
    }
    let repo = ctx.repo()?;
    let target = repo.squad().join(relative);
    let repo_path = format!("{SQUAD}{relative}");

    let text = match read_lossy(&target) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let (mut fm, body, had) = match parse_front_matter(&text) {
        Ok((fm, at)) => (fm, text.get(at..).unwrap_or(""), true),
        Err(FrontMatterError::Missing) => (
            FrontMatter::default(),
            text.trim_start_matches('\u{feff}'),
            false,
        ),
        Err(FrontMatterError::Syntax(line)) => {
            return Err(CliError::Internal(format!(
                "{repo_path}의 front matter를 읽을 수 없습니다({line}번째 줄). 고친 뒤 다시 실행합니다."
            )));
        }
    };

    let rows = index_rows(repo).unwrap_or_default();
    let owners: Vec<&DeliverableRow> = rows.iter().filter(|r| rules::owns(r, &repo_path)).collect();
    let owned = rules::identities(&fm);
    let text_of = |fm: &FrontMatter, name: &str| fm.get(name).unwrap_or("").trim().to_string();
    for (name, value) in [("owner", &owner), ("summary", &summary)] {
        if let Some(value) = value {
            fm.set(name, FmValue::Text(value.trim().to_string()));
        }
    }
    if text_of(&fm, "title").is_empty() {
        let heading = body.lines().find_map(|l| l.strip_prefix("# "));
        let stem = target.file_stem().unwrap_or_default().to_string_lossy();
        let title = match owners[..] {
            [row] => row.title.clone(),
            _ => heading.map_or(stem.into_owned(), |h| h.trim().to_string()),
        };
        fm.set("title", FmValue::Text(title));
    }
    let valid = |s: Option<&str>| s.filter(|s| DOC_STATUSES.contains(s)).map(str::to_string);
    // 그 산출물의 상태: 준 값, 문서의 지금 값, 인덱스의 값, draft 순이다
    let state = |fm: &FrontMatter, row: Option<&DeliverableRow>| {
        let current = match row {
            Some(row) if !owned.is_empty() && !owned.contains(&row.id) => None,
            row => valid(rules::document_status(fm, row.map(|r| r.id.as_str()))),
        };
        status
            .clone()
            .or(current)
            .or_else(|| valid(row.map(|r| r.status.as_str())))
            .unwrap_or_else(|| "draft".into())
    };
    if owners.len() > 1 {
        // 산출물 여럿이 한 파일을 원천으로 가진다: id는 목록, 상태는 ID별 statuses다
        let states: BTreeMap<String, String> = owners
            .iter()
            .map(|row| (row.id.clone(), state(&fm, Some(row))))
            .collect();
        fm.set("statuses", FmValue::Map(states));
        if text_of(&fm, "status").is_empty() {
            fm.set("status", FmValue::Text("draft".into()));
        }
        fm.set(
            "id",
            FmValue::List(owners.iter().map(|r| r.id.clone()).collect()),
        );
    } else {
        let effective = state(&fm, owners.first().copied());
        fm.set("status", FmValue::Text(effective));
        if let Some(row) = owners.first() {
            fm.set("id", FmValue::Text(row.id.clone()));
        }
    }
    // shortcut: 날짜 라이브러리가 없어 UTC 날짜다(한국에서는 09시 전까지 어제 날짜가 된다), 로컬 시간대를 얻는 공용 함수가 생기면 그 값으로 바꾼다
    fm.set(
        "updated",
        FmValue::Text(civil(fullops_store::now().div_euclid(86_400))),
    );
    let mut tasks = match fm.list("tasks") {
        Some(list) => list.to_vec(),
        None => Some(text_of(&fm, "tasks"))
            .filter(|t| !t.is_empty())
            .into_iter()
            .collect(),
    };
    tasks.extend(task.filter(|t| !tasks.contains(t)));
    if !tasks.is_empty() {
        fm.set("tasks", FmValue::List(tasks));
    }
    let missing: Vec<String> = ["owner", "summary"]
        .into_iter()
        .filter(|name| text_of(&fm, name).is_empty())
        .map(|name| format!("--{name}"))
        .collect();
    if !missing.is_empty() {
        return Err(CliError::Usage(format!(
            "처음 쓰는 front matter에는 {}가 필요합니다.",
            missing.join(", ")
        )));
    }

    let fm = rules::ordered(&fm);
    let body = match body {
        body if body.trim().is_empty() => format!("\n# {}\n", text_of(&fm, "title")),
        body if had || body.starts_with(['\n', '\r']) => body.to_string(),
        body => format!("\n{body}"),
    };
    fullops_store::write_atomic(&target, format!("{}{body}", fm.render()).as_bytes())?;
    ctx.print(&repo_path, json!({ "path": repo_path }));
    Ok(EXIT_OK)
}

#[cfg(test)]
mod tests {
    use super::civil;

    #[test]
    fn epoch_days_become_dates() {
        for (days, date) in [
            (0, "1970-01-01"),
            (19_782, "2024-02-29"),
            (20_453, "2025-12-31"),
            (20_771, "2026-11-14"),
        ] {
            assert_eq!(civil(days), date);
        }
    }
}
