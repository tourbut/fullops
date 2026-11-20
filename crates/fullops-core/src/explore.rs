//! 탐색(설계 8.14절): 지문, 보낼 수 있는 경로·글 규칙, 색인 항목, 배치, 분류. 전부 순수 함수다(묶음 J).
//! 타입과 시그니처는 설계 10.2절이 정본이다.

use crate::frontmatter::parse_front_matter;
use crate::lint_rules::header_summary;
use crate::messages::{self, MsgId};
use crate::model::{Refusal, RoleId, TaskKey, sha256_hex, words};
use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

words!(
    /// 색인과 `find`의 범위.
    Scope { Code = "code", Documents = "documents" }
);
words!(
    /// `context`의 후보 분류. 보내지 못한 후보, 필수 경로, Jev 실패는 모두 `Keep`이다.
    Decision { Keep = "keep", SuggestOmit = "suggest_omit", Conflict = "conflict", Caution = "caution" }
);

/// 탐색 색인 `search/<scope>.json`의 `files[]` 한 항목(설계 6.3절). 본문과 질의별 점수는 담지 않는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub path: String,
    pub blob: String,
    /// 상한 240자.
    pub summary: String,
    /// 상한 160자 24개.
    pub headings: Vec<String>,
    /// 상한 240자 24개.
    pub definitions: Vec<String>,
    /// 거짓이면(빈 파일, 바이너리, 비밀값 무늬) 후보가 되지 않는다.
    pub searchable: bool,
    pub read_error: Option<String>,
}

/// 탐색 색인 파일(설계 6.3절). 워크트리마다 따로 둔다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchIndex {
    pub v: u32,
    pub parser: String,
    pub config_sha256: String,
    pub files_sha256: String,
    pub files: Vec<IndexEntry>,
}

/// `context` 6번의 Noul 넷.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    pub relevant: f64,
    pub evidence: f64,
    pub contradicts: f64,
    pub injection: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExploreUsage {
    pub calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// 응답에 비용이 없으면 `null`.
    pub cost: Option<f64>,
}

