//! `fullops review`: 리뷰 필요 판정, snapshot, 기록 검사, 면제, 정리(설계 8.9절).
//! 리뷰 대상과 규칙은 여기서 계산한다(OCR CLI를 쓰지 않는다). 순수 판정은 `fullops_core::review`에 있다.

use super::hold::{commit, is_record, known, load_state, refused, task_key, value};
use crate::assets;
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::lint_rules::config_sha256;
use fullops_core::messages::merge as msg;
use fullops_core::model::{
    Accepted, Completion, Config, HeadRef, Known, LintResult, Need, NeedAction, PacketCheck,
    Purpose, Refusal, RoleId, RoleKind, RunOwner, Sha, Snapshot, SnapshotHold, SnapshotState,
    State, TaskKey, Waiver, WorkOutcome, parse_packet, parse_packet_outcomes, path_text, same_path,
    sha256_hex,
};
use fullops_core::review::{
    ReviewResult, RuleFile, Targets, check_lint, check_result, result_skeleton, review_need,
    target_file,
};
use fullops_ext::{Git, GitCli, Orca, OrcaCli};
use fullops_store::Repo;
use lexopt::prelude::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const CONFIG: &str = ".fullops-squad/fullops.toml";
const LINT_JSON: &str = ".fullops-squad/lint/lint.json";
const RULE_JSON: &str = ".fullops-squad/review/rule.json";
/// `review check`의 실패 항목 번호(설계 8.9절)에 붙인 ID.
const ITEMS: [&str; 11] = [
    "RV-00", "RV-01", "RV-02", "RV-03", "RV-04", "RV-05", "RV-06", "RV-07", "RV-08", "RV-09",
    "RV-10",
];

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    fullops_store::read_json(path).yes().flatten()
}

fn file_sha256(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|bytes| sha256_hex(&bytes))
}

/// 작업 트리 안 경로를 커밋 안 경로(`/` 구분)로 적는다.
fn rel(repo: &Repo, path: PathBuf) -> String {
    path_text(path.strip_prefix(&repo.root).unwrap_or(&path))
}

/// 그 커밋의 파일 글. 없으면 `None`.
fn blobs(git: &GitCli, rev: &Sha, paths: &[String]) -> Vec<Option<Vec<u8>>> {
    let items: Vec<(String, String)> = paths.iter().map(|p| (rev.to_string(), p.clone())).collect();
    let mut found = git.show_blobs(&items).yes().unwrap_or_default();
    found.resize(paths.len(), None);
    found
}

fn text(bytes: &Option<Vec<u8>>) -> Option<String> {
    bytes
        .as_ref()
        .map(|b| String::from_utf8_lossy(b).into_owned())
}

/// 필요 판정(설계 8.9절). 정책과 lint 제외는 merge-base 시점의 것이다. 둘째 값은 그 시점 설정을 읽지 못해 기본값을 썼는가다.
pub(super) fn need_for(git: &GitCli, base: &Sha, head: &Sha) -> Result<(Need, bool), CliError> {
    let mb = known(git.merge_base(base.as_str(), head.as_str()), "merge-base")?;
    let at = blobs(git, &mb, &[CONFIG.to_string(), LINT_JSON.to_string()]);
    let (config, lint) = (text(&at[0]), text(&at[1]));
    let baseline = fullops_store::baseline(config.as_deref());
    let exclude = fullops_core::lint_excludes(lint.as_deref(), assets::LINT_JSON);
    let numstat = known(git.numstat(mb.as_str(), head.as_str()), "변경 통계")?;
    let need = review_need(
        &baseline.review,
        &numstat,
        &baseline.code,
        &exclude,
        base.clone(),
        head.clone(),
        mb,
    );
    Ok((need, baseline.fell_back))
}

/// lint를 돌린 head가 `sha`를 덮는가: 같거나, 조상이고 그 사이가 기록 경로뿐이다(설계 8.8절 M-05).
pub(super) fn covers(git: &GitCli, lint_head: &Sha, sha: &Sha) -> bool {
    let (old, new) = (lint_head.as_str(), sha.as_str());
    lint_head == sha
        || (git.is_ancestor(old, new).yes() == Some(true)
            && git
                .diff_names(old, new)
                .yes()
                .is_some_and(|paths| paths.iter().all(|p| is_record(p))))
}

/// 리뷰의 base: 기본 브랜치와 head의 merge-base. 기본 브랜치 위에서 직접 일해 merge-base가 head 자신이면
/// 통합 기준 ref, 세션의 anchor 순으로 넘어간다(dev 직접 작업).
fn review_base(ctx: &Ctx, git: &GitCli, head: &Sha) -> Result<Sha, CliError> {
    let config = ctx.config()?;
    let anchor = ctx.caller().ok().and_then(|c| c.session?.anchor);
    let candidates = [
        Some(format!("refs/heads/{}", config.base)),
        Some(config.base_ref()),
        anchor.map(|a| a.to_string()),
    ];
    let mut base = None;
    for rev in candidates.into_iter().flatten() {
        if let Some(mb) = git.merge_base(&rev, head.as_str()).yes() {
            if &mb != head {
                return Ok(mb);
            }
            base = Some(mb);
        }
    }
    base.ok_or_else(|| {
        CliError::External("git이 리뷰의 base(merge-base)에 답하지 못했습니다.".into())
    })
}

