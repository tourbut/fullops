//! `fullops done`: 완료 기록 보존, 검사, push, 보낼 본문 출력(설계 8.7절). 보내지는 않는다: 전송은 preamble의 `worker_done`이다.

use super::dispatch::{packet_findings, usage, value};
use crate::assets::{HANDOVER_TEMPLATE, LINT_JSON};
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::flow::refusal;
use fullops_core::messages;
use fullops_core::model::{
    Completion, Config, Dispatch, Extra, Followup, HeadRef, Known, LintPass, LintResult, MessageId,
    PacketCheck, Reported, RoleId, RoleKind, Seat, Sha, ShaSource, State, TaskKey, TestLevel,
    WorkOutcome, path_text, same_path,
};
use fullops_ext::{Git, GitCli};
use fullops_store::Repo;
use lexopt::prelude::*;
use serde_json::json;
use std::time::Duration;

/// 기록 경로(설계 6.6절 5번). `changed()`와 lint 통과 인정이 항상 뺀다.
pub(super) fn is_record_path(path: &str) -> bool {
    [
        ".fullops-squad/handovers/",
        ".fullops-squad/docs/evaluations/",
        ".fullops-squad/routes/",
        ".fullops-squad/reviews/",
    ]
    .iter()
    .any(|dir| path.starts_with(dir))
        || path == ".fullops-squad/PLANS.md"
}

/// 세션이 만든 변경(설계 6.6절 `changed()`): 시작점과 대상 파일. 시작점을 정하지 못하면 `None`이다.
// shortcut: X-01(묶음 B)과 같은 계산을 여기서 따로 한다(뼈대에 함께 쓸 함수의 시그니처가 없다), B가 순수 함수를 내놓으면 그것으로 바꾼다
fn changed(
    config: &Config,
    git: &GitCli,
    inbox_base: Option<Sha>,
    anchor: Option<Sha>,
    direct: bool,
) -> Option<(Sha, Vec<String>)> {
    let floor = git.merge_base("HEAD", &config.base_ref()).yes();
    let mut start = if direct {
        anchor
    } else {
        inbox_base.or(anchor)
    };
    if let (Some(s), Some(f)) = (&start, &floor)
        && git.is_ancestor(s.as_str(), f.as_str()) == Known::Yes(true)
    {
        start = floor.clone();
    }
    if start
        .as_ref()
        .is_none_or(|s| git.is_ancestor(s.as_str(), "HEAD") != Known::Yes(true))
    {
        start = floor.clone();
    }
    let start = start?;
    let mut paths = git.diff_names(start.as_str(), "HEAD").yes()?;
    paths.extend(git.status(None).yes()?.into_iter().map(|e| e.path));
    paths.sort();
    paths.dedup();
    // 검사 기준은 merge-base 커밋의 설정이다(설계 6.1.1절)
    let at = floor.as_ref().unwrap_or(&start).to_string();
    let blobs = git
        .show_blobs(&[
            (at.clone(), ".fullops-squad/fullops.toml".to_string()),
            (at, ".fullops-squad/lint/lint.json".to_string()),
        ])
        .yes()
        .unwrap_or_default();
    let text = |i: usize| {
        blobs
            .get(i)
            .cloned()
            .flatten()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    };
    let baseline = fullops_store::baseline(text(0).as_deref());
    let exclude = fullops_core::lint_excludes(text(1).as_deref(), LINT_JSON);
    paths.retain(|path| {
        !is_record_path(path)
            && if direct {
                !exclude.is_match(path)
            } else {
                fullops_core::is_code(path, &baseline.code, &exclude)
            }
    });
    Some((start, paths))
}

/// lint 통과 인정(설계 6.6절 `pass_covers`). `start`가 없으면 head 쪽 조건만 본다.
fn pass_covers(repo: &Repo, git: &GitCli, head: &Sha, start: Option<&Sha>) -> Option<LintPass> {
    let pass = fullops_store::read_lint_pass(repo).yes().flatten()?;
    let covers_head = &pass.head == head
        || (git.is_ancestor(pass.head.as_str(), head.as_str()) == Known::Yes(true)
            && git
                .diff_names(pass.head.as_str(), head.as_str())
                .yes()
                .is_some_and(|paths| paths.iter().all(|p| is_record_path(p))));
    let covers_base = start.is_none_or(|start| {
        &pass.base == start
            || git.is_ancestor(pass.base.as_str(), start.as_str()) == Known::Yes(true)
    });
    (covers_head && covers_base).then_some(pass)
}