/// `<KEY>-find.json`과 `<KEY>-documents-find.json`(설계 6.1.5절).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindResult {
    pub v: u32,
    pub key: TaskKey,
    pub role: RoleId,
    pub round: u32,
    pub head: String,
    pub instruction_sha256: String,
    pub options: FindOptions,
    pub input_sha256: String,
    pub created_at: i64,
    /// 지도(색인의 `searchable` 항목)의 수.
    pub files: u32,
    /// 그 가운데 Jev에게 물은 항목 수. 로컬 결과면 0.
    pub presented_files: u32,
    pub map_sha256: String,
    pub candidates: Vec<FindCandidate>,
    pub seed_paths: Vec<String>,
    pub remaining_candidates: Vec<String>,
    pub unreadable_files: Vec<String>,
    pub existence: Existence,
    pub semantic: bool,
    pub partial: bool,
    pub input_partial: bool,
    pub error: Option<String>,
    pub usage: ExploreUsage,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindOptions {
    pub scope: Scope,
    pub limit: u32,
    pub max_calls: u32,
    pub parser: String,
    pub offline: bool,
    /// `typesafe`·`openrouter`. 오프라인이거나 키가 없으면 `null`.
    pub provider: Option<String>,
    pub cache_bypass: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindCandidate {
    pub path: String,
    pub summary: String,
    /// `local`이면 `null`.
    pub probability: Option<f64>,
    /// `batch`(Jev의 배치 안 확률) 또는 `local`(문자 검색).
    pub basis: String,
    /// 0부터 센 배치 번호. `local`이면 `null`.
    pub batch: Option<u32>,
    /// 그 배치 안에서 0부터 센 순위.
    pub rank: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Existence {
    /// `found`·`unclear`·`not_confirmed`·`unknown`.
    pub status: String,
    pub found_probability: Option<f64>,
}

/// `<KEY>-context.json`(설계 6.1.5절).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextResult {
    pub v: u32,
    pub key: TaskKey,
    pub role: RoleId,
    pub round: u32,
    pub head: String,
    pub instruction_sha256: String,
    pub options: ContextOptions,
    pub input_sha256: String,
    pub created_at: i64,
    pub candidates: Vec<ContextCandidate>,
    pub required_paths: Vec<String>,
    pub refused_paths: Vec<RefusedPath>,
    pub semantic: bool,
    pub error: Option<String>,
    pub usage: ExploreUsage,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextOptions {
    pub paths: Vec<String>,
    pub required: Vec<String>,
    /// 후보 경로 → 작업 트리 파일의 sha256, `oversized`, `unavailable`.
    pub sources: BTreeMap<String, String>,
    pub offline: bool,
    pub provider: Option<String>,
    pub cache_bypass: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextCandidate {
    pub id: String,
    pub path: String,
    pub required: bool,
    pub sent: bool,
    /// `too large`·`not utf-8`·`empty`·`sensitive text`.
    pub unsent_reason: Option<String>,
    /// 보내지 못한 후보는 `null`.
    pub span: Option<Span>,
    pub whole: bool,
    /// 보낸 후보에만 있다.
    pub signal: Option<Signal>,
    pub decision: Decision,
    pub decision_reason: Option<String>,
}

/// 보낸 구간의 첫 줄과 끝 줄(1부터 센다).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefusedPath {
    pub path: String,
    /// `sensitive path`·`outside repo`·`symlink`·`not a file`.
    pub reason: String,
}

// ── 상수 ────────────────────────────────────────────────────────

/// 색인 형식의 이름. 다르면 색인을 전부 다시 만든다. 글 규칙이 바뀌면 올린다(옛 규칙으로 남긴 요약을 다시 본다).
pub const PARSER: &str = "map-v2";
/// 한 파일에서 읽는 양(바이트).
pub const MAX_BLOB: usize = 65_536;
pub use crate::layout::REQUIRED_DOCS;
// 2.1.0에서 패킷을 뗐다. 옛 인박스에 남은 패킷 블록이 지문과 과제 글에 섞이지 않게 표식만 남긴다
const BLOCK_START: &str = "<!-- fullops-packet:start -->";
const BLOCK_END: &str = "<!-- fullops-packet:end -->";
const REPORT_HEADING: &str = "## 완료 보고";
const MODIFIERS: [&str; 10] = [
    "pub",
    "export",
    "default",
    "async",
    "static",
    "public",
    "private",
    "protected",
    "abstract",
    "final",
];
const DEFINERS: [&str; 11] = [
    "fn",
    "def",
    "class",
    "struct",
    "enum",
    "trait",
    "interface",
    "impl",
    "func",
    "function",
    "type",
];

fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `word`가 낱말로(앞뒤가 낱말 글자가 아니게) 들어 있는가.
fn has_word(text: &str, word: &str) -> bool {
    bounded(text, word, is_word)
}

fn bounded(text: &str, word: &str, is_word: fn(char) -> bool) -> bool {
    !word.is_empty()
        && text.match_indices(word).any(|(at, _)| {
            !text[..at].chars().next_back().is_some_and(is_word)
                && !text[at + word.len()..].chars().next().is_some_and(is_word)
        })
}

/// ASCII 식별자 조각(`[A-Za-z_][A-Za-z_0-9]*`) 가운데 `min`자 이상인 것.
fn identifiers(text: &str, min: usize) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map(|w| w.trim_start_matches(|c: char| c.is_ascii_digit()))
        .filter(move |w| w.len() >= min)
}

/// J 규칙의 거절 한 건. 문구는 `messages::explore`에 있다.
pub fn refuse(rule: &'static str, args: &[String], key: &TaskKey, role: &RoleId) -> Refusal {
    Refusal {
        rule,
        reason: messages::reason(MsgId(rule), args),
        escape: messages::explore::escape(rule, key.as_str(), role.as_str()),
        exit: 2,
    }
}

// ── 지시서 글과 지문(설계 6.1.5절, 8.14절 공통) ──────────────────

/// 지시서 파일의 레포 상대 경로.
pub fn instruction_path(role: &RoleId) -> String {
    crate::layout::inbox(role)
}

/// front matter 뒤의 글. 값의 문법이 틀려도 닫는 `---`까지를 뗀다(지문이 front matter에 기대지 않게 한다).
fn body(text: &str) -> &str {
    let text = text.trim_start_matches('\u{feff}');
    let mut at = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        at += line.len();
        match (index, line.trim() == "---") {
            (0, false) => return text,
            (0, true) => {}
            (_, true) => return &text[at..],
            _ => {}
        }
    }
    text
}

/// 옛 패킷 블록과 그 뒤 빈 줄을 뺀다.
fn without_block(text: &str) -> String {
    let found = text.find(BLOCK_START).and_then(|start| {
        let end = start + text[start..].find(BLOCK_END)? + BLOCK_END.len();
        Some((start, end))
    });
    match found {
        Some((start, end)) => format!(
            "{}{}",
            &text[..start],
            text[end..].trim_start_matches(['\r', '\n'])
        ),
        None => text.to_string(),
    }
}

/// 체크한 줄(`- [x]`, `* [X]`. 들여써도 같다)을 안 한 줄로 바꾼다.
fn uncheck(line: &str) -> String {
    let item = line
        .trim_start()
        .strip_prefix(['-', '*'])
        .filter(|after| after.starts_with(char::is_whitespace))
        .map(str::trim_start)
        .filter(|item| item.starts_with("[x]") || item.starts_with("[X]"));
    match item {
        Some(item) => format!("{}[ ]{}", &line[..line.len() - item.len()], &item[3..]),
        None => line.to_string(),
    }
}

