//! `fullops why`: hook 추적과 이벤트별 실행 표식(설계 8.13절). 묶음 B가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
