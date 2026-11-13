//! `fullops plan`: 대기 목록 읽기와 편집(설계 8.13절). 묶음 C가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
