//! `fullops merge`: 리뷰 판정, 병합, lint, push, 기록, 동기화(설계 8.8절). 묶음 E가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
