//! 레포 폴더 `.fullops/` 안의 자리(설계 6.1절). 레포 상대 경로(슬래시 표기)는 이 모듈에서만 만든다.
//! 옛 자리 `.fullops-squad/`에서 새 자리로 가는 대응(`moved`, `rewrite`)도 여기 있다(설계 13.6절).

use crate::model::{RoleId, TaskKey};
use std::fmt::Display;

pub const DIR: &str = ".fullops";
/// 2.0.x까지의 레포 폴더. `migrate --layout`이 `DIR`로 옮긴다.
pub const LEGACY_DIR: &str = ".fullops-squad";

/// 흐름 기록의 pathspec(설계 6.6절 5번). 판정은 `is_record`가 한다.
pub const RECORDS: [&str; 3] = [".fullops/PLANS.md", ".fullops/inbox/", ".fullops/tasks/"];

/// `.fullops/<rel>`.
pub fn at(rel: impl Display) -> String {
    format!("{DIR}/{rel}")
}

/// `.fullops/` 아래 경로면 그 폴더 기준 상대 경로.
pub fn inside(path: &str) -> Option<&str> {
    path.strip_prefix(DIR)?.strip_prefix('/')
}

/// 활성 표식이자 설정.
pub const fn config_toml() -> &'static str {
    ".fullops/fullops.toml"
}
pub const fn plans() -> &'static str {
    ".fullops/PLANS.md"
}
pub const fn lint_json() -> &'static str {
    ".fullops/config/lint.json"
}
pub const fn review_json() -> &'static str {
    ".fullops/config/review.json"
}
/// 역할 인박스가 있는 폴더(끝에 `/`가 없다).
pub const fn inbox_dir() -> &'static str {
    ".fullops/inbox"
}
pub fn inbox(role: impl Display) -> String {
    format!(".fullops/inbox/to_{role}.md")
}
/// 과제 폴더들이 있는 폴더(끝에 `/`가 없다).
pub const fn tasks_dir() -> &'static str {
    ".fullops/tasks"
}
pub fn task_dir(key: impl Display) -> String {
    format!(".fullops/tasks/{key}")
}
pub fn route(key: impl Display) -> String {
    format!(".fullops/tasks/{key}/route.json")
}
pub fn request(key: impl Display) -> String {
    format!(".fullops/tasks/{key}/request.md")
}
/// 끝난 지시서와 완료 보고. `done`이 인박스를 여기로 옮긴다.
pub fn handover_log(key: impl Display, role: impl Display, round: u32) -> String {
    format!(".fullops/tasks/{key}/{role}.r{round}.md")
}
pub fn reviews_dir(key: impl Display) -> String {
    format!(".fullops/tasks/{key}/reviews")
}
pub fn review_dir(key: impl Display, head12: impl Display) -> String {
    format!(".fullops/tasks/{key}/reviews/{head12}")
}
pub fn qa_dir(key: impl Display) -> String {
    format!(".fullops/tasks/{key}/qa")
}
pub fn qa_report(key: impl Display) -> String {
    format!(".fullops/tasks/{key}/qa/report.md")
}
pub const fn deliverables_index() -> &'static str {
    ".fullops/docs/deliverables/README.md"
}
pub fn guide(name: impl Display) -> String {
    format!(".fullops/guide/{name}")
}
/// 역할 교훈 파일들이 있는 폴더(끝에 `/`가 없다).
pub const fn roles_dir() -> &'static str {
    ".fullops/guide/roles"
}
/// 그 역할의 결정·교훈 메모.
pub fn role_note(role: impl Display) -> String {
    format!(".fullops/guide/roles/{role}.md")
}
pub fn template(name: impl Display) -> String {
    format!(".fullops/templates/{name}")
}
pub const fn archive_dir() -> &'static str {
    ".fullops/archive"
}
/// 커밋하지 않는 작업용 폴더.
pub const fn local_dir() -> &'static str {
    ".fullops/local"
}

/// `context`의 필수 문서(고정 여섯. 설계 8.14절).
pub const REQUIRED_DOCS: [&str; 6] = [
    ".fullops/guide/FULLOPS.md",
    ".fullops/guide/project.md",
    ".fullops/guide/rules/common/README.md",
    ".fullops/guide/rules/common/coding-style.md",
    ".fullops/guide/rules/common/security.md",
    ".fullops/guide/rules/common/testing.md",
];

