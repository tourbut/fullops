//! lint 내장 검사와 레포 정규식 규칙(설계 8.11절), 문서 front matter 검사(설계 8.12절). 순수 로직이다.
//! `header_summary`·`lint_excludes`·`parse_deliverable_index`는 다른 묶음이 부르는 공개 함수다(설계 10.2절 끝의 표).

use crate::config::{CODE_EXTENSIONS, is_code};
use crate::frontmatter::{FmValue, FrontMatter, parse_front_matter};
use crate::messages::lint as msg;
use crate::model::{CodeDef, DeliverableRow, LintFinding, Severity, TestLevel};
use fancy_regex::Regex;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::LazyLock;

/// 문서 상태 어휘(설계 8.12절).
pub const DOC_STATUSES: [&str; 3] = ["draft", "review", "approved"];
const DOC_REQUIRED: [&str; 6] = ["id", "title", "status", "updated", "owner", "summary"];
const DOC_LISTS: [&str; 3] = ["tasks", "upstream", "downstream"];
/// front matter를 쓰는 순서. 모르는 키는 그 뒤에 이름순이다.
const DOC_ORDER: [&str; 10] = [
    "id",
    "title",
    "status",
    "statuses",
    "updated",
    "owner",
    "tasks",
    "upstream",
    "downstream",
    "summary",
];
/// 커밋되는 파일의 폴더. 산출물 인덱스의 원천 경로가 이 폴더 기준이다.
pub const SQUAD: &str = ".fullops-squad/";
const HASH_EXT: [&str; 6] = [".sh", ".bash", ".rb", ".gd", ".pl", ".r"];
const EVAL_EXT: [&str; 9] = [
    ".py", ".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs", ".svelte", ".vue",
];
const DEPENDENCY_FILES: [&str; 5] = [
    "package.json",
    "pyproject.toml",
    "requirements*.txt",
    "go.mod",
    "build.gradle*",
];
/// 범위 없이 매번 도는 명령이 이 초를 넘으면 LINT-002다.
pub const SLOW_SECONDS: f64 = 120.0;

// ---- glob ----

/// 1.24.0의 fnmatch처럼 `*`가 `/`도 넘는 glob 묶음. 읽지 못하는 무늬는 버린다.
pub fn globs(patterns: &[String]) -> GlobSet {
    let mut set = GlobSetBuilder::new();
    for glob in patterns.iter().filter_map(|p| Glob::new(p).ok()) {
        set.add(glob);
    }
    set.build().unwrap_or_else(|_| GlobSet::empty())
}

/// `lint.json`의 `exclude`. 글이 `None`이거나 `exclude`가 없으면 내장 템플릿 `lint.json`(`assets::LINT_JSON`)의 `exclude`다.
pub fn lint_excludes(lint_json: Option<&str>, template_lint_json: &str) -> GlobSet {
    let read = |text: &str| -> Option<Vec<String>> {
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        let list = value.get("exclude")?.as_array()?;
        Some(
            list.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
        )
    };
    globs(
        &lint_json
            .and_then(read)
            .or_else(|| read(template_lint_json))
            .unwrap_or_default(),
    )
}

// ---- lint.json ----

fn yes() -> bool {
    true
}
fn timeout_default() -> u64 {
    600
}
fn warning() -> Severity {
    Severity::Warning
}

