//! `fullops merge`: 리뷰 판정, 병합, lint, push, 기록, 동기화(설계 8.8절).
//! 어느 단계에서 끊겨도 다시 실행하면 이어 간다: 병합 기록 커밋이 있으면 M-04~M-08을 건너뛴다.
//! 기록 커밋 없이 기본 브랜치에 들어와 있는 SHA(날것 `git merge`)는 push 전이면 M-04·M-05를 다시 판정한다.

use super::decision::{self, Ask};
use super::hold::{
    ack, archive_done, fetch, is_record, known, landing, load_state, mark_landed, pick, refused,
    task_key, value,
};
use super::review::{self, Evidence};
use super::sync::{self, SyncReport};
use crate::assets;
use crate::cli::{CliError, Ctx, EXIT_OK, session_handle};
use crate::hook::changes::{self, GitGate};
use fullops_core::messages::merge as msg;
use fullops_core::model::{
    AckBy, Completion, Decision, DecisionId, DecisionStatus, Derived, Dispatch, DispatchId,
    Landing, Need, NeedAction, Sha, SnapshotState, State, TaskKey, ack_by, orca_wakes,
};
use fullops_ext::{Git, GitCli, MergeMode, MergeOutcome, Worktree};
use lexopt::prelude::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn git_in(cwd: &Path, args: &[&str], timeout: Duration) -> fullops_ext::ProcResult {
    let argv: Vec<String> = std::iter::once("git")
        .chain(args.iter().copied())
        .map(String::from)
        .collect();
    fullops_ext::run(&argv, cwd, timeout, &[("GIT_TERMINAL_PROMPT", "0")], None)
}

/// M-04에서 리뷰가 필요할 때의 표(설계 8.8절). `role`은 그 보고를 쓴 역할, `design`은 그것이 설계 역할일 때의 route class다(수정 배정 안내).
fn review_gate(
    ctx: &Ctx,
    state: &State,
    key: &TaskKey,
    need: &Need,
    role: &str,
    design: &str,
) -> Result<(), CliError> {
    let repo = ctx.repo()?;
    let (k, short) = (key.as_str(), need.head.short(7));
    match review::evidence(repo, key, &need.base, &need.head) {
        Evidence::Accepted | Evidence::Waived => return Ok(()),
        Evidence::Rejected => return Err(refused("M-04", "rejected", &[k, short, role, design])),
        Evidence::Unfinished => return Err(refused("M-04", "unfinished", &[k, short])),
        Evidence::None => {}
    }
    if need.action == NeedAction::Review {
        return Err(refused("M-04", "auto", &[k, short, &need.reason]));
    }
    let now = fullops_store::now();
    // 그 SHA에 묶인 결정(버린 것은 뺀다)
    let bound =
        |d: &&Decision| d.sha.as_ref() == Some(&need.head) && d.status != DecisionStatus::Dropped;
    let ask = |id: &DecisionId| CliError::Human {
        decision: Some(id.clone()),
        message: msg::human(
            "M-04",
            "ask",
            &[k, short, &need.reason, id.as_str(), need.head.as_str()],
        ),
    };
    match state.decisions.iter().rev().find(bound) {
        Some(d) if d.status == DecisionStatus::Answered => {
            let skip = d.answer.as_deref().map(str::trim) == Some("생략");
            let case = if skip { "skip" } else { "proceed" };
            Err(refused("M-04", case, &[k, short, d.id.as_str()]))
        }
        Some(d) if !decision::expired(d, now) => Err(ask(&d.id)),
        stale => {
            let stale = stale.map(|d| d.id.clone());
            let origin = fullops_store::read_route(repo, key)
                .yes()
                .flatten()
                .map(|r| r.origin);
            let question = format!(
                "{k} {short}: 독립 리뷰를 진행할까요? 판정 사유: {} 예상 비용: 리뷰 worker 한 번(코드 파일 {}개, {}줄).",
                need.reason, need.code_files, need.code_lines
            );
            let id = ctx.update_state(None, |state| {
                // 만료된 결정은 답을 받을 수 없으므로 닫고 다시 묻는다
                let old = |d: &&mut Decision| Some(&d.id) == stale.as_ref();
                if let Some(old) = state.decisions.iter_mut().find(old) {
                    old.status = DecisionStatus::Dropped;
                    old.drop_reason = Some("만료".into());
                    old.closed_at = Some(now);
                }
                let ask = Ask {
                    key: Some(key.clone()),
                    question,
                    options: vec!["진행".into(), "생략".into()],
                    blocks: vec![k.to_string()],
                    sha: Some(need.head.clone()),
                    origin,
                    ..Ask::default()
                };
                decision::open(state, ask, now)
            })?;
            Err(ask(&id))
        }
    }
}

