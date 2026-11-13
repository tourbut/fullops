//! 탐색(설계 8.14절): 지문, 보낼 수 있는 경로·글 규칙, 색인 항목, 배치, 분류, 패킷 만들기와 검사. 전부 순수 함수다(묶음 J).
//! 타입과 시그니처는 설계 10.2절이 정본이다. 다른 묶음이 부르는 것은 `check_packet` 하나다.

use crate::frontmatter::parse_front_matter;
use crate::lint_rules::header_summary;
use crate::messages::{self, MsgId};
use crate::model::{
    DeliverableRow, EXPLORE_V, Known, Packet, PacketCategory, PacketCheck, PacketDecision,
    PacketDelivered, PacketEvidence, PacketItem, PacketItemStatus, PacketOptions, PacketOutcomes,
    PacketProducer, PacketUnknown, Refusal, RoleId, Route, SourceBasis, TaskKey, sha256_hex, words,
};
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

/// 경로 하나를 읽은 결과. `Unreadable`의 글은 패킷 `unknown[].reason`이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Text { text: String, basis: SourceBasis },
    Unreadable(&'static str),
}

/// `build_packet`의 입력. 읽기는 부르는 쪽(`cmd/packet.rs`)이 끝낸다.
pub struct PacketInput<'a> {
    pub key: &'a TaskKey,
    pub role: &'a RoleId,
    pub round: u32,
    pub head: &'a str,
    pub created_at: i64,
    /// 지시서 파일의 레포 상대 경로와 인박스 글.
    pub instruction_path: &'a str,
    pub instruction: &'a str,
    pub worktree_dirty: bool,
    /// code와 documents 색인을 합친 것. 잡음 폴더를 거르는 일은 `build_packet`이 한다.
    pub map: &'a [IndexEntry],
    /// 파일이 있고 `v == 1`인 것. 낡았는지는 `build_packet`이 가린다.
    pub find: Option<&'a FindResult>,
    pub documents_find: Option<&'a FindResult>,
    pub context: Option<&'a ContextResult>,
    /// producer 이름 → 결과 파일 바이트의 sha256.
    pub producer_sha256: &'a BTreeMap<String, String>,
    /// `--seed`와 미커밋 변경 경로(기록 경로 제외).
    pub seeds: &'a [String],
    pub required: &'a [String],
    pub update_docs: &'a [String],
    pub route_deliverables: &'a [String],
    pub decisions: &'a [PacketDecision],
    /// `packet_source_paths`가 돌려준 경로마다 읽은 결과.
    pub sources: &'a BTreeMap<String, Source>,
    pub deliverables: &'a [DeliverableRow],
}

// ── 상수 ────────────────────────────────────────────────────────

/// 색인 형식의 이름. 다르면 색인을 전부 다시 만든다.
pub const PARSER: &str = "map-v1";
/// 한 파일에서 읽는 양(바이트).
pub const MAX_BLOB: usize = 65_536;
/// 필수 문서(고정 여섯).
pub const REQUIRED_DOCS: [&str; 6] = [
    ".fullops-squad/FULLOPS.md",
    ".fullops-squad/project.md",
    ".fullops-squad/rules/common/README.md",
    ".fullops-squad/rules/common/coding-style.md",
    ".fullops-squad/rules/common/security.md",
    ".fullops-squad/rules/common/testing.md",
];
pub const BLOCK_START: &str = "<!-- fullops-packet:start -->";
pub const BLOCK_END: &str = "<!-- fullops-packet:end -->";
const REPORT_HEADING: &str = "## 완료 보고";
/// 문자 검색의 지도에서 빼는 폴더. 지난 산출물과 보관 기록은 명시·필수·producer 추천으로만 들어온다.
const NOISE: [&str; 4] = [
    "artifacts/",
    ".fullops-squad/docs/evaluations/",
    ".fullops-squad/contexts/archive/",
    ".fullops-squad/handovers/logs/",
];
const SEARCH_REASON: &str = "symbol or keyword occurrence";
/// 심볼에서 빼는 흔한 낱말.
const COMMON: [&str; 20] = [
    "Task", "read", "None", "True", "False", "with", "from", "import", "rename", "return",
    "returns", "update", "callers", "tests", "response", "contract", "and", "add", "the", "API",
];
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