/// 흐름 기록인가: `PLANS.md`, `inbox/`, `tasks/` 아래. `changed()`의 대상에서 빼고 기록 전용 커밋을 가릴 때 쓴다.
pub fn is_record(path: &str) -> bool {
    path == RECORDS[0] || RECORDS[1..].iter().any(|dir| path.starts_with(dir))
}

/// `tasks/<KEY>/qa/<안쪽 경로>`의 `(KEY, 안쪽 경로)`. 레포 상대 경로를 받는다.
pub fn qa_parts(path: &str) -> Option<(&str, &str)> {
    let (key, rest) = path.strip_prefix(".fullops/tasks/")?.split_once('/')?;
    Some((key, rest.strip_prefix("qa/")?))
}

// ── 옛 자리 → 새 자리 ─────────────────────────────────────────

/// 파일 하나의 대응 `(옛 자리, 새 자리)`. 둘 다 레포 폴더 기준이다.
const FILES: [(&str, &str); 9] = [
    ("lint/lint.json", "config/lint.json"),
    ("review/rule.json", "config/review.json"),
    ("lint/README.md", "guide/lint.md"),
    ("FULLOPS.md", "guide/FULLOPS.md"),
    ("project.md", "guide/project.md"),
    ("orca-agents.md", "guide/orca-agents.md"),
    ("handovers/_TEMPLATE.md", "templates/handover.md"),
    ("review/_REPORT.md", "templates/review-report.md"),
    ("docs/planning/intents/_TEMPLATE.md", "templates/intent.md"),
];

/// 폴더의 대응. 긴 접두어가 앞이다.
const FOLDERS: [(&str, &str); 7] = [
    ("contexts/archive/", "archive/contexts/"),
    ("contexts/", "guide/roles/"),
    ("rules/", "guide/rules/"),
    ("docs/agents/", "guide/agents/"),
    ("board/", "archive/board/"),
    ("lint/", "config/"),
    ("review/", "config/"),
];

/// 글에 끝 `/` 없이 적힌 폴더의 대응.
const BARE: [(&str, &str); 8] = [
    ("handovers/logs", "tasks"),
    ("handovers", "inbox"),
    ("routes", "tasks"),
    ("reviews", "tasks"),
    ("docs/evaluations/qa-reports", "tasks"),
    ("contexts", "guide/roles"),
    ("rules", "guide/rules"),
    ("docs/agents", "guide/agents"),
];

/// 1.24.0 OCR이 `*-review/`에 남긴 중간 파일. 옮기지 않고 추적에서 뺀다.
const OCR_SCRATCH: [&str; 3] = ["preview.json", "rules.json", "lint.json"];

/// 글에 적힌 자리표시(`<KEY>`, `<역할>`)인가.
fn slot(text: &str) -> bool {
    text.len() > 2 && text.starts_with('<') && text.ends_with('>')
}
fn is_key(text: &str) -> bool {
    TaskKey::new(text).is_ok() || slot(text)
}
fn is_role(text: &str) -> bool {
    RoleId::new(text).is_ok() || slot(text)
}
/// 폴더만 가리키는 꼬리인가(빈 글, glob).
fn whole(rest: &str) -> bool {
    rest.is_empty() || rest.starts_with('*')
}

/// `<KEY>.<역할>.r<차수>.md`의 `(KEY, <역할>.r<차수>.md)`.
fn log_parts(name: &str) -> Option<(&str, &str)> {
    let (head, round) = name.strip_suffix(".md")?.rsplit_once('.')?;
    let (key, role) = head.rsplit_once('.')?;
    let n = round.strip_prefix('r')?;
    let numbered = !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit());
    (is_key(key) && is_role(role) && (numbered || slot(n))).then(|| (key, &name[key.len() + 1..]))
}

