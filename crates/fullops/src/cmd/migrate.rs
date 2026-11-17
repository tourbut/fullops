//! `fullops migrate`: Python판(1.24.0)에서 전환과 되돌리기(설계 13절).
//! `--from-python`은 쓰기 전에 전제를 모두 보고, 되돌리기 기록을 먼저 쓴 뒤 새 표식 `fullops.toml` 쓰기와 `fullops.json` 이름 바꾸기를 마지막에 한다.
//! 중간에 실패하면 쓴 것을 되돌린다(전부 되거나 전혀 안 되거나). 커밋은 하지 않는다.
//! Python판의 상태 폴더는 읽기만 하고 `<git-common-dir>/fullops/py-backup/`에 사본을 둔다.

use super::init::{self, NewConfig, NewRole};
use crate::assets;
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::messages::install as msg;
use fullops_core::model::{
    ArchiveLine, ArchivedTask, Completion, Decision, Known, Mode, Parallel, Plan, Refusal, RoleId,
    RoleKind, Routing, TaskKey, TestLevel, path_text, sha256_hex,
};
use fullops_ext::{Git, GitCli};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const PY_MARKER: &str = ".fullops-squad/fullops.json";
const PY_MARKER_MOVED: &str = ".fullops-squad/fullops.json.py-1.24.0";
const TOML_MARKER: &str = ".fullops-squad/fullops.toml";
const TOML_ROLLED_BACK: &str = ".fullops-squad/fullops.toml.rolled-back";
const PY_BLOCK: (&str, &str) = ("<!-- fullops-squad:start -->", "<!-- fullops-squad:end -->");
const PY_STATE_DIRS: [&str; 5] = [
    "fullops-flow",
    "fullops-integration",
    "fullops-decisions",
    "fullops-autonomy",
    "fullops-issues",
];
/// Python판 흐름 노드의 순서. `report` 이상이면 완료 보고까지 간 과제다.
const FLOW_NODES: [&str; 9] = [
    "intent", "design", "assign", "build", "report", "review", "merge", "qa", "done",
];
const ROLLBACK_RECORD: &str = "migrate.json";
const IGNORE_REL: &str = ".fullops-squad/.gitignore";
/// 새 템플릿이 없는 1.24.0 템플릿 문서. 남은 Python판 명령만 알린다.
const PY_ONLY_DOCS: [&str; 1] = ["docs/evaluations/scenarios/README.md"];

fn refuse(reasons: &[String], escape: &str) -> CliError {
    CliError::Refused(vec![Refusal {
        rule: "migrate",
        reason: reasons.join(" / "),
        escape: escape.to_string(),
        exit: 2,
    }])
}

fn json_files(dir: &Path) -> Vec<(PathBuf, Value)> {
    let mut out: Vec<(PathBuf, Value)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|p| {
            let value = fullops_store::read_json(&p).yes().flatten()?;
            Some((p, value))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// `YYYY-MM-DDTHH:MM:SS`로 시작하는 UTC 시각 글을 에폭 초로. 읽지 못하면 `None`.
fn epoch_of(value: &Value) -> Option<i64> {
    if let Some(n) = value.as_i64() {
        return Some(n);
    }
    let text = value.as_str()?;
    let num = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hh, mm, ss) = (num(11..13)?, num(14..16)?, num(17..19)?);
    // 그레고리력 날짜를 1970-01-01부터의 날 수로
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some(days * 86_400 + hh * 3_600 + mm * 60 + ss)
}

/// `orca-agents.md`의 `## 라우팅 기준` 절에서 읽은 것.
#[derive(Default)]
struct Guide {
    /// 표식 줄 `- <이름> 역할: `<역할>``.
    marks: BTreeMap<String, String>,
    /// 역할 설명 줄 `- `<역할>`: <글>`.
    about: BTreeMap<String, String>,
    /// 읽지 못한 `- ` 줄.
    unread: Vec<String>,
}

fn read_guide(squad: &Path) -> Guide {
    let text = std::fs::read_to_string(squad.join("orca-agents.md")).unwrap_or_default();
    let mut guide = Guide::default();
    let (mut inside, mut fenced) = (false, false);
    for line in text.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
        } else if line.starts_with("## ") {
            inside = line.trim() == "## 라우팅 기준";
        }
        let Some(item) = line.strip_prefix("- ").filter(|_| inside && !fenced) else {
            continue;
        };
        // 백틱 안의 역할 ID와 그 앞뒤 글
        let mut parts = item.splitn(3, '`');
        let (before, role, after) = (
            parts.next().unwrap_or(""),
            parts.next().unwrap_or(""),
            parts.next().unwrap_or(""),
        );
        let mark = before
            .trim()
            .strip_suffix("역할:")
            .map(|l| l.trim().to_string());
        match (mark, before.is_empty(), after.trim().strip_prefix(':')) {
            (Some(label), _, _) if !role.is_empty() => {
                guide.marks.insert(label, role.to_string());
            }
            (None, true, Some(about)) if !role.is_empty() => {
                guide
                    .about
                    .insert(role.to_string(), about.trim().to_string());
            }
            _ => guide.unread.push(line.to_string()),
        }
    }
    guide
}

struct Built {
    text: String,
    /// 보고서에 내는 문장.
    report: Vec<String>,
}