/// 리뷰할 것: head, base, 그 head의 완료 보고.
struct Subject {
    head: Sha,
    base: Sha,
    completion: Option<Completion>,
    /// 그 과제의 통합되지 않은 완료 보고가 있는데 어느 것도 이 head가 아니다(수정 커밋 뒤의 옛 리뷰).
    stale: bool,
}

/// head는 `--sha`, 없으면 그 과제의 통합되지 않은 완료 보고의 SHA, dev 직접 작업은 `HEAD`다.
fn subject(
    ctx: &Ctx,
    git: &GitCli,
    state: &State,
    key: &TaskKey,
    sha: Option<&str>,
) -> Result<Subject, CliError> {
    let open: Vec<&Completion> = state
        .completions
        .iter()
        .filter(|c| {
            c.key.as_ref() == Some(key)
                && !c.direct
                && c.outcome == WorkOutcome::Succeeded
                && c.landed_at.is_none()
                && c.sha.is_some()
        })
        .collect();
    let head = match (sha, open.as_slice()) {
        (Some(rev), _) => commit(git, rev),
        (None, [one]) => one.sha.clone(),
        (None, []) if ctx.caller()?.actor.direct_dev => commit(git, "HEAD"),
        _ => None,
    };
    let head = head.ok_or_else(|| refused("RV-00", "nohead", &[key.as_str()]))?;
    let completion = open.iter().find(|c| c.sha.as_ref() == Some(&head));
    Ok(Subject {
        base: review_base(ctx, git, &head)?,
        completion: completion.map(|c| (*c).clone()),
        stale: !open.is_empty() && completion.is_none(),
        head,
    })
}

/// 리뷰 규칙 파일과 그 지문. 체크아웃마다 줄끝이 달라도 같은 지문이 되게 LF로 맞춘다. 파일이 없으면 빈 규칙이다.
fn rules(repo: &Repo) -> (RuleFile, String) {
    let bytes = std::fs::read(repo.root.join(RULE_JSON)).unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
    (
        serde_json::from_str(&text).unwrap_or_default(),
        sha256_hex(text.as_bytes()),
    )
}

/// 등록된 모든 워크트리의 `lint/result.json` 가운데 head를 덮는 것.
fn find_lint(git: &GitCli, head: &Sha) -> Option<Vec<u8>> {
    git.worktrees().yes()?.iter().find_map(|worktree| {
        let repo = fullops_store::discover(&worktree.path).ok()??;
        let bytes = std::fs::read(repo.lint_dir().join("result.json")).ok()?;
        let lint: LintResult = serde_json::from_slice(&bytes).ok()?;
        covers(git, &lint.head, head).then_some(bytes)
    })
}

/// 유효한 `accepted.json`: 두 해시가 지금 파일과 같다.
pub fn accepted(dir: &Path) -> Option<Accepted> {
    let record: Accepted = read(&dir.join("accepted.json"))?;
    let same =
        |name: &str, recorded: &str| file_sha256(&dir.join(name)).as_deref() == Some(recorded);
    (same("result.json", &record.result_sha256) && same("lint.json", &record.lint_sha256))
        .then_some(record)
}

/// 그 base·head의 리뷰 증거(설계 8.8절 M-04의 표).
pub(super) enum Evidence {
    Accepted,
    Waived,
    /// 결론이 reject이거나 미해결 critical·high가 있다.
    Rejected,
    /// 기록은 있는데 수락되지 않았다.
    Unfinished,
    None,
}

pub(super) fn evidence(repo: &Repo, key: &TaskKey, base: &Sha, head: &Sha) -> Evidence {
    let dir = repo.review_dir(key, head);
    if accepted(&dir).is_some_and(|a| &a.head == head) {
        return Evidence::Accepted;
    }
    let waived = |w: Waiver| &w.base == base && &w.head == head;
    if read::<Waiver>(&dir.join("waiver.json")).is_some_and(waived) {
        return Evidence::Waived;
    }
    if !dir.join("result.json").exists() {
        return Evidence::None;
    }
    if rejected(repo, key, head) {
        Evidence::Rejected
    } else {
        Evidence::Unfinished
    }
}

/// 그 SHA의 리뷰 결론이 reject이거나 미해결 critical·high가 있다.
pub(super) fn rejected(repo: &Repo, key: &TaskKey, head: &Sha) -> bool {
    read::<ReviewResult>(&repo.review_dir(key, head).join("result.json"))
        .is_some_and(|r| r.conclusion.as_deref() == Some("reject") || r.blocking())
}