/// 옛 자리의 경로(`.fullops-squad/` 기준)가 새 자리(`.fullops/` 기준)에서 가는 곳(설계 13.6절의 표).
/// `None`은 옮기지 않고 추적에서 빼는 파일이다. `has_task(KEY)`는 그 키의 `tasks/<KEY>/`가 있는가다(QA 폴더의 자리를 정한다).
/// 글 안의 경로도 이 함수로 고치므로 폴더만 적은 것(`handovers/`), glob(`handovers/**`), 자리표시(`<KEY>`)도 받는다.
pub fn moved(old: &str, has_task: &dyn Fn(&str) -> bool) -> Option<String> {
    let exact = |(from, _): &&(&str, &str)| *from == old;
    if let Some((_, new)) = FILES.iter().find(exact).or(BARE.iter().find(exact)) {
        return Some(new.to_string());
    }
    if old.starts_with("docs/evaluations/jev/") {
        return None;
    }
    if let Some(name) = old.strip_prefix("handovers/logs/") {
        return Some(match log_parts(name) {
            Some((key, file)) => format!("tasks/{key}/{file}"),
            None if whole(name) => format!("tasks/{name}"),
            None => format!("archive/handovers/{name}"),
        });
    }
    if let Some(name) = old.strip_prefix("handovers/") {
        return Some(if whole(name) || name.starts_with("to_") {
            format!("inbox/{name}")
        } else {
            format!("archive/handovers/{name}")
        });
    }
    if let Some(name) = old.strip_prefix("routes/") {
        let key = |suffix: &str| name.strip_suffix(suffix).filter(|k| is_key(k));
        return Some(if let Some(key) = key(".request.md") {
            format!("tasks/{key}/request.md")
        } else if let Some(key) = key(".json") {
            format!("tasks/{key}/route.json")
        } else if whole(name) {
            format!("tasks/{name}")
        } else {
            format!("archive/routes/{name}")
        });
    }
    if let Some(rest) = old.strip_prefix("reviews/") {
        return Some(match rest.split_once('/') {
            Some((key, tail)) if is_key(key) => format!("tasks/{key}/reviews/{tail}"),
            _ if whole(rest) => format!("tasks/{rest}"),
            _ => format!("archive/reviews/{rest}"),
        });
    }
    if let Some(rest) = old.strip_prefix("docs/evaluations/qa-reports/") {
        if whole(rest) {
            return Some(format!("tasks/{rest}"));
        }
        let (folder, tail) = rest.split_once('/').unwrap_or((rest, ""));
        if let Some(stem) = folder.strip_suffix("-test") {
            let task = |key: &str| has_task(key) || slot(key);
            if task(stem) {
                return Some(format!("tasks/{stem}/qa/{tail}"));
            }
            // `<KEY>-<역할>-test`: 가장 긴 키부터 맞춘다
            let split = stem.rmatch_indices('-').find_map(|(at, _)| {
                let (key, role) = (&stem[..at], &stem[at + 1..]);
                (task(key) && is_role(role)).then_some((key, role))
            });
            if let Some((key, role)) = split {
                return Some(if tail == "report.md" {
                    format!("tasks/{key}/qa/{role}.md")
                } else {
                    format!("tasks/{key}/qa/{tail}")
                });
            }
        }
        if folder.ends_with("-review") && OCR_SCRATCH.contains(&tail) {
            return None;
        }
        return Some(format!("archive/qa-reports/{rest}"));
    }
    if let Some((from, to)) = FOLDERS.iter().find(|(from, _)| old.starts_with(from)) {
        return Some(format!("{to}{}", &old[from.len()..]));
    }
    Some(old.to_string())
}

/// 글에서 경로가 끝나는 자리. 공백·따옴표·괄호에서 끝나지만 자리표시(`<과제 키>`) 안의 공백은 경로의 일부다.
fn token_end(path: &str) -> usize {
    let mut open = false;
    for (at, c) in path.char_indices() {
        match c {
            '`' | '\n' => return at,
            // 같은 줄에서 닫히는 `<`만 자리표시로 본다
            '<' if !open => {
                let line = path[at..].lines().next().unwrap_or("");
                open = line.split('`').next().is_some_and(|l| l.contains('>'));
            }
            '>' => open = false,
            _ if !open && (c.is_whitespace() || "\"'()[]{},|;".contains(c)) => return at,
            _ => {}
        }
    }
    path.len()
}

