//! `fullops sync`: 예약된 워크트리 동기화(설계 8.8절). `merge`의 M-11이 같은 `sync_role`을 쓴다.

use super::hold::{known, load_state, refused, value};
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::messages::merge as msg;
use fullops_core::model::{RoleId, Sha, State, path_text, same_path};
use fullops_ext::{Git, GitCli, MergeMode, MergeOutcome, Worktree};
use lexopt::prelude::*;
use serde_json::json;
use std::path::Path;

pub(super) enum Synced {
    Done,
    /// 지금은 반영하지 못한다. 글은 사유다.
    Pending(&'static str),
    /// 병합이 충돌해 되돌렸다.
    Conflict,
}

/// 기본 브랜치를 그 워크트리에 반영한다: ff, 아니면 병합 커밋. 강제 reset과 삭제는 쓰지 않는다.
fn merge_base_into(worktree: &Path, base_tip: &Sha) -> Synced {
    let git = GitCli::cli(worktree.to_path_buf());
    if git
        .merge(worktree, base_tip.as_str(), MergeMode::FfOnly)
        .is_ok()
    {
        return Synced::Done;
    }
    match git.merge(worktree, base_tip.as_str(), MergeMode::NoEdit) {
        Ok(MergeOutcome::Done) => Synced::Done,
        Ok(MergeOutcome::Conflict) => {
            let _ = git.merge_abort(worktree);
            Synced::Conflict
        }
        Err(_) => Synced::Pending("기본 브랜치를 병합하지 못했다"),
    }
}

/// 역할 워크트리 하나의 동기화(M-11). 정산되지 않은 dispatch가 있거나 깨끗하지 않으면 예약으로 남긴다.
pub(super) fn sync_role(state: &State, worktree: &Worktree, base_tip: &Sha) -> Synced {
    let path = path_text(&worktree.path);
    let working = |d: &fullops_core::model::Dispatch| {
        d.settled.is_none() && !d.is_reservation() && same_path(&d.worktree, &path)
    };
    if state.dispatches.iter().any(working) {
        return Synced::Pending("정산되지 않은 dispatch가 있다");
    }
    match GitCli::cli(worktree.path.clone()).status(None).yes() {
        Some(changes) if changes.is_empty() => merge_base_into(&worktree.path, base_tip),
        Some(_) => Synced::Pending("커밋하지 않은 변경이 있다"),
        None => Synced::Pending("워크트리 상태를 읽지 못했다"),
    }
}

#[derive(Default)]
pub(super) struct SyncReport {
    pub done: Vec<RoleId>,
    pub pending: Vec<(RoleId, &'static str)>,
    pub conflict: Vec<RoleId>,
}

impl SyncReport {
    /// 반영하지 못한 역할과 사유.
    pub fn left(&self) -> Vec<String> {
        let pending = self.pending.iter().map(|(r, why)| format!("{r}({why})"));
        let conflict = self.conflict.iter().map(|r| format!("{r}(충돌)"));
        pending.chain(conflict).collect()
    }
}

/// 등록 역할(`only`가 있으면 그 역할만)의 워크트리를 동기화하고 `sync_pending`을 맞춘다.
pub(super) fn sync_roles(
    ctx: &Ctx,
    git: &GitCli,
    only: Option<&[RoleId]>,
) -> Result<SyncReport, CliError> {
    let config = ctx.config()?;
    let base_tip = known(
        git.rev_parse(&format!("refs/heads/{}", config.base)),
        "기본 브랜치 tip",
    )?;
    let worktrees = known(git.worktrees(), "워크트리 목록")?;
    let state = load_state(ctx)?;
    let mut report = SyncReport::default();
    for (role, def) in &config.roles {
        if only.is_some_and(|only| !only.contains(role)) || def.branch == config.base {
            continue;
        }
        let checkout = |w: &&Worktree| w.branch.as_deref() == Some(def.branch.as_str());
        let Some(worktree) = worktrees.iter().find(checkout) else {
            continue;
        };
        match sync_role(&state, worktree, &base_tip) {
            Synced::Done => report.done.push(role.clone()),
            Synced::Pending(why) => report.pending.push((role.clone(), why)),
            Synced::Conflict => report.conflict.push(role.clone()),
        }
    }
    ctx.update_state(None, |state| {
        for role in &report.done {
            state.sync_pending.remove(role);
        }
        let left = report.pending.iter().map(|(r, _)| r);
        for role in left.chain(&report.conflict) {
            state.sync_pending.insert(role.clone(), base_tip.clone());
        }
    })?;
    Ok(report)
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let (mut role, mut own) = (None, false);
    while let Some(arg) = args.next()? {
        match arg {
            Long("role") => {
                let id = RoleId::new(value(&mut args)?).map_err(|e| CliError::Usage(e.to_string()));
                role = Some(id?);
            }
            Long("self") => own = true,
            other => return Err(other.unexpected().into()),
        }
    }
    let (repo, config) = (ctx.repo()?, ctx.config()?);
    let git = GitCli::cli(repo.root.clone());
    if own {
        let base_tip = known(
            git.rev_parse(&format!("refs/heads/{}", config.base)),
            "기본 브랜치 tip",
        )?;
        return match merge_base_into(&repo.root, &base_tip) {
            Synced::Done => {
                ctx.print(
                    "이 체크아웃에 기본 브랜치를 반영했습니다.",
                    json!({ "self": true }),
                );
                Ok(EXIT_OK)
            }
            Synced::Conflict => Err(CliError::Human {
                decision: None,
                message: msg::human("M-11", "conflict", &["이 체크아웃"]),
            }),
            Synced::Pending(why) => Err(refused("M-11", "pending", &[why])),
        };
    }
    let roles: Vec<RoleId> = match role {
        Some(role) => vec![role],
        None => load_state(ctx)?.sync_pending.into_keys().collect(),
    };
    let report = sync_roles(ctx, &git, Some(&roles))?;
    if !report.conflict.is_empty() {
        return Err(CliError::Human {
            decision: None,
            message: msg::human("M-11", "conflict", &[&report.left().join(", ")]),
        });
    }
    if !report.pending.is_empty() {
        return Err(refused("M-11", "pending", &[&report.left().join(", ")]));
    }
    let done: Vec<&str> = report.done.iter().map(RoleId::as_str).collect();
    ctx.print(
        &format!(
            "동기화한 역할: {}",
            if done.is_empty() {
                "없음".into()
            } else {
                done.join(", ")
            }
        ),
        json!({ "synced": done }),
    );
    Ok(EXIT_OK)
}
