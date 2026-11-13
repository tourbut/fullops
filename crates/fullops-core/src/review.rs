//! 리뷰 필요 판정과 리뷰 기록의 형식·검사(설계 8.9절). 파일과 프로세스에 닿지 않는다.

use crate::config::is_code;
use crate::model::{
    CodeDef, FileStat, LintResult, LintStatus, Need, NeedAction, ReviewPolicy, Severity, Sha,
    TaskKey,
};
use serde::{Deserialize, Serialize};

/// `numstat`은 `git diff --numstat <merge_base> <head>`의 결과다.
pub fn review_need(
    policy: &ReviewPolicy,
    numstat: &[FileStat],
    code: &CodeDef,
    exclude: &globset::GlobSet,
    base: Sha,
    head: Sha,
    merge_base: Sha,
) -> Need {
    let changed = numstat.iter().filter(|f| is_code(&f.path, code, exclude));
    let (files, lines) = changed.fold((0u32, 0u32), |(files, lines), f| {
        (files + 1, lines + f.added + f.deleted)
    });
    let always = globs(&policy.always);
    let forced: Vec<String> = numstat
        .iter()
        .filter(|f| always.is_match(&f.path))
        .map(|f| f.path.clone())
        .collect();
    let required =
        !forced.is_empty() || lines >= policy.min_code_lines || files >= policy.min_code_files;
    let limits = format!(
        "기준 {}개 또는 {}줄",
        policy.min_code_files, policy.min_code_lines
    );
    let reason = if !forced.is_empty() {
        let shown: Vec<&str> = forced.iter().take(5).map(String::as_str).collect();
        format!("review.always 경로 변경: {}", shown.join(", "))
    } else if required {
        format!("대규모 코드 변경: 코드 파일 {files}개, {lines}줄({limits})")
    } else {
        format!("소규모 변경: 코드 파일 {files}개, {lines}줄({limits} 미만)")
    };
    Need {
        required,
        action: match (required, policy.auto) {
            (false, _) => NeedAction::Skip,
            (true, true) => NeedAction::Review,
            (true, false) => NeedAction::Ask,
        },
        reason,
        code_files: files,
        code_lines: lines,
        always: forced,
        base,
        head,
        merge_base,
    }
}

/// glob 목록. 문법에 안 맞는 무늬는 건너뛴다.
pub fn globs(patterns: &[String]) -> globset::GlobSet {
    let mut set = globset::GlobSetBuilder::new();
    for glob in patterns.iter().filter_map(|p| globset::Glob::new(p).ok()) {
        set.add(glob);
    }
    set.build().unwrap_or_else(|_| globset::GlobSet::empty())
}

/// `review/rule.json`. 1.24.0의 형식 그대로 읽는다. `include`는 쓰지 않는다(대상은 바뀐 파일 전부다).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RuleFile {
    pub exclude: Vec<String>,
    pub rules: Vec<PathRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathRule {
    pub path: String,
    pub rule: String,
}

/// `targets.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Targets {
    pub v: u32,
    pub key: TaskKey,
    pub base: Sha,
    pub merge_base: Sha,
    pub head: Sha,
    pub policy_sha256: String,
    pub files: Vec<TargetFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetFile {
    pub path: String,
    /// `A`·`M`·`D`·`R`.
    pub status: String,
    pub rule: Option<String>,
    /// `skipped` 또는 `null`.
    pub preset: Option<String>,
    pub reason: String,
}

/// `result.json`. 리뷰어가 손으로 채우므로 빠진 필드는 빈 값으로 읽고 `check`가 가려낸다.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReviewResult {
    pub v: u32,
    pub key: String,
    pub base: String,
    pub head: String,
    pub reviewer_session: Option<String>,
    pub implementer_session: Option<String>,
    pub snapshot_path: String,
    pub snapshot_head: String,
    pub read_only: bool,
    pub conclusion: Option<String>,
    pub files: Vec<ResultFile>,
    pub findings: Vec<ReviewFinding>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResultFile {
    pub path: String,
    pub status: String,
    pub review_status: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReviewFinding {
    pub path: String,
    pub content: String,
    pub severity: String,
    pub resolved: bool,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
}

const SKIP_REASON: &str = "검증 증거·하네스 기록이다. 파일마다 열람하지 않는다.";
const MEDIA: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".mp4", ".mov", ".log",
];