/// M-05: `reported.lint_head`, 없으면 worker 워크트리의 `lint/pass.json`이 SHA를 덮는가.
/// `mb`는 병합할 변경의 시작(merge-base)이다. `done`이 범위를 확인하지 않은 `pass.json`은 `mb`부터 검사한 것만 받는다.
fn lint_ok(
    git: &GitCli,
    state: &State,
    worktrees: &[Worktree],
    completion: &Completion,
    mb: &Sha,
    sha: &Sha,
) -> bool {
    let dispatch = completion
        .dispatch
        .as_ref()
        .and_then(|id| state.dispatch(id));
    let reported = dispatch
        .and_then(|d| d.reported.as_ref())
        .and_then(|r| r.lint_head.clone());
    if let Some(head) = reported {
        return review::covers(git, &head, sha);
    }
    let of_branch = |w: &&Worktree| w.branch.is_some() && w.branch == completion.branch;
    let path = dispatch
        .map(|d| PathBuf::from(&d.worktree))
        .or_else(|| worktrees.iter().find(of_branch).map(|w| w.path.clone()));
    let repo = path.and_then(|path| fullops_store::discover(&path).ok().flatten());
    repo.is_some_and(|repo| {
        let mut gate = GitGate::cli(git.repo_root.clone());
        let pass = changes::pass_covers(&mut gate, &repo, sha, Some(mb));
        pass.yes().flatten().is_some()
    })
}

/// 기본 워크트리에서 `fullops lint --from <병합 전 tip>`을 한 번 돌린다(M-07). 실패하면 출력의 끝 몇 줄을 돌려준다.
fn run_lint(base: &Path, from: &Sha) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let argv = [
        exe.to_string_lossy().into_owned(),
        "lint".into(),
        "--from".into(),
        from.to_string(),
    ];
    let out = fullops_ext::run(&argv, base, Duration::from_secs(3600), &[], None);
    if out.ok() {
        return Ok(());
    }
    let text = format!("{}{}", out.stdout_text(), out.stderr_text());
    let mut tail: Vec<&str> = text.lines().rev().take(5).collect();
    tail.reverse();
    Err(tail.join(" / "))
}

/// 이스케이프되지 않은 `|`의 자리.
fn pipes(line: &str) -> Vec<usize> {
    let mut previous = ' ';
    let mut out = Vec::new();
    for (at, c) in line.char_indices() {
        if c == '|' && previous != '\\' {
            out.push(at);
        }
        previous = c;
    }
    out
}

/// 과제 표의 그 줄(1부터 센다)에서 상태 칸만 바꾼다. 표를 찾지 못하면 `None`.
fn set_status(text: &str, line: u32, status: &str) -> Option<String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let row = (line as usize).checked_sub(1)?;
    let separator = |l: &str| {
        let l = l.trim();
        l.starts_with('|') && l.contains('-') && l.chars().all(|c| "|-: \t".contains(c))
    };
    let above = |i: &usize| lines.get(*i).is_some_and(|l| separator(l));
    let header = lines.get((1..row).rev().find(above)? - 1)?;
    let cuts = pipes(header);
    let cells: Vec<&str> = cuts.windows(2).map(|w| &header[w[0] + 1..w[1]]).collect();
    let key = cells
        .iter()
        .position(|c| c.contains("과제") || c.contains("키"))?;
    let at = (0..cells.len()).find(|i| *i != key && cells[*i].contains("상태"))?;
    let target = lines.get(row)?;
    let body = target.trim_end_matches(['\r', '\n']);
    let cuts = pipes(body);
    let start = cuts.get(at)? + 1;
    let end = cuts.get(at + 1).copied().unwrap_or(body.len());
    let changed = format!(
        "{} {status} {}{}",
        &body[..start],
        &body[end..],
        &target[body.len()..]
    );
    let mut out: Vec<&str> = lines.clone();
    out[row] = &changed;
    Some(out.concat())
}