/// `fullops.json`과 라우팅 기준 표식 줄에서 `fullops.toml`의 글을 만든다(설계 13.2절 2번).
fn build_config(old: &Value, guide: &Guide) -> Result<Built, String> {
    let mut report = Vec::new();
    let text_of = |key: &str| old[key].as_str().map(str::to_string);
    let roles_json = old["roles"].as_object().cloned().unwrap_or_default();
    let registered = |role: &str| roles_json.contains_key(role);

    let mode_text = text_of("mode").unwrap_or_else(|| {
        report.push("mode가 없어 1.24.0의 실제 동작인 coor로 옮겼다.".into());
        "coor".into()
    });
    let mode = Mode::parse(&mode_text).ok_or(format!("mode 값을 모른다: {mode_text}"))?;
    let level_text = text_of("test_level").unwrap_or_else(|| {
        report.push("test_level이 없어 1.24.0의 실제 동작인 standard로 옮겼다.".into());
        "standard".into()
    });
    let test_level =
        TestLevel::parse(&level_text).ok_or(format!("test_level 값을 모른다: {level_text}"))?;
    let parallel = match old["subagent_level"].as_str() {
        Some("off") => Parallel::Off,
        Some(level @ ("lite" | "standard" | "full")) => {
            report.push(format!("subagent_level {level} → parallel = \"on\"."));
            Parallel::On
        }
        Some(other) => return Err(format!("subagent_level 값을 모른다: {other}")),
        None => {
            report.push(msg::PARALLEL_WAS_OFF.into());
            Parallel::On
        }
    };
    if parallel == Parallel::Off {
        report.push("subagent_level off → parallel = \"off\".".into());
    } else {
        report.push(msg::PARALLEL_HOW_TO_LIMIT.into());
    }

    // 역할 종류: 표식 줄에서 읽는다. coor 모드의 primary_role은 coordinator 역할이다
    let mark = |label: &str| guide.marks.get(label).filter(|r| registered(r)).cloned();
    let design = mark("설계").or_else(|| mark("제품 기획"));
    let coordinator = mark("coordinator")
        .or_else(|| text_of("primary_role").filter(|r| mode == Mode::Coor && registered(r)));
    let tester = mark("tester");
    for (label, role) in &guide.marks {
        let known =
            ["설계", "coordinator", "tester", "제품 기획", "기술 계획"].contains(&label.as_str());
        if !known || !registered(role) {
            report.push(format!("읽지 못한 표식 줄: - {label} 역할: `{role}`"));
        }
    }
    report.extend(
        guide
            .unread
            .iter()
            .map(|l| format!("읽지 못한 표식 줄: {l}")),
    );
    let mut roles = Vec::new();
    for (id, branch) in &roles_json {
        let kind = if design.as_deref() == Some(id) {
            RoleKind::Design
        } else if coordinator.as_deref() == Some(id) {
            RoleKind::Coordinator
        } else if tester.as_deref() == Some(id) {
            RoleKind::Tester
        } else {
            RoleKind::Worker
        };
        roles.push(NewRole {
            id: RoleId::new(id.as_str()).map_err(|_| format!("역할 ID를 읽을 수 없다: {id}"))?,
            branch: branch.as_str().unwrap_or_default().to_string(),
            kind,
            about: guide.about.get(id).cloned(),
        });
    }
    let split = match (mark("제품 기획"), mark("기술 계획")) {
        (Some(_), Some(tech)) => Some(tech),
        _ => None,
    };
    let base = old["git"]["base"]
        .as_str()
        .map(str::to_string)
        .or_else(|| text_of("primary_branch"))
        .unwrap_or_else(|| "main".into());
    let review = old["review_policy"].as_object().map(|policy| {
        ["min_code_lines", "min_code_files", "always", "auto"]
            .iter()
            .filter_map(|key| Some(format!("{key} = {}\n", policy.get(*key)?)))
            .collect::<String>()
    });
    let dev = mode == Mode::Dev;
    let new = NewConfig {
        mode,
        primary_role: text_of("primary_role").filter(|_| dev),
        primary_branch: text_of("primary_branch").filter(|_| dev),
        routing: if split.is_some() {
            Routing::Split
        } else {
            Routing::Legacy
        },
        tech_plan_role: split,
        test_level,
        parallel,
        base,
        remote: old["git"]["remote"].as_str().map(str::to_string),
        roles,
        review,
    };
    let text = init::config_text(&new);
    fullops_store::parse_config(&text).map_err(|e| e.to_string())?;
    Ok(Built { text, report })
}

/// 전제(설계 13.2절). 어긴 것을 모두 돌려준다.
fn blockers(root: &Path, common: &Path, old: &Value) -> Vec<String> {
    let squad = root.join(".fullops-squad");
    let mut out = Vec::new();
    let version = old["plugin_version"].as_str().unwrap_or("없음");
    if version != "1.24.0" {
        out.push(format!(
            "레포 적용 버전(plugin_version)이 {version}다. Python판을 1.24.0으로 맞춘 뒤 전환한다"
        ));
    }
    if squad.join("fullops.toml").exists() {
        out.push("이미 fullops.toml이 있다".into());
    }
    for role in old["roles"].as_object().into_iter().flat_map(|r| r.keys()) {
        let inbox = squad.join(format!("handovers/to_{role}.md"));
        if std::fs::read_to_string(&inbox).is_ok_and(|t| !t.trim().is_empty()) {
            out.push(format!(
                "역할 인박스가 비어 있지 않다: handovers/to_{role}.md"
            ));
        }
    }
    let waiting = json_files(&common.join("fullops-integration"))
        .iter()
        .filter(|(_, item)| item["integrated"] != true && !item["hold"].is_object())
        .count();
    if waiting > 0 {
        out.push(format!(
            "Python판 통합 대기가 {waiting}건 있다(보류는 허용한다). 병합하거나 보류하고 Python판의 `integration.py settle`을 돌린다"
        ));
    }
    let issue: Value = fullops_store::read_json(&common.join("fullops-issues/status.json"))
        .yes()
        .flatten()
        .unwrap_or(Value::Null);
    if issue["enabled"] == true {
        out.push("이슈 자동 모드가 켜져 있다. 끈 뒤 전환한다".into());
    }
    let busy = [
        "claimed",
        "running",
        "reviewing",
        "reconciling",
        "awaiting_author",
    ];
    if issue["job"]["status"]
        .as_str()
        .is_some_and(|s| busy.contains(&s))
    {
        out.push("이슈 자동 모드에 점유하거나 답변을 기다리는 잡이 있다".into());
    }
    match GitCli::cli(root.to_path_buf()).status(None) {
        Known::Yes(entries) if entries.is_empty() => {}
        Known::Yes(_) => out.push("작업 트리가 깨끗하지 않다. 커밋하거나 치운다".into()),
        Known::Unknown(_) => out.push("git 상태를 읽지 못했다".into()),
    }
    out
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    std::fs::copy(from, to).map(|_| ())
}

/// 파일의 글. 없으면 `None`이다. 읽지 못한 파일(폴더다, UTF-8이 아니다)은 오류다: 빈 글로 보고 덮어쓰지 않는다.
fn read_opt(path: &Path) -> std::io::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// 엔트리 문서에서 Python판 관리 블록을 빼고 새 블록을 넣은 글.
fn swap_block(text: &str) -> Result<String, init::BrokenBlock> {
    match init::block_span(text, PY_BLOCK.0, PY_BLOCK.1)? {
        Some(span) => init::with_block(&format!("{}{}", &text[..span.start], &text[span.end..])),
        None => init::with_block(text),
    }
}