/// 그 역할이 인박스 없이 받는 설계 역할이면 그 과제의 route class(수정 배정 안내가 설계 역할 재배정이 된다).
pub(super) fn design_class(
    repo: &Repo,
    config: &Config,
    key: &TaskKey,
    role: Option<&RoleId>,
) -> Option<String> {
    role.and_then(|r| config.roles.get(r))
        .filter(|r| r.kind == RoleKind::Design)?;
    let route = fullops_store::read_route(repo, key).yes().flatten();
    Some(route.map_or("<class>".into(), |r| r.class.to_string()))
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let verb = match args.next()? {
        Some(Value(v)) => v.string()?,
        _ => {
            return Err(CliError::Usage(
                "사용법: fullops review need|snapshot|check|waive|cleanup|supersede|prune".into(),
            ));
        }
    };
    match verb.as_str() {
        "need" => need(ctx, args),
        "snapshot" => snapshot(ctx, args),
        "check" => check(ctx, args),
        "waive" => waive(ctx, args),
        "cleanup" => cleanup_command(ctx, args),
        "supersede" => supersede(ctx, args),
        "prune" => prune(ctx, args),
        other => Err(CliError::Usage(format!("모르는 하위 명령입니다: {other}"))),
    }
}

/// 하위 명령이 함께 받는 인자: 과제 키 하나와 값이 있는 옵션들, 값 없는 옵션들.
struct Parsed {
    key: Option<String>,
    options: Vec<(String, String)>,
    flags: Vec<String>,
}

impl Parsed {
    fn new(mut args: lexopt::Parser, valued: &[&str], flags: &[&str]) -> Result<Self, CliError> {
        let mut out = Parsed {
            key: None,
            options: Vec::new(),
            flags: Vec::new(),
        };
        while let Some(arg) = args.next()? {
            match arg {
                Long(name) if valued.contains(&name) => {
                    let name = name.to_string();
                    out.options.push((name, value(&mut args)?));
                }
                Long(name) if flags.contains(&name) => out.flags.push(name.to_string()),
                Value(v) if out.key.is_none() => out.key = Some(v.string()?),
                other => return Err(other.unexpected().into()),
            }
        }
        Ok(out)
    }

    fn get(&self, name: &str) -> Option<&str> {
        let named = |(n, _): &&(String, String)| n == name;
        self.options.iter().find(named).map(|(_, v)| v.as_str())
    }

    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }
}

fn need(ctx: &Ctx, args: lexopt::Parser) -> Result<u8, CliError> {
    let parsed = Parsed::new(args, &["from", "to"], &[])?;
    let git = GitCli::cli(ctx.repo()?.root.clone());
    let rev = |name: &str, rev: &str| {
        commit(&git, rev)
            .ok_or_else(|| CliError::Usage(format!("--{name} `{rev}`를 커밋으로 풀지 못했습니다.")))
    };
    let (base, head) = match (&parsed.key, parsed.get("from"), parsed.get("to")) {
        (None, Some(from), Some(to)) => (rev("from", from)?, rev("to", to)?),
        (Some(_), None, None) => {
            let key = task_key(parsed.key.clone())?;
            let subject = subject(ctx, &git, &load_state(ctx)?, &key, None)?;
            (subject.base, subject.head)
        }
        _ => {
            return Err(CliError::Usage(
                "사용법: fullops review need <KEY> | --from <ref> --to <ref>".into(),
            ));
        }
    };
    let (need, fell_back) = need_for(&git, &base, &head)?;
    if fell_back {
        ctx.note("기준 시점 설정을 읽지 못해 기본값을 썼습니다.");
    }
    let next = match need.action {
        NeedAction::Skip => "리뷰 없이 병합합니다",
        NeedAction::Ask => "사용자에게 진행·생략을 묻습니다(`fullops merge`가 결정을 엽니다)",
        NeedAction::Review => "`fullops review snapshot`으로 독립 리뷰를 받습니다",
    };
    ctx.print(
        &format!("리뷰 판정 {}: {} 다음: {next}", need.action, need.reason),
        serde_json::to_value(&need).unwrap_or_default(),
    );
    Ok(EXIT_OK)
}