/// 그 제목의 절 끝에 줄을 덧붙인다. 절이 없으면 파일 끝에 만든다. 이미 있는 줄은 다시 넣지 않는다.
fn append_section(text: &mut String, heading: &str, lines: &[String]) {
    let fresh: Vec<&String> = lines
        .iter()
        .filter(|l| !text.lines().any(|t| t.trim_end() == l.as_str()))
        .collect();
    if fresh.is_empty() {
        return;
    }
    let block: String = fresh.iter().map(|l| format!("{l}\n")).collect();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    // 절 안의 마지막 내용 줄 뒤
    let (mut offset, mut end, mut inside) = (0, None, false);
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if inside && trimmed.starts_with('#') {
            break;
        }
        inside |= trimmed == heading;
        offset += line.len();
        if inside && !trimmed.is_empty() {
            end = Some((offset, trimmed == heading));
        }
    }
    match end {
        Some((at, true)) => text.insert_str(at, &format!("\n{block}")),
        Some((at, false)) => text.insert_str(at, &block),
        None => text.push_str(&format!("\n{heading}\n\n{block}")),
    }
}

/// 에폭 초를 `YYYY-MM-DD HH:MM`으로 적는다. `at`은 지역 시간대 오프셋을 더한 값이다.
fn stamp(at: i64) -> String {
    let seconds = at.rem_euclid(86_400);
    format!(
        "{} {:02}:{:02}",
        super::task::date_of(at),
        seconds / 3_600,
        seconds % 3_600 / 60
    )
}

