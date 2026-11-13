//! `fullops review`: 리뷰 필요 판정, snapshot, 기록 검사, 면제, 정리(설계 8.9절). 묶음 E가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
