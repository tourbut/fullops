//! `fullops upgrade [--verify]`: `applied_version` 이후의 릴리스 노트와 필수 적용 검사(설계 10.6절).
//! `--verify`가 없으면 레포를 고치지 않는다. 검사가 전부 통과한 `--verify`만 `applied_version`을 올린다. 낮추지 않는다.

use super::init;
use crate::assets;
use crate::cli::{CliError, Ctx, EXIT_OK, EXIT_REFUSED};
use serde_json::json;
use std::path::Path;

/// 릴리스가 요구하는 레포 적용 하나. `since`보다 낮은 `applied_version`의 레포에서 검사한다.
struct RequiredApply {
    since: &'static str,
    id: &'static str,
    check: fn(&Path) -> bool,
    hint: &'static str,
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_default()
}

const REQUIRED: &[RequiredApply] = &[
    RequiredApply {
        since: "2.0.0",
        id: "managed-block",
        check: |root| read(root, "AGENTS.md").contains(assets::BLOCK_START),
        hint: "`AGENTS.md`·`CLAUDE.md`·`GEMINI.md`에 관리 블록이 없다. `fullops init`을 다시 실행한다.",
    },
    RequiredApply {
        since: "2.0.0",
        id: "explore-history-ignore",
        check: |root| {
            let ignore = read(root, ".fullops-squad/.gitignore");
            assets::GITIGNORE_LINES
                .iter()
                .all(|line| ignore.lines().any(|l| l.trim() == *line))
        },
        hint: "`.fullops-squad/.gitignore`에 `docs/evaluations/jev/history/` 줄을 더한다.",
    },
    RequiredApply {
        since: "2.0.0",
        id: "delegation-rule",
        check: |root| {
            let text = read(root, ".fullops-squad/rules/delegation.md").replace("\r\n", "\n");
            !text.is_empty()
                && fullops_core::model::sha256_hex(text.as_bytes())
                    != assets::DELEGATION_1_24_SHA256
        },
        hint: "`.fullops-squad/rules/delegation.md`가 없거나 1.24.0의 글이다. 병렬 방침에 맞게 고친다(없으면 `fullops init`이 새 템플릿을 둔다).",
    },
];

fn triple(version: &str) -> (u64, u64, u64) {
    let mut parts = version
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|p| p.parse().ok());
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

pub fn run(ctx: &Ctx, mut parser: lexopt::Parser) -> Result<u8, CliError> {
    use lexopt::prelude::*;
    let mut verify = false;
    while let Some(arg) = parser.next()? {
        match arg {
            Long("verify") => verify = true,
            other => return Err(other.unexpected().into()),
        }
    }
    let repo = ctx.repo()?;
    let path = repo.config_path();
    let text = std::fs::read_to_string(&path)?;
    // 정책 오류인 레포에서도 돌아야 하므로 버전은 글에서 직접 읽는다
    let applied = toml::from_str::<toml::Table>(&text)
        .ok()
        .and_then(|t| t.get("applied_version")?.as_str().map(str::to_string))
        .unwrap_or_else(|| "0.0.0".into());
    let current = env!("CARGO_PKG_VERSION");
    let mut lines = vec![format!("레포 적용 버전 {applied}, 설치 버전 {current}")];

    let notes: Vec<&(&str, &str)> = assets::RELEASES
        .iter()
        .filter(|(version, _)| triple(version) > triple(&applied))
        .collect();
    for (version, body) in &notes {
        lines.push(format!("\n--- 릴리스 {version} ---\n{}", body.trim_end()));
    }
    let failed: Vec<&RequiredApply> = REQUIRED
        .iter()
        .filter(|r| triple(r.since) > triple(&applied) && !(r.check)(&repo.root))
        .collect();
    if failed.is_empty() {
        lines.push("\n필수 적용: 남은 것이 없다.".into());
    } else {
        lines.push("\n필수 적용(모두 적용한다):".into());
        lines.extend(failed.iter().map(|r| format!("- [{}] {}", r.id, r.hint)));
    }

    let mut raised = false;
    if verify && failed.is_empty() && triple(current) > triple(&applied) {
        let next = init::set_top_key(&text, "applied_version", current);
        fullops_store::write_atomic(&path, next.as_bytes())?;
        lines.push(format!(
            "applied_version을 {current}로 올렸다. 변경을 커밋한다."
        ));
        raised = true;
    }
    let ok = failed.is_empty();
    ctx.print(
        &lines.join("\n"),
        json!({
            "ok": ok || !verify,
            "applied_version": if raised { current } else { applied.as_str() },
            "installed_version": current,
            "releases": notes.iter().map(|(v, _)| *v).collect::<Vec<_>>(),
            "required": failed.iter().map(|r| json!({"id": r.id, "hint": r.hint})).collect::<Vec<_>>(),
            "raised": raised,
        }),
    );
    Ok(if verify && !ok { EXIT_REFUSED } else { EXIT_OK })
}
