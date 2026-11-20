//! `fullops task new|reopen|recover`: 인박스 생성, 같은 키 재오픈, 끊긴 worker 복구(설계 8.4절).

use super::dispatch::{CliFacts, is_clean, main_checkout, usage, value};
use crate::assets::HANDOVER_TEMPLATE;
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::flow;
use fullops_core::frontmatter::{FmValue, FrontMatter};
use fullops_core::layout;
use fullops_core::memo;
use fullops_core::messages;
use fullops_core::model::{
    Completion, Config, DEFAULT_BUDGET, Extra, HeadRef, Known, RoleId, RoleKind, RouteClass, Seat,
    Settled, SettledHow, State, TaskKey, TaskNewRequest, WorkOutcome, path_text, same_path,
};
use fullops_ext::{Git, GitCli, MergeMode, MergeOutcome};
use lexopt::prelude::*;
use serde_json::json;
use std::path::Path;
use std::time::Duration;

/// 에폭 초의 UTC 날짜(`YYYY-MM-DD`). 날짜 라이브러리를 쓰지 않는다(설계 10.1절).
pub(super) fn date_of(epoch: i64) -> String {
    let days = epoch.div_euclid(86_400) + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (day, month) = (
        doy - (153 * mp + 2) / 5 + 1,
        if mp < 10 { mp + 3 } else { mp - 9 },
    );
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// 사람이 읽는 날짜(`updated`, 배정 기록)에 쓰는 지역 시간대 오프셋(초). 기계가 읽는 `*_at`에는 쓰지 않는다.
/// 날짜 크레이트 없이 git이 아는 값(`git var GIT_COMMITTER_IDENT` 끝의 `+0900`)을 읽는다. 못 읽으면 0(UTC)이다.
pub(super) fn local_offset(root: &std::path::Path) -> i64 {
    // 신원 설정이 없는 레포에서도 `git var`가 실패하지 않게 이름·메일을 준다
    let env = [
        ("GIT_COMMITTER_NAME", "fullops"),
        ("GIT_COMMITTER_EMAIL", "fullops@localhost"),
    ];
    let argv = ["git", "var", "GIT_COMMITTER_IDENT"].map(String::from);
    let ident = fullops_ext::run(&argv, root, Duration::from_secs(10), &env, None).stdout_text();
    let tz = ident.split_whitespace().last().unwrap_or_default();
    let sign = match tz.get(..1) {
        Some("+") => 1,
        Some("-") => -1,
        _ => return 0,
    };
    let num = |range: std::ops::Range<usize>| tz.get(range).and_then(|t| t.parse::<i64>().ok());
    match (tz.len(), num(1..3), num(3..5)) {
        (5, Some(h), Some(m)) => sign * (h * 3_600 + m * 60),
        _ => 0,
    }
}

struct Options {
    role: RoleId,
    key: TaskKey,
    goal: Option<String>,
    base: Option<String>,
}

fn parse(mut args: lexopt::Parser) -> Result<Options, CliError> {
    let (mut role, mut key, mut goal, mut base) = (None, None, None, None);
    while let Some(arg) = args.next()? {
        match arg {
            Long("role") => role = Some(value(&mut args)?),
            Long("key") => key = Some(value(&mut args)?),
            Long("goal") => goal = Some(value(&mut args)?),
            Long("base") => base = Some(value(&mut args)?),
            other => return Err(other.unexpected().into()),
        }
    }
    let (Some(role), Some(key)) = (role, key) else {
        return Err(usage("`--role <R>`과 `--key <KEY>`가 필요하다."));
    };
    Ok(Options {
        role: RoleId::new(role).map_err(|e| usage(e.to_string()))?,
        key: TaskKey::new(key).map_err(|e| usage(e.to_string()))?,
        goal,
        base,
    })
}

/// 그 과제의 완료 기록 가운데 가장 큰 차수.
fn last_round(ctx: &Ctx, key: &TaskKey, role: &RoleId) -> Option<u32> {
    let logs = ctx.repo().ok()?.root.join(layout::task_dir(key));
    let prefix = format!("{role}.r");
    std::fs::read_dir(logs)
        .ok()?
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| {
            name.strip_prefix(&prefix)?
                .strip_suffix(".md")?
                .parse()
                .ok()
        })
        .max()
}

