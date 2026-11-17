//! `fullops env link`: 환경 파일 연결(설계 8.5절 6단계). `dispatch`와 `task recover`가 같은 함수를 쓴다.

use super::dispatch::{main_checkout, usage};
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::layout;
use fullops_ext::{Git, GitCli};
use lexopt::prelude::*;
use serde_json::json;
use std::path::Path;

fn symlink(source: &Path, target: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    return std::os::windows::fs::symlink_file(source, target);
    #[cfg(not(windows))]
    return std::os::unix::fs::symlink(source, target);
}

/// 원본 체크아웃의 미추적 `.fullops/.env*` 가운데 대상에 없는 것을 심볼릭 링크로 만들고, 안 되면 복사한다.
/// 추적 파일과 이미 있는 파일은 건드리지 않는다. 연결한 파일 이름을 돌려준다.
pub(super) fn link_env(source_root: &Path, target_root: &Path) -> Vec<String> {
    let (source, target) = (source_root.join(layout::DIR), target_root.join(layout::DIR));
    if source == target || !target.is_dir() {
        return Vec::new();
    }
    let mut names: Vec<String> = std::fs::read_dir(&source)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(".env"))
        .collect();
    names.sort();
    let git = GitCli::cli(source_root.to_path_buf());
    let tracked = |name: &str| {
        let item = ("HEAD".to_string(), layout::at(name));
        git.show_blobs(&[item])
            .yes()
            .is_none_or(|found| found.first().is_some_and(Option::is_some))
    };
    names
        .into_iter()
        .filter(|name| !target.join(name).exists() && !tracked(name))
        .filter(|name| {
            let (from, to) = (source.join(name), target.join(name));
            symlink(&from, &to).is_ok() || std::fs::copy(&from, &to).is_ok()
        })
        .collect()
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    match args.next()? {
        Some(Value(v)) if v == "link" => {}
        _ => return Err(usage("사용법: fullops env link")),
    }
    let repo = ctx.repo()?;
    let linked = link_env(&main_checkout(repo), &repo.root);
    let human = if linked.is_empty() {
        "연결할 환경 파일이 없다.".to_string()
    } else {
        format!("환경 파일을 연결했다: {}", linked.join(", "))
    };
    ctx.print(&human, json!({ "linked": linked }));
    Ok(EXIT_OK)
}
