//! 개발 작업 실행기(설계 10.1절). `cargo xtask <dist|orca-catalog|bench|compare|baseline>`.

mod baseline;
mod bench;
mod compare;
mod dist;
mod orca_catalog;

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let task = args.next().unwrap_or_default();
    let rest: Vec<String> = args.collect();
    let result = match task.as_str() {
        "dist" => dist::run(&rest),
        "bench" => bench::run(&rest),
        "compare" => compare::run(&rest),
        "baseline" => baseline::run(&rest),
        "orca-catalog" => orca_catalog::run(&rest),
        _ => Err("사용법: cargo xtask <dist|orca-catalog|bench|compare|baseline>".to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
