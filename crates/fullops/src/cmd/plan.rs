//! `fullops plan list|ready|check|add|set`: 대기 목록 읽기와 편집(설계 8.13절). 표의 다른 부분은 건드리지 않고 커밋하지 않는다.

use super::dispatch::{usage, value};
use crate::assets::PLANS_MD;
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::model::{PlanStatus, RoleId, TaskKey};
use lexopt::prelude::*;
use serde_json::json;

/// 표 줄의 칸들(바이트 범위). `\|`는 구분자가 아니다. `|`로 시작하지 않으면 표 줄이 아니다.
fn cell_ranges(line: &str) -> Option<Vec<(usize, usize)>> {
    // 파일 머리의 BOM은 공백처럼 넘긴다
    let blank = |c: char| c.is_whitespace() || c == '\u{feff}';
    let lead = line.len() - line.trim_start_matches(blank).len();
    if !line[lead..].starts_with('|') {
        return None;
    }
    let (mut out, mut start, mut escaped) = (Vec::new(), lead + 1, false);
    for (at, c) in line.char_indices().skip_while(|(at, _)| *at <= lead) {
        match c {
            '|' if !escaped => {
                out.push((start, at));
                start = at + 1;
            }
            _ => {}
        }
        escaped = c == '\\' && !escaped;
    }
    if !line[start..].trim().is_empty() {
        // 줄 끝 표기는 칸에 넣지 않는다
        out.push((start, line.trim_end_matches(['\r', '\n']).len()));
    }
    Some(out)
}

fn is_separator(line: &str) -> bool {
    cell_ranges(line).is_some_and(|cells| {
        !cells.is_empty()
            && cells.iter().all(|(a, b)| {
                let cell = &line[*a..*b];
                cell.contains('-') && cell.chars().all(|c| matches!(c, '-' | ':' | ' ' | '\t'))
            })
    })
}

/// 과제 표의 열 자리(설계 6.1.2절): 키, 상태, 목표, 담당, 원천 순으로 처음 맞는 칸이다.
struct Columns {
    count: usize,
    key: usize,
    status: usize,
    goal: Option<usize>,
    role: Option<usize>,
    source: Option<usize>,
}

fn columns(header: &str, separator: &str) -> Option<Columns> {
    if !is_separator(separator) {
        return None;
    }
    let heads: Vec<&str> = cell_ranges(header)?
        .iter()
        .map(|(a, b)| &header[*a..*b])
        .collect();
    let mut taken = Vec::new();
    let mut find = |words: &[&str]| {
        let at = (0..heads.len())
            .find(|i| !taken.contains(i) && words.iter().any(|w| heads[*i].contains(w)))?;
        taken.push(at);
        Some(at)
    };
    let key = find(&["과제", "키"])?;
    let status = find(&["상태"])?;
    Some(Columns {
        count: heads.len(),
        key,
        status,
        goal: find(&["목표", "결과", "다음", "내용"]),
        role: find(&["담당", "책임"]),
        source: find(&["지시서", "원천"]),
    })
}

/// 과제 표들: (열 자리, 마지막 표 줄의 번호). 코드 펜스 안은 읽지 않는다.
fn tables(lines: &[&str]) -> Vec<(Columns, usize)> {
    let mut fenced = false;
    let visible: Vec<&str> = lines
        .iter()
        .map(|line| {
            let fence = line.trim_start().starts_with("```");
            let skip = fenced || fence;
            fenced ^= fence;
            if skip { "" } else { line }
        })
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < visible.len() {
        let Some(cols) = columns(visible[i], visible[i + 1]) else {
            i += 1;
            continue;
        };
        i += 2;
        while visible
            .get(i)
            .is_some_and(|line| cell_ranges(line).is_some())
        {
            i += 1;
        }
        out.push((cols, i - 1));
    }
    out
}

fn cell_text(text: &str) -> String {
    text.replace('|', "\\|").replace(['\n', '\r'], " ")
}

/// 줄 끝 표기(`\r\n`·`\n`)를 그대로 둔 줄들. 고치지 않는 줄은 바이트가 바뀌지 않는다.
fn split_lines(text: &str) -> Vec<String> {
    text.split_inclusive('\n').map(str::to_string).collect()
}

fn write_plan(ctx: &Ctx, lines: &[String]) -> Result<(), CliError> {
    Ok(fullops_store::write_atomic(
        &ctx.repo()?.plan_path(),
        lines.concat().as_bytes(),
    )?)
}