/// `lint/lint.json`(1.24.0 형식 그대로. 설계 6.1절).
#[derive(Debug, Clone, Deserialize)]
pub struct LintConfig {
    pub schema_version: u32,
    #[serde(default)]
    pub commands: Vec<LintCommand>,
    #[serde(default = "timeout_default")]
    pub timeout_seconds: u64,
    #[serde(default = "yes")]
    pub parallel_tests: bool,
    pub size: SizeLimits,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub rules: Vec<RegexRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SizeLimits {
    pub severity: Severity,
    /// 0이면 끈다.
    pub max_code_lines: u32,
    pub max_doc_lines: u32,
    pub max_diff_added: u32,
}

fn dot() -> String {
    ".".into()
}
fn lint_kind() -> String {
    "lint".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct LintCommand {
    pub name: String,
    /// 셸을 거치지 않는 인자 배열.
    pub run: Vec<String>,
    /// 레포 루트 기준. 레포 안이어야 한다.
    #[serde(default = "dot")]
    pub cwd: String,
    /// `test`면 시험 명령이다. 그 밖(`lint`·`build` 등)은 앞 단계 명령이다.
    #[serde(default = "lint_kind")]
    pub kind: String,
    /// 시험 명령이 도는 최소 레벨.
    #[serde(default)]
    pub level: Option<TestLevel>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub paths: Option<Vec<String>>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

impl LintCommand {
    pub fn is_test(&self) -> bool {
        self.kind == "test"
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegexRule {
    pub code: String,
    #[serde(default)]
    pub description: String,
    pub pattern: String,
    #[serde(default)]
    pub file_extensions: Vec<String>,
    #[serde(default = "warning")]
    pub severity: Severity,
    #[serde(default)]
    pub suggestion: String,
    /// 줄 내용(여러 줄 규칙은 일치 구간)의 정규식.
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    /// 레포 상대 경로 glob.
    #[serde(default)]
    pub exclude_paths: Vec<String>,
    #[serde(default)]
    pub multiline: bool,
    #[serde(default = "yes")]
    pub enabled: bool,
}

/// `cwd`가 글자만 봐서 레포 밖으로 나가지 않는가.
fn inside_repo(cwd: &str) -> bool {
    let mut depth = 0i32;
    for part in cwd.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => depth -= 1,
            _ => depth += 1,
        }
        if depth < 0 {
            return false;
        }
    }
    !cwd.starts_with(['/', '\\']) && !cwd.contains(':')
}

/// 결과 파일의 `config_sha256`(설계 8.11절 10번): 기준 시점 `lint.json` blob 바이트의 sha256이고, 없으면 내장 템플릿 글의 sha256이다.
/// `fullops lint`와 리뷰 check 9번이 같은 값을 쓴다.
pub fn config_sha256(lint_json: Option<&[u8]>, template_lint_json: &str) -> String {
    crate::model::sha256_hex(lint_json.unwrap_or(template_lint_json.as_bytes()))
}

/// 기준 시점의 `lint.json` 글을 읽는다. 없으면(`None`) 내장 템플릿이고, 있으면 템플릿 위에 최상위 키를 덮는다(1.24.0과 같다).
/// 오류 글은 사람에게 보여 줄 사유다.
pub fn parse_lint_config(lint_json: Option<&str>, template: &str) -> Result<LintConfig, String> {
    type Object = serde_json::Map<String, serde_json::Value>;
    let mut merged: Object = serde_json::from_str(template).map_err(|e| e.to_string())?;
    if let Some(text) = lint_json {
        let over: Object =
            serde_json::from_str(text).map_err(|e| format!("JSON 객체가 아닙니다({e})"))?;
        for (key, value) in over {
            match (merged.get_mut(&key), value) {
                // size는 일부 키만 적어도 나머지가 템플릿 값으로 남게 한 단계 더 합친다
                (Some(serde_json::Value::Object(base)), serde_json::Value::Object(over))
                    if key == "size" =>
                {
                    base.extend(over)
                }
                (_, value) => {
                    merged.insert(key, value);
                }
            }
        }
    }
    let config: LintConfig = serde_json::from_value(serde_json::Value::Object(merged))
        .map_err(|e| format!("형식이 맞지 않습니다({e})"))?;
    if config.schema_version != 1 {
        return Err(format!(
            "지원하지 않는 schema_version입니다: {}",
            config.schema_version
        ));
    }
    for command in &config.commands {
        let name = &command.name;
        if name.is_empty() || command.run.is_empty() {
            return Err("명령에는 name과 비어 있지 않은 문자열 배열 run이 필요합니다".into());
        }
        if command.level.is_some() && !command.is_test() {
            return Err(format!("level은 kind가 test인 명령만 씁니다: {name}"));
        }
        if command
            .paths
            .as_ref()
            .is_some_and(|p| p.is_empty() || p.iter().any(String::is_empty))
        {
            return Err(format!(
                "paths는 비어 있지 않은 glob 문자열 배열입니다: {name}"
            ));
        }
        if !inside_repo(&command.cwd) {
            return Err(format!("cwd가 레포 밖입니다: {name}"));
        }
    }
    Ok(config)
}

/// 명령을 건너뛰는 사유(설계 8.11절 6번). 실행할 명령이면 `None`이다.
/// `changed`는 바뀐 파일의 경로(이름 변경은 앞뒤 모두), `code_changed`는 그 가운데 코드 파일이 있는가다.
pub fn skip_reason(
    command: &LintCommand,
    level: TestLevel,
    changed: &[String],
    code_changed: bool,
) -> Option<String> {
    if let Some(paths) = &command.paths {
        let set = globs(paths);
        if !changed.iter().any(|p| set.is_match(p)) {
            return Some(msg::SKIP_PATHS.into());
        }
    }
    if !command.is_test() || command.required {
        return None;
    }
    match command.level {
        Some(min) if min > level => Some(format!("적용 레벨 {level}, 명령의 최소 레벨 {min}")),
        _ if !code_changed => Some(msg::SKIP_NO_CODE.into()),
        _ => None,
    }
}

/// LINT-002: 범위 없이 매번 도는 명령(`required`인 시험 명령, `paths`·`level`이 없는 명령)이 120초를 넘었다.
pub fn slow_finding(command: &LintCommand, seconds: f64, config_path: &str) -> Option<LintFinding> {
    let always = (command.required && command.is_test())
        || (command.paths.is_none() && command.level.is_none());
    (always && seconds > SLOW_SECONDS).then(|| {
        finding(
            "LINT-002",
            Severity::Warning,
            config_path,
            None,
            &[command.name.clone(), format!("{seconds:.0}")],
        )
    })
}

// ---- 파일 종류 ----

/// 점을 포함한 소문자 확장자. 없으면 빈 글이다.
pub fn ext_of(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match name.rfind('.') {
        Some(dot) if dot > 0 => name[dot..].to_lowercase(),
        _ => String::new(),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Py,
    Hash,
    C,
}

fn style(ext: &str) -> Option<Style> {
    if ext == ".py" {
        Some(Style::Py)
    } else if HASH_EXT.contains(&ext) {
        Some(Style::Hash)
    } else if CODE_EXTENSIONS.contains(&ext) {
        Some(Style::C)
    } else {
        None
    }
}

/// 테스트 파일인가(이름과 폴더로 본다).
pub fn is_test(path: &str) -> bool {
    let mut parts: Vec<&str> = path.split('/').collect();
    let name = parts.pop().unwrap_or(path);
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    name.starts_with("test_")
        || (!ext.is_empty()
            && [".test", "_test", ".spec", "_spec"]
                .iter()
                .any(|s| stem.ends_with(s)))
        || (["swift", "kt", "java", "cs"].contains(&ext)
            && (stem.ends_with("Test") || stem.ends_with("Tests")))
        || parts
            .iter()
            .any(|p| ["test", "tests", "spec", "specs", "__tests__"].contains(p))
}

/// 파일 머리 설명(주석·docstring 1~3줄)의 첫 줄. 없으면 빈 글이다. DOC-001과 탐색 색인이 함께 쓴다.
/// `ext`는 점을 포함한 확장자다.
pub fn header_summary(text: &str, ext: &str) -> String {
    let ext = ext.to_lowercase();
    let lines: Vec<&str> = text
        .lines()
        .take(40)
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && !["#!", "# -*-", "// @ts-", "'use ", "\"use "]
                    .iter()
                    .any(|skip| l.starts_with(skip))
        })
        .collect();
    let Some((first, rest)) = lines.split_first() else {
        return String::new();
    };
    if ext == ".md" {
        return match first.strip_prefix('#') {
            Some(title) => title.trim_start_matches('#').trim().to_string(),
            None => String::new(),
        };
    }
    let kind = style(&ext);
    if kind == Some(Style::Py) {
        for quote in ["\"\"\"", "'''"] {
            if let Some(body) = first.strip_prefix(quote) {
                let body = body.split(quote).next().unwrap_or("").trim();
                return match body {
                    "" => rest.iter().find(|l| !l.starts_with(quote)).unwrap_or(&""),
                    body => body,
                }
                .to_string();
            }
        }
    }
    if matches!(kind, Some(Style::Py | Style::Hash)) && first.starts_with('#') {
        return first.trim_start_matches('#').trim().to_string();
    }
    if kind == Some(Style::C) && ["//", "/*", "<!--"].iter().any(|m| first.starts_with(m)) {
        let strip = |line: &str| {
            line.trim_start_matches("<!--")
                .replace("*/", "")
                .replace("-->", "")
                .trim_start_matches(['/', '*', '!', ' '])
                .trim_end_matches(['*', ' '])
                .to_string()
        };
        return std::iter::once(first)
            .chain(rest)
            .map(|l| strip(l))
            .find(|l| !l.is_empty())
            .unwrap_or_default();
    }
    String::new()
}

/// 주석과 빈 줄을 뺀 줄 수. `.md`는 전체 줄 수다.
pub fn code_lines(text: &str, ext: &str) -> u32 {
    let count = |keep: &dyn Fn(&str) -> bool| text.lines().filter(|l| keep(l.trim())).count();
    (match style(ext) {
        _ if ext == ".md" => text.lines().count(),
        Some(Style::Py) => python_lines(text),
        Some(Style::Hash) => count(&|l| !l.is_empty() && !l.starts_with('#')),
        _ => {
            // shortcut: 문자열 안의 주석 표식을 가리지 않는다, 상한 검사라 적게 세는 쪽이 안전하다
            let (mut n, mut closer) = (0, None::<&str>);
            for raw in text.lines() {
                let mut s = raw.trim();
                while !s.is_empty() {
                    if let Some(end) = closer {
                        match s.split_once(end) {
                            Some((_, after)) => (s, closer) = (after.trim(), None),
                            None => s = "",
                        }
                        continue;
                    }
                    let Some((open, close)) = [("/*", "*/"), ("<!--", "-->")]
                        .into_iter()
                        .find(|(open, _)| s.starts_with(open))
                    else {
                        break;
                    };
                    (s, closer) = (&s[open.len()..], Some(close));
                }
                if !s.is_empty() && !s.starts_with("//") {
                    n += 1;
                }
            }
            n
        }
    }) as u32
}

// shortcut: 토큰을 풀지 않고 줄 머리의 삼중 따옴표로 docstring을 가린다, 코드 줄 끝 주석 안의 따옴표에 속으면 줄 훑기를 토큰 단위로 바꾼다
fn python_lines(text: &str) -> usize {
    let quote_in = |s: &str| ["\"\"\"", "'''"].into_iter().find(|q| s.contains(q));
    // 열린 여러 줄 문자열: (닫는 따옴표, 코드로 세는가)
    let (mut n, mut open) = (0, None::<(&str, bool)>);
    for raw in text.lines() {
        let s = raw.trim();
        if let Some((quote, counted)) = open {
            n += usize::from(counted);
            if s.contains(quote) {
                open = None;
            }
            continue;
        }
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        let body = s.trim_start_matches(['r', 'R', 'u', 'U', 'b', 'B', 'f', 'F']);
        if let Some(quote) = ["\"\"\"", "'''"].into_iter().find(|q| body.starts_with(q)) {
            // 줄 머리의 삼중 따옴표는 docstring이다
            if !body[quote.len()..].contains(quote) {
                open = Some((quote, false));
            }
            continue;
        }
        n += 1;
        if let Some(quote) = quote_in(s)
            && s.matches(quote).count() % 2 == 1
        {
            open = Some((quote, true));
        }
    }
    n
}

// ---- 추가된 줄 ----

/// git이 따옴표로 감싼 경로를 푼다.
fn unquote(text: &str) -> String {
    let Some(inner) = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')) else {
        return text.to_string();
    };
    let mut out = Vec::new();
    let mut bytes = inner.bytes().peekable();
    while let Some(b) = bytes.next() {
        if b != b'\\' {
            out.push(b);
            continue;
        }
        match bytes.next() {
            Some(b'n') => out.push(b'\n'),
            Some(b't') => out.push(b'\t'),
            Some(b'r') => out.push(b'\r'),
            Some(digit @ b'0'..=b'7') => {
                let mut value = u32::from(digit - b'0');
                for _ in 0..2 {
                    match bytes.peek() {
                        Some(next @ b'0'..=b'7') => {
                            value = value * 8 + u32::from(*next - b'0');
                            bytes.next();
                        }
                        _ => break,
                    }
                }
                out.push(value as u8);
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `@@ -a,b +c,d @@`에서 (옛 줄 수, 새 시작 줄, 새 줄 수).
fn hunk(line: &str) -> Option<(u32, u32, u32)> {
    let mut parts = line.strip_prefix("@@ -")?.split(' ');
    let count = |range: &str| match range.split_once(',') {
        Some((_, n)) => n.parse().ok(),
        None => Some(1),
    };
    let old = count(parts.next()?)?;
    let new = parts.next()?.strip_prefix('+')?;
    let start = new.split(',').next()?.parse().ok()?;
    Some((old, start, count(new)?))
}

/// `git diff -U0`의 글에서 새 경로마다 추가된 줄 번호(1부터)를 뽑는다. 바뀐 파일은 추가 줄이 없어도 항목이 있다.
pub fn added_lines(diff: &str) -> BTreeMap<String, Vec<u32>> {
    let mut out: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let mut current = None;
    let mut lines = diff.split('\n');
    while let Some(line) = lines.next() {
        if let Some(rest) = line.strip_prefix("+++ ") {
            let rest = rest.trim_end_matches('\r');
            let path = unquote(rest.strip_suffix('\t').unwrap_or(rest));
            current = path.strip_prefix("b/").map(str::to_string);
            if let Some(path) = &current {
                out.entry(path.clone()).or_default();
            }
        } else if let Some((old, start, new)) = hunk(line) {
            if let Some(numbers) = current.as_ref().and_then(|p| out.get_mut(p)) {
                numbers.extend(start..start + new);
            }
            // 본문 줄이 `+++ `나 `@@ `로 시작해도 머리로 읽지 않게 줄 수만큼 건너뛴다
            let mut left = old + new;
            while left > 0 {
                match lines.next() {
                    Some(body) if body.starts_with('\\') => {}
                    Some(_) => left -= 1,
                    None => break,
                }
            }
        }
    }
    out
}

// ---- 내장 검사 ----

/// 내장 정규식. 컴파일이나 일치 검사가 실패하면 안 맞은 것으로 본다.
struct Pat(Option<Regex>);

impl Pat {
    fn new(pattern: &str) -> Self {
        Pat(Regex::new(pattern).ok())
    }
    fn hit(&self, text: &str) -> bool {
        self.0
            .as_ref()
            .is_some_and(|re| re.is_match(text).unwrap_or(false))
    }
    fn count(&self, text: &str) -> usize {
        self.0
            .as_ref()
            .map_or(0, |re| re.find_iter(text).filter(|m| m.is_ok()).count())
    }
}

/// 범위 없는 억제 무늬와 좁힌 표기.
static SUPPRESSIONS: LazyLock<Vec<(Pat, &'static str)>> = LazyLock::new(|| {
    vec![
        (
            Pat::new(r"#\s*type:\s*ignore(?!\s*\[)"),
            "# type: ignore[<code>]",
        ),
        (
            Pat::new(r"#\s*pyright:\s*ignore(?!\s*\[)"),
            "# pyright: ignore[<rule>]",
        ),
        (Pat::new(r"(?i)#\s*noqa(?!\s*:)"), "# noqa: <code>"),
        (
            Pat::new(r"@ts-ignore|@ts-nocheck"),
            "정확한 타입 또는 @ts-expect-error <사유>",
        ),
        (
            Pat::new(r"eslint-disable(?:-next-line|-line)?\s*(?:\*/|-->)?\s*$"),
            "eslint-disable-next-line <rule>",
        ),
    ]
});
static CASE: LazyLock<Pat> = LazyLock::new(|| {
    Pat::new(concat!(
        r"\b[xf]?(?:it|test|describe)(?:\.\w+)?\s*\(|\bdef test_\w+|\bfunc Test\w+|#\[test\]|@Test\b|",
        r#"\bfunc test\w+\s*\(|\b(?:it|test)\s+"[^"]*"\s+do\b"#
    ))
});
static SKIP: LazyLock<Pat> = LazyLock::new(|| {
    Pat::new(concat!(
        r"\.(?:skip|todo|only)\s*\(|\b[xf](?:it|test|describe)\s*\(|@pytest\.mark\.(?:skip|xfail)|",
        r"\bpytest\.(?:skip|xfail)\(|@unittest\.skip|\bt\.Skip(?:f|Now)?\(|#\[ignore\]|@Ignore\b|",
        r"@Disabled\b|XCTSkip|\bpending\s*\("
    ))
});
static EVAL: LazyLock<Pat> = LazyLock::new(|| Pat::new(r"(?<![\w.$])(?:eval|exec)\s*\("));
static SECRET: LazyLock<Pat> = LazyLock::new(|| {
    Pat::new(concat!(
        r"(?i)(?:password|passwd|pwd|api[_-]?key|apikey|secret(?:_key)?|(?:access_|auth_)?token|",
        r#"db_password|database_password)\s*[:=]\s*["'][^"']+["']"#
    ))
});
static SECRET_OK: LazyLock<Pat> = LazyLock::new(|| {
    Pat::new(concat!(
        r#"(?i)[:=]\s*["'](?:test|example|dummy|placeholder|changeme|xxx|your_|<)|"#,
        r"os\.environ|process\.env|import\.meta\.env|settings\."
    ))
});

fn finding(
    code: &str,
    severity: Severity,
    path: &str,
    line: Option<u32>,
    args: &[String],
) -> LintFinding {
    LintFinding {
        code: code.into(),
        severity,
        path: path.into(),
        line,
        message: msg::reason(code, args).unwrap_or_default(),
        suggestion: msg::suggestion(code, args),
    }
}

/// 컴파일한 레포 정규식 규칙.
pub struct Rule {
    code: String,
    severity: Severity,
    description: String,
    suggestion: String,
    extensions: Vec<String>,
    exclude_paths: GlobSet,
    pattern: Regex,
    exclude: Vec<Regex>,
    multiline: bool,
}

/// 켜진 규칙을 컴파일한다. 뒤의 목록은 컴파일하지 못한 규칙의 (코드, 오류)다.
pub fn compile_rules(rules: &[RegexRule]) -> (Vec<Rule>, Vec<(String, String)>) {
    let (mut ok, mut broken) = (Vec::new(), Vec::new());
    for rule in rules.iter().filter(|r| r.enabled) {
        let exclude: Result<Vec<Regex>, _> = rule
            .exclude_patterns
            .iter()
            .map(|p| Regex::new(p))
            .collect();
        match (Regex::new(&rule.pattern), exclude) {
            (Ok(pattern), Ok(exclude)) => ok.push(Rule {
                code: rule.code.clone(),
                severity: rule.severity,
                description: rule.description.clone(),
                suggestion: rule.suggestion.clone(),
                extensions: rule.file_extensions.clone(),
                exclude_paths: globs(&rule.exclude_paths),
                pattern,
                exclude,
                multiline: rule.multiline,
            }),
            (Err(e), _) | (_, Err(e)) => broken.push((rule.code.clone(), e.to_string())),
        }
    }
    (ok, broken)
}

impl Rule {
    fn excluded(&self, text: &str) -> bool {
        self.exclude
            .iter()
            .any(|re| re.is_match(text).unwrap_or(false))
    }
    fn hit(&self, path: &str, line: u32) -> LintFinding {
        LintFinding {
            code: self.code.clone(),
            severity: self.severity,
            path: path.into(),
            line: Some(line),
            message: self.description.clone(),
            suggestion: self.suggestion.clone(),
        }
    }
}

/// 바뀐 파일 하나.
pub struct FileChange<'a> {
    pub path: &'a str,
    /// 기준 시점의 글. 새 파일이면 `None`이다.
    pub old: Option<&'a str>,
    pub new: &'a str,
    /// 추가된 줄 번호(1부터).
    pub added: &'a [u32],
}

/// 바뀐 파일 하나의 내장 검사와 정규식 규칙(설계 8.11절 9번). 내용 검사는 추가된 줄에만 적용한다.
pub fn check_file(
    change: &FileChange,
    config: &LintConfig,
    rules: &[Rule],
    code: &CodeDef,
) -> Vec<LintFinding> {
    let FileChange {
        path,
        old,
        new,
        added,
    } = *change;
    let ext = ext_of(path);
    let is_md = ext == ".md";
    let is_code = is_code(path, code, &GlobSet::empty());
    let old = old.filter(|o| !o.is_empty());
    let mut out = Vec::new();

    let limit = if is_md {
        config.size.max_doc_lines
    } else {
        config.size.max_code_lines
    };
    if limit > 0 && (is_md || is_code) {
        let after = code_lines(new, &ext);
        let before = old.map_or(0, |o| code_lines(o, &ext));
        if after > limit && after > before {
            let args = [after, before, limit].map(|n| n.to_string());
            out.push(finding("SIZE-001", config.size.severity, path, None, &args));
        }
    }
    let test = is_test(path);
    if old.is_none() && is_code && style(&ext).is_some() && header_summary(new, &ext).is_empty() {
        out.push(finding("DOC-001", Severity::Warning, path, None, &[]));
    }
    if let (true, Some(old)) = (test, old) {
        let (before, after) = (CASE.count(old), CASE.count(new));
        if after < before {
            let args = [(before - after).to_string()];
            out.push(finding("ANTI-005", Severity::Warning, path, None, &args));
        }
    }

    let rules: Vec<&Rule> = rules
        .iter()
        .filter(|r| {
            (r.extensions.is_empty() || r.extensions.contains(&ext))
                && !r.exclude_paths.is_match(path)
        })
        .collect();
    for rule in rules.iter().filter(|r| r.multiline) {
        for found in rule.pattern.find_iter(new).filter_map(Result::ok) {
            let line_at = |at: usize| new[..at].matches('\n').count() as u32 + 1;
            let (start, end) = (line_at(found.start()), line_at(found.end()));
            if added.iter().any(|n| (start..=end).contains(n)) && !rule.excluded(found.as_str()) {
                out.push(rule.hit(path, start));
            }
        }
    }
    let lines: Vec<&str> = new.lines().collect();
    for &number in added {
        let Some(line) = (number as usize).checked_sub(1).and_then(|i| lines.get(i)) else {
            continue;
        };
        if is_code {
            for (_, hint) in SUPPRESSIONS.iter().filter(|(p, _)| p.hit(line)) {
                let args = [hint.to_string()];
                out.push(finding(
                    "ANTI-003",
                    Severity::Error,
                    path,
                    Some(number),
                    &args,
                ));
            }
        }
        if test && SKIP.hit(line) {
            out.push(finding(
                "ANTI-004",
                Severity::Error,
                path,
                Some(number),
                &[],
            ));
        }
        if EVAL_EXT.contains(&ext.as_str()) && EVAL.hit(line) {
            out.push(finding(
                "ANTI-002",
                Severity::Error,
                path,
                Some(number),
                &[],
            ));
        }
        if SECRET.hit(line) && !SECRET_OK.hit(line) {
            let severity = if test || is_md {
                Severity::Warning
            } else {
                Severity::Error
            };
            out.push(finding("SEC-001", severity, path, Some(number), &[]));
        }
        for rule in rules.iter().filter(|r| !r.multiline) {
            if rule.pattern.is_match(line).unwrap_or(false) && !rule.excluded(line) {
                out.push(rule.hit(path, number));
            }
        }
    }
    out
}

/// DEP-001: 의존성 선언 파일의 변경·삭제·이름 변경. `paths`는 그 변경의 앞뒤 경로다.
pub fn dependency_finding(target: &str, paths: &[&str]) -> Option<LintFinding> {
    let names = globs(&DEPENDENCY_FILES.map(String::from));
    paths
        .iter()
        .any(|p| names.is_match(p.rsplit('/').next().unwrap_or(p)))
        .then(|| finding("DEP-001", Severity::Warning, target, None, &[]))
}

/// ANTI-005: 테스트 파일 삭제.
pub fn deleted_test_finding(path: &str) -> Option<LintFinding> {
    is_test(path).then(|| finding("ANTI-005", Severity::Warning, path, None, &[]))
}

/// SIZE-002: 추가 줄 합이 상한을 넘었다(상한 0은 끈다).
pub fn diff_size_finding(added: u32, limit: u32) -> Option<LintFinding> {
    (limit > 0 && added > limit).then(|| {
        let args = [added.to_string(), limit.to_string()];
        finding("SIZE-002", Severity::Warning, "", None, &args)
    })
}

/// LINT-000(`commands`가 비었다)과 LINT-001(브랜치가 `lint.json`을 바꿨다).
pub fn config_finding(code: &str, config_path: &str) -> LintFinding {
    finding(code, Severity::Warning, config_path, None, &[])
}

// ---- 문서 front matter ----

fn value<'a>(fm: &'a FrontMatter, name: &str) -> Option<&'a FmValue> {
    fm.fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)
}

