//! `fullops done`: 완료 기록 보존, 검사, push, 보낼 본문 출력(설계 8.7절). 묶음 C가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
