//! `fullops` 실행 파일. 호스트 hook과 CLI가 같은 바이너리다(설계 1절).

// 뼈대 단계: 다른 묶음이 쓸 공용 규약이 아직 불리지 않는다. 묶음을 합친 뒤 지운다.
#![allow(dead_code)]

mod assets;
mod cli;
mod cmd;
mod hook;
mod usage;

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    // hook은 인자 파서보다 먼저 본다(설계 7.1절)
    if first.as_deref().is_some_and(|a| a == "hook") {
        return hook::run(args.collect());
    }
    cli::run(first, args.collect())
}
