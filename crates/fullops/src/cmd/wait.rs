//! `fullops wait`: heartbeat 흡수, 완료 보고 기록, 처리할 메시지 반환(설계 8.6절). 묶음 C가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
