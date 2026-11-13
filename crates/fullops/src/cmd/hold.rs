//! `fullops hold`: 보류, 재개, 통합 일괄 확인, SHA 지정(`hold`·`resume`·`settle`·`completion`)(설계 8.8절).
//! 통합 쪽 명령(`merge`·`sync`·`review`·`decision`·`gc`)이 함께 쓰는 작은 함수도 여기에 둔다.

use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::messages::merge as msg;
use fullops_core::model::{
    ArchiveLine, ArchivedDispatch, ArchivedTask, Completion, Config, Derived, DispatchId, Hold,
    Known, Landing, Message, MessageId, Sha, ShaSource, State, TaskKey, WorkOutcome,
};
use fullops_ext::{Git, GitCli};
use fullops_store::Repo;
use lexopt::prelude::*;
use serde_json::json;
use std::collections::BTreeSet;
use std::time::Duration;

/// 기록 경로(설계 6.6절). 이 경로만 바꾼 커밋과 미커밋 변경은 코드 변경으로 세지 않는다.
const RECORD_PATHS: [&str; 5] = [
    ".fullops-squad/handovers/",
    ".fullops-squad/docs/evaluations/",
    ".fullops-squad/routes/",
    ".fullops-squad/reviews/",
    ".fullops-squad/PLANS.md",
];

pub(super) fn is_record(path: &str) -> bool {
    RECORD_PATHS.iter().any(|p| path.starts_with(p))
}

/// 옵션의 값.
pub(super) fn value(args: &mut lexopt::Parser) -> Result<String, CliError> {
    Ok(args.value()?.string()?)
}

pub(super) fn task_key(text: Option<String>) -> Result<TaskKey, CliError> {
    let text = text.ok_or_else(|| CliError::Usage("과제 키가 필요합니다.".into()))?;
    TaskKey::new(text).map_err(|e| CliError::Usage(e.to_string()))
}

pub(super) fn refused(rule: &'static str, case: &str, args: &[&str]) -> CliError {
    CliError::Refused(vec![msg::refusal(rule, case, args)])
}

/// git이 답하지 못하면 바깥 도구 실패(종료 코드 4)다.
pub(super) fn known<T>(value: Known<T>, what: &str) -> Result<T, CliError> {
    match value {
        Known::Yes(v) => Ok(v),
        Known::Unknown(why) => Err(CliError::External(format!(
            "git이 {what}에 답하지 못했습니다({why})."
        ))),
    }
}

/// `state.json`. 읽히지 않으면 `Ctx::update_state`의 복구 길(옮기고 종료 코드 1)을 탄다.
pub(super) fn load_state(ctx: &Ctx) -> Result<State, CliError> {
    match fullops_store::read_state(ctx.repo()?) {
        Known::Yes(state) => Ok(state),
        Known::Unknown(_) => ctx.update_state(None, |state| state.clone()),
    }
}

/// 짧은 SHA나 ref를 커밋으로 푼다.
pub(super) fn commit(git: &GitCli, rev: &str) -> Option<Sha> {
    git.rev_parse(&format!("{rev}^{{commit}}")).yes()
}

/// 원격이 있으면 기본 브랜치를 fetch한다. `FULLOPS_OFFLINE=1`이면 건너뛰고 알린다.
pub(super) fn fetch(ctx: &Ctx, git: &GitCli, config: &Config) -> Result<(), CliError> {
    let Some(remote) = &config.remote else {
        return Ok(());
    };
    if std::env::var("FULLOPS_OFFLINE").is_ok_and(|v| v == "1") {
        ctx.note("FULLOPS_OFFLINE=1이라 fetch를 건너뛰었습니다.");
        return Ok(());
    }
    git.fetch(remote, &config.base)
        .map_err(|e| CliError::External(format!("git fetch {remote} {}: {e}", config.base)))
}

/// 통합 위치(설계 6.6절).
pub(super) fn landing(git: &GitCli, config: &Config, sha: &Sha) -> Known<Landing> {
    let local = format!("refs/heads/{}", config.base);
    match git.is_ancestor(sha.as_str(), &config.base_ref()) {
        Known::Yes(true) => Known::Yes(Landing::Landed),
        Known::Yes(false) if config.remote.is_some() => {
            git.is_ancestor(sha.as_str(), &local).map(|merged| {
                if merged {
                    Landing::PushLeft
                } else {
                    Landing::MergeLeft
                }
            })
        }
        Known::Yes(false) => Known::Yes(Landing::MergeLeft),
        Known::Unknown(why) => Known::Unknown(why),
    }
}

