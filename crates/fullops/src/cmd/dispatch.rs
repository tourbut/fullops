//! `fullops dispatch`: 선행 조건 검사, 준비 커밋, 동기화, Orca 배정(설계 8.5절). 묶음 C가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
