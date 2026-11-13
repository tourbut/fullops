//! `fullops task`: 인박스 생성, 같은 키 재오픈, 끊긴 worker 복구(설계 8.4절). 묶음 C가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
