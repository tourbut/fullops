//! `fullops migrate`: Python판에서 전환과 되돌리기(설계 13절). 묶음 F가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