/// 리뷰 대상 하나(설계 8.9절 `targets.json`). `status`는 `git diff --name-status`의 첫 글자다.
pub fn target_file(path: &str, status: char, rules: &RuleFile) -> TargetFile {
    let lower = path.to_ascii_lowercase();
    let record = path.starts_with(".fullops-squad/handovers/logs/")
        || path.starts_with(".fullops-squad/routes/")
        || lower.ends_with(".bak")
        || (path.contains("qa-reports/") && MEDIA.iter().any(|ext| lower.ends_with(ext)));
    let skipped = record || globs(&rules.exclude).is_match(path);
    let matching = |r: &&PathRule| globs(std::slice::from_ref(&r.path)).is_match(path);
    TargetFile {
        path: path.to_string(),
        status: match status {
            'C' => 'A',
            'T' => 'M',
            other => other,
        }
        .to_string(),
        rule: rules.rules.iter().find(matching).map(|r| r.rule.clone()),
        preset: skipped.then(|| "skipped".to_string()),
        reason: if skipped { SKIP_REASON } else { "" }.to_string(),
    }
}

/// `result.json`의 뼈대. 미리 건너뛴 대상은 `skipped`와 사유를 채워 둔다.
pub fn result_skeleton(
    targets: &Targets,
    implementer_session: Option<String>,
    snapshot_path: &str,
) -> ReviewResult {
    ReviewResult {
        v: 1,
        key: targets.key.to_string(),
        base: targets.base.to_string(),
        head: targets.head.to_string(),
        reviewer_session: None,
        implementer_session,
        snapshot_path: snapshot_path.to_string(),
        snapshot_head: targets.head.to_string(),
        read_only: true,
        conclusion: None,
        files: targets
            .files
            .iter()
            .map(|f| ResultFile {
                path: f.path.clone(),
                status: f.status.clone(),
                review_status: f.preset.clone(),
                reason: f.reason.clone(),
            })
            .collect(),
        findings: Vec::new(),
    }
}

impl ReviewResult {
    /// 미해결 critical·high가 있다.
    pub fn blocking(&self) -> bool {
        self.findings
            .iter()
            .any(|f| !f.resolved && matches!(f.severity.as_str(), "critical" | "high"))
    }
}

/// `check`의 2·3·4·6·7번(설계 8.9절). 걸린 항목의 번호와 사유를 돌려준다.
pub fn check_result(targets: &Targets, result: &ReviewResult) -> Vec<(u8, String)> {
    let mut out = Vec::new();
    let mut expected: Vec<(&str, &str)> = targets
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.status.as_str()))
        .collect();
    let mut actual: Vec<(&str, &str)> = result
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.status.as_str()))
        .collect();
    expected.sort();
    actual.sort();
    if expected != actual {
        out.push((
            2,
            "result.json의 files가 targets.json과 다릅니다(누락·추가).".into(),
        ));
    }
    let unfinished = |f: &&ResultFile| match f.review_status.as_deref() {
        Some("reviewed") => false,
        Some("skipped") => f.reason.trim().is_empty(),
        _ => true,
    };
    if let Some(file) = result.files.iter().find(unfinished) {
        out.push((
            3,
            format!(
                "{}의 review_status가 reviewed·skipped가 아니거나 skipped의 reason이 비었습니다.",
                file.path
            ),
        ));
    }
    let reviewer = result.reviewer_session.as_deref().unwrap_or("").trim();
    if reviewer.is_empty() || Some(reviewer) == result.implementer_session.as_deref() {
        out.push((
            4,
            "reviewer_session이 비었거나 구현자 세션과 같습니다.".into(),
        ));
    } else if !result.read_only {
        out.push((4, "read_only가 참이 아닙니다.".into()));
    }
    if !matches!(result.conclusion.as_deref(), Some("accept" | "reject")) {
        out.push((6, "conclusion이 accept·reject가 아닙니다.".into()));
    }
    if result.blocking() {
        out.push((7, "미해결 critical·high finding이 있습니다.".into()));
    }
    out
}

