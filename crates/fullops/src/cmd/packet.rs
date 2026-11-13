//! `fullops packet build|check`: 탐색 패킷을 만들고 검사한다(설계 8.14절). 선택이다. 묶음 J가 채운다.

use crate::cli::{CliError, Ctx};
use fullops_core::model::{Refusal, RoleId, TaskKey};
use fullops_store::Repo;
use std::path::PathBuf;

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}

/// 묶음 사이에 부르는 진입점(설계 10.2절). `task recover`(C)가 부른다.
/// 패킷 파일이 없거나 `v != 1`이면 `Ok(None)`. 있으면 그 `options`(`seeds`에 `extra_seeds`를 더한다)와 `decisions`로
/// `packet build --force --write-inbox`와 같은 일을 하고 패킷 경로를 돌려준다. 커밋은 부르는 쪽이 한다.
/// 뼈대의 임시 구현은 항상 `Ok(None)`이다.
pub fn rebuild_packet(
    _repo: &Repo,
    _key: &TaskKey,
    _role: &RoleId,
    _extra_seeds: &[String],
) -> Result<Option<PathBuf>, Refusal> {
    Ok(None)
}
