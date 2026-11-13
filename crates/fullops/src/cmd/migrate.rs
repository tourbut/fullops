//! `fullops migrate`: Python판(1.24.0)에서 전환과 되돌리기(설계 13절).
//! `--from-python`은 1~6단계까지 Python판 동작에 영향이 없고 7단계(`fullops.json` 이름 바꾸기)가 전환 순간이다. 커밋은 하지 않는다.
//! Python판의 상태 폴더는 읽기만 하고 `<git-common-dir>/fullops/py-backup/`에 사본을 둔다.

use super::init::{self, NewConfig, NewRole};
use crate::assets;
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::messages::install as msg;
use fullops_core::model::{
    ArchiveLine, ArchivedTask, Completion, Decision, Known, Mode, Parallel, Refusal, RoleId,
    RoleKind, Routing, TaskKey, TestLevel, path_text, sha256_hex,
};
use fullops_ext::{Git, GitCli};
use serde_json::{Value, json};
use std::collections::BTreeMap;
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

/// 엔트리 문서의 Python판 관리 블록을 새 블록으로 바꾼다. 돌려주는 것은 `(새 글, 있던 Python 블록)`.
fn swap_block(text: &str) -> (String, Option<String>) {
    let span = text.find(PY_BLOCK.0).and_then(|start| {
        let end = text[start..].find(PY_BLOCK.1)? + start + PY_BLOCK.1.len();
        Some((start, end))
    });
    match span {
        Some((start, end)) => {
            let old = text[start..end].to_string();
            let without = format!("{}{}", &text[..start], &text[end..]);
            (init::with_block(&without), Some(old))
        }
        None => (init::with_block(text), None),
    }
}

