//! 메모 위생(설계 6.1.6절). 역할 교훈 파일의 크기·이름과 안내 글(`guide/`·`templates/`)의 깨진 경로를 본다.
//! 파일 읽기와 경로가 있는지는 호출자가 준다. `doctor`가 알리고, `merge`가 알릴 것이 있으면 정리 과제 행을 하나 더한다.

use crate::layout;
use crate::model::{Plan, PlanStatus, RoleId, TaskKey};

/// 역할 교훈 파일 하나의 한도. 넘으면 정리할 때다.
pub const NOTE_MAX_LINES: usize = 150;
pub const NOTE_MAX_BYTES: usize = 10 * 1024;
/// 정리 과제 행의 키 머리. 행 키는 `<머리>-<YYYYMMDD>`다.
pub const TIDY_KEY: &str = "MEMO-TIDY";
/// `project.md`에서 두 번 나온 실수를 모으는 절.
pub const MISTAKES_HEADING: &str = "## 자주 틀리는 것";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// 레포 상대 경로(슬래시 표기).
    pub path: String,
    /// 그 메모의 주인 역할. 역할 교훈 파일일 때만 있다.
    pub role: Option<RoleId>,
    pub text: String,
}

/// `guide/roles/` 바로 아래 파일 하나. `name`은 파일 이름, `is_role`은 `fullops.toml`에 있는 역할인가다.
pub fn note_issues(name: &str, text: &str, is_role: &dyn Fn(&str) -> bool) -> Vec<Issue> {
    let path = format!("{}/{name}", layout::roles_dir());
    let role = name
        .strip_suffix(".md")
        .filter(|stem| is_role(stem))
        .and_then(|stem| RoleId::new(stem).ok());
    let mut out = Vec::new();
    let (lines, bytes) = (text.lines().count(), text.len());
    if lines > NOTE_MAX_LINES || bytes > NOTE_MAX_BYTES {
        out.push(Issue {
            path: path.clone(),
            role: role.clone(),
            text: format!(
                "{lines}줄·{}KB로 한도({NOTE_MAX_LINES}줄·{}KB)를 넘었다",
                bytes.div_ceil(1024),
                NOTE_MAX_BYTES / 1024
            ),
        });
    }
    if role.is_none() {
        out.push(Issue {
            path,
            role: None,
            text:
                "역할 ID의 메모가 아니다. 과제 기록은 `tasks/<KEY>/`에, 교훈은 `<역할>.md`에 둔다"
                    .into(),
        });
    }
    out
}

/// 글이 가리키는 경로 후보 `(경로, 링크인가)`: Markdown 링크의 대상과, 슬래시가 든 백틱 경로. 코드 펜스 안은 보지 않는다.
fn refs(text: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let mut rest = line;
        while let Some(at) = rest.find("](") {
            rest = &rest[at + 2..];
            let end = rest.find(')').unwrap_or(rest.len());
            out.push((rest[..end].trim().to_string(), true));
        }
        for (n, span) in line.split('`').enumerate() {
            let path_like = span.contains('/')
                && !span.ends_with('/')
                && !span.chars().any(char::is_whitespace)
                && !span.contains([':', '=', '(', '|', '\\'])
                && !span.starts_with(['~', '$', '%', '-', '/', '@']);
            if n % 2 == 1 && path_like {
                out.push((span.to_string(), false));
            }
        }
    }
    out
}

