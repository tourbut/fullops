//! 탐색(설계 8.14절): 색인 항목, 배치, 분류, 패킷 만들기와 검사. 전부 순수 함수다. 묶음 J가 채운다.
//! 타입과 시그니처는 설계 10.2절이 정본이다. 다른 묶음이 부르는 것은 `check_packet` 하나다.

use crate::model::{
    DeliverableRow, Known, Packet, PacketCheck, PacketDecision, PacketOutcomes, Refusal, RoleId,
    Route, SourceBasis, TaskKey, words,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

/// 지시서 지문(설계 6.1.5절). 임시 구현은 빈 글이다.
pub fn instruction_sha256(_inbox_text: &str) -> String {
    String::new()
}

/// 패킷 항목의 원문 지문. `is_instruction`이면 지시서 지문, 아니면 CRLF를 LF로 바꾼 글의 sha256이다. 임시 구현은 빈 글이다.
pub fn source_sha256(_text: &str, _is_instruction: bool) -> String {
    String::new()
}

/// Jev에 보낼 수 있는 경로인가(설계 8.14절 표). 임시 구현은 보내지 않는 쪽이다.
pub fn sendable_path(_path: &str) -> bool {
    false
}

/// Jev에 보낼 수 있는 글인가(설계 8.14절 표). 임시 구현은 보내지 않는 쪽이다.
pub fn sendable_text(_text: &str) -> bool {
    false
}

/// 색인 항목 하나. `prefix`는 blob의 앞부분, `size`는 전체 크기다. 임시 구현은 검색하지 않는 항목이다.
pub fn index_entry(path: &str, blob: &str, _prefix: &[u8], _size: u64) -> IndexEntry {
    IndexEntry {
        path: path.to_string(),
        blob: blob.to_string(),
        summary: String::new(),
        headings: Vec::new(),
        definitions: Vec::new(),
        searchable: false,
        read_error: None,
    }
}

/// `find` 4번의 배치 나누기. 각 배치는 `entries`의 색인 목록이다. 임시 구현은 배치가 없다.
pub fn batches(_entries: &[IndexEntry], _task: &str) -> Vec<Vec<usize>> {
    Vec::new()
}

/// `context`의 후보 분류와 그 사유. 임시 구현은 `Keep`이다.
pub fn triage(
    _signal: &Signal,
    _sent: bool,
    _whole: bool,
    _required: bool,
    _path: &str,
) -> (Decision, Option<&'static str>) {
    (Decision::Keep, None)
}

/// `packet build` 4번이 읽을 경로(지도 ∪ 명시 ∪ 필수 ∪ 문서 ∪ 문맥). 임시 구현은 빈 목록이다.
pub fn packet_source_paths(
    _map: &[IndexEntry],
    _find: Option<&FindResult>,
    _documents_find: Option<&FindResult>,
    _context: Option<&ContextResult>,
    _seeds: &[String],
    _required: &[String],
    _instruction_path: &str,
) -> Vec<String> {
    Vec::new()
}

/// `packet build` 2~10번(J-03). 임시 구현은 거절 없는 실패다.
pub fn build_packet(_input: &PacketInput) -> Result<Packet, Vec<Refusal>> {
    Err(Vec::new())
}

/// 패킷 검사(J-10~J-17). 패킷 조건이 거짓이면 빈 목록이다. 지문 계산은 이 안에서 `source_sha256`으로 한다.
/// `is_ancestor`는 패킷의 `head`가 검사 커밋의 조상인지, `read_text`는 경로의 지금 글(`None`은 파일 없음)이다.
/// 뼈대의 임시 구현은 항상 빈 목록이다(패킷 검사가 배정·완료·리뷰를 막지 않는다).
#[allow(clippy::too_many_arguments)]
pub fn check_packet(
    _mode: PacketCheck,
    _route: Option<&Route>,
    _packet: Option<&Packet>,
    _outcomes: Option<&PacketOutcomes>,
    _instruction: Option<&str>,
    _key: &TaskKey,
    _role: &RoleId,
    _round: u32,
    _is_ancestor: &mut dyn FnMut(&str) -> Known<bool>,
    _read_text: &mut dyn FnMut(&str) -> Known<Option<String>>,
) -> Vec<Refusal> {
    Vec::new()
}