/// 그 과제의 통합되지 않은 성공 보고들(보류 중인 것도 센다).
fn unlanded<'a>(state: &'a State, key: &TaskKey) -> impl Iterator<Item = &'a Completion> {
    state.completions.iter().filter(move |c| {
        c.key.as_ref() == Some(key)
            && !c.direct
            && c.outcome == WorkOutcome::Succeeded
            && c.landed_at.is_none()
    })
}

/// 통합되지 않은 설계 역할의 보고(설계 역할이 쓴 지시서) 가운데 가장 늦은 것(RT-02, reopen의 H-03).
/// 그보다 먼저 이전 차수 기록을 가져오면 그 보고의 병합이 같은 인박스를 다르게 바꿔 기록과 지시서를 섞는다.
pub(super) fn pending_brief<'a>(
    config: &Config,
    state: &'a State,
    key: &TaskKey,
) -> Option<&'a Completion> {
    let design = |c: &&Completion| {
        c.role
            .as_ref()
            .and_then(|r| config.roles.get(r))
            .is_some_and(|r| r.kind == RoleKind::Design)
    };
    unlanded(state, key)
        .filter(design)
        .max_by_key(|c| c.received_at)
}

/// 병합 전 재오픈의 조건(설계 8.4절): lead 체크아웃에 그 키의 인박스가 남아 있고, 역할 브랜치에 그 차수의 완료 기록이 있으며,
/// 그 키의 성공 보고가 아직 통합되지 않았다(리뷰 reject, 병합 뒤 lint 실패). 맞으면 (역할 브랜치 ref, 완료 기록 경로)다.
fn unmerged_record(
    ctx: &Ctx,
    git: &GitCli,
    state: &State,
    role: &RoleId,
    key: &TaskKey,
) -> Option<(String, String)> {
    let (repo, config) = (ctx.repo().ok()?, ctx.config().ok()?);
    let inbox = fullops_store::read_inbox(repo, role).yes().flatten()?;
    let branch = format!("refs/heads/{}", config.roles.get(role)?.branch);
    let log = layout::handover_log(key, role, inbox.round);
    let waiting = unlanded(state, key).next().is_some();
    (inbox.key == *key && waiting && git.rev_parse(&format!("{branch}:{log}")).yes().is_some())
        .then_some((branch, log))
}

/// lead 체크아웃의 인박스를 역할 브랜치의 완료 기록으로 바꿔 커밋한다(worker의 `done`이 한 이동과 같은 결과).
/// 내용까지 같아야 뒤의 동기화와 병합이 양쪽의 같은 이동으로 보고 기록을 섞지 않는다.
fn adopt_record(
    ctx: &Ctx,
    git: &GitCli,
    role: &RoleId,
    key: &TaskKey,
    branch: &str,
    log: &str,
) -> Result<(), CliError> {
    let repo = ctx.repo()?;
    let inbox = layout::inbox(role);
    // 커밋하지 않은 이전 인박스의 글(lead가 미리 적은 수정 지시)을 말없이 버리지 않는다
    if git.status(Some(&inbox)).yes().is_none_or(|e| !e.is_empty()) {
        return Err(super::dispatch::step_refusal(
            "reopen",
            format!("lead 체크아웃의 `{inbox}`에 커밋하지 않은 변경이 있다."),
            &format!(
                "적어 둔 수정 지시를 다른 곳에 옮기고 `git checkout -- {inbox}`로 되돌린 뒤 다시 실행한다. 수정 지시는 새 인박스에 적는다"
            ),
        ));
    }
    // 역할 브랜치가 그 차수를 받은 뒤 lead 쪽에서 바뀐 인박스는 완료 기록에 없다. 기록으로 바꾸면 그 변경을 말없이 잃는다
    let fork = git.merge_base("HEAD", branch).yes();
    let blob = |rev: &str| git.rev_parse(&format!("{rev}:{inbox}")).yes();
    let kept = fork
        .as_ref()
        .is_some_and(|fork| blob(fork.as_str()).is_some_and(|b| Some(b) == blob("HEAD")));
    if !kept {
        let fork = fork.as_ref().map_or("<merge-base>", |f| f.as_str());
        return Err(super::dispatch::step_refusal(
            "reopen",
            format!(
                "lead 체크아웃의 `{inbox}`가 `{role}` 역할이 그 차수를 받은 뒤 바뀌어 역할 브랜치의 완료 기록과 다르다. 기록을 덮어쓰지 않는다."
            ),
            &format!(
                "바뀐 글을 새 차수의 수정 목표에 옮겨 적고 `git checkout {fork} -- {inbox}`로 그 차수의 지시서로 되돌려 커밋한 뒤 다시 실행한다"
            ),
        ));
    }
    for args in [
        vec!["git", "checkout", "--quiet", branch, "--", log],
        vec!["git", "rm", "--quiet", "--force", "--", &inbox],
    ] {
        let argv: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let out = fullops_ext::run(&argv, &repo.root, Duration::from_secs(30), &[], None);
        if !out.ok() {
            return Err(CliError::External(format!(
                "이전 차수의 완료 기록을 가져오지 못했다: {}",
                out.stderr_text().trim()
            )));
        }
    }
    git.commit_only(
        &[inbox, log.to_string()],
        &format!("fullops: 재오픈 준비 {key} ({role})"),
    )?;
    Ok(())
}