/// 지시서 지문(설계 6.1.5절의 다섯 단계). 진행 체크, 완료 보고, 패킷 블록, front matter 변경은 지문을 바꾸지 않는다.
pub fn instruction_sha256(inbox_text: &str) -> String {
    let mut out = String::new();
    for line in without_block(body(inbox_text)).lines() {
        if line.trim_end() == REPORT_HEADING {
            break;
        }
        out.push_str(uncheck(line).trim_end());
        out.push('\n');
    }
    sha256_hex(out.trim_end_matches('\n').as_bytes())
}

/// 머리 필드 `input_sha256`: `{key, role, round, head, instruction_sha256, options}`를 키 정렬한 JSON의 sha256.
pub fn input_sha256<T: Serialize>(
    key: &TaskKey,
    role: &RoleId,
    round: u32,
    head: &str,
    instruction_sha256: &str,
    options: &T,
) -> String {
    // serde_json의 객체는 키 순서로 쓰인다(preserve_order를 켜지 않는다)
    let value = serde_json::json!({
        "key": key, "role": role, "round": round, "head": head,
        "instruction_sha256": instruction_sha256, "options": options,
    });
    sha256_hex(value.to_string().as_bytes())
}

/// 과제 글: `<KEY>` 한 줄 뒤에 지시서 본문. 3,400자를 넘으면 앞뒤 1,700자만 싣고 둘째 값이 참이다(`input_partial`).
pub fn task_text(key: &TaskKey, inbox_text: &str) -> (String, bool) {
    let text = without_block(body(inbox_text));
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 3400 {
        return (format!("{key}\n{text}"), false);
    }
    let head: String = chars.iter().take(1700).collect();
    let tail: String = chars.iter().skip(chars.len() - 1700).collect();
    (
        format!("{key}\n{head}\n[partial: read full inbox]\n{tail}"),
        true,
    )
}

/// 작성 글: 지시서 본문에서 내장 템플릿과 같은 줄과 `task new`의 메모 경로 줄을 뺀 것. 리터럴과 키워드는 여기서만 뽑는다.
pub fn authored(inbox_text: &str, template: &str) -> String {
    let known: BTreeSet<&str> = template.lines().map(str::trim_end).collect();
    without_block(body(inbox_text))
        .lines()
        // `task new`가 넣은 메모 경로 줄도 템플릿처럼 검색어에서 뺀다
        .filter(|line| !known.contains(line.trim_end()) && !crate::memo::is_inbox_line(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 한 줄 안의 백틱 리터럴(1~240자).
fn backticked(text: &str) -> BTreeSet<&str> {
    let mut found = BTreeSet::new();
    for line in text.lines().filter(|l| !l.trim_start().starts_with("```")) {
        let pieces: Vec<&str> = line.split('`').collect();
        let closed = pieces.len().saturating_sub(1);
        found.extend(
            pieces
                .into_iter()
                .take(closed)
                .skip(1)
                .step_by(2)
                .filter(|p| (1..=240).contains(&p.chars().count())),
        );
    }
    found
}

/// `find` 2번의 seed 리터럴: 작성 글의 백틱 리터럴을 정렬한 앞 64개.
pub fn literals(authored: &str) -> Vec<String> {
    backticked(authored)
        .into_iter()
        .take(64)
        .map(str::to_string)
        .collect()
}

/// `context` 3번의 키워드: 작성 글의 3자 이상 식별자.
pub fn keywords(authored: &str) -> BTreeSet<String> {
    identifiers(authored, 3).map(str::to_string).collect()
}

// ── 보낼 수 있는 경로와 글(설계 8.14절 표) ───────────────────────

fn sensitive(piece: &str) -> bool {
    // Windows는 끝의 점·공백을 뗀 이름으로 파일을 연다(`.netrc.`은 `.netrc`다). `:` 뒤는 대체 데이터 스트림이다
    let p = piece.trim_end_matches(['.', ' ']).to_ascii_lowercase();
    if p.is_empty() || p.contains(':') {
        return true;
    }
    let stem = p.split('.').next().unwrap_or_default();
    p.starts_with(".env")
        || p.starts_with(".secret")
        || p.split(['.', '_', '-']).any(|word| word == "env")
        || [
            ".git",
            ".codex",
            ".agents",
            ".claude",
            ".grok",
            ".gemini",
            ".ssh",
            ".aws",
            ".netrc",
            "_netrc",
            ".npmrc",
            ".pypirc",
            ".git-credentials",
            ".credentials",
            ".kube",
            ".docker",
            ".dockercfg",
            ".pgpass",
            ".htpasswd",
        ]
        .contains(&p.as_str())
        || [
            "secret",
            "secrets",
            "credential",
            "credentials",
            "kubeconfig",
        ]
        .contains(&stem)
        || ["id_rsa", "id_dsa", "id_ecdsa", "id_ed25519"]
            .contains(&p.strip_suffix(".pub").unwrap_or(&p))
        || [".pem", ".key", ".p12", ".pfx"]
            .iter()
            .any(|end| p.ends_with(end))
}

/// Jev에 보낼 수 있는 경로인가: 레포 상대 경로이고 어느 조각도 비밀 경로 무늬에 안 걸린다.
/// 심볼릭 링크와 일반 파일 여부는 파일 시스템을 보는 쪽(색인은 tree의 모드, 명령은 작업 트리)이 가린다.
pub fn sendable_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && path
            .split('/')
            .all(|piece| !piece.is_empty() && piece != "." && piece != ".." && !sensitive(piece))
}

static SECRET: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(concat!(
        // 이름 뒤에 복수형이나 `_`·`-`로 이은 말(`SECRET_ACCESS_KEY`), 닫는 따옴표(JSON·YAML의 키, JSON 글 안에서는 `\"`)가 올 수 있다
        r"(?i:API[_-]?KEY|ACCESS[_-]?KEY|PRIVATE[_-]?KEY|CLIENT-KEY-DATA|SECRET|PASSWORD|PASSWD|PASSPHRASE|TOKEN|CREDENTIAL)",
        r#"(?:s|S|[_-][\w-]*)?(?:\\*["'])?\s*[:=]\s*\S"#,
        r"|-----BEGIN[^\n]*PRIVATE KEY-----",
        r"|sk-[A-Za-z0-9_-]{12,}",
        // GitHub·Slack 토큰, AWS 접근 키 ID, JWT
        r"|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|xox[abprs]-[A-Za-z0-9-]{10,}",
        r"|AKIA[0-9A-Z]{16}|eyJ[\w-]{8,}\.[\w-]{8,}\.",
        // 주소에 든 `사용자:비밀번호@`, netrc의 `login … password …`, `Bearer <토큰>`
        r"|://[^/\s:@]+:[^/\s@]+@",
        r"|(?i:login\s+\S+\s+password\s+\S|bearer\s+[\w.~+/-]{12,})",
    ))
    .ok()
});