/// `## 완료 보고` 절의 줄들.
fn report_lines(text: &str) -> Vec<&str> {
    text.lines()
        .skip_while(|line| line.trim_end() != "## 완료 보고")
        .skip(1)
        .take_while(|line| !line.starts_with("## "))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

/// `HEAD` 커밋의 제목이 `fullops: 완료 기록 <KEY> (<R>)`이면 그 키와 역할.
fn logged(repo: &Repo) -> Option<(TaskKey, RoleId)> {
    let argv = ["git", "log", "-1", "--format=%s"].map(String::from);
    let out = fullops_ext::run(&argv, &repo.root, Duration::from_secs(30), &[], None);
    let subject = out.stdout_text();
    let rest = subject.trim().strip_prefix("fullops: 완료 기록 ")?;
    let (key, role) = rest.split_once(" (")?;
    Some((
        TaskKey::new(key).ok()?,
        RoleId::new(role.strip_suffix(')')?).ok()?,
    ))
}

/// R-08의 둘째 길: 이 워크트리에서 그 과제를 하는, 정산되지 않은 Build·Fix dispatch가 정확히 하나일 때 그것.
fn dispatch_here<'a>(state: &'a State, repo: &Repo, key: &TaskKey) -> Option<&'a Dispatch> {
    let here = path_text(&repo.root);
    let mut found = state.dispatches.iter().filter(|d| {
        d.counts()
            && d.is_work()
            && d.settled.is_none()
            && &d.key == key
            && same_path(&d.worktree, &here)
    });
    match (found.next(), found.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let (mut failed, mut no_checks, mut followups) = (None, None, Vec::new());
    while let Some(arg) = args.next()? {
        match arg {
            Long("failed") => failed = Some(value(&mut args)?),
            Long("no-checks") => no_checks = Some(value(&mut args)?),
            Long("followup") => {
                let text = value(&mut args)?;
                let parsed = text
                    .split_once(':')
                    .and_then(|(role, text)| Some((RoleId::new(role.trim()).ok()?, text.trim())))
                    .filter(|(_, text)| !text.is_empty());
                let Some((role, text)) = parsed else {
                    return Err(usage("`--followup \"<역할>: <글>\"` 꼴로 준다."));
                };
                followups.push(Followup {
                    role,
                    text: text.to_string(),
                    extra: Extra::new(),
                });
            }
            other => return Err(other.unexpected().into()),
        }
    }
    let (repo, config) = (ctx.repo()?, ctx.config()?);
    let caller = ctx.caller()?;
    let actor = &caller.actor;
    let git = GitCli::cli(repo.root.clone());
    let state = fullops_store::read_state(repo).yes().unwrap_or_default();

    // R-01
    if actor.sub {
        return Err(CliError::Refused(vec![refusal(
            "R-01",
            &["sub"],
            "`fullops done` 없이 preamble의 `worker_done`을 보낸다",
            2,
        )]));
    }
    let role = match &actor.seat {
        Seat::Role { id, .. } => Some(id.clone()),
        Seat::Lead if actor.direct_dev => config.primary_role.clone(),
        _ => None,
    };
    let Some(role) = role else {
        return Err(CliError::Refused(vec![refusal(
            "R-01",
            &["seat"],
            "배정받은 역할 워크트리에서 실행한다",
            2,
        )]));
    };
    let direct = actor.direct_dev;
    let design = config
        .roles
        .get(&role)
        .is_some_and(|r| r.kind == RoleKind::Design);

    // 이 과제의 키: 묶인 dispatch → 자기 인박스 → 이미 남긴 완료 기록 커밋
    let inbox = fullops_store::read_inbox(repo, &role).yes().flatten();
    let record = logged(repo).filter(|(_, r)| r == &role).map(|(key, _)| key);
    let bound = actor.dispatch.as_ref().and_then(|id| state.dispatch(id));
    let key = bound
        .map(|d| d.key.clone())
        .or(inbox.as_ref().map(|i| i.key.clone()))
        .or(record.clone());
    let Some(key) = key else {
        ctx.print("보낼 완료 보고가 없다.", json!({ "body": null }));
        return Ok(EXIT_OK);
    };
    // 대상 dispatch는 한 번 정하고 R-11과 R-08이 같은 값을 쓴다
    let target = bound.or_else(|| dispatch_here(&state, repo, &key)).cloned();
    // 인박스가 이미 옮겨졌으면 R-06까지는 끝난 것이다
    let already = inbox.is_none() && record.as_ref() == Some(&key);
    let head = |git: &GitCli| {
        git.rev_parse("HEAD")
            .yes()
            .ok_or_else(|| CliError::External("HEAD를 읽지 못했다.".into()))
    };

    let mut refused = Vec::new();
    let mut evidence = None;
    if failed.is_none() {
        // R-02
        let dirty = git.status(None).yes().map_or(1, |entries| entries.len());
        if dirty > 0 {
            refused.push(refusal("R-02", &[&dirty.to_string()], "커밋한다", 2));
        }
        let now_head = head(&git)?;
        if already {
            evidence = pass_covers(repo, &git, &now_head, None);
        } else {
            // R-03
            let anchor = caller.session.as_ref().and_then(|s| s.anchor.clone());
            let base = inbox.as_ref().map(|i| i.base.clone());
            match changed(config, &git, base, anchor, direct) {
                Some((start, code)) => {
                    evidence = pass_covers(repo, &git, &now_head, Some(&start));
                    if !code.is_empty() && evidence.is_none() && no_checks.is_none() {
                        refused.push(refusal(
                            "R-03",
                            &[&code.len().to_string()],
                            &format!("`fullops lint --from {start}`. 적용할 검사가 없으면 `--no-checks \"<사유>\"`"),
                            2,
                        ));
                    }
                }
                None => ctx.note("변경 범위를 계산하지 못해 lint 통과 기록을 확인하지 않았다."),
            }
            if let Some(text) = inbox.as_ref().and_then(|_| {
                fullops_store::read_text(&repo.inbox_path(&role))
                    .yes()
                    .flatten()
            }) {
                // R-04
                let template = report_lines(HANDOVER_TEMPLATE);
                if !report_lines(&text)
                    .iter()
                    .any(|line| !template.contains(line))
                {
                    refused.push(refusal(
                        "R-04",
                        &[],
                        "인박스의 `## 완료 보고`에 브랜치·SHA·검증 결과를 적고 커밋한다",
                        2,
                    ));
                }
                // R-10
                if let Some(found) = packet_findings(repo, PacketCheck::Completion, &key, &role) {
                    refused.push(refusal(
                        "R-10",
                        &[&found],
                        &format!("`{key}-packet-outcomes.json`에 전달 항목마다 처리 결과와 이유를 적고 커밋"),
                        2,
                    ));
                }
            }
            // R-11
            if let Some(d) = target.as_ref().filter(|d| state.sub_open(&d.id)) {
                let run = state
                    .runs
                    .iter()
                    .find(|r| r.closed_at.is_none() && matches!(&r.owner, fullops_core::model::RunOwner::Dispatch(id) if id == &d.id))
                    .map_or("<R>".to_string(), |r| r.id.to_string());
                refused.push(refusal(
                    "R-11",
                    &[],
                    &format!("`fullops wait --run {run}`로 하위 worker의 보고를 받고, 결과를 확인해 커밋한 뒤 `fullops wait --run {run} --ack <ID> --no-wait`"),
                    2,
                ));
            }
        }
        if !refused.is_empty() {
            return Err(CliError::Refused(refused));
        }
        // R-05: 알림
        let report = repo
            .squad()
            .join("docs/evaluations/qa-reports")
            .join(format!("{key}-test"))
            .join("report.md");
        if let Some(text) = fullops_store::read_text(&report).yes().flatten() {
            if matches!(config.test_level, TestLevel::Minimal | TestLevel::Lite)
                && text.lines().count() > 40
            {
                ctx.note(&format!(
                    "테스트 레벨 `{}`인데 QA 보고서가 40줄을 넘는다.",
                    config.test_level
                ));
            }
            if text.contains("raw/") {
                ctx.note("QA 보고서가 `raw/`를 링크한다. `raw/`는 커밋되지 않는다.");
            }
        }
        // R-06
        if let Some(inbox) = &inbox {
            let from = format!(".fullops-squad/handovers/to_{role}.md");
            let to = format!(
                ".fullops-squad/handovers/logs/{key}.{role}.r{}.md",
                inbox.round
            );
            let mut message = format!("fullops: 완료 기록 {key} ({role})");
            if let Some(why) = &no_checks {
                message.push_str(&format!("\n\nlint 생략: {why}"));
            }
            let moved = git
                .mv(&from, &to)
                .and_then(|()| git.commit_only(&[from, to], &message));
            if let Err(e) = moved {
                return Err(CliError::Internal(format!(
                    "완료 기록 커밋을 만들지 못했다: {e}"
                )));
            }
        }
    }
    let sha = head(&git)?;
    let branch = match fullops_store::head(repo)? {
        HeadRef::Branch(branch) if direct => branch,
        _ => config
            .roles
            .get(&role)
            .map(|r| r.branch.clone())
            .unwrap_or_default(),
    };
    // R-07
    if let (Some(remote), None) = (&config.remote, &failed) {
        git.push(remote, &format!("HEAD:refs/heads/{branch}"))?;
    }
    // R-08
    let reported = Reported {
        branch: branch.clone(),
        sha: sha.clone(),
        lint_head: evidence.as_ref().map(|pass| pass.head.clone()),
        followups: followups.clone(),
        at: fullops_store::now(),
        extra: Extra::new(),
    };
    let wrote = ctx.update_state(None, |s| {
        if direct && failed.is_none() {
            let id = MessageId::new(format!("direct:{}", sha.short(12)));
            if let Ok(id) = id
                && !s.completions.iter().any(|c| c.id == id)
            {
                s.completions.push(Completion {
                    id,
                    dispatch: None,
                    key: Some(key.clone()),
                    role: Some(role.clone()),
                    branch: Some(branch.clone()),
                    sha: Some(sha.clone()),
                    sha_source: Some(ShaSource::Done),
                    direct: true,
                    outcome: WorkOutcome::Succeeded,
                    received_at: fullops_store::now(),
                    hold: None,
                    released_hold: None,
                    landed_at: None,
                    drift: None,
                    extra: Extra::new(),
                });
            }
        }
        let found = target.as_ref().and_then(|d| s.dispatch_mut(&d.id));
        found.map(|d| d.reported = Some(reported)).is_some()
    })?;
    if !wrote && !direct {
        ctx.note(messages::flow::DONE_NO_DISPATCH);
    }

    // R-09
    let follow = if followups.is_empty() {
        "없음".to_string()
    } else {
        followups
            .iter()
            .map(|f| format!("{}: {}", f.role, f.text))
            .collect::<Vec<_>>()
            .join("; ")
    };
    let body = match (&failed, &evidence) {
        (Some(why), _) => format!("[실패] {key} | 사유: {why}"),
        (None, pass) => {
            let check = match (pass, &no_checks) {
                (Some(pass), _) => {
                    let result: Option<LintResult> =
                        fullops_store::read_json(&repo.lint_dir().join("result.json"))
                            .yes()
                            .flatten();
                    let warnings = result
                        .filter(|r| r.head == pass.head)
                        .map_or(0, |r| r.warnings);
                    format!("lint {} ERROR 0 WARNING {warnings}", pass.head.short(7))
                }
                (None, Some(why)) => format!("lint 생략({why})"),
                (None, None) => "lint 기록 없음".to_string(),
            };
            let mut body = format!(
                "[{}] {key} | 브랜치 {branch} | SHA {sha} | 검증: {check} | 후속: {follow}",
                if design { "설계" } else { "완료" }
            );
            if design {
                body.push_str(&format!(
                    " | 지시서: {}",
                    written_inboxes(repo, config, &role)
                ));
            }
            body
        }
    };
    let human = if direct {
        format!("완료를 기록했다: {key} ({sha})")
    } else {
        body.clone()
    };
    if failed.is_some() {
        ctx.note("preamble의 `worker_done` 명령에 `--outcome failed`를 쓴다.");
    }
    ctx.print(
        &human,
        json!({"key": key, "role": role, "branch": branch, "sha": sha, "reported": wrote,
               "body": if direct { None } else { Some(body) }}),
    );
    Ok(EXIT_OK)
}

/// 설계 역할이 이번 과제에서 만든 인박스 경로들(이 체크아웃의 인박스 가운데 `from`이 자기 역할인 것).
fn written_inboxes(repo: &Repo, config: &Config, role: &RoleId) -> String {
    let paths: Vec<String> = config
        .roles
        .keys()
        .filter(|other| *other != role)
        .filter(|other| {
            fullops_store::read_inbox(repo, other)
                .yes()
                .flatten()
                .is_some_and(|inbox| inbox.from == role.as_str())
        })
        .map(|other| format!(".fullops-squad/handovers/to_{other}.md"))
        .collect();
    if paths.is_empty() {
        "없음".into()
    } else {
        paths.join(", ")
    }
}