/// `a/b/../c`를 `a/c`로. 레포 밖으로 나가면 `None`.
fn normalize(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// 자리표시(`<KEY>`)·glob이 든 조각 앞까지. 그 조각이 없으면 경로 그대로다.
fn concrete(path: &str) -> (&str, bool) {
    let mut end = 0;
    for part in path.split('/') {
        if part.contains(['<', '*', '{']) {
            return (path[..end].trim_end_matches('/'), true);
        }
        end += part.len() + 1;
    }
    (path, false)
}

/// 안내 글 하나(`rel`은 레포 상대 경로)에서 가리키는 곳이 없는 경로. `exists`는 레포 상대 경로의 파일·폴더가 있는가다.
/// 자리표시가 든 경로는 앞쪽 폴더가 없고 옛 자리(설계 13.6절)를 가리킬 때만 알린다. 처음 과제 전에는 없는 폴더가 있어서다.
/// 백틱 경로는 앞으로 쓸 파일일 수 있어 그 폴더가 있으면 넘어간다. 링크는 파일이 있어야 한다.
pub fn broken_refs(rel: &str, text: &str, exists: &dyn Fn(&str) -> bool) -> Vec<Issue> {
    let dir = rel.rsplit_once('/').map_or("", |(d, _)| d);
    let role = rel
        .strip_prefix(layout::roles_dir())
        .and_then(|n| n.strip_prefix('/')?.strip_suffix(".md"))
        .and_then(|stem| RoleId::new(stem).ok());
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for (target, link) in refs(text) {
        let target = target.split('#').next().unwrap_or("");
        let target = target.strip_prefix("./").unwrap_or(target);
        if target.is_empty() || target.contains("://") || target.starts_with("mailto") {
            continue;
        }
        let (base, slotted) = concrete(target);
        if base.is_empty() || seen.contains(&target.to_string()) {
            continue;
        }
        seen.push(target.to_string());
        let bases = if base.starts_with(layout::DIR) {
            vec![normalize(base)]
        } else {
            vec![
                normalize(&format!("{dir}/{base}")),
                normalize(&layout::at(base)),
                normalize(base),
            ]
        };
        // 글 기준으로 레포 밖을 가리키면 판단하지 않는다. 다른 기준의 밖은 후보가 아닐 뿐이다
        if bases[0].is_none() || bases.iter().flatten().any(|b| exists(b)) {
            continue;
        }
        let text = match layout::moved(target, &|_| false) {
            Some(new) if new != target => {
                format!("`{target}`는 옛 자리 경로다. 지금 자리는 `{new}`다")
            }
            _ if slotted => continue,
            _ if !link
                && bases
                    .iter()
                    .flatten()
                    .any(|b| b.rsplit_once('/').is_some_and(|(parent, _)| exists(parent))) =>
            {
                continue;
            }
            _ => format!("`{target}`가 가리키는 파일이 없다"),
        };
        out.push(Issue {
            path: rel.to_string(),
            role: role.clone(),
            text,
        });
    }
    out
}

/// front matter와 제목·빈 줄 말고 내용이 있는가.
pub fn has_body(text: &str) -> bool {
    let mut lines = text.lines();
    let mut body: Vec<&str> = Vec::new();
    if text.starts_with("---") {
        lines.next();
        lines.by_ref().find(|l| l.trim_end() == "---");
    }
    body.extend(lines);
    body.iter()
        .any(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
}

/// `project.md`의 `## 자주 틀리는 것` 절에 항목(`- `·`* `로 시작하는 줄)이 있는가. 안내 문단은 판(版)마다 달라 세지 않는다.
pub fn has_mistakes(project_md: &str) -> bool {
    let mut inside = false;
    for line in project_md.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            inside = trimmed == MISTAKES_HEADING;
        } else if inside && (trimmed.starts_with("- ") || trimmed.starts_with("* ")) {
            return true;
        }
    }
    false
}