fn push_unique<T: PartialEq>(list: &mut Vec<T>, value: T) {
    if !list.contains(&value) {
        list.push(value);
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `word`가 낱말로(앞뒤가 낱말 글자가 아니게) 들어 있는가.
fn has_word(text: &str, word: &str) -> bool {
    bounded(text, word, is_word)
}

/// 산출물 ID가 글에 나오는가. 한글 조사가 바로 붙는 글("D05를")에서도 찾게 앞뒤의 ASCII 영숫자만 낱말 글자로 본다.
fn mentions(text: &str, id: &str) -> bool {
    bounded(text, id, |c| c.is_ascii_alphanumeric())
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
    format!(".fullops-squad/handovers/to_{role}.md")
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

/// 패킷 블록과 그 뒤 빈 줄을 뺀다.
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

/// 패킷 항목의 원문 지문. `is_instruction`이면 지시서 지문, 아니면 CRLF를 LF로 바꾼 글의 sha256이다.
pub fn source_sha256(text: &str, is_instruction: bool) -> String {
    if is_instruction {
        instruction_sha256(text)
    } else {
        sha256_hex(text.replace("\r\n", "\n").as_bytes())
    }
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

/// 작성 글: 지시서 본문에서 내장 템플릿과 같은 줄을 뺀 것. 리터럴과 키워드는 여기서만 뽑는다.
pub fn authored(inbox_text: &str, template: &str) -> String {
    let known: BTreeSet<&str> = template.lines().map(str::trim_end).collect();
    without_block(body(inbox_text))
        .lines()
        .filter(|line| !known.contains(line.trim_end()))
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
    let p = piece.to_ascii_lowercase();
    let stem = p.split('.').next().unwrap_or_default();
    p.starts_with(".env")
        || p.starts_with(".secret")
        || p.split(['.', '_', '-']).any(|word| word == "env")
        || [
            ".git", ".codex", ".agents", ".claude", ".grok", ".gemini", ".ssh", ".aws", ".netrc",
            ".npmrc", ".pypirc",
        ]
        .contains(&p.as_str())
        || ["secret", "secrets", "credential", "credentials"].contains(&stem)
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
        r"(?i:API[_-]?KEY|SECRET|PASSWORD|TOKEN)\s*[:=]\s*\S",
        r"|-----BEGIN[^\n]*PRIVATE KEY-----",
        r"|sk-[A-Za-z0-9_-]{12,}",
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
    let trusted = path.starts_with(".fullops-squad/")
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

// ── 패킷 만들기(설계 8.14절 `packet build` 2~11번) ───────────────

fn unknown_of(
    path: Option<&str>,
    source: Option<&str>,
    doc_id: Option<&str>,
    reason: &str,
) -> PacketUnknown {
    PacketUnknown {
        path: path.map(str::to_string),
        source: source.map(str::to_string),
        doc_id: doc_id.map(str::to_string),
        reason: reason.to_string(),
    }
}

fn evidence(
    method: &str,
    symbol: Option<&str>,
    line: Option<usize>,
    seed: Option<&str>,
) -> PacketEvidence {
    PacketEvidence {
        method: method.to_string(),
        symbol: symbol.map(str::to_string),
        line: line.map(|n| n as u32),
        seed: seed.map(str::to_string),
    }
}

fn read_category(path: &str) -> PacketCategory {
    match path.ends_with(".md") {
        true => PacketCategory::DocumentRead,
        false => PacketCategory::ImpactCheck,
    }
}

/// find 결과의 후보와 seed 경로.
fn find_paths(result: Option<&FindResult>) -> impl Iterator<Item = String> + '_ {
    result.into_iter().flat_map(|f| {
        f.candidates
            .iter()
            .map(|c| &c.path)
            .chain(&f.seed_paths)
            .cloned()
    })
}

/// 읽을 경로: 지도(잡음 폴더 밖) ∪ 명시 ∪ 필수 ∪ 문서 ∪ 문맥.
fn source_set(
    map: &[IndexEntry],
    find: Option<&FindResult>,
    documents_find: Option<&FindResult>,
    context: Option<&ContextResult>,
    seeds: &[String],
    required: &[String],
    instruction_path: &str,
) -> BTreeSet<String> {
    map.iter()
        .filter(|e| e.searchable && !NOISE.iter().any(|n| e.path.starts_with(n)))
        .map(|e| e.path.clone())
        .chain(seeds.iter().cloned())
        .chain(REQUIRED_DOCS.iter().map(|p| p.to_string()))
        .chain(required.iter().cloned())
        .chain([instruction_path.to_string()])
        .chain(find_paths(find))
        .chain(find_paths(documents_find))
        .chain(context.into_iter().flat_map(|c| {
            c.candidates
                .iter()
                .map(|c| &c.path)
                .chain(&c.required_paths)
                .cloned()
        }))
        .collect()
}

/// `packet build` 4번이 읽을 경로. 부르는 쪽이 이 경로들을 읽어 `PacketInput.sources`를 채운다.
pub fn packet_source_paths(
    map: &[IndexEntry],
    find: Option<&FindResult>,
    documents_find: Option<&FindResult>,
    context: Option<&ContextResult>,
    seeds: &[String],
    required: &[String],
    instruction_path: &str,
) -> Vec<String> {
    source_set(
        map,
        find,
        documents_find,
        context,
        seeds,
        required,
        instruction_path,
    )
    .into_iter()
    .collect()
}

/// `](대상)` 꼴 링크의 대상들.
fn link_targets(line: &str) -> impl Iterator<Item = &str> {
    line.split("](").skip(1).filter_map(|rest| {
        let end = rest.find(|c: char| c == ')' || c.is_whitespace())?;
        rest[end..].starts_with(')').then_some(&rest[..end])
    })
}

/// 문서 `from`의 로컬 링크가 가리키는 레포 상대 경로. 바깥 주소, 문서 안 위치, 레포 밖이면 `None`이다.
fn link_path(from: &str, target: &str) -> Option<String> {
    let target = target.split('#').next().unwrap_or_default();
    let scheme = target
        .split_once(':')
        .is_some_and(|(head, _)| !head.is_empty() && head.bytes().all(|b| b.is_ascii_lowercase()));
    if target.is_empty() || scheme || target.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for piece in target.split('/') {
        match piece {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

struct Items<'a> {
    map: BTreeMap<String, PacketItem>,
    required: &'a BTreeSet<String>,
    sources: &'a BTreeMap<String, Source>,
    hashes: &'a BTreeMap<&'a str, String>,
}

impl Items<'_> {
    fn add(
        &mut self,
        path: &str,
        category: PacketCategory,
        reason: &str,
        evidence: Option<PacketEvidence>,
        doc_id: Option<&str>,
    ) {
        let item = self.map.entry(path.to_string()).or_insert_with(|| {
            let basis = match self.sources.get(path) {
                Some(Source::Text { basis, .. }) => Some(*basis),
                _ => None,
            };
            PacketItem {
                path: path.to_string(),
                categories: Vec::new(),
                required: self.required.contains(path),
                optional: false,
                status: match basis {
                    Some(_) => PacketItemStatus::Inferred,
                    None => PacketItemStatus::Unknown,
                },
                source_sha256: self.hashes.get(path).cloned(),
                source_basis: basis,
                reasons: Vec::new(),
                evidence: Vec::new(),
                doc_ids: Vec::new(),
                decision: None,
            }
        });
        push_unique(&mut item.categories, category);
        push_unique(&mut item.reasons, reason.to_string());
        if let Some(evidence) = evidence {
            push_unique(&mut item.evidence, evidence);
        }
        if let Some(id) = doc_id {
            push_unique(&mut item.doc_ids, id.to_string());
        }
    }
}

/// `packet build` 2~10번(J-03). 작성 글에서 템플릿 줄을 빼지 않는 꼴이다(`build_packet_from`에 빈 템플릿).
pub fn build_packet(input: &PacketInput) -> Result<Packet, Vec<Refusal>> {
    build_packet_from(input, "")
}

/// `build_packet`에 인박스의 내장 템플릿 글을 더 받는다. 작성 글(심볼의 백틱 식별자, 산출물 ID 언급)에서 템플릿 줄을 뺀다.
pub fn build_packet_from(input: &PacketInput, template: &str) -> Result<Packet, Vec<Refusal>> {
    let instruction_sha = instruction_sha256(input.instruction);
    let md = |path: &str| path.ends_with(".md");

    // 2번: 지금 지시서·커밋과 같은 producer만 쓴다
    let mut unknown = Vec::new();
    let mut producers = BTreeMap::new();
    let mut producer_sha256 = BTreeMap::new();
    let mut note = |name: &str, state: Option<(bool, bool, Option<String>)>| -> bool {
        let unused = |reason: &str| PacketProducer::Unused {
            used: false,
            reason: reason.into(),
        };
        let producer = match state {
            None => unused("absent"),
            Some((false, ..)) => {
                unknown.push(unknown_of(None, Some(name), None, "stale"));
                unused("stale")
            }
            Some((true, partial, error)) => {
                if partial {
                    unknown.push(unknown_of(None, Some(name), None, "partial"));
                }
                if error.is_some() {
                    unknown.push(unknown_of(None, Some(name), None, "error"));
                }
                if let Some(sha) = input.producer_sha256.get(name) {
                    producer_sha256.insert(name.to_string(), sha.clone());
                }
                PacketProducer::Used {
                    used: true,
                    partial,
                    error,
                }
            }
        };
        let used = matches!(producer, PacketProducer::Used { .. });
        producers.insert(name.to_string(), producer);
        used
    };
    let fresh = |key: &TaskKey, role: &RoleId, round: u32, head: &str, sha: &str| {
        key == input.key
            && role == input.role
            && round == input.round
            && head == input.head
            && sha == instruction_sha
    };
    let state_of = |f: &FindResult| {
        let same = fresh(&f.key, &f.role, f.round, &f.head, &f.instruction_sha256);
        (same, f.partial, f.error.clone())
    };
    let find_used = note("find", input.find.map(state_of));
    let documents_used = note("documents-find", input.documents_find.map(state_of));
    let context_used = note(
        "context",
        input.context.map(|c| {
            let same = fresh(&c.key, &c.role, c.round, &c.head, &c.instruction_sha256);
            (same, false, c.error.clone())
        }),
    );
    let find = input.find.filter(|_| find_used);
    let documents_find = input.documents_find.filter(|_| documents_used);
    let context = input.context.filter(|_| context_used);

    // 3번: 묶음
    let explicit: BTreeSet<String> = input.seeds.iter().cloned().collect();
    let required: BTreeSet<String> = REQUIRED_DOCS
        .iter()
        .map(|p| p.to_string())
        .chain(input.required.iter().cloned())
        .chain([input.instruction_path.to_string()])
        .chain(context.into_iter().flat_map(|c| c.required_paths.clone()))
        .collect();
    let update_ids: BTreeSet<&String> = input
        .update_docs
        .iter()
        .chain(input.route_deliverables)
        .collect();
    let recommended: BTreeSet<String> = find_paths(find).collect();
    let mut documents: BTreeSet<String> = find_paths(documents_find).collect();
    documents.extend(recommended.iter().filter(|p| md(p)).cloned());
    let contexts: BTreeMap<&str, Decision> = context
        .into_iter()
        .flat_map(|c| c.candidates.iter().map(|c| (c.path.as_str(), c.decision)))
        .collect();
    let omit = |path: &str| contexts.get(path) == Some(&Decision::SuggestOmit);

    // 4번: 원문. 지문은 지시서 파일만 지시서 지문이다
    let wanted = source_set(
        input.map,
        find,
        documents_find,
        context,
        input.seeds,
        input.required,
        input.instruction_path,
    );
    let hashes: BTreeMap<&str, String> = input
        .sources
        .iter()
        .filter_map(|(path, source)| match source {
            Source::Text { text, .. } => Some((
                path.as_str(),
                source_sha256(text, path == input.instruction_path),
            )),
            Source::Unreadable(_) => None,
        })
        .collect();
    // 지시서는 패킷 블록을 뺀 글을 훑는다(지난 블록의 경로가 근거가 되지 않게 한다)
    let instruction_text = without_block(input.instruction);
    let texts: Vec<(&str, &str)> = wanted
        .iter()
        .filter_map(|path| match input.sources.get(path) {
            Some(Source::Text { .. }) if path == input.instruction_path => {
                Some((path.as_str(), instruction_text.as_str()))
            }
            Some(Source::Text { text, .. }) => Some((path.as_str(), text.as_str())),
            _ => None,
        })
        .collect();
    for path in &wanted {
        match input.sources.get(path) {
            Some(Source::Text { .. }) => {}
            Some(Source::Unreadable(reason)) => {
                unknown.push(unknown_of(Some(path), None, None, reason))
            }
            None => unknown.push(unknown_of(Some(path), None, None, "missing")),
        }
    }

    // 5번: 항목
    let mut items = Items {
        map: BTreeMap::new(),
        required: &required,
        sources: input.sources,
        hashes: &hashes,
    };
    for path in &explicit {
        let category = match md(path) {
            true => PacketCategory::DocumentUpdate,
            false => PacketCategory::DirectEdit,
        };
        items.add(path, category, "explicit seed or diff", None, None);
    }
    let read: BTreeSet<&str> = required
        .iter()
        .chain(&documents)
        .chain(&recommended)
        .map(String::as_str)
        .chain(contexts.keys().copied().filter(|p| !omit(p)))
        .collect();
    for path in read {
        let reason = "required or recommended context";
        items.add(path, read_category(path), reason, None, None);
    }

    // 6번: 문자 검색 근거. 호출 관계의 확정 근거가 아니다
    let authored_text = authored(input.instruction, template);
    let stop: BTreeSet<String> = input
        .key
        .as_str()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .chain([input.role.as_str()])
        .map(str::to_lowercase)
        .collect();
    let skip = |s: &str| s.is_empty() || COMMON.contains(&s) || stop.contains(&s.to_lowercase());
    let mut symbols: Vec<&str> = Vec::new();
    for path in explicit.iter().filter(|p| !md(p)) {
        let Some(Source::Text { text, .. }) = input.sources.get(path) else {
            continue;
        };
        let names = text
            .lines()
            .enumerate()
            .filter_map(|(n, line)| Some((n + 1, definition_name(line).filter(|s| !skip(s))?)));
        for (line, name) in names.take(64) {
            let found = evidence("definition", Some(name), Some(line), Some(path));
            let category = PacketCategory::DirectEdit;
            items.add(path, category, "seed definition", Some(found), None);
            push_unique(&mut symbols, name);
        }
    }
    let title = body(input.instruction)
        .lines()
        .find(|line| line.starts_with("# "))
        .unwrap_or_default();
    let named = |s: &&str| s.contains('_') && identifiers(s, 1).next() == Some(*s);
    let terms: BTreeSet<&str> = identifiers(title, 4)
        .chain(backticked(&authored_text).into_iter().filter(named))
        .filter(|s| !skip(s))
        .collect();
    for term in terms {
        push_unique(&mut symbols, term);
    }
    symbols.truncate(64);
    // 레포 전체에 깔린 이름은 근거가 아니다
    let present: Vec<Vec<&str>> = texts
        .iter()
        .map(|(_, text)| {
            symbols
                .iter()
                .copied()
                .filter(|s| has_word(text, s))
                .collect()
        })
        .collect();
    let limit = 25.max(texts.len() / 10);
    let common = |symbol: &str| present.iter().filter(|p| p.contains(&symbol)).count() > limit;
    let ubiquitous: BTreeSet<&str> = symbols.iter().copied().filter(|s| common(s)).collect();
    for ((path, text), found) in texts.iter().zip(&present) {
        let found: Vec<&str> = match explicit.contains(*path) {
            true => Vec::new(),
            false => found
                .iter()
                .copied()
                .filter(|s| !ubiquitous.contains(s))
                .collect(),
        };
        if found.is_empty() && !md(path) {
            continue;
        }
        for (n, line) in text.lines().enumerate() {
            for symbol in found.iter().filter(|s| has_word(line, s)).take(4) {
                let found = evidence("string-search", Some(symbol), Some(n + 1), None);
                items.add(path, read_category(path), SEARCH_REASON, Some(found), None);
            }
            let seeds = link_targets(line)
                .filter(|_| md(path))
                .filter_map(|target| link_path(path, target))
                .filter(|seed| explicit.contains(seed));
            for seed in seeds {
                let found = evidence("markdown-link", None, Some(n + 1), Some(&seed));
                let category = PacketCategory::DocumentRead;
                items.add(path, category, "local link to seed", Some(found), None);
            }
        }
    }

    // 7번: 산출물 인덱스
    for row in input.deliverables {
        let update = update_ids.contains(&row.id);
        for source in &row.sources {
            let found = Some(evidence("deliverable-index", None, None, None));
            if update {
                let category = PacketCategory::DocumentUpdate;
                items.add(source, category, "deliverable update", found, Some(&row.id));
            } else if items.map.contains_key(source) || mentions(&authored_text, &row.id) {
                let category = PacketCategory::DocumentRead;
                let reason = "deliverable reference";
                items.add(source, category, reason, found, Some(&row.id));
            }
        }
    }
    let sourced = |id: &str| {
        input
            .deliverables
            .iter()
            .any(|row| row.id == id && !row.sources.is_empty())
    };
    for id in update_ids.iter().filter(|id| !sourced(id)) {
        unknown.push(unknown_of(None, None, Some(id), "no source"));
    }

    // 8번: 선택 항목과 context의 판정
    for (path, decision) in &contexts {
        if omit(path) && !items.map.contains_key(*path) {
            items.add(path, read_category(path), "optional context", None, None);
            if let Some(item) = items.map.get_mut(*path) {
                item.optional = true;
            }
        }
        if let Some(item) = items.map.get_mut(*path) {
            item.decision = Some(*decision);
        }
    }

    // 9번: 의견. 항목 목록은 바꾸지 않는다
    let refusals: Vec<Refusal> = input
        .decisions
        .iter()
        .filter_map(|d| {
            let why = match items.map.get(&d.path) {
                None => "패킷에 없는 경로다",
                Some(_) if !["add", "exclude", "change"].contains(&d.action.as_str()) => {
                    "action이 add·exclude·change가 아니다"
                }
                Some(_) if d.reason.trim().is_empty() => "reason이 비었다",
                Some(item) if d.action == "exclude" && item.required => {
                    "필수 항목은 exclude할 수 없다"
                }
                Some(_) => return None,
            };
            let args = [format!("`{}` {why}", d.path)];
            Some(refuse("J-03", &args, input.key, input.role))
        })
        .collect();
    if !refusals.is_empty() {
        return Err(refusals);
    }

    // 10번: 전달 목록
    let mut items: Vec<PacketItem> = items.map.into_values().collect();
    items.sort_by(|a, b| {
        (a.optional, !a.required, &a.path).cmp(&(b.optional, !b.required, &b.path))
    });
    let mut primary: Vec<&PacketItem> = items
        .iter()
        .filter(|i| !i.required && !i.optional)
        .collect();
    primary.sort_by_key(|i| {
        (
            i.reasons == [SEARCH_REASON],
            std::cmp::Reverse(i.evidence.len()),
            &i.path,
        )
    });
    let mut context_paths: Vec<String> = primary.iter().map(|i| i.path.clone()).collect();
    let remaining_context_paths = context_paths.split_off(context_paths.len().min(20));
    let edits = [PacketCategory::DirectEdit, PacketCategory::DocumentUpdate];
    let delivered = items
        .iter()
        .filter(|i| {
            !i.optional
                && (i.required
                    || context_paths.contains(&i.path)
                    || i.categories.iter().any(|c| edits.contains(c)))
        })
        .flat_map(|i| {
            i.categories.iter().map(|category| PacketDelivered {
                path: i.path.clone(),
                category: *category,
            })
        })
        .collect();

    let source_hashes: BTreeMap<&str, Option<&String>> = input
        .sources
        .keys()
        .map(|path| (path.as_str(), hashes.get(path.as_str())))
        .collect();
    let options = PacketOptions {
        seeds: explicit.into_iter().collect(),
        required: required.iter().cloned().collect(),
        update_docs: update_ids.into_iter().cloned().collect(),
        decisions: input.decisions.to_vec(),
        sources_sha256: sha256_hex(
            serde_json::to_string(&source_hashes)
                .unwrap_or_default()
                .as_bytes(),
        ),
        producer_sha256,
    };
    Ok(Packet {
        v: EXPLORE_V,
        key: input.key.clone(),
        role: input.role.clone(),
        round: input.round,
        head: input.head.to_string(),
        input_sha256: input_sha256(
            input.key,
            input.role,
            input.round,
            input.head,
            &instruction_sha,
            &options,
        ),
        instruction_sha256: instruction_sha,
        created_at: input.created_at,
        worktree_dirty: input.worktree_dirty,
        partial: !unknown.is_empty(),
        optional_context_paths: items
            .iter()
            .filter(|i| i.optional)
            .map(|i| i.path.clone())
            .collect(),
        required_paths: options.required.clone(),
        options,
        items,
        delivered,
        context_paths,
        remaining_context_paths,
        unknown,
        producers,
        decisions: input.decisions.to_vec(),
    })
}

/// 지시서에 넣는 패킷 블록(설계 8.14절 `packet build` 11번). 목록 줄은 `delivered`의 경로마다 하나다.
pub fn packet_block(packet: &Packet, packet_path: &str) -> String {
    let mut lines = vec![
        BLOCK_START.to_string(),
        "### 탐색 근거와 읽을 구간".to_string(),
        String::new(),
        format!(
            "정본: `{packet_path}` / SHA `{}` / partial={}",
            packet.head, packet.partial
        ),
    ];
    let paths: BTreeSet<&str> = packet.delivered.iter().map(|d| d.path.as_str()).collect();
    for item in packet
        .items
        .iter()
        .filter(|i| paths.contains(i.path.as_str()))
    {
        let categories: Vec<&str> = item.categories.iter().map(|c| c.as_str()).collect();
        let found: BTreeSet<u32> = item.evidence.iter().filter_map(|e| e.line).collect();
        // 근거 줄이 많으면 지시서가 길어지므로 앞 12개만 적는다. 전체는 정본에 있다
        let mut spans: Vec<String> = found.iter().take(12).map(u32::to_string).collect();
        if found.len() > 12 {
            spans.push("…".into());
        }
        lines.push(format!(
            "- `{}` ({}) · 줄 {} · {}{}{}",
            item.path,
            categories.join(", "),
            match spans.is_empty() {
                true => "전체".to_string(),
                false => spans.join(", "),
            },
            item.status,
            if item.required { " · 필수" } else { "" },
            item.decision.map(|d| format!(" · {d}")).unwrap_or_default(),
        ));
    }
    lines.push(format!(
        "나머지 참조 후보 {}건은 정본의 remaining_context_paths에 있다. 완료 검사는 위 목록만 요구한다.",
        packet.remaining_context_paths.len()
    ));
    for d in &packet.decisions {
        lines.push(format!(
            "- 의견(원래 추천 유지): `{}` {} — {}",
            d.path, d.action, d.reason
        ));
    }
    if !packet.unknown.is_empty() {
        let first: Vec<String> = packet
            .unknown
            .iter()
            .take(5)
            .map(|u| {
                let what = u.path.as_ref().or(u.source.as_ref()).or(u.doc_id.as_ref());
                format!("{}({})", what.map_or("-", String::as_str), u.reason)
            })
            .collect();
        lines.push(format!(
            "미확인 {}건: {}. 정본의 unknown을 확인한다.",
            packet.unknown.len(),
            first.join(", ")
        ));
    }
    lines.push(BLOCK_END.to_string());
    lines.join("\n")
}

/// 지시서의 `## 완료 보고` 줄 바로 앞에 블록을 넣는다(있던 블록은 바꾼다). 그 줄이 없으면 끝에 붙인다.
pub fn with_block(inbox_text: &str, block: &str) -> String {
    let text = without_block(inbox_text);
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if line.trim_end() == REPORT_HEADING {
            return format!("{}{block}\n\n{}", &text[..at], &text[at..]);
        }
        at += line.len();
    }
    format!("{}\n\n{block}\n", text.trim_end())
}

// ── 패킷 검사(설계 8.14절 `check_packet`) ────────────────────────

/// 문구에 싣는 목록. 앞 8건만 적는다.
fn listed(items: &[String]) -> String {
    let more = match items.len() > 8 {
        true => format!(" 외 {}건", items.len() - 8),
        false => String::new(),
    };
    let first: Vec<&str> = items.iter().take(8).map(String::as_str).collect();
    format!("{}{more}", first.join(", "))
}

/// 패킷 검사(J-10~J-17). 패킷 조건(패킷 파일이 있고 `v == 1`이다, 또는 route의 `requires_packet`이 참이다)이 거짓이면 빈 목록이다.
/// `is_ancestor`는 패킷의 `head`가 검사 커밋의 조상인지, `read_text`는 경로의 지금 글(`None`은 파일 없음)이다.
/// 지문은 이 안에서 `source_sha256`으로 구한다. 지시서 파일 항목은 `instruction`의 글로 구하고 `read_text`를 부르지 않는다.
/// 모르는 사실(`Unknown`, `instruction`이 없음)은 걸리지 않은 것으로 본다.
#[allow(clippy::too_many_arguments)]
pub fn check_packet(
    mode: PacketCheck,
    route: Option<&Route>,
    packet: Option<&Packet>,
    outcomes: Option<&PacketOutcomes>,
    instruction: Option<&str>,
    key: &TaskKey,
    role: &RoleId,
    round: u32,
    is_ancestor: &mut dyn FnMut(&str) -> Known<bool>,
    read_text: &mut dyn FnMut(&str) -> Known<Option<String>>,
) -> Vec<Refusal> {
    let refuse = |rule: &'static str, arg: String| refuse(rule, &[arg], key, role);
    let Some(packet) = packet else {
        return match route.is_some_and(|r| r.requires_packet) {
            true => vec![refuse("J-17", key.to_string())],
            false => Vec::new(),
        };
    };
    let now = instruction.map(instruction_sha256);
    let differs: Vec<&str> = [
        ("key", &packet.key != key),
        ("role", &packet.role != role),
        ("round", packet.round != round),
        (
            "instruction_sha256",
            now.as_ref()
                .is_some_and(|sha| *sha != packet.instruction_sha256),
        ),
    ]
    .iter()
    .filter_map(|(name, differs)| differs.then_some(*name))
    .collect();
    if !differs.is_empty() {
        return vec![refuse("J-10", differs.join("·"))];
    }
    let mut out = Vec::new();
    let stale = is_ancestor(&packet.head) == Known::Yes(false);
    match mode {
        PacketCheck::Delivery => {
            if stale {
                out.push(refuse("J-11", clip(&packet.head, 12)));
            }
            let inbox = instruction_path(role);
            let mut moved = Vec::new();
            for item in packet
                .items
                .iter()
                .filter(|i| !i.optional && (i.required || i.status == PacketItemStatus::Inferred))
            {
                let sha = match item.path == inbox {
                    true => match &now {
                        Some(sha) => Known::Yes(Some(sha.clone())),
                        None => Known::Unknown("instruction"),
                    },
                    false => read_text(&item.path).map(|t| t.map(|t| source_sha256(&t, false))),
                };
                match sha {
                    Known::Yes(None) => moved.push(format!("`{}`(없음)", item.path)),
                    Known::Yes(Some(sha))
                        if item.source_sha256.as_ref().is_some_and(|made| *made != sha) =>
                    {
                        moved.push(format!("`{}`", item.path))
                    }
                    _ => {}
                }
            }
            if !moved.is_empty() {
                out.push(refuse("J-12", listed(&moved)));
            }
        }
        PacketCheck::Completion => {
            if stale {
                out.push(refuse("J-13", clip(&packet.head, 12)));
            }
            let Some(outcomes) = outcomes else {
                out.push(refuse("J-14", "파일이 없거나 v가 1이 아니다".into()));
                return out;
            };
            if outcomes.packet_input_sha256 != packet.input_sha256 || outcomes.round != packet.round
            {
                let why = "packet_input_sha256 또는 round가 패킷과 다르다";
                out.push(refuse("J-14", why.into()));
                return out;
            }
            let allowed: BTreeSet<(&str, PacketCategory)> = packet
                .items
                .iter()
                .filter(|i| !i.optional)
                .flat_map(|i| i.categories.iter().map(|c| (i.path.as_str(), *c)))
                .collect();
            let mut seen = BTreeSet::new();
            let mut wrong = Vec::new();
            for row in &outcomes.items {
                let pair = (row.path.as_str(), row.category);
                let why = if !seen.insert(pair) {
                    "두 번 있다"
                } else if !allowed.contains(&pair) {
                    "패킷의 항목이 아니다"
                } else if !["completed", "no_change"].contains(&row.status.as_str()) {
                    "status가 completed·no_change가 아니다"
                } else if row.reason.trim().is_empty() {
                    "reason이 비었다"
                } else {
                    continue;
                };
                wrong.push(format!("`{}` {} {why}", row.path, row.category));
            }
            let missing = packet
                .delivered
                .iter()
                .filter(|d| !seen.contains(&(d.path.as_str(), d.category)))
                .map(|d| format!("`{}` {} 빠졌다", d.path, d.category));
            wrong.extend(missing);
            if !wrong.is_empty() {
                out.push(refuse("J-15", listed(&wrong)));
            }
            if packet.partial && outcomes.uncertainty_review.trim().is_empty() {
                out.push(refuse("J-16", packet.unknown.len().to_string()));
            }
        }
    }
    out
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
            with_block(INBOX, BLOCK),
        ];
        for text in &same {
            assert_eq!(instruction_sha256(text), base, "{text}");
        }
        // 블록은 완료 보고 바로 앞에 들어가고, 다시 넣어도 글이 같다
        assert!(same[5].contains(&format!("{BLOCK}\n\n## 완료 보고\n")));
        assert_eq!(with_block(&same[5], BLOCK), same[5]);
        let no_report = INBOX.split("## 완료 보고").next().unwrap();
        let appended = with_block(no_report, BLOCK);
        assert_eq!(with_block(&appended, BLOCK), appended);
        assert_eq!(instruction_sha256(&appended), base);
        for text in [
            INBOX.replace("버튼을 고친다", "버튼을 지운다"),
            INBOX.replace("- [ ] 버튼", "[x] 버튼"),
            INBOX.replace("  * [ ] 테스트", "* [ ] 테스트"),
        ] {
            assert_ne!(instruction_sha256(&text), base, "{text}");
        }
        assert_eq!(source_sha256(INBOX, true), base);
        assert_eq!(source_sha256("a\r\nb", false), source_sha256("a\nb", false));
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
        ] {
            assert!(!sendable_path(path), "{path}");
        }
        for path in [
            "src/environment.rs",
            ".github/workflows/ci.yml",
            ".gitignore",
            "docs/secretary.md",
            "src/pay/retry.rs",
            ".fullops-squad/project.md",
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
                ".fullops-squad/rules/a.md",
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