/// 그 키의 아직 통합되지 않은 성공 보고 가운데 하나를 고른다(M-02). `held`가 참이면 보류된 것에서, 아니면 통합 대기에서 고른다.
pub(super) fn pick(
    state: &State,
    key: &TaskKey,
    id: Option<&str>,
    held: bool,
) -> Result<Completion, CliError> {
    let open = |c: &&Completion| {
        c.key.as_ref() == Some(key)
            && !c.direct
            && c.outcome == WorkOutcome::Succeeded
            && c.landed_at.is_none()
            && id.is_none_or(|id| c.id.as_str() == id)
    };
    let (wanted, other): (Vec<&Completion>, Vec<&Completion>) = state
        .completions
        .iter()
        .filter(open)
        .partition(|c| c.hold.is_some() == held);
    match (wanted.as_slice(), held) {
        ([one], _) => Ok((*one).clone()),
        ([], true) => Err(refused("HD-01", "", &[key.as_str()])),
        ([], false) if !other.is_empty() => Err(refused("M-02", "held", &[key.as_str()])),
        ([], false) => Err(refused("M-02", "none", &[key.as_str()])),
        (many, _) => {
            let ids: Vec<&str> = many.iter().map(|c| c.id.as_str()).collect();
            Err(refused("M-02", "many", &[key.as_str(), &ids.join(", ")]))
        }
    }
}

/// 처리 표시(설계 6.2.1절 `acked_at`).
pub(super) fn ack(state: &mut State, now: i64, which: impl Fn(&Message) -> bool) {
    for message in &mut state.messages {
        if message.acked_at.is_none() && which(message) {
            message.acked_at = Some(now);
        }
    }
}

/// 통합이 확인된 completion을 닫는다: `landed_at`, 보류는 `released_hold`로, 그 보고 메시지의 처리 표시.
pub(super) fn mark_landed(state: &mut State, id: &MessageId, drift: Option<u32>, now: i64) {
    let open = |c: &&mut Completion| &c.id == id && c.landed_at.is_none();
    if let Some(completion) = state.completions.iter_mut().find(open) {
        completion.landed_at = Some(now);
        completion.drift = drift;
        if let Some(hold) = completion.hold.take() {
            completion.released_hold = Some(hold);
        }
    }
    ack(state, now, |m| &m.id == id);
}

/// 통합되고 열린 dispatch가 없는 과제를 `archive.jsonl`로 옮긴다(설계 6.2.1절). `update_state`의 잠금 안에서 부른다.
/// 줄을 먼저 쓰고 state에서 뺀다: 중간에 죽으면 줄이 겹칠 수는 있어도 과제가 사라지지는 않는다.
pub(super) fn archive_done(repo: &Repo, state: &mut State) -> Vec<TaskKey> {
    let keys: BTreeSet<TaskKey> = state
        .completions
        .iter()
        .filter_map(|c| c.key.clone())
        .collect();
    let mut done = Vec::new();
    for key in keys {
        let succeeded = || {
            let ours = |c: &&Completion| {
                c.key.as_ref() == Some(&key) && c.outcome == WorkOutcome::Succeeded
            };
            state.completions.iter().filter(ours)
        };
        let working = |d: &&fullops_core::model::Dispatch| {
            d.key == key && d.settled.is_none() && !d.is_reservation()
        };
        let Some(last) = succeeded().max_by_key(|c| c.received_at) else {
            continue;
        };
        if succeeded().any(|c| c.landed_at.is_none())
            || state.dispatches.iter().any(|d| working(&d))
        {
            continue;
        }
        let ours = |d: &&fullops_core::model::Dispatch| d.key == key && !d.is_reservation();
        let line = ArchiveLine::Task(ArchivedTask {
            key: key.clone(),
            role: last.role.clone(),
            sha: last.sha.clone(),
            received_at: last.received_at,
            landed_at: last.landed_at,
            rounds: state.tasks.get(&key).map_or(0, |t| t.rounds),
            dispatches: state
                .dispatches
                .iter()
                .filter(ours)
                .map(|d| ArchivedDispatch {
                    purpose: d.purpose,
                    started_at: d.started_at,
                    reason: d.reason,
                    sub: d.is_sub(),
                })
                .collect(),
            direct: last.direct,
        });
        if fullops_store::append_archive(repo, &line).is_err() {
            continue;
        }
        let dispatches: Vec<DispatchId> = state
            .dispatches
            .iter()
            .filter(ours)
            .map(|d| d.id.clone())
            .collect();
        let reports: Vec<MessageId> = state
            .completions
            .iter()
            .filter(|c| c.key.as_ref() == Some(&key))
            .map(|c| c.id.clone())
            .collect();
        // 처리 표시가 없는 메시지(질문 등)는 남긴다
        state.messages.retain(|m| {
            let ours = reports.contains(&m.id)
                || m.dispatch.as_ref().is_some_and(|d| dispatches.contains(d));
            !(ours && m.acked_at.is_some())
        });
        state.dispatches.retain(|d| !dispatches.contains(&d.id));
        state.completions.retain(|c| c.key.as_ref() != Some(&key));
        state.tasks.remove(&key);
        done.push(key);
    }
    done
}