/// `## 먼저 읽을 문서`의 안내 문단 뒤에 그 역할의 교훈 메모와 `project.md`의 `## 자주 틀리는 것`을 적는다(설계 8.4절).
/// 파일이 없거나 내용이 없으면 적지 않는다.
fn with_memo_paths(body: &str, root: &Path, role: &RoleId) -> String {
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap_or_default();
    let lines = memo::inbox_lines(
        role,
        &read(&layout::role_note(role)),
        &read(&layout::guide("project.md")),
    );
    let heading = "## 먼저 읽을 문서\n\n";
    let Some(at) = body.find(heading) else {
        return body.to_string();
    };
    if lines.is_empty() {
        return body.to_string();
    }
    // 안내 문단(제목 뒤 첫 빈 줄까지) 뒤
    let after = at + heading.len();
    let end = body[after..]
        .find("\n\n")
        .map_or(body.len(), |n| after + n + 2);
    format!("{}{}\n\n{}", &body[..end], lines.join("\n"), &body[end..])
}

fn write_inbox(ctx: &Ctx, options: Options, reopen: bool) -> Result<u8, CliError> {
    let (repo, config) = (ctx.repo()?, ctx.config()?);
    let Options {
        role,
        key,
        goal,
        base,
    } = options;
    let goal = goal.ok_or_else(|| usage("`--goal \"<한 줄>\"`이 필요하다."))?;
    let caller = ctx.caller()?;
    let request = TaskNewRequest {
        role: role.clone(),
        key: key.clone(),
        goal: goal.clone(),
        reopen,
    };
    let git = GitCli::cli(repo.root.clone());
    let mut refused = flow::check_task_new(&request, &mut CliFacts::new(ctx, &caller)?);
    // 병합 전 재오픈: 남은 인박스는 끝난 차수의 것이라 H-03에 걸리지 않는다
    let state = fullops_store::read_state(repo).yes().unwrap_or_default();
    let unmerged = reopen
        .then(|| unmerged_record(ctx, &git, &state, &role, &key))
        .flatten();
    if unmerged.is_some() {
        refused.retain(|r| r.rule != "H-03");
        // 다른 역할(설계 역할)의 지시서 보고가 병합 전이면 그것이 먼저다
        let own = |c: &&Completion| match &caller.actor.seat {
            Seat::Role { id, .. } => c.role.as_ref() == Some(id),
            _ => false,
        };
        if let Some(brief) = pending_brief(config, &state, &key).filter(|c| !own(c)) {
            refused.push(flow::refusal(
                "H-03",
                &["brief", role.as_str(), brief.id.as_str()],
                &format!(
                    "먼저 그 보고를 병합(`fullops merge {key} --completion {}`)한 뒤 다시 실행한다. 그 보고가 `{role}`의 다음 차수 지시서를 썼으면 그 차수를 `fullops dispatch {key} --role {role} --purpose fix`로 배정한다",
                    brief.id
                ),
                2,
            ));
        }
    }
    if !refused.is_empty() {
        return Err(CliError::Refused(refused));
    }
    if let Some((branch, log)) = &unmerged {
        adopt_record(ctx, &git, &role, &key, branch, log)?;
        ctx.note(&format!(
            "이전 차수의 지시서를 완료 기록으로 옮겨 커밋했다: {log}"
        ));
    }
    let base_rev = base.unwrap_or_else(|| format!("refs/heads/{}", config.base));
    let Known::Yes(base) = git.rev_parse(&format!("{base_rev}^{{commit}}")) else {
        return Err(usage(format!("기준 `{base_rev}`를 커밋으로 풀지 못했다.")));
    };
    let round = if reopen {
        last_round(ctx, &key, &role).map_or(0, |n| n + 1)
    } else {
        0
    };
    let (rounds, budget) = if reopen {
        ctx.update_state(None, |s| {
            let counters = s.tasks.entry(key.clone()).or_default();
            counters.rounds += 1;
            (counters.rounds, counters.budget)
        })?
    } else {
        (0, DEFAULT_BUDGET)
    };
    let class = fullops_store::read_route(repo, &key)
        .yes()
        .flatten()
        .map(|r| r.class)
        .filter(|c| matches!(c, RouteClass::Design | RouteClass::Product))
        .unwrap_or(RouteClass::Direct);
    let from = match &caller.actor.seat {
        Seat::Role { id, .. } => id.to_string(),
        _ => "lead".to_string(),
    };
    let title = format!("{key} — {goal}");
    let text = |v: &str| FmValue::Text(v.to_string());
    let fields = [
        ("id", text(&format!("handover-{role}"))),
        ("title", text(&title)),
        ("status", text("draft")),
        (
            "updated",
            text(&date_of(fullops_store::now() + local_offset(&repo.root))),
        ),
        ("owner", text(role.as_str())),
        ("summary", text(&goal)),
        ("key", text(key.as_str())),
        ("role", text(role.as_str())),
        ("goal", text(&goal)),
        ("base", text(base.as_str())),
        ("route", text(class.as_str())),
        ("from", text(&from)),
        ("round", text(&round.to_string())),
        ("budget", text(&budget.to_string())),
        ("test_level", text(config.test_level.as_str())),
        ("parallel", text(config.parallel.as_str())),
    ];
    let front = FrontMatter {
        fields: fields
            .into_iter()
            .map(|(name, v)| (name.to_string(), v))
            .collect(),
    };
    // 템플릿의 첫 줄(제목 자리)만 이 과제의 제목으로 바꾼다
    let body = HANDOVER_TEMPLATE
        .split_once('\n')
        .map_or("", |(_, rest)| rest);
    let body = with_memo_paths(body, &repo.root, &role);
    let path = repo.inbox_path(&role);
    fullops_store::write_atomic(
        &path,
        format!("{}# {title}\n{body}", front.render()).as_bytes(),
    )?;
    if rounds > budget {
        ctx.note(&messages::flow::over_budget(budget));
    }
    ctx.print(
        &format!(
            "인박스를 만들었다: {}\n본문을 채운 뒤 `fullops dispatch {key} --role {role}{}`",
            path_text(&path),
            if reopen { " --purpose fix" } else { "" }
        ),
        json!({"path": path_text(&path), "key": key, "role": role, "round": round, "base": base}),
    );
    Ok(EXIT_OK)
}