fn snapshot(ctx: &Ctx, args: lexopt::Parser) -> Result<u8, CliError> {
    let parsed = Parsed::new(args, &["sha", "implementer-session"], &[])?;
    let key = task_key(parsed.key.clone())?;
    let repo = ctx.repo()?;
    let git = GitCli::cli(repo.root.clone());
    let state = load_state(ctx)?;
    let Subject {
        head,
        base,
        completion,
        ..
    } = subject(ctx, &git, &state, &key, parsed.get("sha"))?;
    let review = format!("{key}@{}", head.short(12));
    let dir = repo.review_dir(&key, &head);
    let live = |s: &&Snapshot| {
        s.review == review && matches!(s.state, SnapshotState::Creating | SnapshotState::Active)
    };
    let previous = state.snapshots.iter().find(live);
    // 쓰다 멈춘(creating) 리뷰만 이어 만든다. 그 밖에는 있는 기록을 덮어쓰지 않는다
    let resuming = previous.is_some_and(|s| s.state == SnapshotState::Creating);
    if previous.is_some_and(|s| s.state == SnapshotState::Active)
        || (dir.join("targets.json").exists() && !resuming)
    {
        return Err(refused("RV-00", "exists", &[&rel(repo, dir)]));
    }
    let registered = known(git.worktrees(), "워크트리 목록")?;
    let kept = previous.filter(|s| {
        Path::new(&s.path).is_dir()
            && registered
                .iter()
                .any(|w| same_path(&path_text(&w.path), &s.path))
    });
    let now = fullops_store::now();
    let path = match kept {
        Some(snapshot) => PathBuf::from(&snapshot.path),
        None => {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos());
            let seed = format!("{review}{now}{nanos}{}", std::process::id());
            let name = format!(".fullops-review-{}", &sha256_hex(seed.as_bytes())[..8]);
            repo.root.parent().unwrap_or(&repo.root).join(name)
        }
    };
    let caller = ctx.caller()?;
    let reporter = || {
        let reported = || state.dispatch(completion.as_ref()?.dispatch.as_ref()?);
        // 보고가 없는 SHA(`--sha`)는 그 과제의 가장 늦은 구현 배정이 만든 것으로 본다
        let work = |d: &&fullops_core::model::Dispatch| d.key == key && d.counts() && d.is_work();
        let latest = || {
            state
                .dispatches
                .iter()
                .filter(work)
                .max_by_key(|d| d.started_at)
        };
        Some(
            reported()
                .or_else(latest)?
                .bound_session
                .as_ref()?
                .to_string(),
        )
    };
    let own = || {
        caller
            .sid
            .as_ref()
            .filter(|_| caller.actor.direct_dev)
            .map(|s| s.to_string())
    };
    let implementer = parsed
        .get("implementer-session")
        .map(str::to_string)
        .or_else(reporter)
        .or_else(own)
        .filter(|s| !s.trim().is_empty());
    // 구현자를 모르면 check가 독립 검토인지 가릴 수 없다(RV-04)
    if implementer.is_none() {
        return Err(refused("RV-00", "implementer", &[key.as_str()]));
    }
    let record = Snapshot {
        review: review.clone(),
        path: path_text(&path),
        head: head.clone(),
        base: base.clone(),
        state: SnapshotState::Creating,
        implementer_session: implementer.clone(),
        reviewer_dispatch: None,
        owner: caller
            .actor
            .dispatch
            .clone()
            .map_or(RunOwner::Lead, RunOwner::Dispatch),
        created_at: now,
        hold: None,
        extra: Default::default(),
    };
    ctx.update_state(None, |state| {
        state.snapshots.retain(|s| !live(&s));
        state.snapshots.push(record.clone());
    })?;
    if kept.is_none() {
        git.worktree_add_detached(&path, head.as_str())?;
    }
    if !dir.join("targets.json").exists() {
        let (rule_file, policy_sha256) = rules(repo);
        let changed = known(git.name_status(base.as_str(), head.as_str()), "바뀐 파일")?;
        let targets = Targets {
            v: 1,
            key: key.clone(),
            base: base.clone(),
            merge_base: base.clone(),
            head: head.clone(),
            policy_sha256,
            files: changed
                .iter()
                .map(|c| target_file(&c.path, c.status, &rule_file))
                .collect(),
        };
        fullops_store::write_json(
            &dir.join("result.json"),
            &result_skeleton(&targets, implementer, &record.path),
        )?;
        let report = format!(
            "# {key} 리뷰 보고\n\n- base: {base}\n- head: {head}\n- snapshot: {}\n\n## 요약\n\n## 발견 사항\n\n## 결론\n",
            record.path
        );
        std::fs::write(dir.join("report.md"), report)?;
        if let Some(lint) = find_lint(&git, &head) {
            std::fs::write(dir.join("lint.json"), lint)?;
        }
        // targets.json을 마지막에 쓴다: 이 파일이 있으면 기록이 다 쓰인 것이다
        fullops_store::write_json(&dir.join("targets.json"), &targets)?;
    }
    ctx.update_state(None, |state| {
        for snapshot in state.snapshots.iter_mut().filter(|s| live(&&**s)) {
            snapshot.state = SnapshotState::Active;
        }
    })?;
    let folder = rel(repo, dir);
    ctx.print(
        &format!(
            "리뷰 snapshot을 만들었습니다: {}\n리뷰 폴더: {folder}\n다음: `fullops dispatch {key} --purpose review --sha {head}`, 리뷰 뒤 `fullops review check {key}`",
            record.path
        ),
        json!({ "review": review, "snapshot": record.path, "folder": folder, "base": base, "head": head }),
    );
    Ok(EXIT_OK)
}