/// 통합 시점에 기본 브랜치가 보고 SHA 뒤의 역할 브랜치 커밋(기록 경로만 바꾼 커밋 제외)을 함께 담았으면 그 수(설계 8.8절).
fn drift(repo: &Repo, git: &GitCli, config: &Config, completion: &Completion) -> Option<u32> {
    let (sha, branch) = (completion.sha.as_ref()?, completion.branch.as_ref()?);
    let tip = git.rev_parse(&format!("refs/heads/{branch}")).yes()?;
    let contains = |old: &Sha, new: &str| git.is_ancestor(old.as_str(), new).yes() == Some(true);
    if &tip == sha || !contains(sha, tip.as_str()) || !contains(&tip, &config.base_ref()) {
        return None;
    }
    let mut argv: Vec<String> = [
        "git",
        "rev-list",
        "--count",
        "--first-parent",
        "--no-merges",
    ]
    .iter()
    .map(|a| a.to_string())
    .collect();
    argv.extend([format!("{sha}..{tip}"), "--".into(), ".".into()]);
    argv.extend(RECORD_PATHS.iter().map(|p| format!(":(exclude){p}")));
    let out = fullops_ext::run(&argv, &repo.root, Duration::from_secs(30), &[], None);
    let count: u32 = out.ok().then(|| out.stdout_text().trim().parse().ok())??;
    (count > 0).then_some(count)
}

/// 통합되지 않은 성공 보고마다 통합 위치를 다시 계산한다. 통합된 것의 (ID, drift)와 나머지의 위치를 돌려준다.
pub(super) fn recompute(
    repo: &Repo,
    git: &GitCli,
    config: &Config,
    state: &State,
) -> (Vec<(MessageId, Option<u32>)>, Derived) {
    let mut landed = Vec::new();
    let mut derived = Derived::default();
    let open = |c: &&Completion| c.landed_at.is_none() && c.outcome == WorkOutcome::Succeeded;
    for completion in state.completions.iter().filter(open) {
        let Some(sha) = &completion.sha else { continue };
        match landing(git, config, sha) {
            Known::Yes(Landing::Landed) => {
                landed.push((completion.id.clone(), drift(repo, git, config, completion)))
            }
            Known::Yes(other) => {
                derived.landing.insert(completion.id.to_string(), other);
            }
            Known::Unknown(_) => {}
        }
    }
    (landed, derived)
}

pub fn run(ctx: &Ctx, args: lexopt::Parser) -> Result<u8, CliError> {
    match ctx.command {
        "hold" => hold(ctx, args),
        "resume" => resume(ctx, args),
        "settle" => settle(ctx, args),
        _ => completion(ctx, args),
    }
}

fn hold(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let (mut key, mut id) = (None, None);
    let (mut reason, mut owner, mut resume) = (String::new(), String::new(), String::new());
    while let Some(arg) = args.next()? {
        match arg {
            Long("reason") => reason = value(&mut args)?,
            Long("owner") => owner = value(&mut args)?,
            Long("resume") => resume = value(&mut args)?,
            Long("completion") => id = Some(value(&mut args)?),
            Value(v) if key.is_none() => key = Some(v.string()?),
            other => return Err(other.unexpected().into()),
        }
    }
    let key = task_key(key)?;
    if [&reason, &owner, &resume]
        .iter()
        .any(|v| v.trim().is_empty())
    {
        return Err(CliError::Usage(
            "보류에는 --reason, --owner, --resume 셋이 모두 필요합니다.".into(),
        ));
    }
    let picked = pick(&load_state(ctx)?, &key, id.as_deref(), false)?;
    let now = fullops_store::now();
    ctx.update_state(None, |state| {
        let open = |c: &&mut Completion| c.id == picked.id;
        if let Some(completion) = state.completions.iter_mut().find(open) {
            completion.hold = Some(Hold {
                reason: reason.clone(),
                owner: owner.clone(),
                resume: resume.clone(),
                at: now,
                extra: Default::default(),
            });
        }
        // 보류는 lead가 그 보고를 처리했다는 뜻이다
        ack(state, now, |m| m.id == picked.id);
    })?;
    ctx.print(
        &format!(
            "{key} 보류: {reason} (담당 {owner}, 재개 조건 {resume}). 재개는 `fullops resume {key}`"
        ),
        json!({ "key": key, "completion": picked.id, "held": true }),
    );
    Ok(EXIT_OK)
}