/// Python판 명령(`python3`, `<이름>.py`)이 있는 줄의 번호.
fn python_lines(text: &str) -> Vec<String> {
    let script = |line: &str| {
        line.match_indices(".py")
            .any(|(at, _)| !line[at + 3..].starts_with(|c: char| c.is_alphanumeric() || c == '_'))
    };
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.contains("python3") || script(line))
        .map(|(index, _)| (index + 1).to_string())
        .collect()
}

/// 전환이 바꾸는 파일 하나(레포 루트 기준 경로). `before`가 `None`이면 없던 파일이고 `after`가 `None`이면 지우는 파일이다.
struct Change {
    rel: String,
    before: Option<String>,
    after: Option<String>,
}

/// 바꿀 파일을 계획에 넣는다. 심볼릭 링크는 대상을 고치지 않고 `links`에 적는다(거짓을 돌려준다).
fn stage(root: &Path, change: Change, changes: &mut Vec<Change>, links: &mut Vec<String>) -> bool {
    let linked = init::linked(root, &root.join(&change.rel));
    if linked {
        links.push(change.rel);
    } else {
        changes.push(change);
    }
    !linked
}

/// 2·6단계가 쓸 것을 쓰지 않고 계산한다. 전제를 어긴 것은 `stops`에, 알릴 것은 `report`에 넣는다.
/// 새 표식 `fullops.toml`이 맨 뒤다(쓰는 순서다).
fn plan_changes(
    root: &Path,
    old: &Value,
    config: Option<&str>,
    stops: &mut Vec<String>,
    report: &mut Vec<String>,
) -> Result<Vec<Change>, CliError> {
    let (mut changes, mut links) = (Vec::new(), Vec::new());
    let mut read = |rel: &str| {
        read_opt(&root.join(rel))
            .map_err(|e| stops.push(format!("`{rel}`을 읽지 못했다: {e}")))
            .ok()
    };
    let mut broken = Vec::new();
    for name in assets::ENTRY_DOCS {
        let Some(before) = read(name) else { continue };
        match swap_block(before.as_deref().unwrap_or("")) {
            Ok(after) if before.as_deref() == Some(after.as_str()) => {}
            Ok(after) => {
                let change = Change {
                    rel: name.to_string(),
                    before,
                    after: Some(after),
                };
                stage(root, change, &mut changes, &mut links);
            }
            Err(_) => broken.push(name),
        }
    }
    // 없는 .gitignore는 아래에서 템플릿으로 만든다(필요한 줄이 들어 있다)
    if let Some(Some(before)) = read(IGNORE_REL) {
        let mut after = before.clone();
        for line in assets::GITIGNORE_LINES {
            if !after.lines().any(|l| l.trim() == *line) {
                if !after.is_empty() && !after.ends_with('\n') {
                    after.push('\n');
                }
                after.push_str(&format!("{line}\n"));
            }
        }
        if after != before {
            let change = Change {
                rel: IGNORE_REL.to_string(),
                before: Some(before),
                after: Some(after),
            };
            stage(root, change, &mut changes, &mut links);
        }
    }
    // 1.24.0 기본 글 그대로인 템플릿 문서는 새 템플릿으로 바꾸고, 사람이 고친 문서는 두고 남은 Python판 명령을 알린다
    let (mut replaced, mut python, mut delegation_kept) = (Vec::new(), Vec::new(), false);
    // 1.24.0에서 하네스 명령을 담던 문서만 본다(프로젝트 자신의 Python 이야기를 잡지 않는다)
    let docs = assets::STOCK_1_24.iter().map(|(rel, _)| *rel);
    for rel in docs.chain(PY_ONLY_DOCS) {
        let shown = format!(".fullops-squad/{rel}");
        let Some(Some(before)) = read(&shown) else {
            continue;
        };
        let template = assets::REPO_FILES
            .iter()
            .find(|(path, _)| *path == rel)
            .filter(|_| assets::is_stock_1_24(rel, &before));
        match template {
            Some((_, template)) => {
                // setup이 찍은 front matter는 남긴다(lint의 DOC-003이 요구한다)
                let lf = before.replace("\r\n", "\n");
                let front = assets::split_front_matter(&lf).0;
                let gap = if front.is_empty() { "" } else { "\n" };
                let change = Change {
                    rel: shown.clone(),
                    after: Some(format!("{front}{gap}{template}")),
                    before: Some(before),
                };
                if stage(root, change, &mut changes, &mut links) {
                    replaced.push(shown);
                }
            }
            None => {
                delegation_kept |= rel == "rules/delegation.md";
                let lines = python_lines(&before);
                if !lines.is_empty() {
                    python.push(format!("{shown} {}번 줄", lines.join(", ")));
                }
            }
        }
    }
    // 없는 템플릿: 쓰지 않고 목록만 받는다
    let mut missing = init::Applied::default();
    init::add_templates(root, true, &mut missing)?;
    for shown in &missing.created {
        let text = assets::REPO_FILES
            .iter()
            .find(|(rel, _)| shown.strip_prefix(".fullops-squad/") == Some(*rel))
            .map_or("", |(_, text)| *text);
        changes.push(Change {
            rel: shown.clone(),
            before: None,
            after: Some(text.to_string()),
        });
    }
    links.extend(missing.refused);
    // 새 판은 인박스 파일이 없는 것이 빈 인박스다. Python판이 남긴 빈 파일은 지운다
    let mut inboxes = Vec::new();
    for role in old["roles"].as_object().into_iter().flat_map(|r| r.keys()) {
        let rel = format!(".fullops-squad/handovers/to_{role}.md");
        if let Some(Some(before)) = read(&rel) {
            let change = Change {
                rel: rel.clone(),
                before: Some(before),
                after: None,
            };
            if stage(root, change, &mut changes, &mut links) {
                inboxes.push(rel);
            }
        }
    }
    if let Some(text) = config {
        changes.push(Change {
            rel: TOML_MARKER.to_string(),
            before: None,
            after: Some(text.to_string()),
        });
    }

    // 쓰다가 실패할 것이 미리 보이면 시작하지 않는다
    if init::linked(root, &root.join(TOML_MARKER)) {
        stops.push(
            "`.fullops-squad`나 `fullops.toml`이 심볼릭 링크다. 링크 대상에는 쓰지 않는다".into(),
        );
    }
    if root.join(PY_MARKER_MOVED).exists() {
        stops.push(format!("이미 `{PY_MARKER_MOVED}`이 있다"));
    }
    for change in &changes {
        let read_only = std::fs::metadata(root.join(&change.rel))
            .is_ok_and(|meta| meta.permissions().readonly());
        if read_only {
            stops.push(format!("`{}`이 읽기 전용이라 바꿀 수 없다", change.rel));
        }
    }
    stops.extend(broken.iter().map(|name| {
        format!("`{name}`의 관리 블록 표식이 한 쌍이 아니다. 시작·끝 표식을 한 쌍으로 고친다")
    }));
    if delegation_kept {
        report.push(msg::DELEGATION_KEPT.into());
    }
    for (label, items) in [
        (
            "1.24.0 기본 글 그대로라 새 템플릿으로 바꾸는 파일",
            replaced.join(", "),
        ),
        (
            "Python판 명령이 남아 있는 파일(사람이 고친 글이라 두었다. 새 명령에 맞게 고친다)",
            python.join(" / "),
        ),
        ("더하는 템플릿 파일", missing.created.join(", ")),
        ("지우는 빈 인박스 파일", inboxes.join(", ")),
        ("심볼릭 링크라 건드리지 않는 것", links.join(", ")),
    ] {
        if !items.is_empty() {
            report.push(format!("{label}: {items}."));
        }
    }
    Ok(changes)
}