/// 글 안의 `.fullops-squad/<옛 경로>`를 `.fullops/<새 경로>`로, 그 밖의 `.fullops-squad`를 `.fullops`로 고친다.
/// 사용자 홈의 `~/.fullops-squad`(Python판 런타임)와 다른 이름의 일부(`.fullops-squad-x`)는 두고, 지워지는 경로는 접두어만 바꾼다.
pub fn rewrite(text: &str, has_task: &dyn Fn(&str) -> bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(LEGACY_DIR) {
        let (before, after) = (&rest[..at], &rest[at + LEGACY_DIR.len()..]);
        out.push_str(before);
        let home = before.ends_with("~/") || before.ends_with("~\\");
        let joined = after.starts_with(|c: char| c.is_alphanumeric() || c == '-' || c == '_');
        if home || joined {
            out.push_str(LEGACY_DIR);
            rest = after;
            continue;
        }
        out.push_str(DIR);
        let Some(path) = after.strip_prefix('/') else {
            rest = after;
            continue;
        };
        // 문장 끝의 마침표·쌍점은 경로가 아니다
        let token = path[..token_end(path)].trim_end_matches(['.', ':']);
        out.push('/');
        out.push_str(&moved(token, has_task).unwrap_or_else(|| token.to_string()));
        rest = &path[token.len()..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_plans_inbox_and_tasks() {
        for path in [
            ".fullops/PLANS.md",
            ".fullops/inbox/to_dev.md",
            ".fullops/tasks/PAY-12/route.json",
            ".fullops/tasks/PAY-12/dev.r0.md",
            ".fullops/tasks/PAY-12/reviews/0123456789ab/result.json",
            ".fullops/tasks/PAY-12/qa/report.md",
        ] {
            assert!(is_record(path), "{path}");
        }
        for path in [
            ".fullops/PLANS.md.bak",
            ".fullops/fullops.toml",
            ".fullops/config/lint.json",
            ".fullops/guide/FULLOPS.md",
            ".fullops/docs/planning/a.md",
            ".fullops/archive/handovers/2026-10-01.md",
            ".fullops/inbox",
            ".fullops-squad/PLANS.md",
            "src/tasks/a.rs",
        ] {
            assert!(!is_record(path), "{path}");
        }
        assert_eq!(
            qa_parts(".fullops/tasks/PAY-12/qa/shots/a.png"),
            Some(("PAY-12", "shots/a.png"))
        );
        assert_eq!(qa_parts(".fullops/tasks/PAY-12/route.json"), None);
        assert_eq!(inside(".fullops/guide/lint.md"), Some("guide/lint.md"));
        assert_eq!(inside(".fullops-squad/x"), None);
    }

    #[test]
    fn old_paths_move_by_the_table() {
        let tasks = |key: &str| key == "PAY-12";
        let to = |old: &str| moved(old, &tasks);
        for (old, new) in [
            ("fullops.toml", "fullops.toml"),
            ("PLANS.md", "PLANS.md"),
            (".gitignore", ".gitignore"),
            (".env.local", ".env.local"),
            ("local/tool.log", "local/tool.log"),
            ("lint/lint.json", "config/lint.json"),
            ("review/rule.json", "config/review.json"),
            ("lint/README.md", "guide/lint.md"),
            ("FULLOPS.md", "guide/FULLOPS.md"),
            ("project.md", "guide/project.md"),
            ("orca-agents.md", "guide/orca-agents.md"),
            ("rules/common/testing.md", "guide/rules/common/testing.md"),
            ("rules/delegation.md", "guide/rules/delegation.md"),
            ("contexts/dev.md", "guide/roles/dev.md"),
            (
                "contexts/archive/2026/dev.md",
                "archive/contexts/2026/dev.md",
            ),
            ("docs/agents/domain.md", "guide/agents/domain.md"),
            ("handovers/_TEMPLATE.md", "templates/handover.md"),
            ("review/_REPORT.md", "templates/review-report.md"),
            ("docs/planning/intents/_TEMPLATE.md", "templates/intent.md"),
            (
                "docs/planning/intents/login.md",
                "docs/planning/intents/login.md",
            ),
            ("handovers/to_dev.md", "inbox/to_dev.md"),
            ("handovers/logs/PAY-12.dev.r0.md", "tasks/PAY-12/dev.r0.md"),
            (
                "handovers/logs/V1.2.fe-dev.r10.md",
                "tasks/V1.2/fe-dev.r10.md",
            ),
            (
                "handovers/logs/2026-10-01-dev.md",
                "archive/handovers/2026-10-01-dev.md",
            ),
            ("handovers/logs/.gitkeep", "archive/handovers/.gitkeep"),
            ("handovers/notes.md", "archive/handovers/notes.md"),
            ("routes/PAY-12.json", "tasks/PAY-12/route.json"),
            ("routes/PAY-12.request.md", "tasks/PAY-12/request.md"),
            ("routes/README.txt", "archive/routes/README.txt"),
            (
                "reviews/PAY-12/0123456789ab/result.json",
                "tasks/PAY-12/reviews/0123456789ab/result.json",
            ),
            ("reviews/.gitkeep", "archive/reviews/.gitkeep"),
            (
                "docs/evaluations/qa-reports/PAY-12-test/report.md",
                "tasks/PAY-12/qa/report.md",
            ),
            (
                "docs/evaluations/qa-reports/PAY-12-test/shots/a.png",
                "tasks/PAY-12/qa/shots/a.png",
            ),
            (
                "docs/evaluations/qa-reports/PAY-12-designer-test/report.md",
                "tasks/PAY-12/qa/designer.md",
            ),
            (
                "docs/evaluations/qa-reports/PAY-12-designer-test/a.png",
                "tasks/PAY-12/qa/a.png",
            ),
            (
                "docs/evaluations/qa-reports/OLD-1-test/report.md",
                "archive/qa-reports/OLD-1-test/report.md",
            ),
            (
                "docs/evaluations/qa-reports/OLD-1-review/report.md",
                "archive/qa-reports/OLD-1-review/report.md",
            ),
            (
                "docs/evaluations/qa-reports/.gitkeep",
                "archive/qa-reports/.gitkeep",
            ),
            (
                "docs/evaluations/scenarios/a.md",
                "docs/evaluations/scenarios/a.md",
            ),
            ("board/index.html", "archive/board/index.html"),
            // 글에 적히는 꼴: 폴더, glob, 자리표시
            ("handovers/", "inbox/"),
            ("handovers/**", "inbox/**"),
            ("handovers/to_<역할>.md", "inbox/to_<역할>.md"),
            ("handovers/logs/", "tasks/"),
            ("handovers/logs/**", "tasks/**"),
            (
                "handovers/logs/<KEY>.<역할>.r<차수>.md",
                "tasks/<KEY>/<역할>.r<차수>.md",
            ),
            ("routes/<KEY>.json", "tasks/<KEY>/route.json"),
            ("reviews/<KEY>/<head>/", "tasks/<KEY>/reviews/<head>/"),
            ("reviews/**", "tasks/**"),
            ("docs/evaluations/qa-reports/", "tasks/"),
            ("docs/evaluations/qa-reports/**/*.log", "tasks/**/*.log"),
            (
                "docs/evaluations/qa-reports/<KEY>-test/report.md",
                "tasks/<KEY>/qa/report.md",
            ),
            ("contexts/", "guide/roles/"),
            ("lint/", "config/"),
            ("handovers", "inbox"),
            ("handovers/logs", "tasks"),
            ("docs/evaluations/qa-reports", "tasks"),
        ] {
            assert_eq!(to(old).as_deref(), Some(new), "{old}");
        }
        for gone in [
            "docs/evaluations/jev/PAY-12-find.json",
            "docs/evaluations/jev/history/a.json",
            "docs/evaluations/qa-reports/OLD-1-review/preview.json",
            "docs/evaluations/qa-reports/OLD-1-review/rules.json",
            "docs/evaluations/qa-reports/OLD-1-review/lint.json",
        ] {
            assert_eq!(to(gone), None, "{gone}");
        }
    }

    #[test]
    fn rewrite_fixes_paths_in_text() {
        let tasks = |_: &str| false;
        let fix = |text: &str| rewrite(text, &tasks);
        assert_eq!(
            fix(
                "정본은 `.fullops-squad/handovers/to_<역할>.md`다. 대기는 `.fullops-squad/PLANS.md`."
            ),
            "정본은 `.fullops/inbox/to_<역할>.md`다. 대기는 `.fullops/PLANS.md`."
        );
        assert_eq!(
            fix("지도는 .fullops-squad/FULLOPS.md. 다음 줄"),
            "지도는 .fullops/guide/FULLOPS.md. 다음 줄"
        );
        assert_eq!(
            fix(
                r#"["**/dist/**", ".fullops-squad/handovers/**", ".fullops-squad/docs/evaluations/**"]"#
            ),
            r#"["**/dist/**", ".fullops/inbox/**", ".fullops/docs/evaluations/**"]"#
        );
        assert_eq!(
            fix(
                "(`.fullops-squad/` 기준) `.fullops-squad` 폴더, .fullops-squad/docs/evaluations/jev/x.json"
            ),
            "(`.fullops/` 기준) `.fullops` 폴더, .fullops/docs/evaluations/jev/x.json"
        );
        // 공백이 든 자리표시도 경로의 일부다. 닫히지 않는 `<`는 아니다
        assert_eq!(
            fix(
                "기록은 `.fullops-squad/docs/evaluations/qa-reports/<과제 키>-test/report.md`에, .fullops-squad/PLANS.md < 10"
            ),
            "기록은 `.fullops/tasks/<과제 키>/qa/report.md`에, .fullops/PLANS.md < 10"
        );
        // 홈의 Python판 런타임과 다른 이름은 두고, 이미 새 자리인 글은 그대로다
        let kept = "~/.fullops-squad/runtime, .fullops-squad-old/x, tourbut/fullops-squad-plugin, .fullops/PLANS.md";
        assert_eq!(fix(kept), kept);
    }
}