/// Jev에 보낼 수 있는 글인가: 비밀값 무늬가 없다. 판정하지 못하면 보내지 않는 쪽이다.
pub fn sendable_text(text: &str) -> bool {
    SECRET
        .as_ref()
        .is_some_and(|re| matches!(re.is_match(text), Ok(false)))
}

// ── 색인(설계 6.3절, 8.14절 "색인 만들기") ───────────────────────

/// 잘린 끝 글자만 떼고 UTF-8로 읽는다. 그 밖의 깨진 바이트가 있으면 `None`이다.
fn utf8_prefix(bytes: &[u8]) -> Option<&str> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Some(text),
        Err(e) if e.error_len().is_none() => std::str::from_utf8(&bytes[..e.valid_up_to()]).ok(),
        Err(_) => None,
    }
}

/// 정의 줄이면 그 이름(이름을 읽지 못하면 빈 글). 줄 앞 공백과 수식어를 뗀 줄이 정의 낱말과 공백으로 시작해야 한다.
fn definition_name(line: &str) -> Option<&str> {
    let mut rest = line.trim_start();
    loop {
        if let Some(after) = rest.strip_prefix("pub(") {
            rest = after.split_once(')')?.1.trim_start();
            continue;
        }
        let (word, after) = rest.split_once(char::is_whitespace)?;
        if MODIFIERS.contains(&word) {
            rest = after.trim_start();
            continue;
        }
        if !DEFINERS.contains(&word) {
            return None;
        }
        let after = after.trim_start();
        let end = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(after.len());
        return Some(&after[..end]);
    }
}