/// 알릴 것이 있으면 `PLANS.md`에 더할 정리 과제 행 `(키, 담당, 목표)`. 열린 정리 행이 이미 있으면 `None`이다.
/// 담당은 메모 주인 역할 가운데 첫째, 없으면 `fallback`(dev 모드의 주 담당이나 coordinator)이다.
pub fn tidy_row(
    issues: &[Issue],
    plan: &Plan,
    date: &str,
    fallback: Option<&RoleId>,
) -> Option<(TaskKey, RoleId, String)> {
    if issues.is_empty() {
        return None;
    }
    let open = plan.rows.iter().any(|r| {
        r.key.as_str().starts_with(TIDY_KEY)
            && !matches!(&r.status, PlanStatus::Other(s) if s.starts_with("완료"))
    });
    if open {
        return None;
    }
    let role = issues
        .iter()
        .find_map(|i| i.role.clone())
        .or_else(|| fallback.cloned())?;
    let key = TaskKey::new(format!("{TIDY_KEY}-{}", date.replace('-', ""))).ok()?;
    // 같은 날 끝낸 정리 행이 있으면 키가 겹친다. 다음 날 더한다
    if plan.rows.iter().any(|r| r.key == key) {
        return None;
    }
    let mut paths: Vec<&str> = Vec::new();
    for issue in issues {
        if !paths.contains(&issue.path.as_str()) {
            paths.push(&issue.path);
        }
    }
    let shown = paths.iter().take(3).copied().collect::<Vec<_>>().join(", ");
    let more = if paths.len() > 3 { " 외" } else { "" };
    let goal = format!(
        "메모 정리(낮은 effort): `fullops doctor`의 메모 경고 {}건({shown}{more}). `tasks/<KEY>/` 완료 보고와 대조해 틀린 것을 고치고 중복을 합치고, 두 번 나온 실수는 `project.md`·`config/lint.json` rules로 올린다",
        issues.len()
    );
    Some((key, role, goal))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(s: &str) -> bool {
        ["dev", "art"].contains(&s)
    }

    #[test]
    fn notes_over_the_limit_or_not_named_by_a_role_are_reported() {
        assert!(note_issues("dev.md", "- 교훈\n", &role).is_empty());
        let long = "줄\n".repeat(NOTE_MAX_LINES + 1);
        let issues = note_issues("art.md", &long, &role);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].role.as_ref().map(RoleId::as_str), Some("art"));
        let big = "가".repeat(NOTE_MAX_BYTES / 3 + 1);
        assert_eq!(note_issues("dev.md", &big, &role).len(), 1);
        let stray = note_issues("PAY-12-QA.md", "x", &role);
        assert_eq!((stray.len(), stray[0].role.clone()), (1, None));
    }

    #[test]
    fn broken_paths_and_old_places_are_found_but_live_ones_and_slots_are_not() {
        let files = [
            ".fullops/guide/project.md",
            ".fullops/guide/rules/common/README.md",
            ".fullops/config/lint.json",
            ".fullops/guide/roles",
            ".fullops/guide/agents/domain.md",
        ];
        let exists = |p: &str| {
            files
                .iter()
                .any(|f| *f == p || f.starts_with(&format!("{p}/")))
        };
        let text = "\
[규칙](rules/common/README.md) `config/lint.json` `.fullops/guide/project.md`
`tasks/<KEY>/qa/report.md` `guide/roles/<역할>.md` [웹](https://x.y/z) [절](#자주)
`contexts/<역할>.md` `lint/lint.json` [옛](../docs/evaluations/qa-reports/A-test/report.md)
```
`nowhere/in/fence.md`
```
`lint/lint.json` `guide/agents/new.md` `@scope/pkg` [없는 링크](rules/common/gone.md)
";
        let found: Vec<String> = broken_refs(".fullops/guide/FULLOPS.md", text, &exists)
            .into_iter()
            .map(|i| i.text)
            .collect();
        assert_eq!(found.len(), 4, "{found:?}");
        let has = |a: &str, b: &str| found.iter().any(|f| f.contains(a) && f.contains(b));
        assert!(has(
            "`contexts/<역할>.md`는 옛 자리",
            "`guide/roles/<역할>.md`"
        ));
        assert!(has("`lint/lint.json`는 옛 자리", "`config/lint.json`"));
        assert!(has(
            "qa-reports/A-test/report.md`가 가리키는 파일이 없다",
            ""
        ));
        let note = broken_refs(".fullops/guide/roles/art.md", "`no/such.md`", &exists);
        assert_eq!(note[0].role.as_ref().map(RoleId::as_str), Some("art"));
    }

    #[test]
    fn bodies_and_mistakes_ignore_front_matter_headings_and_template_lines() {
        assert!(!has_body("---\ntitle: x\n---\n\n# dev\n\n"));
        assert!(has_body("---\ntitle: x\n---\n# dev\n- 2026-11-20 교훈\n"));
        let template = "# 설정\n\n## 자주 틀리는 것\n\n안내 문장.\n\n## 다음\n\n글\n";
        assert!(!has_mistakes(template));
        let filled = template.replace("안내 문장.\n", "안내 문장.\n- 실수 → 방법\n");
        assert!(has_mistakes(&filled));
        assert!(!has_mistakes(
            &template.replace("글\n", "- 다른 절의 항목\n")
        ));
    }

    #[test]
    fn a_tidy_row_is_added_once_and_needs_an_owner() {
        let dev = RoleId::new("dev").unwrap();
        let issue = |role: Option<RoleId>| Issue {
            path: ".fullops/guide/roles/x.md".into(),
            role,
            text: "t".into(),
        };
        let plan = Plan::default();
        assert!(tidy_row(&[], &plan, "2026-11-20", Some(&dev)).is_none());
        assert!(tidy_row(&[issue(None)], &plan, "2026-11-20", None).is_none());
        let (key, owner, goal) = tidy_row(&[issue(None)], &plan, "2026-11-20", Some(&dev)).unwrap();
        assert_eq!(
            (key.as_str(), owner.as_str()),
            ("MEMO-TIDY-20261120", "dev")
        );
        assert!(goal.contains("낮은 effort") && !goal.contains('|'));

        let header = "| 과제 키 | 목표 | 담당 | 상태 | 지시서·원천 링크 |\n|---|---|---|---|---|\n";
        let open = crate::parse_plan(
            &format!("{header}| MEMO-TIDY-20261101 | x | dev | 대기 | y |\n"),
            "",
        );
        assert!(tidy_row(&[issue(None)], &open, "2026-11-20", Some(&dev)).is_none());
        let done = crate::parse_plan(
            &format!("{header}| MEMO-TIDY-20261101 | x | dev | 완료(abc1234) | y |\n"),
            "",
        );
        assert!(tidy_row(&[issue(None)], &done, "2026-11-20", Some(&dev)).is_some());
        assert!(tidy_row(&[issue(None)], &done, "2026-11-01", Some(&dev)).is_none());
    }
}