fn apply(root: &Path, changes: &[Change]) -> std::io::Result<()> {
    for change in changes {
        let path = root.join(&change.rel);
        match &change.after {
            Some(text) => fullops_store::write_atomic(&path, text.as_bytes())?,
            None => std::fs::remove_file(&path)?,
        }
    }
    Ok(())
}

fn record_path(common: &Path) -> PathBuf {
    common.join("fullops/py-backup").join(ROLLBACK_RECORD)
}

fn read_record(common: &Path) -> Value {
    fullops_store::read_json(&record_path(common))
        .yes()
        .flatten()
        .unwrap_or(Value::Null)
}

fn sha_of(text: &Option<String>) -> Option<String> {
    text.as_ref().map(|t| sha256_hex(t.as_bytes()))
}

/// 끝나지 않은 전환이나 되돌리기가 이 체크아웃에 남아 있는가: Python판 표식이 그대로인데 전환이 쓴 글이 있다.
fn interrupted(root: &Path, common: &Path) -> bool {
    let record = read_record(common);
    let written = |change: &Value| {
        let now = change["path"]
            .as_str()
            .and_then(|rel| read_opt(&root.join(rel)).ok().flatten());
        now.is_some() && sha_of(&now).as_deref() == change["after_sha"].as_str()
    };
    root.join(PY_MARKER).exists()
        && record["changes"]
            .as_array()
            .is_some_and(|c| c.iter().any(written))
}

/// 전환 뒤 사람이 고친 글에서 전환이 넣은 것만 뺀다(엔트리 문서의 블록, `.gitignore`의 줄). 그 밖의 파일은 `None`이다.
fn patch_back(rel: &str, text: &str, before: &str) -> Option<String> {
    if assets::ENTRY_DOCS.contains(&rel) {
        let new = init::block_span(text, assets::BLOCK_START, assets::BLOCK_END).ok()??;
        let old = init::block_span(before, PY_BLOCK.0, PY_BLOCK.1)
            .ok()
            .flatten()
            .map_or("", |span| &before[span]);
        return Some(format!("{}{old}{}", &text[..new.start], &text[new.end..]));
    }
    if rel == IGNORE_REL {
        let added = |line: &str| {
            assets::GITIGNORE_LINES.contains(&line.trim())
                && !before.lines().any(|b| b.trim() == line.trim())
        };
        let kept = text.lines().filter(|l| !added(l));
        return Some(kept.map(|l| format!("{l}\n")).collect());
    }
    None
}

/// 되돌리기 기록대로 7 → 6 → 2 순으로 되돌린다. 전환 실패, 끝나지 않은 전환, `--rollback`이 함께 쓴다.
/// 전환이 쓴 그대로인 파일은 전환 전 글로 되돌린다. 그 뒤 사람이 고친 파일은 전환이 넣은 것만 빼거나, 뺄 수 없으면 두고 돌려준다.
/// `keep_config`면 `fullops.toml`을 지우지 않고 이름을 바꿔 남긴다(13.3절).
fn undo(root: &Path, common: &Path, keep_config: bool) -> Result<Vec<String>, CliError> {
    let record = read_record(common);
    if root.join(PY_MARKER_MOVED).exists() && !root.join(PY_MARKER).exists() {
        GitCli::cli(root.to_path_buf()).mv(PY_MARKER_MOVED, PY_MARKER)?;
    }
    // 새 표식은 이름을 바꿔 남겨 둔다(두 판의 hook이 함께 돌지 않게 한다)
    if keep_config && root.join(TOML_MARKER).exists() {
        std::fs::rename(root.join(TOML_MARKER), root.join(TOML_ROLLED_BACK))?;
    }
    let mut kept = Vec::new();
    for change in record["changes"].as_array().into_iter().flatten().rev() {
        let Some(rel) = change["path"].as_str() else {
            continue;
        };
        let path = root.join(rel);
        let before = change["before"].as_str();
        let now = read_opt(&path)?;
        if now.as_deref() == before {
            continue;
        }
        let restored = if sha_of(&now).as_deref() == change["after_sha"].as_str() {
            before.map(str::to_string)
        } else if let Some(patched) = now
            .as_deref()
            .and_then(|text| patch_back(rel, text, before.unwrap_or("")))
        {
            Some(patched).filter(|text| !text.trim().is_empty())
        } else {
            kept.push(rel.to_string());
            continue;
        };
        match restored {
            Some(text) => fullops_store::write_atomic(&path, text.as_bytes())?,
            None if now.is_some() => std::fs::remove_file(&path)?,
            None => {}
        }
    }
    // 되돌리지 않은 파일이 있으면 전환 전 글이 든 기록을 남긴다
    if kept.is_empty() && record_path(common).exists() {
        std::fs::remove_file(record_path(common))?;
    }
    Ok(kept)
}

fn kept_note(kept: &[String], common: &Path) -> Option<String> {
    (!kept.is_empty()).then(|| {
        format!(
            "전환 뒤 사람이 고친 파일은 되돌리지 않았다: {}. 전환 전 글은 `{}`의 `changes[].before`에 있다.",
            kept.join(", "),
            record_path(common).display()
        )
    })
}