/// 문서가 가진 산출물 ID들. `id`가 목록이면 그 목록이다.
pub fn identities(fm: &FrontMatter) -> Vec<String> {
    match value(fm, "id") {
        Some(FmValue::List(ids)) => ids.clone(),
        Some(FmValue::Text(id)) if !id.is_empty() => vec![id.clone()],
        _ => Vec::new(),
    }
}

/// 산출물 하나의 상태: `statuses[<ID>]`, 없으면 `status`다.
pub fn document_status<'a>(fm: &'a FrontMatter, doc_id: Option<&str>) -> Option<&'a str> {
    doc_id
        .and_then(|id| fm.map("statuses")?.get(id))
        .map(String::as_str)
        .or_else(|| fm.get("status"))
}

fn is_doc_id(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 3 && b[0] == b'D' && b[1].is_ascii_digit() && b[2].is_ascii_digit()
}

fn is_date(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
}

/// 정해진 순서(설계 8.12절)로 다시 늘어놓는다. 목록 필드에 글이 있으면 원소 하나짜리 목록으로 바꾼다.
pub fn ordered(fm: &FrontMatter) -> FrontMatter {
    let mut fields = fm.fields.clone();
    let rank = |name: &str| DOC_ORDER.iter().position(|k| *k == name);
    fields.sort_by(|(a, _), (b, _)| match (rank(a), rank(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.cmp(b),
    });
    for (name, value) in &mut fields {
        if let (true, FmValue::Text(text)) = (DOC_LISTS.contains(&name.as_str()), &*value) {
            *value = FmValue::List(vec![text.clone()]);
        }
    }
    FrontMatter { fields }
}

/// 문서 front matter의 규칙 위반(설계 8.12절. 판정은 1.24.0과 같다). 빈 목록이면 통과다.
/// `doc_id`는 이 문서를 원천으로 가진 산출물 ID, `index_status`는 인덱스 표의 상태다.
pub fn doc_problems(text: &str, doc_id: Option<&str>, index_status: Option<&str>) -> Vec<String> {
    let Ok((fm, body_at)) = parse_front_matter(text) else {
        return vec!["front matter 없음".into()];
    };
    let blank = |name: &str| match value(&fm, name) {
        None => true,
        Some(FmValue::Text(text)) => text.trim().is_empty(),
        Some(_) => false,
    };
    let mut found: Vec<String> = DOC_REQUIRED
        .iter()
        .filter(|k| (**k != "id" || doc_id.is_some()) && blank(k))
        .map(|k| format!("필수 필드 없음: {k}"))
        .collect();
    let ids = identities(&fm);
    if let Some(doc_id) = doc_id {
        if value(&fm, "id").is_some() {
            let mut unique = ids.clone();
            unique.sort();
            unique.dedup();
            if ids.is_empty() || unique.len() != ids.len() || !ids.iter().all(|i| is_doc_id(i)) {
                found.push("id는 산출물 ID 또는 중복 없는 ID 목록이어야 함".into());
            }
        }
        if !ids.is_empty() && !ids.iter().any(|i| i == doc_id) {
            found.push(format!("id가 {doc_id}가 아님: {}", ids.join(", ")));
        }
    }
    match value(&fm, "status") {
        Some(FmValue::Text(s)) if s.is_empty() || DOC_STATUSES.contains(&s.as_str()) => {}
        Some(FmValue::Text(s)) => {
            found.push(format!("status는 draft/review/approved 중 하나: {s}"))
        }
        Some(_) => found.push("status는 draft/review/approved 중 하나".into()),
        None => {}
    }
    match value(&fm, "updated") {
        Some(FmValue::Text(d)) if d.is_empty() || is_date(d) => {}
        Some(FmValue::Text(d)) => found.push(format!("updated는 YYYY-MM-DD: {d}")),
        Some(_) => found.push("updated는 YYYY-MM-DD".into()),
        None => {}
    }
    for name in DOC_LISTS {
        if matches!(value(&fm, name), Some(FmValue::Text(_) | FmValue::Map(_))) {
            found.push(format!("{name}는 [a, b] 목록"));
        }
    }
    let statuses_ok = match value(&fm, "statuses") {
        None => true,
        Some(FmValue::Map(map)) => map
            .iter()
            .all(|(k, v)| ids.contains(k) && DOC_STATUSES.contains(&v.as_str())),
        Some(_) => false,
    };
    if !statuses_ok {
        found.push("statuses는 소유한 산출물 ID와 draft/review/approved 상태의 객체여야 함".into());
    }
    if let (Some(index), Some(effective)) = (index_status, document_status(&fm, doc_id))
        && DOC_STATUSES.contains(&index)
        && DOC_STATUSES.contains(&effective)
        && index != effective
    {
        found.push(format!(
            "인덱스 상태({index})와 문서 상태({effective})가 다름"
        ));
    }
    let bom = text.len() - text.trim_start_matches('\u{feff}').len();
    let block = text.get(bom..body_at).unwrap_or("");
    if found.is_empty() && block.replace("\r\n", "\n") != ordered(&fm).render() {
        found.push("정해진 형식과 다름(필드 순서·목록 표기·주석)".into());
    }
    found
}

// ---- 산출물 인덱스 ----

/// 산출물 인덱스 `docs/deliverables/README.md`의 표를 읽는다.
/// 행은 칸이 다섯(`ID | 단계 | 산출물 | 원천 | 상태`)이고 첫 칸이 `D` + 숫자 두 자리인 표 줄이다. 그 밖의 줄은 읽지 않는다.
/// `title`은 산출물 칸, `summary`는 단계 칸, `status`는 상태 칸의 글 그대로다.
/// `sources`는 원천 칸의 백틱 안 글마다 첫 공백 앞까지를 `.fullops-squad/` 뒤에 붙인 레포 상대 경로다. `/`로 끝나면 그 폴더 아래의 Markdown 파일들이다.
/// 같은 ID가 두 번 나오면 뒤의 행이 이긴다.
pub fn parse_deliverable_index(text: &str) -> Vec<DeliverableRow> {
    let mut rows: Vec<DeliverableRow> = Vec::new();
    for line in text.lines() {
        let Some(inner) = line.trim().strip_prefix('|') else {
            continue;
        };
        let cells: Vec<&str> = inner
            .strip_suffix('|')
            .unwrap_or(inner)
            .split('|')
            .map(str::trim)
            .collect();
        let [id, stage, title, source, status] = cells[..] else {
            continue;
        };
        if !is_doc_id(id) {
            continue;
        }
        // 백틱으로 나눈 조각 가운데 홀수 번째가 백틱 안이다
        let sources = source
            .split('`')
            .skip(1)
            .step_by(2)
            .filter_map(|code| code.split(' ').next().filter(|p| !p.is_empty()))
            .map(|path| format!("{SQUAD}{path}"))
            .collect();
        let row = DeliverableRow {
            id: id.into(),
            title: title.into(),
            summary: stage.into(),
            status: status.into(),
            sources,
        };
        match rows.iter_mut().find(|r| r.id == row.id) {
            Some(same) => *same = row,
            None => rows.push(row),
        }
    }
    rows
}

/// 레포 상대 경로가 그 산출물의 원천인가. 원천이 파일이면 같은 경로, 폴더면 그 안의 Markdown이다.
pub fn owns(row: &DeliverableRow, path: &str) -> bool {
    row.sources.iter().any(|source| {
        path == source
            || (source.ends_with('/') && path.starts_with(source.as_str()) && path.ends_with(".md"))
    })
}

/// DOC-002(산출물 원천 문서)와 DOC-003(그 밖의 `.fullops-squad/**/*.md`). 코드 lint의 `exclude`와 별도로 적용한다.
pub fn doc_findings(path: &str, text: &str, index: &[DeliverableRow]) -> Vec<LintFinding> {
    let Some(relative) = path.strip_prefix(SQUAD) else {
        return Vec::new();
    };
    let hit = |code: &str, problem: String, id: &str| {
        let args = [problem, relative.to_string(), id.to_string()];
        finding(code, Severity::Error, path, None, &args)
    };
    let owners: Vec<&DeliverableRow> = index.iter().filter(|row| owns(row, path)).collect();
    if !owners.is_empty() {
        return owners
            .iter()
            .filter(|row| row.status != "범위 밖")
            .flat_map(|row| {
                doc_problems(text, Some(&row.id), Some(&row.status))
                    .into_iter()
                    .map(|p| hit("DOC-002", p, &row.id))
            })
            .collect();
    }
    let name = relative.rsplit('/').next().unwrap_or(relative);
    // 인박스·로그·템플릿·rules/·FULLOPS.md·PLANS.md는 공통 front matter를 요구하지 않는다
    let exempt = relative.starts_with("handovers/")
        || relative.starts_with("rules/")
        || name.starts_with('_')
        || ["FULLOPS.md", "PLANS.md"].contains(&relative);
    if exempt || !path.ends_with(".md") || text.trim().is_empty() {
        return Vec::new();
    }
    doc_problems(text, None, None)
        .into_iter()
        .map(|p| hit("DOC-003", p, ""))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = include_str!("../../fullops/assets/repo/lint/lint.json");
    const INDEX: &str = include_str!("../../fullops/assets/repo/docs/deliverables/README.md");

    #[test]
    fn template_lint_json_and_index_are_read() {
        let config = parse_lint_config(None, TEMPLATE).expect("템플릿");
        let (rules, broken) = compile_rules(&config.rules);
        assert_eq!((rules.len(), broken.len()), (7, 0), "{broken:?}");
        let exclude = lint_excludes(None, TEMPLATE);
        assert!(exclude.is_match("web/node_modules/a.js") && exclude.is_match("node_modules/a.js"));
        assert!(exclude.is_match("Cargo.lock") && !exclude.is_match("src/a.js"));
        // exclude가 있으면 그것만 쓰고, 없거나 읽히지 않으면 템플릿이다
        let own = lint_excludes(Some(r#"{"exclude":["gen/**"]}"#), TEMPLATE);
        assert!(own.is_match("gen/a.py") && !own.is_match("node_modules/a.js"));
        assert!(lint_excludes(Some("{}"), TEMPLATE).is_match("node_modules/a.js"));
        assert!(lint_excludes(Some("깨짐"), TEMPLATE).is_match("node_modules/a.js"));
        // 일부 키만 적은 size는 템플릿 값과 합친다
        let partial = r#"{"schema_version":1,"size":{"max_code_lines":10}}"#;
        let size = parse_lint_config(Some(partial), TEMPLATE)
            .expect("부분 설정")
            .size;
        assert_eq!((size.max_code_lines, size.max_doc_lines), (10, 500));
        for bad in [
            r#"{"schema_version":2}"#,
            r#"{"commands":[{"name":"a","run":[]}]}"#,
            r#"{"commands":[{"name":"a","run":["x"],"cwd":"../out"}]}"#,
            r#"{"commands":[{"name":"a","run":["x"],"level":"full"}]}"#,
            r#"{"commands":[{"name":"a","run":["x"],"paths":[]}]}"#,
        ] {
            assert!(parse_lint_config(Some(bad), TEMPLATE).is_err(), "{bad}");
        }

        let rows = parse_deliverable_index(INDEX);
        assert_eq!(rows.len(), 13);
        let d06 = &rows[5];
        assert_eq!(
            (d06.id.as_str(), d06.title.as_str(), d06.summary.as_str()),
            ("D06", "엔티티정의서", "분석")
        );
        assert_eq!(d06.status, "미작성");
        assert_eq!(
            d06.sources,
            [".fullops-squad/docs/design-docs/data-model.md"]
        );
        assert_eq!(rows[2].sources.len(), 2);
        assert!(owns(
            &rows[1],
            ".fullops-squad/docs/planning/product-specs/a/login.md"
        ));
        assert!(!owns(
            &rows[1],
            ".fullops-squad/docs/planning/product-specs/a.png"
        ));
    }

    #[test]
    fn diff_hunks_give_added_line_numbers() {
        let diff = "diff --git a/a.py b/a.py\n--- a/a.py\n+++ b/a.py\n@@ -2 +2,2 @@ def f():\n-old\n+++ b/fake.py\n+@@ -1 +9 @@\n\
                    @@ -10,0 +12 @@\n+tail\n\\ No newline at end of file\n\
                    diff --git a/gone.py b/gone.py\n--- a/gone.py\n+++ /dev/null\n@@ -1 +0,0 @@\n-x\n\
                    diff --git a/sp ace.py b/sp ace.py\n--- a/sp ace.py\t\n+++ b/sp ace.py\t\n@@ -0,0 +1 @@\n+x\n\
                    diff --git \"a/q\\\"\\355\\225\\234.py\" \"b/q\\\"\\355\\225\\234.py\"\n--- /dev/null\n+++ \"b/q\\\"\\355\\225\\234.py\"\n@@ -0,0 +1,3 @@\n+1\n+2\n+3\n\
                    diff --git a/mode.sh b/mode.sh\nold mode 100644\nnew mode 100755\n";
        let added = added_lines(diff);
        assert_eq!(added["a.py"], [2, 3, 12]);
        assert_eq!(added["sp ace.py"], [1]);
        assert_eq!(added["q\"한.py"], [1, 2, 3]);
        assert_eq!(added.len(), 3);
    }

    #[test]
    fn code_lines_skip_comments_and_docstrings() {
        let py = "\"\"\"모듈 설명.\n둘째 줄.\n\"\"\"\nimport os  # 주석\n\n# 주석\ndef f():\n    '''한 줄 docstring'''\n    text = \"\"\"\n    본문\n    \"\"\"\n    return text\n";
        assert_eq!(code_lines(py, ".py"), 6);
        let ts = "// 머리\n/* 여러\n줄 */ const a = 1;\n\nconst b = 2; // 끝\n<!-- x -->\n";
        assert_eq!(code_lines(ts, ".ts"), 2);
        assert_eq!(code_lines("#!/bin/sh\n\necho 1\n", ".sh"), 1);
        assert_eq!(code_lines("# 제목\n\n글\n", ".md"), 3);
    }

    #[test]
    fn header_summary_reads_the_first_comment() {
        assert_eq!(
            header_summary("#!/usr/bin/env python3\n\"\"\"결제 재시도.\"\"\"\n", ".py"),
            "결제 재시도."
        );
        assert_eq!(
            header_summary("\"\"\"\n결제 재시도.\n\"\"\"\n", ".py"),
            "결제 재시도."
        );
        assert_eq!(header_summary("# 배포 도구\necho\n", ".sh"), "배포 도구");
        assert_eq!(
            header_summary("//! 색인 만들기.\nfn f() {}\n", ".rs"),
            "색인 만들기."
        );
        assert_eq!(header_summary("/**\n * 버튼.\n */\n", ".TSX"), "버튼.");
        assert_eq!(header_summary("# 제목\n", ".md"), "제목");
        assert_eq!(header_summary("import os\n", ".py"), "");
        assert_eq!(header_summary("", ".py"), "");
    }

    #[test]
    fn test_files_are_recognised_by_name_and_folder() {
        for path in [
            "test_a.py",
            "src/a.test.ts",
            "a_spec.rb",
            "app/FooTests.swift",
            "pkg/tests/util.py",
            "a_test.go",
        ] {
            assert!(is_test(path), "{path}");
        }
        for path in ["src/a.py", "contest/a.py", "latest.py", "testing.md"] {
            assert!(!is_test(path), "{path}");
        }
    }

    #[test]
    fn slow_unscoped_command_is_lint_002() {
        let command = |json: &str| -> LintCommand { serde_json::from_str(json).expect("명령") };
        let plain = command(r#"{"name":"build","run":["make"]}"#);
        assert!(slow_finding(&plain, 121.0, "lint.json").is_some_and(|f| f.code == "LINT-002"));
        assert!(slow_finding(&plain, 120.0, "lint.json").is_none());
        let scoped = command(r#"{"name":"t","kind":"test","run":["x"],"level":"full"}"#);
        assert!(slow_finding(&scoped, 500.0, "lint.json").is_none());
        let required =
            command(r#"{"name":"t","kind":"test","run":["x"],"paths":["a/**"],"required":true}"#);
        assert!(slow_finding(&required, 500.0, "lint.json").is_some());
    }
}