/// 10번: 패킷 조건이 참이면 리뷰 head 커밋의 패킷·처리 기록·지시서로 `check_packet`을 부른다.
fn packet_failures(repo: &Repo, git: &GitCli, key: &TaskKey, subject: &Subject) -> Vec<Refusal> {
    let files = blobs(
        git,
        &subject.head,
        &[
            rel(repo, repo.packet_path(key)),
            rel(repo, repo.packet_outcomes_path(key)),
        ],
    );
    let packet = files[0].as_deref().and_then(parse_packet);
    let outcomes = files[1].as_deref().and_then(parse_packet_outcomes);
    // 읽지 못한 기록을 '없음'으로 내리면 패킷 검사를 건너뛴다
    let (Known::Yes(route), false, false) = (
        fullops_store::read_route(repo, key),
        files[0].is_some() && packet.is_none(),
        files[1].is_some() && outcomes.is_none(),
    ) else {
        return vec![Refusal {
            rule: "packet",
            reason: format!(
                "`routes/{key}.json`이나 리뷰 head의 `{key}-packet.json`, 패킷 처리 기록을 읽을 수 없다."
            ),
            escape: "기록 파일의 JSON을 고쳐 커밋한 뒤 다시 리뷰한다".into(),
            exit: 2,
        }];
    };
    if packet.is_none() && !route.as_ref().is_some_and(|r| r.requires_packet) {
        return Vec::new();
    }
    let role = packet
        .as_ref()
        .map(|p| p.role.clone())
        .or_else(|| subject.completion.as_ref()?.role.clone())
        .or_else(|| route.as_ref()?.role.clone());
    let Some(role) = role else {
        return Vec::new();
    };
    let round = packet.as_ref().map_or(0, |p| p.round);
    // 완료 기록 커밋 뒤에는 지시서가 logs로 옮겨져 있다
    let instruction = blobs(
        git,
        &subject.head,
        &[
            rel(repo, repo.inbox_path(&role)),
            rel(repo, repo.handover_log_path(key, &role, round)),
        ],
    )
    .iter()
    .find_map(text);
    fullops_core::check_packet(
        PacketCheck::Completion,
        route.as_ref(),
        packet.as_ref(),
        outcomes.as_ref(),
        instruction.as_deref(),
        key,
        &role,
        round,
        &mut |older| git.is_ancestor(older, subject.head.as_str()),
        &mut |_| Known::Unknown("review"),
    )
}

fn check(ctx: &Ctx, args: lexopt::Parser) -> Result<u8, CliError> {
    let parsed = Parsed::new(args, &["sha"], &[])?;
    let key = task_key(parsed.key.clone())?;
    let repo = ctx.repo()?;
    let git = GitCli::cli(repo.root.clone());
    let state = load_state(ctx)?;
    let subject = subject(ctx, &git, &state, &key, parsed.get("sha"))?;
    let dir = repo.review_dir(&key, &subject.head);
    let files: (Option<Targets>, Option<ReviewResult>) = (
        read(&dir.join("targets.json")),
        read(&dir.join("result.json")),
    );
    let (Some(targets), Some(mut result)) = files else {
        return Err(refused(
            "RV-00",
            "norecord",
            &[&rel(repo, dir), key.as_str()],
        ));
    };
    let mut failed: Vec<Refusal> = Vec::new();
    let mut fail = |item: usize, why: &str| {
        failed.push(msg::refusal(
            ITEMS.get(item).copied().unwrap_or("RV-00"),
            "",
            &[why],
        ))
    };
    if targets.base != subject.base || targets.head != subject.head || subject.stale {
        fail(1, "targets.json의 base·head가 지금 리뷰할 것과 다릅니다.");
    }
    let review = format!("{key}@{}", subject.head.short(12));
    // 구현자 세션은 snapshot을 만들 때 state에 적은 값이다. 그 기록이 정리됐을 때만 result.json의 값을 쓴다
    let snapshot = state.snapshots.iter().rev().find(|s| s.review == review);
    let implementer = match snapshot {
        Some(snapshot) => snapshot.implementer_session.clone(),
        None => result.implementer_session.clone(),
    };
    // 리뷰어가 자기 세션을 적지 않았으면 그 snapshot에 배정된 리뷰 worker의 세션(배정 기록)을 쓴다
    if result
        .reviewer_session
        .as_deref()
        .is_none_or(|s| s.trim().is_empty())
    {
        result.reviewer_session = snapshot.and_then(|snap| {
            state
                .dispatches
                .iter()
                .filter(|d| {
                    d.key == key
                        && d.purpose == Purpose::Review
                        && same_path(&d.worktree, &snap.path)
                })
                .max_by_key(|d| d.started_at)?
                .bound_session
                .as_ref()
                .map(|s| s.to_string())
        });
    }
    for (item, why) in check_result(&targets, &result, implementer.as_deref()) {
        fail(item as usize, &why);
    }
    let active = |s: &&Snapshot| s.review == review && s.state == SnapshotState::Active;
    if let Some(snapshot) = state.snapshots.iter().find(active) {
        let path = PathBuf::from(&snapshot.path);
        // HEAD 파일을 직접 읽는다: 브랜치를 체크아웃했으면 Detached가 아니다
        let head = fullops_store::discover(&path)
            .ok()
            .flatten()
            .and_then(|at| fullops_store::head(&at).ok());
        let clean = GitCli::cli(path).status(None).yes();
        if head != Some(HeadRef::Detached(subject.head.clone()))
            || !clean.is_some_and(|changes| changes.is_empty())
        {
            fail(
                5,
                "snapshot의 HEAD가 head와 다르거나 detached가 아니거나 깨끗하지 않습니다.",
            );
        }
    }
    if rules(repo).1 != targets.policy_sha256 {
        fail(8, "review/rule.json이 snapshot을 만든 뒤 바뀌었습니다.");
    }
    let lint_path = dir.join("lint.json");
    if !lint_path.exists()
        && let Some(bytes) = find_lint(&git, &subject.head)
    {
        std::fs::write(&lint_path, bytes)?;
    }
    match read::<LintResult>(&lint_path) {
        None => fail(
            9,
            "lint.json이 없습니다. worker 워크트리에서 `fullops lint`를 돌립니다.",
        ),
        Some(lint) if !covers(&git, &lint.head, &subject.head) => {
            fail(9, "lint.json의 head가 리뷰 head를 덮지 못합니다.")
        }
        Some(lint) => {
            // 검사한 범위가 리뷰 범위를 덮어야 한다. `lint --from HEAD`처럼 좁힌 결과는 받지 않는다
            let from = lint.merge_base.as_str();
            if lint.merge_base != targets.base
                && git.is_ancestor(from, targets.base.as_str()).yes() != Some(true)
            {
                fail(
                    9,
                    "lint가 리뷰 base부터 검사하지 않았습니다(lint.json의 merge_base가 리뷰 base나 그 조상이 아니다).",
                );
            }
            let at = blobs(&git, &targets.base, &[LINT_JSON.to_string()]);
            if lint.config_sha256 != config_sha256(at[0].as_deref(), assets::LINT_JSON) {
                fail(
                    9,
                    "lint가 기준 시점 설정으로 돌지 않았습니다(lint.json의 config_sha256이 리뷰 base의 설정과 다르다).",
                );
            }
            let config = text(&at[0]).unwrap_or_else(|| assets::LINT_JSON.to_string());
            let config: serde_json::Value = serde_json::from_str(&config).unwrap_or_default();
            let registered: Vec<String> = config["commands"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c["name"].as_str().map(str::to_string))
                .collect();
            if let Some(why) = check_lint(&lint, &registered) {
                fail(9, &why);
            }
        }
    }
    failed.extend(packet_failures(repo, &git, &key, &subject));
    if !failed.is_empty() {
        return Err(CliError::Refused(failed));
    }
    let accept = result.conclusion.as_deref() == Some("accept");
    if accept && accepted(&dir).is_none() {
        let hash = |name: &str| file_sha256(&dir.join(name)).unwrap_or_default();
        let record = Accepted {
            v: 1,
            key: key.clone(),
            base: targets.base.clone(),
            head: targets.head.clone(),
            result_sha256: hash("result.json"),
            lint_sha256: hash("lint.json"),
            reviewer_session: result.reviewer_session.clone().unwrap_or_default(),
            implementer_session: implementer,
            at: fullops_store::now(),
        };
        fullops_store::write_json(&dir.join("accepted.json"), &record)?;
    }
    let reviewed = result
        .files
        .iter()
        .filter(|f| f.review_status.as_deref() == Some("reviewed"))
        .count();
    let conclusion = result.conclusion.unwrap_or_default();
    let next = if accept {
        format!("accepted.json을 썼습니다. 다음: `fullops merge {key}`")
    } else {
        let role = subject.completion.as_ref().and_then(|c| c.role.as_ref());
        let design = design_class(repo, ctx.config()?, &key, role);
        format!(
            "결론이 reject라 수락 기록을 쓰지 않았습니다. 다음: 수정 배정({})",
            fullops_core::messages::flow::fix_round(
                key.as_str(),
                role.map(|r| r.as_str()),
                design.as_deref()
            )
        )
    };
    ctx.print(
        &format!(
            "기록 검사 통과: reviewed {reviewed}, skipped {}, 결론 {conclusion}. {next}",
            result.files.len() - reviewed
        ),
        json!({ "conclusion": conclusion, "accepted": accept, "reviewed": reviewed, "files": result.files.len() }),
    );
    Ok(EXIT_OK)
}

