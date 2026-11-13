//! `fullops hold`: 보류, 재개, 통합 일괄 확인, SHA 지정(`hold`·`resume`·`settle`·`completion`)(설계 8.8절). 묶음 E가 채운다.

use crate::cli::{CliError, Ctx};

pub fn run(ctx: &Ctx, _args: lexopt::Parser) -> Result<u8, CliError> {
    Err(CliError::unimplemented(ctx.command))
}
