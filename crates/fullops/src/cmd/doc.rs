//! `fullops doc`: 산출물 문서 검사와 front matter 갱신(설계 8.12절). 묶음 D가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
