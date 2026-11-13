//! `fullops context`: 후보 문서를 분류한다(설계 8.14절). 추천이다. 묶음 J가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