fn waive(ctx: &Ctx, args: lexopt::Parser) -> Result<u8, CliError> {
    let parsed = Parsed::new(args, &["decision", "reason"], &[])?;
    let key = task_key(parsed.key.clone())?;
    let (Some(id), Some(reason)) = (
        parsed.get("decision"),
        parsed.get("reason").filter(|r| !r.trim().is_empty()),
    ) else {
        return Err(CliError::Usage(
            "사용법: fullops review waive <KEY> --decision <ID> --reason \"<사용자 지시 요지>\""
                .into(),
        ));
    };
    let repo = ctx.repo()?;
    let git = GitCli::cli(repo.root.clone());
    let state = load_state(ctx)?;
    let subject = subject(ctx, &git, &state, &key, None)?;
    let answered = |d: &&fullops_core::model::Decision| {
        d.id.as_str() == id
            && d.status == fullops_core::model::DecisionStatus::Answered
            && d.sha.as_ref() == Some(&subject.head)
    };
    let Some(decision) = state.decisions.iter().find(answered) else {
        return Err(refused("RV-00", "decision", &[id, subject.head.short(12)]));
    };
    let dir = repo.review_dir(&key, &subject.head);
    let path = dir.join("waiver.json");
    if path.exists() {
        return Err(refused("RV-00", "exists", &[&rel(repo, path)]));
    }
    let (need, _) = need_for(&git, &subject.base, &subject.head)?;
    let via = decision.via.unwrap_or(fullops_core::model::Origin::User);
    let human_turn = decision.extra.get("human_turn") == Some(&json!(true));
    if !human_turn {
        ctx.note(&format!(
            "결정 {id}의 답은 사람 턴을 확인하지 못한 채 기록된 것입니다. 면제 기록에 그대로 남깁니다."
        ));
    }
    let record = Waiver {
        v: 1,
        key: key.clone(),
        base: subject.base.clone(),
        head: subject.head.clone(),
        reason: reason.to_string(),
        decision: decision.id.clone(),
        decided_by: decision
            .decided_by
            .clone()
            .unwrap_or_else(|| via.to_string()),
        via: Some(via),
        human_turn,
        need,
        at: fullops_store::now(),
    };
    fullops_store::write_json(&path, &record)?;
    // 진행 중이던 리뷰는 닫는다. 면제는 수락이 아니다
    let review = format!("{key}@{}", subject.head.short(12));
    let open = |s: &Snapshot| {
        s.review == review && matches!(s.state, SnapshotState::Creating | SnapshotState::Active)
    };
    if state.snapshots.iter().any(open)
        && let Some(why) = cleanup(ctx, &git, &review, Some("사용자 결정으로 리뷰를 면제했다"))?
    {
        ctx.note(&format!(
            "snapshot {review}를 정리하지 못해 사유를 적었습니다: {why}"
        ));
    }
    ctx.print(
        &format!(
            "면제 기록을 썼습니다: {}. 면제는 수락이 아닙니다. 다음: `fullops merge {key}`",
            rel(repo, path)
        ),
        json!({ "key": key, "head": subject.head, "decision": id }),
    );
    Ok(EXIT_OK)
}

