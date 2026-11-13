//! 탐색 색인과 탐색 결과·패킷 파일(설계 6.1.5·6.3·6.5절). 묶음 J가 채운다.
//! `read_packet`·`read_packet_outcomes`는 뼈대의 완성본이고 나머지는 임시 구현이다.

use crate::files::read_text;
use crate::repo::Repo;
use fullops_core::explore::{IndexEntry, Scope};
use fullops_core::model::{
    Known, Packet, PacketOutcomes, TaskKey, TreeEntry, parse_packet, parse_packet_outcomes,
};
use serde::Serialize;
use std::io;
use std::path::PathBuf;

/// 이 체크아웃의 `<KEY>-packet.json`. 없거나 JSON이 아니거나 `v != 1`이면 `Yes(None)`이다(없는 것으로 본다).
pub fn read_packet(repo: &Repo, key: &TaskKey) -> Known<Option<Packet>> {
    read_text(&repo.packet_path(key)).map(|text| parse_packet(text?.as_bytes()))
}

/// 이 체크아웃의 `<KEY>-packet-outcomes.json`. 없거나 `v != 1`이면 `Yes(None)`이다.
pub fn read_packet_outcomes(repo: &Repo, key: &TaskKey) -> Known<Option<PacketOutcomes>> {
    read_text(&repo.packet_outcomes_path(key)).map(|text| parse_packet_outcomes(text?.as_bytes()))
}

/// 탐색 결과·패킷을 `docs/evaluations/jev/<file_name>`에 쓰고 그 경로를 돌려준다.
/// `keep_history`면 있던 파일을 `history/`에 사본으로 남긴다(`--force`). 임시 구현은 쓰지 않는다.
pub fn write_explore_result<T: Serialize>(
    _repo: &Repo,
    _file_name: &str,
    _value: &T,
    _keep_history: bool,
) -> io::Result<PathBuf> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "write_explore_result는 아직 구현되지 않았습니다",
    ))
}

/// 탐색 색인 `search/<scope>.json`을 `tree`에 맞춰 고치고 항목 전체를 돌려준다(설계 6.3·6.5절).
/// `read`는 blob OID 묶음의 (크기, 앞부분)을 읽는다. 잠금을 못 얻거나 색인이 깨졌으면 전부 다시 읽어 돌려주고,
/// 쓰지 못해도 결과는 돌려준다. 임시 구현은 빈 목록이다.
pub fn update_index(
    _repo: &Repo,
    _scope: Scope,
    _config_sha256: &str,
    _tree: &[TreeEntry],
    _read: &mut dyn FnMut(&[String]) -> Known<Vec<Option<(u64, Vec<u8>)>>>,
) -> Vec<IndexEntry> {
    Vec::new()
}