fn status_text(status: &PlanStatus) -> String {
    match status {
        PlanStatus::Ready => "대기".into(),
        PlanStatus::WaitHuman(what) => format!("대기({what})"),
        PlanStatus::Held(why) => format!("보류({why})"),
        PlanStatus::Other(text) => text.clone(),
    }
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let repo = ctx.repo()?;
    let sub = match args.next()? {
        Some(Value(v)) => v.string()?,
        _ => return Err(usage("사용법: fullops plan list|ready|check|add|set")),
    };
    let text = fullops_store::read_text(&repo.plan_path())
        .yes()
        .flatten()
        .unwrap_or_default();
    let plan = fullops_core::parse_plan(&text, PLANS_MD);
    match sub.as_str() {
        "list" => {
            let rows: Vec<_> = plan
                .rows
                .iter()
                .map(|r| json!({"key": r.key, "goal": r.goal, "role": r.role, "status": status_text(&r.status),
                                "after": r.after, "source": r.source, "line": r.line}))
                .collect();
            let human: Vec<String> = plan
                .rows
                .iter()
                .map(|r| {
                    format!(
                        "{} | {} | {} | {}",
                        r.key,
                        r.role.as_ref().map_or("-", RoleId::as_str),
                        status_text(&r.status),
                        r.goal
                    )
                })
                .collect();
            ctx.print(&human.join("\n"), json!({ "rows": rows }));
        }
        "ready" => {
            let state = fullops_store::read_state(repo).yes().unwrap_or_default();
            let archive = fullops_store::archive_keys(repo).yes().unwrap_or_default();
            let ready = fullops_core::ready(&plan, &state, &archive, &|role| {
                repo.inbox_path(role).exists()
            });
            let human: Vec<&str> = ready.iter().map(TaskKey::as_str).collect();
            ctx.print(&human.join("\n"), json!({ "ready": ready }));
        }
        "check" => {
            let human: Vec<String> = plan
                .parse_errors
                .iter()
                .map(|line| format!("PLANS.md {line}번 줄: 과제 키가 문법에 맞지 않는다"))
                .collect();
            ctx.print(&human.join("\n"), json!({ "errors": plan.parse_errors }));
        }
        "add" => {
            let (mut key, mut role, mut goal, mut source, mut after, mut status) =
                (None, None, None, None, None, None);
            while let Some(arg) = args.next()? {
                match arg {
                    Value(v) if key.is_none() => key = Some(v.string()?),
                    Long("role") => role = Some(value(&mut args)?),
                    Long("goal") => goal = Some(value(&mut args)?),
                    Long("source") => source = Some(value(&mut args)?),
                    Long("after") => after = Some(value(&mut args)?),
                    Long("status") => status = Some(value(&mut args)?),
                    other => return Err(other.unexpected().into()),
                }
            }
            let form = "사용법: fullops plan add <KEY> --role <R> --goal \"…\" --source \"…\" [--after K,…] [--status \"…\"]";
            let (Some(key), Some(role), Some(goal), Some(source)) = (key, role, goal, source)
            else {
                return Err(usage(form));
            };
            let key = TaskKey::new(key).map_err(|e| usage(e.to_string()))?;
            if plan.rows.iter().any(|r| r.key == key) {
                return Err(usage(format!(
                    "PLANS.md에 이미 `{key}` 행이 있다. 상태는 `fullops plan set`으로 바꾼다."
                )));
            }
            let mut status = status.unwrap_or_else(|| "대기".into());
            if let Some(after) = after {
                status.push_str(&format!(
                    " 선행: {}",
                    after
                        .replace(',', ", ")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            let mut lines = split_lines(&text);
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            let Some((cols, last)) = tables(&refs).pop() else {
                return Err(usage(
                    "PLANS.md에 과제 표가 없다. `과제 키`와 `상태` 열이 있는 표를 먼저 둔다.",
                ));
            };
            let mut cells = vec![String::new(); cols.count];
            let mut put = |at: Option<usize>, text: &str| {
                if let Some(cell) = at.and_then(|at| cells.get_mut(at)) {
                    *cell = cell_text(text);
                }
            };
            put(Some(cols.key), key.as_str());
            put(Some(cols.status), &status);
            put(cols.goal, &goal);
            put(cols.role, &role);
            put(cols.source, &source);
            // 새 행은 파일의 줄 끝 표기를 따른다. 표 마지막 줄이 줄바꿈 없이 끝나면 줄바꿈을 그 줄에 붙인다
            let row = format!("| {} |", cells.join(" | "));
            let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
            match lines.get_mut(last) {
                Some(line) if !line.ends_with('\n') => {
                    line.push_str(eol);
                    lines.insert(last + 1, row);
                }
                _ => lines.insert(last + 1, format!("{row}{eol}")),
            }
            write_plan(ctx, &lines)?;
            ctx.print(
                &format!("PLANS.md에 `{key}` 행을 더했다."),
                json!({ "key": key }),
            );
        }
        "set" => {
            let form = "사용법: fullops plan set <KEY> \"<상태 칸 글>\"";
            let mut word = || match args.next() {
                Ok(Some(Value(v))) => Ok(v.string()?),
                _ => Err(usage(form)),
            };
            let (key, status) = (word()?, word()?);
            let Some(row) = plan.rows.iter().find(|r| r.key.as_str() == key) else {
                return Err(usage(format!("PLANS.md에 `{key}` 행이 없다.")));
            };
            let mut lines = split_lines(&text);
            let at = (row.line as usize).saturating_sub(1);
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            let cols = tables(&refs)
                .into_iter()
                .find(|(_, last)| at <= *last)
                .map(|(cols, _)| cols);
            let range = cols
                .zip(lines.get(at))
                .and_then(|(cols, line)| cell_ranges(line)?.get(cols.status).copied());
            let (Some((a, b)), Some(line)) = (range, lines.get_mut(at)) else {
                return Err(CliError::Internal(format!(
                    "`{key}` 행의 상태 칸을 찾지 못했다."
                )));
            };
            line.replace_range(a..b, &format!(" {} ", cell_text(&status)));
            write_plan(ctx, &lines)?;
            ctx.print(
                &format!("`{key}`의 상태를 바꿨다: {status}"),
                json!({"key": key, "status": status}),
            );
        }
        other => return Err(usage(format!("모르는 하위 명령이다: plan {other}"))),
    }
    Ok(EXIT_OK)
}