/// Python판 상태 폴더에서 `(열린 결정의 ID, 보류한 통합 기록의 메시지 ID)`.
fn live_ids(dir: &Path) -> (BTreeSet<String>, BTreeSet<String>) {
    let ids = |name: &str, field: &str, live: fn(&Value) -> bool| {
        json_files(&dir.join(name))
            .iter()
            .filter(|(_, item)| live(item))
            .filter_map(|(_, item)| item[field].as_str().map(str::to_string))
            .collect()
    };
    (
        ids("fullops-decisions", "id", |d| d["status"] == "open"),
        ids("fullops-integration", "message", |i| {
            i["integrated"] != true && i["hold"].is_object()
        }),
    )
}

/// 열린 결정과 보류가 있는 통합 기록을 `state.json`으로, 완료 보고까지 간 선행 과제를 `archive.jsonl`로 가져온다(13.2절 4번).
/// `stale`은 지난 전환 때 가져왔는데 그 뒤 Python판에서 닫힌 결정과 풀린 보류다.
fn import_state(
    root: &Path,
    common: &Path,
    plan: &Plan,
    stale: &(BTreeSet<String>, BTreeSet<String>),
    report: &mut Vec<String>,
) -> Result<(), CliError> {
    // 새 표식은 마지막에 쓰므로 `discover`는 아직 이 레포를 찾지 못한다. 자리만 짓는다
    let (git_dir, _) = init::git_dirs(root)
        .ok_or_else(|| CliError::Internal("Git 디렉터리를 찾지 못했습니다".into()))?;
    let repo = fullops_store::Repo {
        root: root.to_path_buf(),
        git_dir,
        common_dir: common.to_path_buf(),
    };
    let now = fullops_store::now();
    let git = GitCli::cli(root.to_path_buf());

    let mut decisions: Vec<Decision> = Vec::new();
    // 닫힌 결정은 가져오지 않지만 번호는 잇는다(새 결정이 Python판의 번호와 겹치지 않게 한다)
    let mut next_decision = 1;
    for (path, item) in json_files(&common.join("fullops-decisions")) {
        let number = item["id"]
            .as_str()
            .and_then(|id| id.trim_start_matches('D').parse::<u32>().ok());
        next_decision = next_decision.max(number.unwrap_or(0) + 1);
        if item["status"] != "open" {
            continue;
        }
        let blocks = match &item["blocks"] {
            Value::String(one) => json!([one]),
            Value::Array(many) => json!(many),
            _ => json!([]),
        };
        let sha = item["sha"].as_str().filter(|s| s.len() == 40);
        let converted = json!({
            "id": item["id"], "status": "open", "key": item["key"], "question": item["question"],
            "options": item.get("options").cloned().unwrap_or(json!([])), "blocks": blocks,
            "asked_at": epoch_of(&item["asked_at"]).unwrap_or(now),
            "origin": item["origin"].as_str().unwrap_or("user"), "sha": sha,
            "risks": item.get("risks").cloned().unwrap_or(json!([])),
            "allow": item.get("allow").cloned().unwrap_or(json!([])),
            "expires_at": epoch_of(&item["expires_at"]), "answer": null, "decided_by": null,
            "via": null, "closed_at": null, "repository": item.get("repository"),
        });
        match serde_json::from_value(converted) {
            Ok(decision) => decisions.push(decision),
            Err(e) => report.push(format!("가져오지 못한 결정 {}: {e}", path.display())),
        }
    }
    let mut holds: Vec<Completion> = Vec::new();
    for (path, item) in json_files(&common.join("fullops-integration")) {
        if item["integrated"] == true || !item["hold"].is_object() {
            continue;
        }
        let sha = item["sha"]
            .as_str()
            .and_then(|short| git.rev_parse(&format!("{short}^{{commit}}")).yes());
        let hold = &item["hold"];
        let converted = json!({
            "id": item["message"], "dispatch": null,
            "key": item["key"].as_str().filter(|k| !k.is_empty()), "role": null, "branch": null,
            "sha": sha, "sha_source": sha.as_ref().map(|_| "body"), "direct": false,
            "outcome": "succeeded", "received_at": now,
            "hold": {"reason": hold["reason"], "owner": hold["owner"], "resume": hold["resume"], "at": now},
            "released_hold": null, "landed_at": null, "drift": null,
        });
        match serde_json::from_value(converted) {
            Ok(completion) => holds.push(completion),
            Err(e) => report.push(format!("가져오지 못한 보류 {}: {e}", path.display())),
        }
    }
    report.push(format!(
        "가져온 것: 열린 결정 {}건, 보류 {}건.",
        decisions.len(),
        holds.len()
    ));
    let dropped = fullops_store::update_state(&repo, fullops_store::CLI_WAIT, None, |state| {
        let before = state.decisions.len() + state.completions.len();
        state
            .decisions
            .retain(|d| d.closed_at.is_some() || !stale.0.contains(d.id.as_str()));
        state
            .completions
            .retain(|c| c.hold.is_none() || !stale.1.contains(c.id.as_str()));
        let dropped = before - state.decisions.len() - state.completions.len();
        state.next_decision = state.next_decision.max(next_decision);
        for decision in decisions {
            if !state.decisions.iter().any(|d| d.id == decision.id) {
                state.decisions.push(decision);
            }
        }
        for hold in holds {
            if !state.completions.iter().any(|c| c.id == hold.id) {
                state.completions.push(hold);
            }
        }
        dropped
    })?;
    if dropped > 0 {
        report.push(format!(
            "지난 전환 때 가져왔고 그 뒤 Python판에서 닫힌 결정·풀린 보류 {dropped}건은 state.json에서 뺐다."
        ));
    }

    // 살아 있는 PLANS.md 행의 선행 키 가운데 완료 보고까지 간 것은 끝난 과제로 넣는다
    let reported = |key: &TaskKey| {
        let flow: Value =
            fullops_store::read_json(&common.join(format!("fullops-flow/{key}.json")))
                .yes()
                .flatten()
                .unwrap_or(Value::Null);
        let node = flow["node"].as_str().unwrap_or("");
        FLOW_NODES
            .iter()
            .position(|n| *n == node)
            .is_some_and(|i| i >= 4)
    };
    let mut landed = fullops_store::archive_keys(&repo).yes().unwrap_or_default();
    for key in plan.rows.iter().flat_map(|row| &row.after) {
        if reported(key) && landed.insert(key.clone()) {
            fullops_store::append_archive(
                &repo,
                &ArchiveLine::Task(ArchivedTask {
                    key: key.clone(),
                    role: None,
                    sha: None,
                    received_at: now,
                    landed_at: Some(now),
                    rounds: 0,
                    dispatches: Vec::new(),
                    direct: false,
                }),
            )?;
            report.push(format!("선행 과제 {key}는 끝난 것으로 넣었다."));
        }
    }
    Ok(())
}

