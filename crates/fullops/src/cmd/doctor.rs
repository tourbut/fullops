//! `fullops doctor`: 설치와 레포 상태 점검(설계 10.6절). 묶음 F가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
