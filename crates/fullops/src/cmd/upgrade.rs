//! `fullops upgrade`: 릴리스 노트와 필수 적용 검사(설계 10.6절). 묶음 F가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