/// 열린 결정과 보류가 있는 통합 기록을 `state.json`으로, 완료 보고까지 간 선행 과제를 `archive.jsonl`로 가져온다(13.2절 4번).
fn import_state(root: &Path, common: &Path, report: &mut Vec<String>) -> Result<(), CliError> {
    let repo = fullops_store::discover(root)?
        .ok_or_else(|| CliError::Internal("전환한 레포를 찾지 못했습니다".into()))?;
    let now = fullops_store::now();
    let git = GitCli::cli(root.to_path_buf());

    let mut decisions: Vec<Decision> = Vec::new();
    for (path, item) in json_files(&common.join("fullops-decisions")) {
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
    fullops_store::update_state(&repo, fullops_store::CLI_WAIT, None, |state| {
        for decision in decisions {
            if !state.decisions.iter().any(|d| d.id == decision.id) {
                let number: u32 = decision
                    .id
                    .as_str()
                    .trim_start_matches('D')
                    .parse()
                    .unwrap_or(0);
                state.next_decision = state.next_decision.max(number + 1);
                state.decisions.push(decision);
            }
        }
        for hold in holds {
            if !state.completions.iter().any(|c| c.id == hold.id) {
                state.completions.push(hold);
            }
        }
    })?;

    // 살아 있는 PLANS.md 행의 선행 키 가운데 완료 보고까지 간 것은 끝난 과제로 넣는다
    let plan = fullops_store::read_plan(&repo, assets::PLANS_MD)
        .yes()
        .unwrap_or_default();
    if !plan.parse_errors.is_empty() {
        let lines: Vec<String> = plan.parse_errors.iter().map(u32::to_string).collect();
        report.push(format!(
            "PLANS.md 문법 오류: {}번 줄(고치지 않았다).",
            lines.join(", ")
        ));
    }
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

/// 보고서에만 내는 확인: lint 정규식 규칙, `requires_packet`이 켜진 옛 route 기록.
fn inspect(squad: &Path, report: &mut Vec<String>) {
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
    let packets: Vec<String> = json_files(&squad.join("docs/evaluations/jev"))
        .into_iter()
        .filter(|(_, v)| v["requires_packet"] == true)
        .filter_map(|(p, _)| {
            Some(
                p.file_name()?
                    .to_str()?
                    .strip_suffix("-route.json")?
                    .to_string(),
            )
        })
        .collect();
    if !packets.is_empty() {
        report.push(format!(
            "requires_packet이 켜진 옛 route 기록: {}. 같은 키를 다시 분류하면 그 값이 옮겨진다.",
            packets.join(", ")
        ));
    }
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
    let guide = read_guide(&squad);
    let mut stops = blockers(&root, &common, &old);
    let mut report = Vec::new();
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
    inspect(&squad, &mut report);
    let delegation = squad.join("rules/delegation.md");
    let delegation_text = std::fs::read_to_string(&delegation).ok();
    let stock = delegation_text.as_ref().is_some_and(|t| {
        sha256_hex(t.replace("\r\n", "\n").as_bytes()) == assets::DELEGATION_1_24_SHA256
    });
    if delegation_text.is_some() && !stock {
        report.push(msg::DELEGATION_KEPT.into());
    }

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

    // 1. Python판 상태 폴더의 사본. 원본은 건드리지 않는다
    let backup = common.join("fullops/py-backup");
    for name in PY_STATE_DIRS {
        let (from, to) = (common.join(name), backup.join(name));
        if from.is_dir() && !to.exists() {
            copy_tree(&from, &to)?;
        }
    }
    // 2. fullops.toml
    fullops_store::write_atomic(&root.join(TOML_MARKER), built.text.as_bytes())?;
    // 4·5. 상태 가져오기와 PLANS.md 검사(3번의 정규식 확인은 위 inspect가 했다)
    import_state(&root, &common, &mut report)?;

    // 6. 관리 블록, .gitignore 줄, 없는 템플릿, 빈 인박스 파일
    let mut blocks = serde_json::Map::new();
    for name in assets::ENTRY_DOCS {
        let path = root.join(name);
        let before = std::fs::read_to_string(&path).unwrap_or_default();
        let (after, old_block) = swap_block(&before);
        if after != before {
            fullops_store::write_atomic(&path, after.as_bytes())?;
        }
        blocks.insert(name.to_string(), json!(old_block));
    }
    let ignore_path = squad.join(".gitignore");
    let mut ignore = std::fs::read_to_string(&ignore_path).unwrap_or_default();
    let mut added_lines = Vec::new();
    for line in assets::GITIGNORE_LINES {
        if !ignore.lines().any(|l| l.trim() == *line) {
            if !ignore.is_empty() && !ignore.ends_with('\n') {
                ignore.push('\n');
            }
            ignore.push_str(&format!("{line}\n"));
            added_lines.push(*line);
        }
    }
    if !added_lines.is_empty() {
        fullops_store::write_atomic(&ignore_path, ignore.as_bytes())?;
    }
    if stock {
        // 1.24.0의 템플릿 그대로면 새 템플릿으로 바꾼다. 옛 글은 되돌리기를 위해 둔다
        std::fs::create_dir_all(&backup)?;
        std::fs::copy(&delegation, backup.join("delegation.md"))?;
        fullops_store::write_atomic(&delegation, assets::DELEGATION_MD.as_bytes())?;
        report.push("rules/delegation.md를 새 템플릿으로 바꿨다.".into());
    }
    let mut done = init::Applied::default();
    init::add_templates(&root, false, &mut done)?;
    if !done.created.is_empty() {
        report.push(format!("더한 템플릿 파일: {}.", done.created.join(", ")));
    }
    // 새 판은 인박스 파일이 없는 것이 빈 인박스다. Python판이 남긴 빈 파일은 지운다
    let mut removed = Vec::new();
    for role in old["roles"].as_object().into_iter().flat_map(|r| r.keys()) {
        let rel = format!(".fullops-squad/handovers/to_{role}.md");
        if std::fs::remove_file(root.join(&rel)).is_ok() {
            removed.push(rel);
        }
    }
    if !removed.is_empty() {
        report.push(format!("빈 인박스 파일을 지웠다: {}.", removed.join(", ")));
    }
    fullops_store::write_json(
        &backup.join(ROLLBACK_RECORD),
        &json!({
            "v": 1, "at": fullops_store::now(), "blocks": blocks, "gitignore_added": added_lines,
            "removed_inboxes": removed, "delegation_replaced": stock,
        }),
    )?;

    // 7. 전환 순간
    GitCli::cli(root.clone()).mv(PY_MARKER, PY_MARKER_MOVED)?;
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
    if !root.join(PY_MARKER_MOVED).exists() {
        return Err(refuse(
            &[format!("되돌릴 전환이 없다(`{PY_MARKER_MOVED}`이 없다)")],
            "전환한 레포의 루트에서 실행한다",
        ));
    }
    let (_, common) = init::git_dirs(&root)
        .ok_or_else(|| CliError::Internal("Git 디렉터리를 찾지 못했습니다".into()))?;
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

    let backup = common.join("fullops/py-backup");
    let record: Value = fullops_store::read_json(&backup.join(ROLLBACK_RECORD))
        .yes()
        .flatten()
        .unwrap_or(Value::Null);
    // 7. 표식을 되돌린다. 새 표식은 이름을 바꿔 남겨 둔다(두 판의 hook이 함께 돌지 않게 한다)
    GitCli::cli(root.clone()).mv(PY_MARKER_MOVED, PY_MARKER)?;
    if root.join(TOML_MARKER).exists() {
        std::fs::rename(root.join(TOML_MARKER), root.join(TOML_ROLLED_BACK))?;
    }
    // 6. 관리 블록과 .gitignore 줄, delegation.md, 빈 인박스 파일
    for name in assets::ENTRY_DOCS {
        let path = root.join(name);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (Some(start), Some(end)) =
            (text.find(assets::BLOCK_START), text.find(assets::BLOCK_END))
        else {
            continue;
        };
        let old_block = record["blocks"][name].as_str().unwrap_or("");
        let end = end + assets::BLOCK_END.len();
        let restored = format!("{}{old_block}{}", &text[..start], &text[end..]);
        if restored.trim().is_empty() {
            std::fs::remove_file(&path)?;
        } else {
            fullops_store::write_atomic(&path, restored.as_bytes())?;
        }
    }
    let added: Vec<&str> = record["gitignore_added"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !added.is_empty()
        && let Ok(text) = std::fs::read_to_string(squad.join(".gitignore"))
    {
        let kept: String = text
            .lines()
            .filter(|l| !added.contains(&l.trim()))
            .map(|l| format!("{l}\n"))
            .collect();
        fullops_store::write_atomic(&squad.join(".gitignore"), kept.as_bytes())?;
    }
    if record["delegation_replaced"] == true && backup.join("delegation.md").is_file() {
        std::fs::copy(
            backup.join("delegation.md"),
            squad.join("rules/delegation.md"),
        )?;
    }
    for rel in record["removed_inboxes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if !root.join(rel).exists() {
            fullops_store::write_atomic(&root.join(rel), b"")?;
        }
    }

    // 전환 뒤 생긴 열린 결정과 보류는 사람이 Python판으로 옮긴다
    let mut lines = vec![format!(
        "되돌렸다: `{PY_MARKER}`을 복원하고 `{TOML_MARKER}`을 `{TOML_ROLLED_BACK}`으로 옮겼다. state.json은 남겨 뒀다."
    )];
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
        json!({ "open_decisions": open, "holds": held }),
    );
    Ok(EXIT_OK)
}

pub fn run(ctx: &Ctx, mut parser: lexopt::Parser) -> Result<u8, CliError> {
    use lexopt::prelude::*;
    let (mut python, mut dry_run, mut back) = (false, false, false);
    while let Some(arg) = parser.next()? {
        match arg {
            Long("from-python") => python = true,
            Long("dry-run") => dry_run = true,
            Long("rollback") => back = true,
            other => return Err(other.unexpected().into()),
        }
    }
    match (python, back) {
        (true, false) => from_python(ctx, dry_run),
        // 되돌리기에는 미리 보기가 없다. --dry-run을 함께 주면 실행하지 않고 사용법을 낸다
        (false, true) if !dry_run => rollback(ctx),
        _ => Err(CliError::Usage(
            "사용법: fullops migrate --from-python [--dry-run] | fullops migrate --rollback".into(),
        )),
    }
}