/// 보고서에만 내는 확인(13.2절 3·5번): lint 정규식 규칙, `PLANS.md` 문법.
/// `--dry-run` 보고서에도 같은 줄이 나온다. 돌려주는 것은 읽은 `PLANS.md`다.
fn inspect(squad: &Path, report: &mut Vec<String>) -> Plan {
    let lint: Value = fullops_store::read_json(&squad.join("lint/lint.json"))
        .yes()
        .flatten()
        .unwrap_or(Value::Null);
    for rule in lint["rules"].as_array().into_iter().flatten() {
        if let Some(Err(e)) = rule["pattern"].as_str().map(fancy_regex::Regex::new) {
            report.push(format!(
                "lint 규칙 {}의 정규식이 컴파일되지 않는다(파일은 고치지 않았다): {e}",
                rule["code"].as_str().unwrap_or("?")
            ));
        }
    }
    let text = std::fs::read_to_string(squad.join("PLANS.md")).unwrap_or_default();
    let plan = fullops_core::parse_plan(&text, assets::PLANS_MD);
    if !plan.parse_errors.is_empty() {
        let lines: Vec<String> = plan.parse_errors.iter().map(u32::to_string).collect();
        report.push(format!(
            "PLANS.md 문법 오류: {}번 줄(고치지 않았다).",
            lines.join(", ")
        ));
    }
    plan
}

fn from_python(ctx: &Ctx, dry_run: bool) -> Result<u8, CliError> {
    let root = init::git_root(&ctx.start)?;
    let squad = root.join(".fullops-squad");
    let old: Value = fullops_store::read_json(&root.join(PY_MARKER))
        .yes()
        .flatten()
        .ok_or_else(|| {
            refuse(
                &[format!("Python판 표식 `{PY_MARKER}`을 읽지 못했다")],
                "Python판을 쓰던 레포의 루트에서 실행한다. 새 레포는 `fullops init`",
            )
        })?;
    let (_, common) = init::git_dirs(&root)
        .ok_or_else(|| CliError::Internal("Git 디렉터리를 찾지 못했습니다".into()))?;
    let mut report = Vec::new();
    // 중간에 멈춘 전환이 남아 있으면 먼저 되돌리고 처음부터 다시 한다
    if interrupted(&root, &common) {
        if dry_run {
            report.push("끝나지 않은 전환이 남아 있다. 실제 실행이 먼저 그것을 되돌린다.".into());
        } else {
            let kept = undo(&root, &common, false)?;
            ctx.note("끝나지 않은 전환을 되돌리고 다시 시작한다.");
            if let Some(note) = kept_note(&kept, &common) {
                ctx.note(&note);
            }
        }
    }
    let guide = read_guide(&squad);
    let mut stops = blockers(&root, &common, &old);
    let built = match build_config(&old, &guide) {
        Ok(built) => {
            report.extend(built.report.iter().cloned());
            Some(built)
        }
        Err(why) => {
            stops.push(format!("fullops.toml을 만들 수 없다: {why}"));
            None
        }
    };
    let plan = inspect(&squad, &mut report);
    // 쓰기 전에 쓸 것을 모두 계산하고 전제를 모두 본다
    let config = built.as_ref().map(|b| b.text.as_str());
    let changes = plan_changes(&root, &old, config, &mut stops, &mut report)?;

    let human = |head: &str, report: &[String]| format!("{head}\n- {}", report.join("\n- "));
    let (Some(built), true) = (built, stops.is_empty()) else {
        ctx.note(&human("보고서(아무것도 바꾸지 않았다):", &report));
        return Err(refuse(
            &stops,
            "남은 것을 정리한 뒤 `fullops migrate --from-python`",
        ));
    };
    if dry_run {
        ctx.print(
            &format!(
                "{}\n\n만들 fullops.toml:\n{}",
                human("보고서(--dry-run. 아무것도 바꾸지 않았다):", &report),
                built.text
            ),
            json!({ "dry_run": true, "report": report, "config": built.text }),
        );
        return Ok(EXIT_OK);
    }

    // 1. Python판 상태 폴더의 사본을 새로 뜬다. 원본은 건드리지 않는다
    let backup = common.join("fullops/py-backup");
    let (was, now) = (live_ids(&backup), live_ids(&common));
    let stale = (
        was.0.difference(&now.0).cloned().collect(),
        was.1.difference(&now.1).cloned().collect(),
    );
    for name in PY_STATE_DIRS {
        let (from, to) = (common.join(name), backup.join(name));
        if to.exists() {
            std::fs::remove_dir_all(&to)?;
        }
        if from.is_dir() {
            copy_tree(&from, &to)?;
        }
    }
    // 되돌릴 재료를 먼저 쓴다: 중간에 멈춰도 다시 실행과 --rollback이 이 기록으로 되돌린다
    let journal: Vec<Value> = changes
        .iter()
        .map(|c| json!({ "path": c.rel, "before": c.before, "after_sha": sha_of(&c.after) }))
        .collect();
    fullops_store::write_json(
        &record_path(&common),
        &json!({ "v": 2, "at": fullops_store::now(), "changes": journal }),
    )?;
    // 4. 상태 가져오기 → 6·2. 파일(새 표식 fullops.toml이 마지막이다) → 7. 전환 순간
    let switched = import_state(&root, &common, &plan, &stale, &mut report)
        .and_then(|()| Ok(apply(&root, &changes)?))
        .and_then(|()| Ok(GitCli::cli(root.clone()).mv(PY_MARKER, PY_MARKER_MOVED)?));
    if let Err(e) = switched {
        // 전부 되거나 전혀 안 되거나: 이미 쓴 것을 되돌린다
        ctx.note(match undo(&root, &common, false) {
            Ok(_) => "전환을 끝내지 못해 바꾼 것을 되돌렸다. 원인을 고친 뒤 다시 실행한다.",
            Err(_) => {
                "전환을 끝내지 못했고 되돌리기도 끝내지 못했다. `fullops migrate --rollback`이 이어서 되돌린다."
            }
        });
        return Err(e);
    }
    report.push(
        "지표는 전환 시점부터 다시 센다. 전환하는 PC마다 `fullops install`을 한 번 실행한다."
            .into(),
    );
    ctx.print(
        &format!(
            "{}\n\n커밋하지 않았다. 변경과 보고서를 확인하고 커밋한다. 되돌리기는 `fullops migrate --rollback`이다.",
            human("전환했다. 보고서:", &report)
        ),
        json!({ "dry_run": false, "report": report, "root": path_text(&root) }),
    );
    Ok(EXIT_OK)
}