/// M-08: 병합 기록 커밋 하나. `PLANS.md`의 상태 칸·배정 기록·후속 요청과 그 과제의 미커밋 리뷰 파일을 넣는다.
fn record(
    ctx: &Ctx,
    base: &GitCli,
    state: &State,
    completion: &Completion,
    key: &TaskKey,
    sha: &Sha,
    body: &[String],
) -> Result<(), CliError> {
    let plan_path = base.repo_root.join(".fullops-squad").join("PLANS.md");
    let before = std::fs::read_to_string(&plan_path).unwrap_or_default();
    let mut text = before.clone();
    let plan = fullops_core::parse_plan(&text, assets::PLANS_MD);
    if let Some(row) = plan.rows.iter().find(|r| &r.key == key)
        && let Some(changed) = set_status(&text, row.line, &format!("완료({})", sha.short(7)))
    {
        text = changed;
    }
    let route = fullops_store::read_route(ctx.repo()?, key).yes().flatten();
    let ours: Vec<&Dispatch> = state
        .dispatches
        .iter()
        .filter(|d| &d.key == key && !d.is_reservation())
        .collect();
    let offset = super::task::local_offset(&base.repo_root);
    let assigned: Vec<String> = ours
        .iter()
        .map(|d| {
            let again = ours
                .iter()
                .filter(|o| o.role == d.role && o.started_at < d.started_at)
                .count();
            let why = route.as_ref().and_then(|r| r.tuples.get(d.role.as_str()));
            let why = match (d.reason, d.is_sub(), why) {
                (Some(reason), _, _) => reason.to_string(),
                (None, true, _) => "하위 배정".to_string(),
                (None, false, Some(t)) if !t.why.trim().is_empty() => t.why.clone(),
                _ => "-".to_string(),
            };
            format!(
                "- {} {key} → {} | {}/{}/{} | {} | run {} | 재배정 {again} | {why}",
                stamp(d.started_at + offset),
                d.role,
                d.tuple.agent,
                d.tuple.model,
                d.tuple.effort,
                d.session_mode,
                d.run
            )
        })
        .collect();
    append_section(&mut text, "## 배정 기록", &assigned);
    let reported = completion
        .dispatch
        .as_ref()
        .and_then(|id| state.dispatch(id))
        .and_then(|d| d.reported.as_ref());
    let followups: Vec<String> = reported
        .into_iter()
        .flat_map(|r| &r.followups)
        .map(|f| format!("- [ ] {}: {} ({key})", f.role, f.text))
        .collect();
    append_section(&mut text, "## 완료 보고 후속 요청", &followups);
    if text != before {
        std::fs::write(&plan_path, text)?;
    }
    // shortcut: lead가 기본 브랜치 밖(coordinator 브랜치)에 있으면 lead 체크아웃의 리뷰 파일은 이 커밋에 들어가지 않는다, 그 배치를 쓰는 레포가 생기면 옮겨 넣는다
    let paths = [
        ".fullops-squad/PLANS.md".to_string(),
        format!(".fullops-squad/reviews/{key}"),
    ];
    let message = format!("fullops: 병합 기록 {key}\n\n{}", body.join("\n"));
    base.commit_only(&paths, &message)
        .map_err(|e| CliError::Internal(format!("병합 기록 커밋을 만들지 못했습니다: {e}")))?;
    Ok(())
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let (mut key, mut id, mut accept_lint) = (None, None, None);
    let (mut no_push, mut resume, mut abort) = (false, false, false);
    while let Some(arg) = args.next()? {
        match arg {
            Long("completion") => id = Some(value(&mut args)?),
            Long("accept-missing-lint") => accept_lint = Some(value(&mut args)?),
            Long("no-push") => no_push = true,
            Long("continue") => resume = true,
            Long("abort") => abort = true,
            Value(v) if key.is_none() => key = Some(v.string()?),
            other => return Err(other.unexpected().into()),
        }
    }
    let key = task_key(key)?;
    if accept_lint.as_ref().is_some_and(|r| r.trim().is_empty()) || (resume && abort) {
        return Err(CliError::Usage(
            "--accept-missing-lint에는 사유가 필요하고, --continue와 --abort는 함께 쓰지 않습니다."
                .into(),
        ));
    }
    let (repo, config) = (ctx.repo()?, ctx.config()?);
    let k = key.as_str();

    // M-01
    let caller = ctx.caller()?;
    if !caller.actor.is_lead() {
        return Err(refused("M-01", "", &[&caller.actor.label()]));
    }
    // M-02
    let state = load_state(ctx)?;
    let completion = pick(&state, Some(&key), id.as_deref(), false)?;
    let Some(sha) = completion.sha.clone() else {
        return Err(refused("M-02", "nosha", &[completion.id.as_str()]));
    };
    let role = completion.role.as_ref().map_or("", |r| r.as_str());
    let design =
        review::design_class(repo, config, &key, completion.role.as_ref()).unwrap_or_default();
    // M-03
    let git = GitCli::cli(repo.root.clone());
    let worktrees = known(git.worktrees(), "워크트리 목록")?;
    let on_base = |w: &&Worktree| w.branch.as_deref() == Some(config.base.as_str());
    let Some(base_wt) = worktrees.iter().find(on_base).map(|w| w.path.clone()) else {
        return Err(refused("M-03", "nowt", &[&config.base]));
    };
    let base = GitCli::cli(base_wt.clone());
    let merge_head = base.rev_parse("MERGE_HEAD").yes();
    let merging = merge_head.is_some();
    let upstream = config.base_ref();
    let unmerged = || -> String {
        let entries = base.status(None).yes().unwrap_or_default();
        let conflicted = entries.into_iter().filter(|e| {
            matches!(
                (e.index, e.worktree),
                ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D')
            )
        });
        conflicted.map(|e| e.path).collect::<Vec<_>>().join(", ")
    };
    // 충돌을 푼 병합을 끝낸다(`--continue`)
    let finish = || {
        let argv = ["commit", "--no-edit", "--quiet"];
        let out = git_in(&base_wt, &argv, Duration::from_secs(60));
        if out.ok() {
            return Ok(());
        }
        let why = out.stderr_text();
        let why = why.lines().next().unwrap_or("해결되지 않은 파일이 있다");
        Err(CliError::Human {
            decision: None,
            message: msg::human("M-06", "", &[k, why]),
        })
    };
    // 진행 중인 병합이 이 과제의 SHA면 게이트(M-04·M-05)를 다시 본 뒤 M-06에서 끝낸다
    let ours = merge_head.as_ref() == Some(&sha);
    // 원격이 먼저 나아가 통합 기준 ref를 합쳤으면, 합치기 전의 로컬 tip
    let mut rejoined = None;
    if abort {
        if merging {
            base.merge_abort(&base_wt)?;
        }
        let human = if merging {
            "진행 중이던 병합을 취소했습니다."
        } else {
            "진행 중인 병합이 없습니다."
        };
        ctx.print(human, json!({ "key": key, "aborted": merging }));
        return Ok(EXIT_OK);
    }
    if merging {
        if !resume {
            return Err(refused("M-03", "merging", &[k]));
        }
        if !ours {
            let local = known(base.rev_parse("HEAD"), "기본 브랜치 tip")?;
            finish()?;
            if merge_head == git.rev_parse(&upstream).yes() {
                rejoined = Some(local);
            }
        }
    } else {
        // 기록 경로의 미커밋 변경(리뷰 파일, PLANS.md)은 병합 기록 커밋이 담으므로 더러운 것으로 세지 않는다
        let dirty: Vec<String> = known(base.status(None), "기본 워크트리 상태")?
            .into_iter()
            .map(|e| e.path)
            .filter(|p| !is_record(p))
            .take(5)
            .collect();
        if !dirty.is_empty() {
            let path = base_wt.to_string_lossy();
            return Err(refused("M-03", "dirty", &[&path, &dirty.join(", ")]));
        }
        fetch(ctx, &git, config)?;
        if config.remote.is_some()
            && git.rev_parse(&upstream).yes().is_some()
            && let Err(e) = base.merge(&base_wt, &upstream, MergeMode::FfOnly)
        {
            // 로컬에 push하지 못한 커밋이 있는데 원격이 먼저 나아갔다: 통합 기준 ref를 병합해 잇는다
            let local = known(base.rev_parse("HEAD"), "기본 브랜치 tip")?;
            match base.merge(&base_wt, &upstream, MergeMode::NoEdit) {
                Ok(MergeOutcome::Done) => rejoined = Some(local),
                Ok(MergeOutcome::Conflict) => {
                    return Err(CliError::Human {
                        decision: None,
                        message: msg::human("M-03", "diverged", &[k, &upstream, &unmerged()]),
                    });
                }
                Err(_) => {
                    return Err(CliError::External(format!(
                        "기본 브랜치를 {upstream}로 fast-forward하지 못했습니다: {e}"
                    )));
                }
            }
        }
    }

    let before = known(base.rev_parse("HEAD"), "기본 브랜치 tip")?;
    let merged = known(base.is_ancestor(sha.as_str(), "HEAD"), "조상 관계")?;
    let landed = known(landing(&git, config, &sha), "통합 위치")? == Landing::Landed;
    // 병합 기록 커밋이 있으면 `fullops merge`가 게이트를 거쳐 병합한 SHA다
    let subject = format!("fullops: 병합 기록 {key}");
    let range = format!("{sha}..HEAD");
    let log = git_in(
        &base_wt,
        &["log", "--ancestry-path", "--format=%s", &range],
        Duration::from_secs(30),
    );
    let recorded = log.ok() && log.stdout_text().lines().any(|l| l == subject);
    // 게이트 없이(날것 git merge·push) 통합 기준 ref에 이미 들어간 SHA. 막을 것이 없으므로 그 사실만 남긴다
    let ungated = landed && !recorded;
    let mut body = Vec::new();
    if ungated {
        body.push(
            "리뷰 판정: 병합 기록 커밋 없이 통합된 SHA라 리뷰·lint를 판정하지 못했다".to_string(),
        );
    } else if !recorded {
        // M-04. 날것 git merge로 먼저 들어온 SHA는 통합 기준 ref에서 갈라진 곳부터 본다
        if merged {
            ctx.note(&format!(
                "{k} {}: 병합 기록 커밋 없이 기본 브랜치에 들어와 있어 push 전에 리뷰·lint 판정을 다시 합니다.",
                sha.short(7)
            ));
        }
        let from = if merged { &upstream } else { before.as_str() };
        let mb = known(git.merge_base(from, sha.as_str()), "merge-base")?;
        let (need, fell_back) = review::need_for(&git, &mb, &sha)?;
        if fell_back {
            ctx.note("기준 시점 설정을 읽지 못해 기본값을 썼습니다.");
        }
        if need.required {
            review_gate(ctx, &state, &key, &need, role, &design)?;
        }
        body.push(format!("리뷰 판정: {}", need.reason));
        // M-05
        match &accept_lint {
            Some(reason) => body.push(format!("lint 증거 없이 병합: {reason}")),
            None if need.code_files > 0
                && !lint_ok(&git, &state, &worktrees, &completion, &mb, &sha) =>
            {
                return Err(refused("M-05", "", &[k, sha.short(7)]));
            }
            None => {}
        }
        // M-06
        if ours {
            finish()?;
        } else if !merged
            && base.merge(&base_wt, sha.as_str(), MergeMode::NoEdit)? == MergeOutcome::Conflict
        {
            return Err(CliError::Human {
                decision: None,
                message: msg::human("M-06", "", &[k, &unmerged()]),
            });
        }
    }

    // M-07, M-08. 병합 기록 커밋이 이미 있으면 둘 다 끝난 것이다
    let head = known(base.rev_parse("HEAD"), "기본 브랜치 tip")?;
    let how = match (merged, head == sha) {
        (true, _) => "already",
        (false, true) => "ff",
        (false, false) => "merge",
    };
    let mut lint = "reused";
    // 병합 전 tip: 이번에 병합했으면 그때의 HEAD, 다시 실행이면 병합 커밋의 첫 부모.
    // 원격의 변경을 합쳤으면 push할 전부(통합 기준 ref부터)를 다시 본다
    let pre = match (&rejoined, merged) {
        (Some(_), _) => git.rev_parse(&upstream).yes(),
        _ if recorded || landed => None,
        (None, false) => Some(before.clone()),
        (None, true) if base.rev_parse("HEAD^2").yes().as_ref() == Some(&sha) => {
            base.rev_parse("HEAD^1").yes()
        }
        (None, true) => None,
    };
    let tree = |rev: &str| base.rev_parse(&format!("{rev}^{{tree}}")).yes();
    if let Some(pre) = pre
        && (rejoined.is_some() || tree("HEAD") != tree(sha.as_str()))
    {
        if let Err(tail) = run_lint(&base_wt, &pre) {
            let back = rejoined.as_ref().unwrap_or(&pre);
            base.reset_keep(&base_wt, back.as_str())?;
            return Err(refused(
                "M-07",
                "",
                &[k, back.short(7), &tail, role, &design],
            ));
        }
        lint = "ran";
    }
    if !recorded {
        body.push(
            match lint {
                "ran" => "lint: 병합 결과의 트리가 달라 병합 뒤 한 번 실행했다",
                _ => "lint: worker의 lint 결과를 재사용했다",
            }
            .to_string(),
        );
        record(ctx, &base, &state, &completion, &key, &sha, &body)?;
    }

    // M-09
    let pushed = match (&config.remote, no_push) {
        (Some(remote), false) => {
            git.push(remote, &config.base).map_err(|e| {
                CliError::External(format!(
                    "git push {remote} {}가 실패했습니다: {e} 완료 보고는 통합 대기로 남았습니다. 다시 실행하면 push만 합니다. 원격이 먼저 나아가 거절됐으면 `FULLOPS_OFFLINE` 없이 다시 실행해 원격의 변경을 합칩니다.",
                    config.base
                ))
            })?;
            true
        }
        _ => false,
    };

    // M-10
    let place = known(landing(&git, config, &sha), "통합 위치")?;
    let now = fullops_store::now();
    let mut derived = Derived::default();
    if place == Landing::Landed {
        // 같은 과제의 앞선 보고(병합 전 수정 배정 앞의 SHA)도 병합한 SHA에 담겼으면 함께 닫는다. 남기면 D-03이 다음 배정을 막는다
        let earlier: Vec<_> = super::hold::recompute(repo, &git, config, &load_state(ctx)?)
            .0
            .into_iter()
            .filter(|(id, _)| {
                let same = |c: &&Completion| &c.id == id && c.key.as_ref() == Some(&key);
                state.completions.iter().any(|c| same(&c))
            })
            .collect();
        ctx.update_state(None, |state| {
            mark_landed(state, &completion.id, None, now);
            for (id, drift) in &earlier {
                mark_landed(state, id, *drift, now);
            }
            // 병합이 끝났으면 그 과제의 리뷰 보고와 앞선 실패 보고도 처리된 것이다
            let of_task = |d: &Dispatch| d.key == key;
            let ours: Vec<DispatchId> = state
                .dispatches
                .iter()
                .filter(|d| of_task(d))
                .map(|d| d.id.clone())
                .collect();
            ack(state, now, |m| {
                m.kind == "worker_done" && m.dispatch.as_ref().is_some_and(|d| ours.contains(d))
            });
            archive_done(repo, state);
        })?;
    } else {
        derived.landing.insert(completion.id.to_string(), place);
    }

    // M-11. 동기화하지 못한 역할은 예약만 남기고 계속한다
    let synced = sync::sync_roles(ctx, &git, None).unwrap_or_else(|e| {
        ctx.note(&format!("역할 워크트리를 동기화하지 못했습니다: {e:?}"));
        SyncReport::default()
    });

    // M-12
    let open = load_state(ctx)?;
    let reviewing =
        |s: &&fullops_core::model::Snapshot| s.key() == k && s.state == SnapshotState::Active;
    for snapshot in open.snapshots.iter().filter(reviewing) {
        if let Ok(Some(why)) = review::cleanup(ctx, &git, &snapshot.review, None) {
            ctx.note(&format!(
                "snapshot {}를 정리하지 못해 사유를 적었습니다: {why}",
                snapshot.review
            ));
        }
    }

    // M-13
    let plan = fullops_store::read_plan(repo, assets::PLANS_MD)
        .yes()
        .unwrap_or_default();
    let archive = fullops_store::archive_keys(repo).yes().unwrap_or_default();
    let inbox = |role: &fullops_core::model::RoleId| repo.inbox_path(role).exists();
    let after = load_state(ctx)?;
    let ready = fullops_core::ready(&plan, &after, &archive, &inbox);
    let mut metrics = fullops_core::metrics(
        &fullops_store::read_turns(repo).yes().unwrap_or_default(),
        &after,
        &fullops_store::read_archive(repo).yes().unwrap_or_default(),
    );
    metrics.at = now;
    derived.ready = Some(ready.clone());
    derived.metrics = Some(metrics);
    ctx.update_state(Some(&derived), |_| ())?;

    let run = completion
        .dispatch
        .as_ref()
        .and_then(|id| state.dispatch(id))
        .map(|d| d.run.clone())
        .or_else(|| {
            let report = state.messages.iter().find(|m| m.id == completion.id);
            report.map(|m| m.run.clone())
        });
    let in_run = |r: &fullops_core::model::RunId| Some(r) == run.as_ref();
    let unacked: Vec<&str> = after
        .messages
        .iter()
        .filter(|m| in_run(&m.run) && m.acked_at.is_none())
        .map(|m| m.id.as_str())
        .collect();
    let working: Vec<String> = after
        .dispatches
        .iter()
        .filter(|d| in_run(&d.run) && d.settled.is_none() && !d.is_reservation())
        .map(|d| format!("{}({})", d.key, d.role))
        .collect();
    let run_text = run.as_ref().map_or("<R>", |r| r.as_str());
    let mut next = Vec::new();
    if place != Landing::Landed {
        next.push(format!(
            "push가 남았습니다: `fullops merge {key}`를 다시 실행합니다"
        ));
    }
    let by_merge = |id: &&str| {
        let id = fullops_core::model::MessageId::new(*id);
        id.is_ok_and(|id| ack_by(&after, &id) == AckBy::Merge)
    };
    let (to_merge, to_ack): (Vec<&str>, Vec<&str>) = unacked.iter().copied().partition(by_merge);
    for id in to_merge {
        let report = after.completions.iter().find(|c| c.id.as_str() == id);
        if let Some(key) = report.and_then(|c| c.key.as_ref()) {
            next.push(format!("`fullops merge {key}`(처리 표시까지 한다)"));
        }
    }
    if !to_ack.is_empty() {
        let acks: Vec<String> = to_ack.iter().map(|id| format!("--ack {id}")).collect();
        next.push(format!(
            "그 밖의 메시지는 처리한 뒤 `fullops wait --run {run_text} {} --no-wait`",
            acks.join(" ")
        ));
    }
    if !ready.is_empty() {
        next.push("준비된 행은 `fullops task new` → `fullops dispatch`".to_string());
    }
    // 표에 없어도 이어 갈 일: 병합으로 들어온 지시서(설계 완료 뒤의 구현 지시서)와 실패한 채 남은 과제
    let left = super::dispatch::leftover(repo, config, &after, &caller.actor);
    if !left.is_empty() {
        next.push(format!("이어서 할 일: {}", left.join(", ")));
    }
    if !working.is_empty() {
        let coordinator = after
            .runs
            .iter()
            .find(|r| in_run(&r.id))
            .and_then(|r| r.coordinator.as_deref());
        next.push(if orca_wakes(session_handle().as_deref(), coordinator) {
            "기다릴 것이 남았으면 턴을 끝낸다. Orca가 깨운다".to_string()
        } else {
            format!("`fullops wait --run {run_text}`로 이어 기다린다")
        });
    }
    let next = next.join(". ");
    let done: Vec<&str> = synced.done.iter().map(|r| r.as_str()).collect();
    let ready_text: Vec<&str> = ready.iter().map(TaskKey::as_str).collect();
    let list = |items: &[&str]| match items {
        [] => "없음".to_string(),
        items => items.join(", "),
    };
    let left = synced.left();
    let working_text: Vec<&str> = working.iter().map(String::as_str).collect();
    let left_text: Vec<&str> = left.iter().map(String::as_str).collect();
    ctx.print(
        &format!(
            "{key} 병합: {} ({how}, lint {lint}, 통합 위치 {place}).{}\n동기화한 역할: {}. 예약으로 남은 역할: {}.\n미처리 메시지: {}. 미정산 dispatch: {}.\n준비된 행: {}.\n다음: {}",
            sha.short(7),
            if ungated {
                " 이 SHA는 병합 기록 커밋 없이 통합돼 있어 리뷰·lint 게이트를 판정하지 못했습니다."
            } else {
                ""
            },
            list(&done),
            list(&left_text),
            list(&unacked),
            list(&working_text),
            list(&ready_text),
            if next.is_empty() { "없음" } else { &next }
        ),
        json!({
            "key": key, "sha": sha, "merge": how, "lint": lint, "pushed": pushed, "landing": place,
            "ungated": ungated,
            "synced": done, "sync_pending": left, "unacked": unacked, "unsettled": working,
            "ready": ready, "next": next,
        }),
    );
    Ok(EXIT_OK)
}
