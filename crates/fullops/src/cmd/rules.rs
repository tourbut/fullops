//! `fullops rules [--markdown|--json]`: 규칙 표를 낸다(설계 7.5·8.13절).

use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::rules::RULES;
use lexopt::prelude::*;
use serde_json::json;

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    while let Some(arg) = args.next()? {
        match arg {
            // 사람용 출력이 곧 Markdown 표다
            Long("markdown") => {}
            other => return Err(other.unexpected().into()),
        }
    }
    if ctx.json {
        let rules: Vec<_> = RULES
            .iter()
            .map(|r| {
                json!({"id": r.id, "event": r.event.as_str(), "strength": r.strength.as_str(),
                       "escape": r.escape, "origin": r.origin})
            })
            .collect();
        println!("{}", json!({ "rules": rules }));
        return Ok(EXIT_OK);
    }
    println!("| ID | 이벤트 | 강도 | 탈출구 | 1.24.0의 대응 동작 |");
    println!("|---|---|---|---|---|");
    for r in RULES {
        let cell = |text: &str| {
            if text.is_empty() {
                "-".to_string()
            } else {
                text.replace('|', "\\|")
            }
        };
        println!(
            "| {} | {} | {} | {} | {} |",
            r.id,
            r.event,
            r.strength.as_str(),
            cell(r.escape),
            cell(r.origin)
        );
    }
    Ok(EXIT_OK)
}