/// 끊긴 worker의 워크트리를 다시 배정할 수 있는 상태로 만든다. 역할 워크트리에서 실행한다.
fn recover(ctx: &Ctx, options: Options) -> Result<u8, CliError> {
    let (repo, config) = (ctx.repo()?, ctx.config()?);
    let Options { role, key, .. } = options;
    let branch = config.roles.get(&role).map(|r| r.branch.as_str());
    if !matches!(fullops_store::head(repo)?, HeadRef::Branch(b) if Some(b.as_str()) == branch) {
        return Err(usage(format!(
            "`task recover`는 `{role}` 역할의 워크트리에서 실행한다(`--repo <그 워크트리>`)."
        )));
    }
    let git = GitCli::cli(repo.root.clone());
    // 미커밋 변경은 WIP 커밋으로 보존한다
    let wip = if is_clean(&repo.root) {
        None
    } else {
        git.commit_only(&[".".to_string()], &format!("wip({role}): {key}"))?
    };
    let base_ref = format!("refs/heads/{}", config.base);
    if git.merge(&repo.root, &base_ref, MergeMode::NoEdit)? == MergeOutcome::Conflict {
        git.merge_abort(&repo.root)?;
        return Err(CliError::Human {
            decision: None,
            message: format!(
                "`{base_ref}`를 병합하다 충돌했다. 병합은 되돌렸고 WIP 커밋({})은 남겼다. 충돌을 손으로 푼다.",
                wip.as_ref().map_or("없음", |s| s.short(7))
            ),
        });
    }
    for linked in super::env::link_env(&main_checkout(repo), &repo.root) {
        ctx.note(&format!("환경 파일을 연결했다: {linked}"));
    }
    // 끊긴 worker의 배정을 정산한다. 남겨 두면 같은 배정을 다시 내는 것이 D-10에 걸린다
    let here = path_text(&repo.root);
    ctx.update_state(None, |s| {
        let stale = |d: &&mut fullops_core::model::Dispatch| {
            d.counts()
                && d.is_work()
                && d.settled.is_none()
                && d.key == key
                && d.role == role
                && same_path(&d.worktree, &here)
        };
        for d in s.dispatches.iter_mut().filter(stale) {
            d.settled = Some(Settled {
                how: SettledHow::Lost,
                outcome: None,
                message: None,
                at: fullops_store::now(),
                extra: Extra::new(),
            });
        }
    })?;
    let head = git.rev_parse("HEAD").yes();
    ctx.print(
        &format!(
            "복구했다: {key} ({role})\nWIP 커밋: {}\n병합한 ref: {base_ref}\nHEAD: {}",
            wip.as_ref().map_or("없음", |s| s.as_str()),
            head.as_ref().map_or("?", |s| s.as_str())
        ),
        json!({"wip": wip, "merged": base_ref, "head": head}),
    );
    Ok(EXIT_OK)
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let sub = match args.next()? {
        Some(Value(v)) => v.string()?,
        _ => {
            return Err(usage(
                "사용법: fullops task new|reopen|recover --role <R> --key <KEY> …",
            ));
        }
    };
    let options = parse(args)?;
    match sub.as_str() {
        "new" => write_inbox(ctx, options, false),
        "reopen" => write_inbox(ctx, options, true),
        "recover" => recover(ctx, options),
        other => Err(usage(format!("모르는 하위 명령이다: task {other}"))),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn dates_from_epoch() {
        assert_eq!(super::date_of(0), "1970-01-01");
        assert_eq!(super::date_of(1_794_500_000), "2026-11-12");
        assert_eq!(super::date_of(951_782_400), "2000-02-29");
    }

    #[test]
    fn inbox_lists_memo_paths_only_when_they_have_content() {
        use fullops_core::model::RoleId;
        let root = std::env::temp_dir().join(format!("fullops-memo-{}", std::process::id()));
        let roles = root.join(".fullops/guide/roles");
        std::fs::create_dir_all(&roles).unwrap();
        let body = crate::assets::HANDOVER_TEMPLATE;
        let dev = RoleId::new("dev").unwrap();
        let stock = crate::assets::REPO_FILES
            .iter()
            .find(|(p, _)| *p == "guide/project.md")
            .unwrap()
            .1;
        // 빈 메모와 템플릿 그대로의 project.md는 적지 않는다
        std::fs::write(roles.join("dev.md"), "---\ntitle: dev\n---\n# dev\n").unwrap();
        std::fs::write(root.join(".fullops/guide/project.md"), stock).unwrap();
        assert_eq!(super::with_memo_paths(body, &root, &dev), body);

        std::fs::write(roles.join("dev.md"), "# dev\n- 2026-11-20 교훈\n").unwrap();
        let filled = stock.replace(
            "## 자주 틀리는 것\n\n",
            "## 자주 틀리는 것\n\n- 실수 → 방법\n\n",
        );
        std::fs::write(root.join(".fullops/guide/project.md"), filled).unwrap();
        let out = super::with_memo_paths(body, &root, &dev);
        let section = out.split("## 먼저 읽을 문서").nth(1).unwrap();
        let section = section.split("\n## ").next().unwrap();
        assert!(section.contains("- `.fullops/guide/roles/dev.md`"));
        assert!(section.contains("- `.fullops/guide/project.md`의 `## 자주 틀리는 것`"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