/// 색인 항목 하나. `prefix`는 blob의 앞 65,536바이트, `size`는 전체 크기다.
/// 빈 파일, 바이너리, 비밀값 무늬가 있는 파일은 `searchable`이 거짓이고 요약을 남기지 않는다.
pub fn index_entry(path: &str, blob: &str, prefix: &[u8], size: u64) -> IndexEntry {
    let mut entry = IndexEntry {
        path: path.to_string(),
        blob: blob.to_string(),
        summary: String::new(),
        headings: Vec::new(),
        definitions: Vec::new(),
        searchable: false,
        read_error: None,
    };
    if size == 0 || prefix.iter().take(8000).any(|b| *b == 0) {
        return entry;
    }
    let Some(text) = utf8_prefix(prefix) else {
        entry.read_error = Some("not utf-8".into());
        return entry;
    };
    if !sendable_text(text) {
        return entry;
    }
    entry.searchable = true;
    if path.ends_with(".md") {
        entry.headings = text
            .lines()
            .filter_map(|line| {
                let title = line.trim_start_matches('#');
                ((1..=6).contains(&(line.len() - title.len())) && title.starts_with(' '))
                    .then(|| clip(title.trim(), 160))
            })
            .take(24)
            .collect();
        let front: Vec<String> = parse_front_matter(text)
            .map(|(fm, _)| {
                ["title", "summary"]
                    .iter()
                    .filter_map(|name| fm.get(name).filter(|v| !v.is_empty()))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        entry.summary = match front.is_empty() {
            true => entry.headings.first().cloned().unwrap_or_default(),
            false => front.join(" — "),
        };
    } else {
        let ext = path
            .rsplit('/')
            .next()
            .and_then(|name| name.rfind('.').map(|at| &name[at..]))
            .unwrap_or_default();
        entry.summary = header_summary(text, ext);
        entry.definitions = text
            .lines()
            .filter(|line| definition_name(line).is_some())
            .map(|line| clip(line.trim().trim_end_matches(['{', ':', ' ', '\t']), 240))
            .take(24)
            .collect();
    }
    entry.summary = clip(&entry.summary, 240);
    entry
}

/// Jev에게 주는 설명: 요약, 제목 앞 4개, 정의 앞 8개를 ` | `로 이은 것의 앞 800자.
pub fn description(entry: &IndexEntry) -> String {
    let parts: Vec<&str> = std::iter::once(&entry.summary)
        .chain(entry.headings.iter().take(4))
        .chain(entry.definitions.iter().take(8))
        .map(String::as_str)
        .filter(|part| !part.is_empty())
        .collect();
    clip(&parts.join(" | "), 800)
}

fn pair_json(entry: &IndexEntry) -> String {
    serde_json::json!([entry.path, description(entry)]).to_string()
}

/// `map_sha256`: 지도의 `[경로, 설명]` 쌍을 순서대로 늘어놓은 JSON 배열의 sha256.
pub fn map_sha256(map: &[IndexEntry]) -> String {
    let pairs: Vec<String> = map.iter().map(pair_json).collect();
    sha256_hex(format!("[{}]", pairs.join(",")).as_bytes())
}

/// `find` 4번의 배치 나누기. 각 배치는 `entries`(지도)의 색인 목록이다.
/// 한 배치는 255개 이하, 글자 예산 24,000자 이하다. 혼자 예산을 넘는 항목은 어느 배치에도 넣지 않는다.
pub fn batches(entries: &[IndexEntry], task: &str) -> Vec<Vec<usize>> {
    const BUDGET: usize = 24_000;
    let start = task.chars().count() + 1200;
    let (mut out, mut current, mut used) = (Vec::new(), Vec::new(), start);
    for (index, entry) in entries.iter().enumerate() {
        let cost = pair_json(entry).chars().count() + 32;
        if start + cost > BUDGET {
            continue;
        }
        if current.len() == 255 || used + cost > BUDGET {
            out.push(std::mem::take(&mut current));
            used = start;
        }
        current.push(index);
        used += cost;
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

// ── find의 순수 부분(설계 8.14절 `find` 3·7·9번) ─────────────────

/// 로컬 후보: 과제 글의 토큰이 `경로 + 요약`에 든 수로 고른다.
pub fn local_candidates(task: &str, map: &[IndexEntry], limit: usize) -> Vec<FindCandidate> {
    let tokens: BTreeSet<String> = task
        .split(|c: char| !(is_word(c) || matches!(c, '.' | '/' | '-')))
        .filter(|token| token.chars().count() >= 3)
        .map(str::to_lowercase)
        .collect();
    let mut scored: Vec<(usize, &IndexEntry)> = map
        .iter()
        .map(|entry| {
            let hay = format!("{} {}", entry.path, entry.summary).to_lowercase();
            let hits = tokens.iter().filter(|t| hay.contains(t.as_str())).count();
            (hits, entry)
        })
        .filter(|(hits, _)| *hits > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    scored
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(rank, (_, entry))| FindCandidate {
            path: entry.path.clone(),
            summary: entry.summary.clone(),
            probability: None,
            basis: "local".into(),
            batch: None,
            rank: rank as u32,
        })
        .collect()
}

/// 배치의 후보: 확률이 높은 순으로, 누적 확률이 0.99에 닿거나 `limit`개가 될 때까지(적어도 하나). `(선택지 순번, 확률)`.
pub fn pick(probabilities: &[f64], limit: usize) -> Vec<(usize, f64)> {
    let mut order: Vec<(usize, f64)> = probabilities.iter().copied().enumerate().collect();
    order.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let (mut out, mut sum) = (Vec::new(), 0.0);
    for item in order {
        if !out.is_empty() && (sum >= 0.99 || out.len() >= limit) {
            break;
        }
        sum += item.1;
        out.push(item);
    }
    out
}

/// 존재 판정. `exists`는 배치들의 값이고 비면(부르지 못했거나 지도가 비었다) `unknown`이다. 어떤 검사에도 쓰지 않는다.
pub fn existence(exists: &[f64]) -> Existence {
    let max = exists.iter().copied().reduce(f64::max);
    let status = match max {
        None => "unknown",
        Some(max) if max >= 0.7 => "found",
        Some(max) if max <= 0.35 => "not_confirmed",
        Some(_) => "unclear",
    };
    Existence {
        status: status.into(),
        found_probability: max,
    }
}

// ── context의 순수 부분(설계 8.14절 `context` 3·7번) ─────────────

/// 보낼 구간과 그 글, 구간이 파일 전체인가. 키워드가 낱말로 처음 나오는 35번째 줄 이후의 줄을 찾아
/// 있으면 그 5줄 앞에서, 없으면 첫 줄에서 시작해 34줄과 3,500자 안에서 끊는다. 한 줄도 못 실으면 `None`이다.
pub fn span(text: &str, keywords: &BTreeSet<String>) -> Option<(Span, String, bool)> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let hit = lines
        .iter()
        .enumerate()
        .skip(34)
        .find(|(_, line)| keywords.iter().any(|k| has_word(line, k)))
        .map(|(n, _)| n);
    let start = hit.map_or(0, |n| n - 5);
    let (mut end, mut size) = (start, 0);
    while let Some(line) = lines.get(end).filter(|_| end < start + 34) {
        size += line.chars().count();
        if size > 3500 {
            break;
        }
        end += 1;
    }
    (end > start).then(|| {
        let span = Span {
            start_line: start as u32 + 1,
            end_line: end as u32,
        };
        (
            span,
            lines[start..end].concat(),
            start == 0 && end == lines.len(),
        )
    })
}

/// `context`의 후보 분류와 그 사유(설계 8.14절 `context` 7번의 표). 경계값은 1.24.0의 값이다.
pub fn triage(
    signal: &Signal,
    sent: bool,
    whole: bool,
    required: bool,
    path: &str,
) -> (Decision, Option<&'static str>) {
    if !sent || required {
        return (Decision::Keep, None);
    }
    // 지시문이 있는 것이 정상인 문서
    let trusted = crate::layout::inside(path).is_some()
        || ["AGENTS.md", "CLAUDE.md", "GEMINI.md", "README.md"].contains(&path);
    if signal.injection > 0.7 && !trusted {
        return match whole && signal.evidence < 0.5 {
            true => (Decision::SuggestOmit, Some("instructions")),
            false => (Decision::Caution, Some("instructions")),
        };
    }
    if signal.contradicts > 0.8 || (signal.contradicts > 0.55 && signal.relevant >= 0.3) {
        return (Decision::Conflict, Some("contradicts task"));
    }
    if whole && signal.relevant < 0.1 && signal.evidence < 0.2 {
        return (Decision::SuggestOmit, Some("irrelevant"));
    }
    (Decision::Keep, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INBOX: &str = "---\nid: handover-dev\nstatus: draft\nkey: PAY-12\nround: 0\n---\n# PAY-12 — 결제 재시도\n\n- [ ] 버튼을 고친다\n  * [ ] 테스트를 더한다\n\n## 완료 보고\n\n브랜치 / SHA\n";
    const BLOCK: &str = "<!-- fullops-packet:start -->\n- `a.rs`\n<!-- fullops-packet:end -->";

    // 설계 6.1.5절: 진행 체크, 완료 보고, 패킷 블록, front matter 변경은 지문을 바꾸지 않고 지시 본문의 변경만 바꾼다
    #[test]
    fn instruction_fingerprint_follows_only_the_instruction_body() {
        let base = instruction_sha256(INBOX);
        let same = [
            INBOX.replace("status: draft", "status: \"review\nupdated: 2026-11-14"),
            INBOX
                .replace("- [ ] 버튼", "- [x] 버튼")
                .replace("  * [ ] 테스트", "  * [X] 테스트"),
            INBOX.replace("브랜치 / SHA", "fullops/dev / 4f0c1a2"),
            INBOX.replace('\n', "\r\n"),
            INBOX.replace("버튼을 고친다\n", "버튼을 고친다 \t\n"),
            // 옛 인박스(2.0.x)에 남은 패킷 블록
            INBOX.replace("## 완료 보고", &format!("{BLOCK}\n\n## 완료 보고")),
        ];
        for text in &same {
            assert_eq!(instruction_sha256(text), base, "{text}");
        }
        for text in [
            INBOX.replace("버튼을 고친다", "버튼을 지운다"),
            INBOX.replace("- [ ] 버튼", "[x] 버튼"),
            INBOX.replace("  * [ ] 테스트", "* [ ] 테스트"),
        ] {
            assert_ne!(instruction_sha256(&text), base, "{text}");
        }
        // input_sha256은 키 순서로 쓴 JSON에 기댄다
        let unordered = serde_json::json!({ "b": 1, "a": 2 }).to_string();
        assert_eq!(unordered, r#"{"a":2,"b":1}"#);
        // 작성 글은 템플릿과 같은 줄을 뺀다. 과제 글은 3,400자를 넘으면 앞뒤만 싣는다
        assert_eq!(
            authored(INBOX, "## 완료 보고\n\n브랜치 / SHA\n- [ ] 버튼을 고친다\n"),
            "# PAY-12 — 결제 재시도\n  * [ ] 테스트를 더한다"
        );
        assert_eq!(literals("`b` 와 `a`, 닫지 않은 `c"), ["a", "b"]);
        let key = TaskKey::new("PAY-12").unwrap();
        let long = format!("{INBOX}{}", "가".repeat(4000));
        let (text, partial) = task_text(&key, &long);
        assert!(partial && text.contains("\n[partial: read full inbox]\n"));
        assert_eq!(text.chars().count(), 7 + 1700 + 28 + 1700);
        assert!(!task_text(&key, INBOX).1);
    }

    // 설계 8.14절 "보낼 수 있는 경로와 글"의 표
    #[test]
    fn path_and_text_rules_follow_the_design_table() {
        for path in [
            ".env",
            "app/.env.local",
            ".secrets/x",
            "config/prod.env",
            "env.py",
            "a/my_env-1.txt",
            ".git/config",
            ".claude/settings.json",
            "home/.SSH/known_hosts",
            "secret.yaml",
            "x/Credentials",
            "id_rsa",
            "keys/id_ed25519.pub",
            "a.pem",
            "tls/server.KEY",
            "b.p12",
            "../x",
            "a//b",
            "a\\b",
            "",
            // R2-2: Windows가 같은 파일로 여는 이름(끝의 점·공백, 대체 데이터 스트림)
            ".netrc.",
            "id_rsa. ",
            ".env::$DATA",
            "docs/a.md:stream",
            "a/ /b",
            // R2-3: 자격 증명 파일
            ".git-credentials",
            "home/.credentials",
            "_netrc",
            "deploy/kubeconfig",
            "kubeconfig.yaml",
            ".kube/config",
            ".docker/config.json",
            ".pgpass",
        ] {
            assert!(!sendable_path(path), "{path}");
        }
        for path in [
            "src/environment.rs",
            ".github/workflows/ci.yml",
            ".gitignore",
            "docs/secretary.md",
            "src/pay/retry.rs",
            ".fullops/guide/project.md",
            "keys/id_rsa_notes.md",
        ] {
            assert!(sendable_path(path), "{path}");
        }
        for text in [
            "API_KEY=abc",
            "api-key : x",
            "ApiKey= 1",
            "my secret: yes",
            "PASSWORD =\thunter2",
            "token:abc",
            "-----BEGIN RSA PRIVATE KEY-----",
            "key sk-abcDEF123456_-x",
            // R2-3: 따옴표로 닫은 키, 접미사가 붙은 이름, 토큰, 주소의 비밀번호
            r#"{"password": "x"}"#,
            r#"{\"password\": \"x\"}"#,
            "'token' : x",
            "AWS_SECRET_ACCESS_KEY=abc",
            "def f(secret_access_key=\"x\")",
            "aws_access_key_id = abc",
            "secrets:\n  db: x",
            "db_passwd=x",
            "private_key: x",
            "client-key-data: LS0t",
            "credentials = x",
            "ghp_abcdefghijklmnopqrstuvwxyz0123",
            "github_pat_abcdefghijklmnopqrstuv",
            "xoxb-1234567890-abc",
            "id AKIAIOSFODNN7EXAMPLE",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.sig",
            "postgres://admin:hunter2@db.internal/prod",
            "machine a.example login bob password hunter2",
            "Authorization: Bearer abcdefghijkl",
        ] {
            assert!(!sendable_text(text), "{text}");
        }
        for text in [
            "",
            "tokens are counted",
            "the secret is safe",
            "sk-short",
            "SK-ABCDEFGHIJKLMNOP",
            "-----BEGIN CERTIFICATE-----",
            "password\n\nnext",
            "tokenizer = make()",
            "secretary: kim",
            "https://example.com:8080/a@b",
            "git@github.com:org/repo.git",
            "ghp_short",
            "the bearer of news",
        ] {
            assert!(sendable_text(text), "{text}");
        }
    }

    // 설계 8.14절 "색인 만들기" 4~6번
    #[test]
    fn index_entry_reads_metadata_only() {
        let code = "use std::fmt;\n\npub(crate) async fn retry(order: &Order) -> Receipt {\n    todo()\n}\nexport default class Cart:\ntype = 3\nlet fn_name = 1;\n";
        let entry = index_entry(
            "src/pay/retry.rs",
            "oid",
            code.as_bytes(),
            code.len() as u64,
        );
        assert!(entry.searchable && entry.read_error.is_none());
        assert_eq!(
            entry.definitions,
            [
                "pub(crate) async fn retry(order: &Order) -> Receipt",
                "export default class Cart",
                "type = 3"
            ]
        );
        let doc = "---\ntitle: 결제\nsummary: 재시도 정책\n---\n# 결제 설계\n\n## 재시도\n#태그\n";
        let entry = index_entry("docs/pay.md", "oid", doc.as_bytes(), doc.len() as u64);
        assert_eq!(
            (entry.summary.as_str(), entry.headings.len()),
            ("결제 — 재시도 정책", 2)
        );
        assert_eq!(
            description(&entry),
            "결제 — 재시도 정책 | 결제 설계 | 재시도"
        );
        let plain = index_entry("docs/a.md", "oid", "# 제목만\n".as_bytes(), 12);
        assert_eq!(plain.summary, "제목만");
        // 빈 파일, 바이너리, 비밀값 무늬는 후보가 되지 않고 요약을 남기지 않는다. 잘린 끝 글자는 읽기 오류가 아니다
        for (bytes, size) in [
            (&b""[..], 0),
            (b"fn a() {}\n\0", 11),
            (b"fn a() {}\nTOKEN = x\n", 20),
        ] {
            let entry = index_entry("a.rs", "oid", bytes, size);
            assert!(
                !entry.searchable && entry.definitions.is_empty() && entry.read_error.is_none()
            );
        }
        assert!(index_entry("a.rs", "oid", &"fn a() {} // 가".as_bytes()[..14], 99).searchable);
        let broken = index_entry("a.rs", "oid", b"fn a() \xff\xfe {}\n", 12);
        assert_eq!(
            (broken.searchable, broken.read_error.as_deref()),
            (false, Some("not utf-8"))
        );
    }

    // 설계 8.14절 `context` 7번의 표
    #[test]
    fn triage_follows_the_table_in_order() {
        let s = |relevant, evidence, contradicts, injection| Signal {
            relevant,
            evidence,
            contradicts,
            injection,
        };
        let omit = (Decision::SuggestOmit, Some("instructions"));
        let caution = (Decision::Caution, Some("instructions"));
        let conflict = (Decision::Conflict, Some("contradicts task"));
        let keep = (Decision::Keep, None);
        // (신호, whole, required, 경로) → 판정
        for (signal, whole, required, path, expected) in [
            (s(0.5, 0.4, 0.9, 0.8), true, false, "docs/a.md", omit),
            (s(0.5, 0.4, 0.9, 0.8), false, false, "docs/a.md", caution),
            (s(0.5, 0.5, 0.0, 0.8), true, false, "docs/a.md", caution),
            (
                s(0.5, 0.4, 0.9, 0.8),
                true,
                false,
                ".fullops/guide/rules/a.md",
                conflict,
            ),
            (s(0.5, 0.4, 0.0, 0.8), true, false, "README.md", keep),
            (s(0.0, 0.0, 0.81, 0.0), true, false, "docs/a.md", conflict),
            (s(0.3, 0.9, 0.56, 0.0), true, false, "docs/a.md", conflict),
            (s(0.29, 0.9, 0.56, 0.0), true, false, "docs/a.md", keep),
            (
                s(0.09, 0.19, 0.0, 0.0),
                true,
                false,
                "docs/a.md",
                (Decision::SuggestOmit, Some("irrelevant")),
            ),
            (s(0.09, 0.19, 0.0, 0.0), false, false, "docs/a.md", keep),
            (s(0.1, 0.19, 0.0, 0.0), true, false, "docs/a.md", keep),
            (s(0.0, 0.0, 0.9, 0.9), true, true, "docs/a.md", keep),
        ] {
            assert_eq!(
                triage(&signal, true, whole, required, path),
                expected,
                "{signal:?} {path}"
            );
        }
        assert_eq!(
            triage(&s(0.0, 0.0, 0.9, 0.9), false, false, false, "docs/a.md"),
            keep
        );
        // 보낼 구간: 키워드가 35번째 줄 이후에 처음 나오면 그 5줄 앞에서 34줄
        let text: String = (1..=80)
            .map(|n| format!("줄 {n}{}\n", if n == 50 { " retry" } else { "" }))
            .collect();
        let keywords = BTreeSet::from(["retry".to_string()]);
        let (found, part, whole) = span(&text, &keywords).unwrap();
        assert_eq!((found.start_line, found.end_line, whole), (45, 78, false));
        assert!(part.starts_with("줄 45\n") && part.ends_with("줄 78\n"));
        let (found, _, whole) = span("한 줄\n둘째 줄", &keywords).unwrap();
        assert_eq!((found.start_line, found.end_line, whole), (1, 2, true));
        assert!(span(&"가".repeat(3501), &keywords).is_none());
    }
}