/// snapshot에 살아 있는 터미널. 없어질 때까지 최대 8초 다시 본다.
fn live_terminals(repo: &Repo, path: &Path) -> Result<Vec<String>, String> {
    let orca = OrcaCli::new(repo.root.clone(), Duration::from_secs(10));
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match orca.terminals_in(path) {
            Ok(live) if live.is_empty() || Instant::now() >= deadline => return Ok(live),
            Ok(_) => std::thread::sleep(Duration::from_secs(1)),
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// snapshot을 지운다(설계 8.9절 cleanup). 지우지 못하면 snapshot의 `hold`에 사유를 쓰고 그 사유를 돌려준다.
/// `abandon`은 중단 사유다. 있으면 `abandoned`로 닫고 `abandoned.json`을 쓴다.
pub(super) fn cleanup(
    ctx: &Ctx,
    git: &GitCli,
    review: &str,
    abandon: Option<&str>,
) -> Result<Option<String>, CliError> {
    let repo = ctx.repo()?;
    let open = |s: &Snapshot| {
        s.review == review && matches!(s.state, SnapshotState::Creating | SnapshotState::Active)
    };
    let state = load_state(ctx)?;
    let Some(snapshot) = state.snapshots.iter().find(|s| open(s)) else {
        return Err(refused("RV-00", "nosnapshot", &[review]));
    };
    let path = PathBuf::from(&snapshot.path);
    let problem = if !path.exists() {
        None
    } else {
        match live_terminals(repo, &path) {
            Err(e) => Some(format!("터미널을 확인하지 못했습니다({e}).")),
            Ok(live) if !live.is_empty() => {
                Some(format!("살아 있는 터미널이 있습니다: {}.", live.join(", ")))
            }
            Ok(_) => {
                let argv = [
                    "git",
                    "status",
                    "--porcelain",
                    "--untracked-files=all",
                    "--ignored",
                ];
                let argv: Vec<String> = argv.iter().map(|a| a.to_string()).collect();
                let status = fullops_ext::run(&argv, &path, Duration::from_secs(30), &[], None);
                if !status.ok() {
                    Some("snapshot의 상태를 읽지 못했습니다.".into())
                } else if !status.stdout.is_empty() {
                    Some("snapshot에 미커밋·미추적·무시 파일이 있습니다.".into())
                } else {
                    git.worktree_remove(&path)
                        .err()
                        .map(|e| format!("git worktree remove가 실패했습니다({e})."))
                }
            }
        }
    };
    let now = fullops_store::now();
    ctx.update_state(None, |state| {
        for snapshot in state.snapshots.iter_mut().filter(|s| open(s)) {
            match &problem {
                Some(reason) => {
                    snapshot.hold = Some(SnapshotHold {
                        reason: reason.clone(),
                        at: now,
                        extra: Default::default(),
                    })
                }
                None => {
                    snapshot.hold = None;
                    snapshot.state = match abandon {
                        Some(_) => SnapshotState::Abandoned,
                        None => SnapshotState::Removed,
                    };
                }
            }
        }
    })?;
    if let (None, Some(reason), Ok(key)) = (&problem, abandon, TaskKey::new(snapshot.key())) {
        let record =
            json!({ "v": 1, "key": key, "head": snapshot.head, "reason": reason, "at": now });
        fullops_store::write_json(
            &repo.review_dir(&key, &snapshot.head).join("abandoned.json"),
            &record,
        )?;
    }
    Ok(problem)
}

fn cleanup_command(ctx: &Ctx, args: lexopt::Parser) -> Result<u8, CliError> {
    let parsed = Parsed::new(args, &["sha", "reason"], &["finished", "abandon"])?;
    let key = task_key(parsed.key.clone())?;
    let reason = parsed.get("reason").filter(|r| !r.trim().is_empty());
    let abandon = match (parsed.flag("finished"), parsed.flag("abandon"), reason) {
        (true, false, None) => None,
        (false, true, Some(reason)) => Some(reason),
        _ => {
            return Err(CliError::Usage(
                "사용법: fullops review cleanup <KEY> [--sha <SHA>] (--finished | --abandon --reason \"<사유>\")".into(),
            ));
        }
    };
    let git = GitCli::cli(ctx.repo()?.root.clone());
    let state = load_state(ctx)?;
    let wanted = match parsed.get("sha") {
        Some(rev) => {
            let head = commit(&git, rev)
                .ok_or_else(|| CliError::Usage(format!("`{rev}`를 커밋으로 풀지 못했습니다.")))?;
            Some(format!("{key}@{}", head.short(12)))
        }
        None => None,
    };
    let open = |s: &&Snapshot| {
        matches!(s.state, SnapshotState::Creating | SnapshotState::Active)
            && s.key() == key.as_str()
            && wanted.as_ref().is_none_or(|w| &s.review == w)
    };
    let mut found = state.snapshots.iter().filter(open);
    let (Some(snapshot), None) = (found.next(), found.next()) else {
        return Err(refused("RV-00", "nosnapshot", &[key.as_str()]));
    };
    if let Some(why) = cleanup(ctx, &git, &snapshot.review, abandon)? {
        return Err(refused("RV-00", "held", &[&snapshot.review, &why]));
    }
    ctx.print(
        &format!("snapshot {}를 정리했습니다.", snapshot.review),
        json!({ "review": snapshot.review, "abandoned": abandon.is_some() }),
    );
    Ok(EXIT_OK)
}

fn supersede(ctx: &Ctx, args: lexopt::Parser) -> Result<u8, CliError> {
    let parsed = Parsed::new(args, &["old", "new"], &[])?;
    let key = task_key(parsed.key.clone())?;
    let prefix = |name: &str| {
        let hex = |v: &&str| v.len() == 12 && v.bytes().all(|b| b.is_ascii_hexdigit());
        parsed.get(name).filter(hex).ok_or_else(|| {
            CliError::Usage(
                "사용법: fullops review supersede <KEY> --old <head 12자> --new <head 12자>".into(),
            )
        })
    };
    let (old, new) = (prefix("old")?, prefix("new")?);
    let repo = ctx.repo()?;
    let root = repo.squad().join("reviews").join(key.as_str());
    let (old_dir, new_dir) = (root.join(old), root.join(new));
    let no = |why: &str| refused("RV-00", "supersede", &[old, new, why]);
    let Some(targets) = read::<Targets>(&old_dir.join("targets.json")) else {
        return Err(no("옛 리뷰 기록이 없습니다."));
    };
    let Some(newer) = accepted(&new_dir) else {
        return Err(no("새 리뷰의 accepted.json이 없거나 유효하지 않습니다."));
    };
    let git = GitCli::cli(repo.root.clone());
    if targets.head == newer.head
        || git
            .is_ancestor(targets.head.as_str(), newer.head.as_str())
            .yes()
            != Some(true)
    {
        return Err(no("새 head가 옛 head의 후손이 아닙니다."));
    }
    let path = old_dir.join("superseded.json");
    if path.exists() {
        return Err(refused("RV-00", "exists", &[&rel(repo, path)]));
    }
    let record = json!({
        "by": newer.head,
        "old_result_sha256": file_sha256(&old_dir.join("result.json")),
        "new_result_sha256": newer.result_sha256,
        "at": fullops_store::now(),
    });
    fullops_store::write_json(&path, &record)?;
    ctx.print(
        &format!("{key} 리뷰 {old}를 {new}로 대체했다고 적었습니다."),
        json!({ "key": key, "old": old, "new": new }),
    );
    Ok(EXIT_OK)
}

fn prune(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    if let Some(arg) = args.next()? {
        return Err(arg.unexpected().into());
    }
    let git = GitCli::cli(ctx.repo()?.root.clone());
    let registered = known(git.worktrees(), "워크트리 목록")?;
    // S-03과 같다: 경로와 Git 등록이 모두 사라진 snapshot을 닫는다. 있는 워크트리는 지우지 않는다
    let gone = |s: &Snapshot| {
        matches!(s.state, SnapshotState::Creating | SnapshotState::Active)
            && !Path::new(&s.path).exists()
            && !registered
                .iter()
                .any(|w| same_path(&path_text(&w.path), &s.path))
    };
    let state = ctx.update_state(None, |state| {
        for snapshot in state.snapshots.iter_mut().filter(|s| gone(s)) {
            snapshot.state = SnapshotState::Missing;
        }
        state.clone()
    })?;
    let active: Vec<&Snapshot> = state
        .snapshots
        .iter()
        .filter(|s| s.state == SnapshotState::Active)
        .collect();
    let lines: Vec<String> = active
        .iter()
        .map(|s| {
            let hold = s
                .hold
                .as_ref()
                .map(|h| format!(" (정리 보류: {})", h.reason));
            format!("{} {}{}", s.review, s.path, hold.unwrap_or_default())
        })
        .collect();
    let human = if lines.is_empty() {
        "열린 snapshot이 없습니다.".to_string()
    } else {
        lines.join("\n")
    };
    ctx.print(&human, json!({ "active": active }));
    Ok(EXIT_OK)
}