/// 7 → 6 순으로 되돌린다(설계 13.3절). `state.json`은 남겨 둔다.
fn rollback(ctx: &Ctx) -> Result<u8, CliError> {
    let root = init::git_root(&ctx.start)?;
    let squad = root.join(".fullops-squad");
    let (_, common) = init::git_dirs(&root)
        .ok_or_else(|| CliError::Internal("Git 디렉터리를 찾지 못했습니다".into()))?;
    if !root.join(PY_MARKER_MOVED).exists() {
        // 중간에 멈춘 전환이나 되돌리기는 여기서 끝까지 되돌린다
        if interrupted(&root, &common) {
            let kept = undo(&root, &common, false)?;
            let mut lines = vec![
                "끝나지 않은 전환을 되돌렸다. Python판 표식이 그대로이고 새 표식은 없다."
                    .to_string(),
            ];
            lines.extend(kept_note(&kept, &common));
            ctx.print(&lines.join("\n"), json!({ "kept": kept }));
            return Ok(EXIT_OK);
        }
        return Err(refuse(
            &[format!("되돌릴 전환이 없다(`{PY_MARKER_MOVED}`이 없다)")],
            "전환한 레포의 루트에서 실행한다",
        ));
    }
    let state = fullops_store::discover(&root)?
        .and_then(|repo| fullops_store::read_state(&repo).yes())
        .unwrap_or_default();
    let mut stops = Vec::new();
    for entry in std::fs::read_dir(squad.join("handovers"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let busy = std::fs::read_to_string(entry.path()).is_ok_and(|t| !t.trim().is_empty());
        if name.starts_with("to_") && name.ends_with(".md") && busy {
            stops.push(format!("역할 인박스가 비어 있지 않다: handovers/{name}"));
        }
    }
    let waiting = state.pending().count();
    if waiting > 0 {
        stops.push(format!(
            "통합 대기가 {waiting}건 있다. `fullops merge`나 `fullops hold`로 정리한다"
        ));
    }
    if !stops.is_empty() {
        return Err(refuse(
            &stops,
            "남은 것을 정리한 뒤 `fullops migrate --rollback`",
        ));
    }

    let recorded = read_record(&common)["changes"].is_array();
    let kept = undo(&root, &common, true)?;

    // 전환 뒤 생긴 열린 결정과 보류는 사람이 Python판으로 옮긴다
    let mut lines = vec![format!(
        "되돌렸다: `{PY_MARKER}`을 복원하고 `{TOML_MARKER}`을 `{TOML_ROLLED_BACK}`으로 옮겼다. state.json은 남겨 뒀다."
    )];
    if !recorded {
        lines.push(
            "되돌리기 기록(migrate.json)이 이 PC에 없어 표식만 되돌렸다. 엔트리 문서와 템플릿은 전환 커밋을 `git revert`로 되돌린다."
                .into(),
        );
    }
    lines.extend(kept_note(&kept, &common));
    let open: Vec<String> = state
        .decisions
        .iter()
        .filter(|d| d.closed_at.is_none())
        .map(|d| format!("{}: {}", d.id, d.question))
        .collect();
    let held: Vec<String> = state
        .completions
        .iter()
        .filter_map(|c| {
            let hold = c.hold.as_ref()?;
            let key = c.key.as_ref().map_or("?".to_string(), ToString::to_string);
            Some(format!(
                "{key}: {} (담당 {}, 재개 {})",
                hold.reason, hold.owner, hold.resume
            ))
        })
        .collect();
    if !open.is_empty() {
        lines.push(format!(
            "열린 결정(Python판의 `decision.py add`로 옮긴다):\n  {}",
            open.join("\n  ")
        ));
    }
    if !held.is_empty() {
        lines.push(format!(
            "보류(Python판의 `integration.py hold`로 옮긴다):\n  {}",
            held.join("\n  ")
        ));
    }
    lines.push("커밋하지 않았다. 변경을 확인하고 커밋한다.".into());
    ctx.print(
        &lines.join("\n"),
        json!({ "open_decisions": open, "holds": held, "kept": kept }),
    );
    Ok(EXIT_OK)
}

/// 전환 뒤 정리를 기다리는 날 수(설계 13.4절의 '2주').
const FINALIZE_DAYS: i64 = 14;

/// 되돌리기 재료. 정리에서 맨 마지막에 옮긴다.
const PY_BACKUP: &str = "fullops/py-backup";

/// 작업 트리마다(`--absolute-git-dir` 아래) Python판이 쓰던 것.
const PY_PER_TREE: [&str; 4] = [
    "fullops-gate",
    "fullops-search",
    "fullops-setup.json",
    "fullops-lint.lock",
];

/// Git 공용 디렉터리 기준으로 정리하는 것: Python판 상태(작업 트리별 것은 `worktrees/<이름>/` 아래), 맨 끝에 되돌리기 재료.
fn retired_paths(common: &Path) -> Vec<String> {
    let mut rels: Vec<String> = PY_STATE_DIRS.iter().map(|d| d.to_string()).collect();
    rels.extend(["fullops-snapshots", "fullops-mode-transition.json"].map(String::from));
    rels.extend(PY_PER_TREE.map(String::from));
    for tree in std::fs::read_dir(common.join("worktrees"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = tree.file_name().to_string_lossy().into_owned();
        rels.extend(PY_PER_TREE.map(|d| format!("worktrees/{name}/{d}")));
    }
    rels.push(PY_BACKUP.to_string());
    rels.retain(|rel| common.join(rel).exists());
    rels
}

/// 전환 2주 뒤 정리(설계 13.5절). `--early`는 날짜 검사만 넘긴다. 기본은 계획만 내고 `--apply`에서 `<common>/fullops/py-retired/<시각>/`로 옮긴다. 지우지 않는다.
fn finalize(ctx: &Ctx, apply: bool, early: bool) -> Result<u8, CliError> {
    let root = init::git_root(&ctx.start)?;
    let (_, common) = init::git_dirs(&root)
        .ok_or_else(|| CliError::Internal("Git 디렉터리를 찾지 못했습니다".into()))?;
    let mut stops = Vec::new();
    if root.join(PY_MARKER).exists() || !root.join(TOML_MARKER).exists() {
        stops.push(
            "전환한 레포가 아니다(`fullops.toml`이 없거나 `fullops.json`이 남아 있다)".into(),
        );
    }
    // 옛 hook이 아직 상태 폴더를 쓰는 워크트리가 있으면 옮기지 않는다
    match GitCli::cli(root.clone()).worktrees() {
        Known::Yes(trees) => stops.extend(
            trees
                .iter()
                .filter(|t| t.path.join(PY_MARKER).exists())
                .map(|t| format!("아직 Python판 표식이 있는 워크트리: {}", t.path.display())),
        ),
        Known::Unknown(_) => stops.push("워크트리 목록을 읽지 못했다".into()),
    }
    let mut notes = Vec::new();
    if !record_path(&common).exists() {
        notes.push(
            "되돌리기 기록(migrate.json)이 이 PC에 없다. 전환 시각을 몰라 날짜를 보지 않았다. 이 정리 뒤에는 `--rollback`이 표식만 되돌린다."
                .to_string(),
        );
    } else {
        match read_record(&common)["at"].as_i64() {
            Some(at) => {
                let days = (fullops_store::now() - at) / (24 * 60 * 60);
                if days < FINALIZE_DAYS && !early {
                    stops.push(format!(
                        "전환한 지 {days}일이다. {FINALIZE_DAYS}일이 지난 뒤 정리한다"
                    ));
                }
            }
            // 기록이 있는데 시각을 못 읽으면 날짜를 모른다. 없는 것으로 치지 않는다
            None if !early => stops
                .push("되돌리기 기록(migrate.json)에서 전환 시각(`at`)을 읽지 못했다".to_string()),
            None => {}
        }
    }
    if !stops.is_empty() {
        return Err(refuse(
            &stops,
            "모든 워크트리를 옮기고 전환 뒤 2주가 지난 다음 `fullops migrate --finalize`. 날짜를 무시하려면 `--early`",
        ));
    }
    let moves = retired_paths(&common);
    let to = common
        .join("fullops/py-retired")
        .join(fullops_store::now().to_string());
    let git_rm = root
        .join(PY_MARKER_MOVED)
        .exists()
        .then(|| format!("git rm {PY_MARKER_MOVED}"));
    if apply {
        // 하나가 실패해도 나머지를 옮기고, 무엇을 어디로 옮겼는지 남긴다
        let (mut moved, mut failed) = (Vec::new(), Vec::new());
        for rel in &moves {
            // 다른 것이 실패했으면 되돌리기 재료는 두어 `--rollback`이 그대로 되게 한다
            if rel == PY_BACKUP && !failed.is_empty() {
                failed.push(format!("{rel}: 다른 항목이 실패해 옮기지 않았다"));
                continue;
            }
            let target = to.join(rel);
            let result = std::fs::create_dir_all(target.parent().unwrap_or(&to))
                .and_then(|()| std::fs::rename(common.join(rel), &target));
            match result {
                Ok(()) => moved.push(format!("  {rel}")),
                Err(e) => failed.push(format!("{rel}: {e}")),
            }
        }
        if !failed.is_empty() {
            let mut text = vec![format!(
                "정리 중 일부를 옮기지 못했다. 옮긴 것은 {}에 같은 상대 경로로 있다(되돌리려면 제자리로 다시 옮긴다):",
                to.display()
            )];
            text.extend(moved);
            text.push("옮기지 못한 것(파일을 연 프로그램을 닫고 `fullops migrate --finalize --apply`를 다시 실행하면 남은 것만 새 폴더로 옮긴다):".into());
            text.extend(failed.iter().map(|f| format!("  {f}")));
            return Err(CliError::Internal(text.join("\n")));
        }
    }
    let mut lines = vec![if moves.is_empty() {
        "옮길 것이 없다.".to_string()
    } else if apply {
        format!(
            "옮겼다(되돌리려면 같은 상대 경로로 다시 옮긴다) → {}:",
            to.display()
        )
    } else {
        format!(
            "계획(--apply로 실행한다. 지우지 않고 옮긴다) → {}:",
            to.display()
        )
    }];
    lines.extend(moves.iter().map(|rel| format!("  {rel}")));
    lines.extend(notes.iter().cloned());
    if let Some(cmd) = &git_rm {
        lines.push(format!(
            "옛 표식은 사람이 지우고 커밋한다: `{cmd}`. 커밋하지 않았다."
        ));
    }
    ctx.print(
        &lines.join("\n"),
        json!({ "apply": apply, "to": path_text(&to), "moves": moves, "notes": notes, "git_rm": git_rm }),
    );
    Ok(EXIT_OK)
}

pub fn run(ctx: &Ctx, mut parser: lexopt::Parser) -> Result<u8, CliError> {
    use lexopt::prelude::*;
    let (mut python, mut dry_run, mut back) = (false, false, false);
    let (mut fin, mut apply, mut early) = (false, false, false);
    while let Some(arg) = parser.next()? {
        match arg {
            Long("from-python") => python = true,
            Long("dry-run") => dry_run = true,
            Long("rollback") => back = true,
            Long("finalize") => fin = true,
            Long("apply") => apply = true,
            Long("early") => early = true,
            other => return Err(other.unexpected().into()),
        }
    }
    let plain = !(apply || early);
    match (python, back, fin) {
        (true, false, false) if plain => from_python(ctx, dry_run),
        // 되돌리기에는 미리 보기가 없다. --dry-run을 함께 주면 실행하지 않고 사용법을 낸다
        (false, true, false) if !dry_run && plain => rollback(ctx),
        // 정리는 기본이 계획이다. --dry-run은 받지 않는다(실행은 --apply로만 한다)
        (false, false, true) if !dry_run => finalize(ctx, apply, early),
        _ => Err(CliError::Usage(
            "사용법: fullops migrate --from-python [--dry-run] | fullops migrate --rollback | fullops migrate --finalize [--apply] [--early]".into(),
        )),
    }
}
