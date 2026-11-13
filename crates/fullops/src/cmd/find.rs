//! `fullops find`: 지시서에 맞는 코드·문서 후보를 찾는다(설계 8.14절). 추천이다. 묶음 J가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