fn resume(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let (mut key, mut id) = (None, None);
    while let Some(arg) = args.next()? {
        match arg {
            Long("completion") => id = Some(value(&mut args)?),
            Value(v) if key.is_none() => key = Some(v.string()?),
            other => return Err(other.unexpected().into()),
        }
    }
    let key = task_key(key)?;
    let picked = pick(&load_state(ctx)?, &key, id.as_deref(), true)?;
    ctx.update_state(None, |state| {
        let open = |c: &&mut Completion| c.id == picked.id;
        if let Some(completion) = state.completions.iter_mut().find(open) {
            completion.released_hold = completion.hold.take();
        }
    })?;
    ctx.print(
        &format!("{key} 재개: 통합 대기로 돌아갔습니다. 다음: `fullops merge {key}`"),
        json!({ "key": key, "completion": picked.id, "held": false }),
    );
    Ok(EXIT_OK)
}

fn settle(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    if let Some(arg) = args.next()? {
        return Err(arg.unexpected().into());
    }
    let (repo, config) = (ctx.repo()?, ctx.config()?);
    let git = GitCli::cli(repo.root.clone());
    fetch(ctx, &git, config)?;
    let (landed, derived) = recompute(repo, &git, config, &load_state(ctx)?);
    let now = fullops_store::now();
    let archived = ctx.update_state(Some(&derived), |state| {
        for (id, drift) in &landed {
            mark_landed(state, id, *drift, now);
        }
        archive_done(repo, state)
    })?;
    let ids: Vec<&str> = landed.iter().map(|(id, _)| id.as_str()).collect();
    let drifted: Vec<String> = landed
        .iter()
        .filter_map(|(id, drift)| Some(format!("{id}(+{}커밋)", (*drift)?)))
        .collect();
    let mut human = format!(
        "통합 확인 {}건, 통합 대기로 남은 것 {}건.",
        landed.len(),
        derived.landing.len()
    );
    if !drifted.is_empty() {
        human.push_str(&format!(
            " 기본 브랜치가 보고 SHA 뒤의 커밋을 함께 담았습니다: {}.",
            drifted.join(", ")
        ));
    }
    ctx.print(
        &human,
        json!({ "landed": ids, "archived": archived, "pending": derived.landing }),
    );
    Ok(EXIT_OK)
}

fn completion(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let (mut words, mut key) = (Vec::new(), None);
    while let Some(arg) = args.next()? {
        match arg {
            Long("key") => key = Some(task_key(Some(value(&mut args)?))?),
            Value(v) => words.push(v.string()?),
            other => return Err(other.unexpected().into()),
        }
    }
    let [verb, id, rev] = words.as_slice() else {
        return Err(CliError::Usage(
            "사용법: fullops completion set-sha <메시지 ID> <SHA> [--key <KEY>]".into(),
        ));
    };
    if verb != "set-sha" {
        return Err(CliError::Usage(format!("모르는 하위 명령입니다: {verb}")));
    }
    let git = GitCli::cli(ctx.repo()?.root.clone());
    let sha = commit(&git, rev).ok_or_else(|| refused("HD-02", "sha", &[rev.as_str()]))?;
    let found = |c: &Completion| c.id.as_str() == id;
    match load_state(ctx)?.completions.iter().find(|c| found(c)) {
        None => return Err(refused("HD-02", "unknown", &[id.as_str()])),
        Some(c) if c.landed_at.is_some() => return Err(refused("HD-02", "landed", &[id.as_str()])),
        Some(_) => {}
    }
    ctx.update_state(None, |state| {
        if let Some(completion) = state.completions.iter_mut().find(|c| found(c)) {
            completion.sha = Some(sha.clone());
            completion.sha_source = Some(ShaSource::Manual);
            if key.is_some() {
                completion.key = key.clone();
            }
        }
    })?;
    ctx.print(
        &format!("완료 보고 {id}의 SHA를 {sha}로 적었습니다."),
        json!({ "completion": id, "sha": sha, "key": key }),
    );
    Ok(EXIT_OK)
}