/// `check`의 9번 가운데 파일만으로 보는 부분: ERROR, 사유 없는 `unrunnable`, 등록된 명령의 결과 누락·중복.
/// `registered`는 기준 시점 `lint.json`의 `commands[].name`이다.
pub fn check_lint(lint: &LintResult, registered: &[String]) -> Option<String> {
    let failed = |s: LintStatus| matches!(s, LintStatus::Failed | LintStatus::Timeout);
    let errors = lint
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count()
        + lint.commands.iter().filter(|c| failed(c.status)).count();
    if errors > 0 || lint.errors > 0 {
        return Some(format!(
            "lint ERROR가 {}건 있습니다.",
            errors.max(lint.errors as usize)
        ));
    }
    let silent = |c: &&crate::model::LintCommandResult| {
        c.status == LintStatus::Unrunnable && c.reason.trim().is_empty()
    };
    if let Some(command) = lint.commands.iter().find(silent) {
        return Some(format!(
            "unrunnable 명령 {}의 reason이 비었습니다.",
            command.name
        ));
    }
    let count = |name: &String| lint.commands.iter().filter(|c| &c.name == name).count();
    registered
        .iter()
        .find(|name| count(name) != 1)
        .map(|name| format!("등록된 명령 {name}의 결과가 없거나 중복입니다."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn need(policy: &ReviewPolicy, stats: &[(&str, u32, u32)]) -> Need {
        let sha = |c: &str| Sha::new(c.repeat(40)).unwrap();
        let stats: Vec<FileStat> = stats
            .iter()
            .map(|(path, added, deleted)| FileStat {
                path: path.to_string(),
                added: *added,
                deleted: *deleted,
            })
            .collect();
        let exclude = globs(&["**/vendor/**".to_string()]);
        let code = CodeDef::default();
        review_need(
            policy,
            &stats,
            &code,
            &exclude,
            sha("a"),
            sha("b"),
            sha("a"),
        )
    }

    #[test]
    fn need_follows_baseline_policy() {
        let policy = ReviewPolicy::default();
        // 399줄은 skip, 400줄은 ask. 추가와 삭제를 더한다
        let small = need(&policy, &[("src/a.py", 300, 99)]);
        assert_eq!(
            (small.required, small.action, small.code_lines),
            (false, NeedAction::Skip, 399)
        );
        let large = need(&policy, &[("src/a.py", 300, 100)]);
        assert_eq!((large.required, large.action), (true, NeedAction::Ask));
        // 15파일
        let names: Vec<String> = (0..15).map(|i| format!("src/f{i}.rs")).collect();
        let many: Vec<(&str, u32, u32)> = names.iter().map(|n| (n.as_str(), 1, 0)).collect();
        assert_eq!(need(&policy, &many).code_files, 15);
        assert!(need(&policy, &many).required && !need(&policy, &many[1..]).required);
        // 문서와 제외 경로는 세지 않는다
        let docs = need(&policy, &[("docs/a.md", 900, 0), ("vendor/x/a.py", 900, 0)]);
        assert_eq!((docs.required, docs.code_files), (false, 0));
        // always 경로는 코드가 아니어도 걸리고, auto면 review다
        let forced = ReviewPolicy {
            always: vec!["db/migrations/**".into()],
            auto: true,
            ..ReviewPolicy::default()
        };
        let hit = need(&forced, &[("db/migrations/001.sql", 1, 0)]);
        assert_eq!(
            (hit.action, hit.always.as_slice()),
            (
                NeedAction::Review,
                &["db/migrations/001.sql".to_string()][..]
            )
        );
    }

    #[test]
    fn targets_and_result_checks() {
        let rules: RuleFile = serde_json::from_str(
            r#"{"include":["**/*.md"],"exclude":["**/*.snap"],"rules":[{"path":"**/*.md","rule":"문서 규칙"}]}"#,
        )
        .unwrap();
        let file = |path: &str| target_file(path, 'M', &rules);
        assert_eq!(file("docs/a.md").rule.as_deref(), Some("문서 규칙"));
        assert_eq!(file("src/a.py").preset, None);
        for skipped in [
            "ui/a.snap",
            "old/a.py.bak",
            ".fullops-squad/routes/A-1.json",
            ".fullops-squad/handovers/logs/A-1.dev.r0.md",
            ".fullops-squad/docs/evaluations/qa-reports/A-1-test/shot.PNG",
        ] {
            assert_eq!(
                file(skipped).preset.as_deref(),
                Some("skipped"),
                "{skipped}"
            );
        }
        let sha = Sha::new("a".repeat(40)).unwrap();
        let targets = Targets {
            v: 1,
            key: TaskKey::new("A-1").unwrap(),
            base: sha.clone(),
            merge_base: sha.clone(),
            head: sha,
            policy_sha256: String::new(),
            files: vec![file("src/a.py"), file("ui/a.snap")],
        };
        let mut result = result_skeleton(&targets, Some("impl".into()), "/tmp/snap");
        let numbers = |r: &ReviewResult| -> Vec<u8> {
            check_result(&targets, r).iter().map(|(n, _)| *n).collect()
        };
        // 뼈대 그대로면 미검토 파일, 리뷰어 없음, 결론 없음
        assert_eq!(numbers(&result), [3, 4, 6]);
        result.files[0].review_status = Some("reviewed".into());
        result.reviewer_session = Some("impl".into());
        result.conclusion = Some("accept".into());
        assert_eq!(numbers(&result), [4]);
        result.reviewer_session = Some("rev".into());
        result.findings.push(ReviewFinding {
            severity: "high".into(),
            ..ReviewFinding::default()
        });
        assert_eq!(numbers(&result), [7]);
        result.findings[0].resolved = true;
        result.files.pop();
        assert_eq!(numbers(&result), [2]);
    }
}
