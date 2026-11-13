//! `fullops dispatch`: 선행 조건 검사, 준비 커밋, 동기화, Orca 배정(설계 8.5절). worker가 부르면 하위 배정이다(8.15절).
//! 흐름 명령이 함께 쓰는 CLI 쪽 `Facts`와 작은 도우미도 여기에 둔다.

use crate::cli::{Caller, CliError, Ctx, EXIT_OK, session_handle};
use fullops_core::config::PolicyError;
use fullops_core::facts::Facts;
use fullops_core::flow::{self, ReviewSpec, SpecInput, refusal};
use fullops_core::messages;
use fullops_core::model::{
    Actor, Caps, ChangeSet, Config, Decision, DecisionId, DecisionStatus, Dispatch, DispatchId,
    DispatchReason, DispatchRequest, Extra, HeadRef, HostId, Inbox, Known, Landing, ModelTuple,
    Need, Origin, PacketCheck, Plan, Purpose, Refusal, ReviewEvidence, RoleId, Route, RouteTuple,
    Run, RunId, RunOwner, Session, SessionMode, Sha, SkillText, SnapshotState, State, TaskKey,
    TestLevel, caps_of, orca_wakes, path_text, same_path, sha256_hex,
};
use fullops_ext::{
    ExtError, Git, GitCli, MergeMode, MergeOutcome, Orca, OrcaCli, StartOutcome, StartRequest,
    StartTarget,
};
use fullops_store::Repo;
use lexopt::prelude::*;
use serde_json::json;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 옵션의 값 하나.
pub(super) fn value(args: &mut lexopt::Parser) -> Result<String, CliError> {
    Ok(args.value()?.string()?)
}

pub(super) fn usage(text: impl Into<String>) -> CliError {
    CliError::Usage(text.into())
}

/// 원본 체크아웃: Git 공용 디렉터리가 있는 주 체크아웃이다. 사람이 `.env*`를 두는 자리다.
pub(super) fn main_checkout(repo: &Repo) -> PathBuf {
    repo.common_dir
        .parent()
        .map_or_else(|| repo.root.clone(), Path::to_path_buf)
}

/// 그 브랜치를 체크아웃한 워크트리.
pub(super) fn worktree_of(git: &GitCli, branch: &str) -> Option<PathBuf> {
    git.worktrees()
        .yes()?
        .into_iter()
        .find(|w| w.branch.as_deref() == Some(branch))
        .map(|w| w.path)
}

/// 깨끗한가(미추적 포함, 무시 파일 제외). 읽지 못하면 더러운 것으로 본다.
pub(super) fn is_clean(root: &Path) -> bool {
    GitCli::cli(root.to_path_buf())
        .status(None)
        .yes()
        .is_some_and(|entries| entries.is_empty())
}

/// CLI 검사가 쓰는 `Facts`(설계 10.2절). 호출자는 `Ctx::caller`의 결과이고 경로는 이 체크아웃의 루트 기준이다.
/// git이 필요한 사실(`changed`·`landing` 등)은 `Unknown`이다: 그것을 쓰는 검사는 명령이 직접 한다.
pub(super) struct CliFacts<'a> {
    repo: &'a Repo,
    config: &'a Config,
    actor: Actor,
    session: Option<Session>,
    state: Known<State>,
    archive: Known<BTreeSet<TaskKey>>,
    plan: Known<Plan>,
}

impl<'a> CliFacts<'a> {
    pub(super) fn new(ctx: &'a Ctx, caller: &Caller) -> Result<Self, CliError> {
        let repo = ctx.repo()?;
        Ok(Self {
            repo,
            config: ctx.config()?,
            actor: caller.actor.clone(),
            session: caller.session.clone(),
            state: fullops_store::read_state(repo),
            archive: fullops_store::archive_keys(repo),
            plan: fullops_store::read_plan(repo, crate::assets::PLANS_MD),
        })
    }
}

const CLI_ONLY: &str = "cli";

impl Facts for CliFacts<'_> {
    fn now(&self) -> i64 {
        fullops_store::now()
    }
    fn host(&self) -> HostId {
        self.session.as_ref().map_or(HostId::Claude, |s| s.host)
    }
    fn caps(&self) -> Caps {
        caps_of(self.host())
    }
    fn config(&mut self) -> Result<&Config, PolicyError> {
        Ok(self.config)
    }
    fn policy_error(&mut self) -> Option<PolicyError> {
        None
    }
    fn actor(&mut self) -> Known<Actor> {
        Known::Yes(self.actor.clone())
    }
    fn session(&mut self) -> Known<&Session> {
        self.session
            .as_ref()
            .map_or(Known::Unknown("no_session"), Known::Yes)
    }
    fn state(&mut self) -> Known<&State> {
        self.state.as_ref()
    }
    fn archive_keys(&mut self) -> Known<&BTreeSet<TaskKey>> {
        self.archive.as_ref()
    }
    fn plan(&mut self) -> Known<&Plan> {
        self.plan.as_ref()
    }
    fn inbox(&mut self, role: &RoleId) -> Known<Option<Inbox>> {
        fullops_store::read_inbox(self.repo, role)
    }
    fn route(&mut self, key: &TaskKey) -> Known<Option<Route>> {
        fullops_store::read_route(self.repo, key)
    }
    fn file_text(&mut self, path: &Path) -> Known<Option<String>> {
        fullops_store::read_text(&self.repo.root.join(path))
    }
    fn path_exists(&mut self, path: &Path) -> Known<bool> {
        Known::Yes(self.repo.root.join(path).exists())
    }
    fn changed(&mut self) -> Known<ChangeSet> {
        Known::Unknown(CLI_ONLY)
    }
    fn pass_covers(&mut self, _head: &Sha, _start: &Sha) -> Known<bool> {
        Known::Unknown(CLI_ONLY)
    }
    fn head(&mut self) -> Known<Sha> {
        GitCli::cli(self.repo.root.clone()).rev_parse("HEAD")
    }
    fn review_need(&mut self, _base: &Sha, _head: &Sha) -> Known<Need> {
        Known::Unknown(CLI_ONLY)
    }
    fn review_evidence(&mut self, _head: &Sha) -> Known<ReviewEvidence> {
        Known::Unknown(CLI_ONLY)
    }
    fn landing(&mut self, _sha: &Sha) -> Known<Landing> {
        Known::Unknown(CLI_ONLY)
    }
    fn qa_commit_candidates(&mut self, _include_unstaged: bool) -> Known<Vec<String>> {
        Known::Unknown(CLI_ONLY)
    }
    fn orca_settled(&mut self, _id: &DispatchId) -> Known<bool> {
        Known::Unknown(CLI_ONLY)
    }
    fn waiter_alive(&mut self, _run: &RunId) -> Known<bool> {
        Known::Unknown(CLI_ONLY)
    }
    fn session_handle(&self) -> Option<String> {
        session_handle()
    }
    fn user_skill(&mut self, _name: &str) -> Known<Option<SkillText>> {
        Known::Unknown(CLI_ONLY)
    }
}

/// 규칙 ID가 없는 단계의 거절(기록 게시, 동기화, `wait`의 Run 고르기). 출력은 `FullOps[<단계>]`다.
pub(super) fn step_refusal(step: &'static str, reason: String, escape: &str) -> CliError {
    CliError::Refused(vec![Refusal {
        rule: step,
        reason,
        escape: escape.to_string(),
        exit: 2,
    }])
}

fn squad_path(rest: &str) -> String {
    format!(".fullops-squad/{rest}")
}

/// 3단계 기록 게시(설계 8.5절): lead의 준비 커밋을 기본 브랜치와 원격에 올린다. 이미 된 일은 건너뛴다.
fn publish_records(repo: &Repo, config: &Config, git: &GitCli) -> Result<(), CliError> {
    let base_ref = format!("refs/heads/{}", config.base);
    let on_base = matches!(fullops_store::head(repo)?, HeadRef::Branch(b) if b == config.base);
    let head = git.rev_parse("HEAD").yes();
    let base_tip = git.rev_parse(&base_ref).yes();
    if let (false, Some(head), Some(base_tip)) = (on_base, &head, &base_tip)
        && head != base_tip
    {
        if git.is_ancestor(base_tip.as_str(), head.as_str()) != Known::Yes(true) {
            return Err(step_refusal(
                "publish",
                format!(
                    "lead의 브랜치가 기본 브랜치 `{}`를 담지 않는다.",
                    config.base
                ),
                "`fullops sync --self`",
            ));
        }
        match worktree_of(git, &config.base) {
            Some(path) if !is_clean(&path) => {
                return Err(step_refusal(
                    "publish",
                    format!(
                        "기본 브랜치 워크트리({})에 커밋하지 않은 변경이 있다.",
                        path_text(&path)
                    ),
                    "그 워크트리의 변경을 커밋하거나 치운 뒤 다시 실행한다",
                ));
            }
            Some(path) => {
                git.merge(&path, head.as_str(), MergeMode::FfOnly)?;
            }
            None => git.update_ref(&base_ref, head, base_tip)?,
        }
    }
    // 원격이 이미 담고 있으면 올릴 것이 없다. 원격이 앞서 있는 경우는 D-11이 알린다
    if let Some(remote) = &config.remote
        && git.is_ancestor(&base_ref, &config.base_ref()) != Known::Yes(true)
    {
        git.push(remote, &config.base)?;
    }
    Ok(())
}

/// 4단계 대상 워크트리 동기화. 실패하면 `sync_pending`에 적고 멈춘다.
fn sync_target(
    ctx: &Ctx,
    config: &Config,
    role: &RoleId,
    target: &Path,
    base_tip: &Sha,
) -> Result<(), CliError> {
    let git = GitCli::cli(target.to_path_buf());
    if git.is_ancestor(base_tip.as_str(), "HEAD") == Known::Yes(true) {
        return Ok(());
    }
    let pending = |ctx: &Ctx| {
        ctx.update_state(None, |s| {
            s.sync_pending.insert(role.clone(), base_tip.clone());
        })
    };
    if !is_clean(target) {
        pending(ctx)?;
        return Err(step_refusal(
            "sync",
            format!(
                "`{role}` 워크트리에 커밋하지 않은 변경이 있어 기본 브랜치 `{}`를 반영하지 못했다.",
                config.base
            ),
            &format!("그 worker의 완료 뒤 `fullops sync --role {role}`"),
        ));
    }
    let merged = match git.merge(target, base_tip.as_str(), MergeMode::FfOnly) {
        Ok(outcome) => outcome,
        Err(_) => git.merge(target, base_tip.as_str(), MergeMode::NoEdit)?,
    };
    if merged == MergeOutcome::Conflict {
        git.merge_abort(target)?;
        pending(ctx)?;
        return Err(CliError::Human {
            decision: None,
            message: format!(
                "`{role}` 워크트리에 기본 브랜치를 병합하다 충돌했다. 병합은 되돌렸다. 충돌을 풀고 `fullops sync --role {role}`"
            ),
        });
    }
    ctx.update_state(None, |s| {
        s.sync_pending.remove(role);
    })
}

/// 패킷 조건이 참일 때의 `check_packet`. 걸린 J 규칙을 한 줄로 돌려준다(D-12와 R-10이 함께 쓴다).
pub(super) fn packet_findings(
    target: &Repo,
    mode: PacketCheck,
    key: &TaskKey,
    role: &RoleId,
) -> Option<String> {
    let route = fullops_store::read_route(target, key).yes().flatten();
    let packet = fullops_store::read_packet(target, key).yes().flatten();
    if packet.is_none() && !route.as_ref().is_some_and(|r| r.requires_packet) {
        return None;
    }
    let outcomes = fullops_store::read_packet_outcomes(target, key)
        .yes()
        .flatten();
    let instruction = fullops_store::read_text(&target.inbox_path(role))
        .yes()
        .flatten();
    let round = fullops_store::read_inbox(target, role)
        .yes()
        .flatten()
        .map_or(0, |i| i.round);
    let git = GitCli::cli(target.root.clone());
    let found = fullops_core::check_packet(
        mode,
        route.as_ref(),
        packet.as_ref(),
        outcomes.as_ref(),
        instruction.as_deref(),
        key,
        role,
        round,
        &mut |head| git.is_ancestor(head, "HEAD"),
        &mut |path| fullops_store::read_text(&target.root.join(path)),
    );
    (!found.is_empty()).then(|| {
        found
            .iter()
            .map(|r| format!("{} {}", r.rule, r.reason))
            .collect::<Vec<_>>()
            .join(" / ")
    })
}

/// grok의 신뢰 목록에 원본 체크아웃이나 그 상위 폴더가 있는가(읽기만 한다. 설계 8.5절 7단계).
fn grok_trusted(checkout: &Path) -> bool {
    let home = std::env::var_os("GROK_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| fullops_store::home().map(|h| h.join(".grok")));
    let Some(text) =
        home.and_then(|h| std::fs::read_to_string(h.join("trusted_folders.toml")).ok())
    else {
        return false;
    };
    let Ok(table) = text.parse::<toml::Table>() else {
        return false;
    };
    let folders = table.get("folders").and_then(|f| f.as_table());
    folders.is_some_and(|folders| {
        folders.iter().any(|(path, entry)| {
            entry.get("trusted").and_then(|t| t.as_bool()) == Some(true)
                && checkout
                    .ancestors()
                    .any(|dir| same_path(&path_text(dir), path))
        })
    })
}

/// 인박스 본문의 lint 명령 줄에 적힌 `--level` 가운데 가장 높은 것.
fn lint_level_in(inbox: &str) -> Option<TestLevel> {
    inbox
        .lines()
        .filter(|line| line.contains("fullops lint"))
        .filter_map(|line| {
            let (_, rest) = line.split_once("--level")?;
            let word = rest
                .trim_start_matches(['=', ' '])
                .split(|c: char| !c.is_ascii_alphabetic())
                .next()?;
            TestLevel::parse(word)
        })
        .max()
}

fn new_request() -> String {
    let seed = format!("{:?}{}", std::time::SystemTime::now(), std::process::id());
    format!("req_{}", &sha256_hex(seed.as_bytes())[..16])
}

struct Options {
    req: DispatchRequest,
    why: Option<String>,
    dry_run: bool,
}

fn parse(mut args: lexopt::Parser) -> Result<Options, CliError> {
    let (mut key, mut role, mut purpose) = (None, None, Purpose::Build);
    let (mut run, mut terminal, mut sha, mut reason, mut brief) = (None, None, None, None, None);
    let (mut agent, mut model, mut effort, mut why, mut dry_run) = (None, None, None, None, false);
    while let Some(arg) = args.next()? {
        match arg {
            Value(v) if key.is_none() => key = Some(v.string()?),
            Long("role") => role = Some(value(&mut args)?),
            Long("purpose") => {
                let word = value(&mut args)?;
                purpose = Purpose::parse(&word).ok_or_else(|| {
                    usage(format!(
                        "모르는 목적이다: {word}. build|fix|review|research"
                    ))
                })?;
            }
            Long("run") => run = Some(value(&mut args)?),
            Long("terminal") => terminal = Some(value(&mut args)?),
            Long("agent") => agent = Some(value(&mut args)?),
            Long("model") => model = Some(value(&mut args)?),
            Long("effort") => effort = Some(value(&mut args)?),
            Long("why") => why = Some(value(&mut args)?),
            Long("sha") => sha = Some(value(&mut args)?),
            Long("reason") => {
                let word = value(&mut args)?;
                reason = Some(
                    DispatchReason::parse(&word)
                        .ok_or_else(|| usage(format!("모르는 이유다: {word}. limit")))?,
                );
            }
            Long("brief") => brief = Some(value(&mut args)?),
            Long("dry-run") => dry_run = true,
            other => return Err(other.unexpected().into()),
        }
    }
    let key = key.ok_or_else(|| usage("사용법: fullops dispatch <KEY> [--role <R>] …"))?;
    let tuple = match (agent, model, effort) {
        (Some(agent), Some(model), Some(effort)) => Some(ModelTuple {
            agent,
            model,
            effort,
        }),
        (None, None, None) => None,
        _ => {
            return Err(usage(
                "`--agent`·`--model`·`--effort`는 셋 다 주거나 다 주지 않는다.",
            ));
        }
    };
    let bad = |e: fullops_core::model::IdError| usage(e.to_string());
    Ok(Options {
        req: DispatchRequest {
            key: TaskKey::new(key).map_err(bad)?,
            role: role.map(RoleId::new).transpose().map_err(bad)?,
            purpose,
            run: run.map(RunId::new).transpose().map_err(bad)?,
            terminal,
            tuple,
            sha,
            reason,
            brief,
            sub: false,
        },
        why,
        dry_run,
    })
}

/// D-07의 횟수 초과: 결정을 한 번만 연다.
fn open_limit_decision(ctx: &Ctx, key: &TaskKey, origin: Origin) -> Result<DecisionId, CliError> {
    let question = format!("{key} 한도 재배정 2회 초과");
    ctx.update_state(None, |s| {
        let open = |d: &&Decision| d.status == DecisionStatus::Open && d.question == question;
        if let Some(found) = s.decisions.iter().find(open) {
            return found.id.clone();
        }
        let id = DecisionId::from_number(s.next_decision);
        s.next_decision += 1;
        s.decisions.push(Decision {
            id: id.clone(),
            status: DecisionStatus::Open,
            key: Some(key.clone()),
            question: question.clone(),
            options: vec!["같은 모델로 다시 배정".into(), "중단".into()],
            blocks: vec![key.to_string()],
            asked_at: fullops_store::now(),
            origin,
            sha: None,
            risks: Vec::new(),
            allow: Vec::new(),
            expires_at: None,
            answer: None,
            decided_by: None,
            via: None,
            closed_at: None,
            repository: None,
            drop_reason: None,
            extra: Extra::new(),
        });
        id
    })
}

pub fn run(ctx: &Ctx, args: lexopt::Parser) -> Result<u8, CliError> {
    let Options {
        mut req,
        why,
        dry_run,
    } = parse(args)?;
    let (repo, config) = (ctx.repo()?, ctx.config()?);
    let caller = ctx.caller()?;
    req.sub = caller.actor.dispatch.is_some();
    if !req.sub && req.brief.is_some() {
        return Err(usage(
            "`--brief`는 dispatch를 받은 세션의 하위 배정에만 쓴다. lead의 지시는 인박스에 적는다.",
        ));
    }
    let mut facts = CliFacts::new(ctx, &caller)?;
    let mut refused = flow::check_dispatch(&req, &mut facts);
    let found = flow::dispatch_defaults(&req, &mut facts);
    let state = fullops_store::read_state(repo).yes().unwrap_or_default();
    let route = fullops_store::read_route(repo, &req.key).yes().flatten();
    let git = GitCli::cli(repo.root.clone());
    let key = req.key.clone();
    let review = req.purpose == Purpose::Review;

    // 대상 워크트리. 하위 배정은 호출자 자신의 워크트리, 리뷰는 그 리뷰의 snapshot이다
    let snapshot = state.snapshots.iter().find(|s| {
        s.state == SnapshotState::Active
            && s.key() == key.as_str()
            && Some(s.head.as_str()) == req.sha.as_deref()
    });
    let target = if req.sub {
        Some(repo.root.clone())
    } else if review {
        snapshot.map(|s| PathBuf::from(&s.path))
    } else {
        found
            .role
            .as_ref()
            .and_then(|r| config.roles.get(r))
            .and_then(|r| worktree_of(&git, &r.branch))
    };
    if let (true, Some(path), Some(snapshot)) = (review, &target, snapshot) {
        let snap = GitCli::cli(path.clone());
        let detached = fullops_store::discover(path)
            .ok()
            .flatten()
            .and_then(|r| fullops_store::head(&r).ok())
            .is_some_and(|h| matches!(h, HeadRef::Detached(_)));
        let problem = if snap.rev_parse("HEAD").yes().as_ref() != Some(&snapshot.head) {
            Some("배정할 SHA와 다른 커밋에 있다")
        } else if !detached {
            Some("detached가 아니다")
        } else if !is_clean(path) {
            Some("깨끗하지 않다")
        } else {
            None
        };
        if let Some(problem) = problem {
            refused.push(refusal(
                "D-06",
                &["state", problem],
                &format!("`fullops review snapshot {key}`"),
                2,
            ));
        }
    }
    if let (false, false, None, Some(role)) = (req.sub, review, &target, &found.role)
        && config.roles.contains_key(role)
    {
        refused.push(refusal(
            "D-05",
            &["worktree", role.as_str()],
            "Orca로 역할 워크트리를 만든다(bootstrap)",
            2,
        ));
    }
    if !refused.is_empty() {
        // D-07의 횟수 초과는 결정을 열고 사람에게 올린다
        if refused.iter().any(|r| r.rule == "D-07" && r.exit == 3) {
            let origin = route.as_ref().map_or(Origin::User, |r| r.origin);
            let decision = open_limit_decision(ctx, &key, origin)?;
            if let [only] = refused.as_slice() {
                return Err(CliError::Human {
                    decision: Some(decision.clone()),
                    message: format!(
                        "{} (결정 {decision})",
                        messages::refusal(only.rule, &only.reason, &only.escape)
                    ),
                });
            }
        }
        return Err(CliError::Refused(refused));
    }
    let (Some(role), Some(tuple), Some(target)) = (found.role.clone(), found.tuple.clone(), target)
    else {
        return Err(CliError::Internal("배정 대상을 정하지 못했다.".into()));
    };
    let parent = found.parent.as_ref();
    let work = matches!(req.purpose, Purpose::Build | Purpose::Fix);
    let lead_inbox = fullops_store::read_inbox(repo, &role).yes().flatten();
    let base_ref = format!("refs/heads/{}", config.base);

    // spec의 재료. 인박스는 준비 커밋과 동기화로 대상 워크트리에 간다
    let handover = (work && lead_inbox.is_some())
        .then(|| path_text(&target.join(squad_path(&format!("handovers/to_{role}.md")))));
    let base = lead_inbox
        .as_ref()
        .map(|i| i.base.to_string())
        .or_else(|| git.rev_parse(&base_ref).yes().map(|s| s.to_string()))
        .unwrap_or_default();
    let read_only = flow::read_only(&req, config.parallel, caller.actor.direct_dev);
    let review_folder = snapshot.map(|s| path_text(&repo.review_dir(&key, &s.head)));
    let fixed_bin = fullops_store::fixed_bin().map_or("fullops".to_string(), |p| path_text(&p));
    let spec_of = |packet: bool| {
        flow::spec(&SpecInput {
            key: &key,
            purpose: req.purpose,
            role: &role,
            handover: handover.as_deref(),
            base: &base,
            test_level: config.test_level,
            parallel: config.parallel,
            packet: packet
                .then(|| squad_path(&format!("docs/evaluations/jev/{key}-packet.json")))
                .as_deref(),
            request: (handover.is_none() && work && !req.sub)
                .then(|| route.as_ref().and_then(|r| r.request_file.as_deref()))
                .flatten(),
            review: match (snapshot, &review_folder) {
                (Some(s), Some(folder)) => Some(ReviewSpec {
                    sha: s.head.as_str(),
                    implementer: s.implementer_session.as_deref().unwrap_or("unknown"),
                    folder,
                }),
                _ => None,
            },
            read_only,
            sub: parent
                .zip(req.brief.as_deref())
                .map(|(p, brief)| (&p.id, brief)),
            agent: &tuple.agent,
            fixed_bin: &fixed_bin,
        })
    };
    let new_terminal = req.terminal.is_none();
    let effort = flow::orca_effort(&tuple, config.parallel, new_terminal);
    if dry_run {
        let spec = spec_of(
            fullops_store::read_packet(repo, &key)
                .yes()
                .flatten()
                .is_some(),
        );
        ctx.print(
            &format!(
                "배정 계획(실행하지 않는다)\n대상 워크트리: {}\nagent/model/effort: {}/{}/{effort}\nspec: {spec}",
                path_text(&target),
                tuple.agent,
                tuple.model
            ),
            json!({"dry_run": true, "key": key, "role": role, "worktree": path_text(&target), "spec": spec}),
        );
        return Ok(EXIT_OK);
    }

    let ready_timeout = Duration::from_secs(config.wait.ready_timeout_seconds);
    let orca = OrcaCli::new(repo.root.clone(), ready_timeout + Duration::from_secs(30));
    let handle = session_handle();

    // 1. Run 정하기. 만든 Run은 바로 적어 다시 실행해도 같은 Run을 쓴다
    let owner = match parent {
        Some(p) => RunOwner::Dispatch(p.id.clone()),
        None => RunOwner::Lead,
    };
    let own_run = state
        .runs
        .iter()
        .filter(|r| r.closed_at.is_none() && r.owner == owner)
        .max_by_key(|r| r.opened_at)
        .map(|r| r.id.clone());
    let run = match req.run.clone().or(own_run) {
        Some(run) => run,
        None => {
            let goal: String = lead_inbox
                .as_ref()
                .map(|i| format!(": {}", i.goal.chars().take(80).collect::<String>()))
                .unwrap_or_default();
            orca.run_create(&format!("fullops {key}{goal}"))?
        }
    };
    let add_run = |s: &mut State| {
        if !s.runs.iter().any(|r| r.id == run) {
            s.runs.push(Run {
                id: run.clone(),
                owner: owner.clone(),
                coordinator: handle.clone(),
                opened_at: fullops_store::now(),
                closed_at: None,
                extra: Extra::new(),
            });
        }
    };
    ctx.update_state(None, add_run)?;

    let mut packet = false;
    if !req.sub {
        // 2. 준비 커밋. tuple 기록이 이 커밋에 들어가도록 먼저 쓴다
        if let Some(mut route) = route.clone() {
            let slot = if review {
                "reviewer".to_string()
            } else {
                role.to_string()
            };
            let why = why
                .clone()
                .or_else(|| route.tuples.get(&slot).map(|t| t.why.clone()))
                .unwrap_or_default();
            let record = RouteTuple {
                agent: tuple.agent.clone(),
                model: tuple.model.clone(),
                effort: tuple.effort.clone(),
                why,
            };
            if route.tuples.get(&slot) != Some(&record) {
                route.tuples.insert(slot, record);
                fullops_store::write_route(repo, &route)?;
            }
        }
        let jev = |name: &str| squad_path(&format!("docs/evaluations/jev/{key}-{name}.json"));
        let paths = [
            squad_path(&format!("routes/{key}.json")),
            squad_path(&format!("handovers/to_{role}.md")),
            squad_path("PLANS.md"),
            jev("find"),
            jev("documents-find"),
            jev("context"),
            jev("packet"),
        ];
        git.commit_only(&paths, &format!("fullops: 배정 준비 {key}"))?;
        // 3. 기록 게시
        publish_records(repo, config, &git)?;
        if !review {
            // 4. 대상 워크트리 동기화
            let base_tip = git.rev_parse(&base_ref).yes().ok_or_else(|| {
                CliError::External(format!("기본 브랜치 `{}`를 읽지 못했다.", config.base))
            })?;
            sync_target(ctx, config, &role, &target, &base_tip)?;
            // 5. 기준선과 패킷 확인
            let tgit = GitCli::cli(target.clone());
            let behind = |rev: &str| tgit.is_ancestor(rev, "HEAD") == Known::Yes(false);
            if behind(&base_ref) || behind(&config.base_ref()) {
                return Err(CliError::Refused(vec![refusal(
                    "D-11",
                    &[role.as_str()],
                    &format!(
                        "`fullops sync --role {role}`. 원격이 앞서 있으면 `fullops settle`로 fetch한 뒤 다시"
                    ),
                    2,
                )]));
            }
            if let (true, Some(_), Some(target_repo)) =
                (work, &handover, fullops_store::discover(&target)?)
            {
                packet = fullops_store::read_packet(&target_repo, &key)
                    .yes()
                    .flatten()
                    .is_some()
                    || route.as_ref().is_some_and(|r| r.requires_packet);
                if let Some(found) =
                    packet_findings(&target_repo, PacketCheck::Delivery, &key, &role)
                {
                    return Err(CliError::Refused(vec![refusal(
                        "D-12",
                        &[&found],
                        &format!(
                            "`fullops packet build {key} --role {role} --write-inbox` 뒤 다시 배정"
                        ),
                        2,
                    )]));
                }
            }
            // 6. 환경 파일 연결
            for linked in super::env::link_env(&main_checkout(repo), &target) {
                ctx.note(&format!("환경 파일을 연결했다: {linked}"));
            }
        }
    }

    // 7. 알림. 배정을 막지 않는다
    let inbox_text = handover
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok());
    if let Some(level) = inbox_text.as_deref().and_then(lint_level_in)
        && level > config.test_level
    {
        ctx.note(&format!(
            "인박스의 lint 명령이 `--level {level}`을 쓴다. 이 레포의 테스트 레벨은 `{}`다.",
            config.test_level
        ));
    }
    if tuple.agent == "grok" && !grok_trusted(&main_checkout(repo)) {
        ctx.note(messages::flow::GROK_TRUST);
    }

    // 8. 예약 기록을 쓰고 Orca를 부른다. 같은 (key, role, purpose, brief)의 예약 기록이 있으면 그 요청 ID를 다시 쓴다
    let brief_fp = req
        .brief
        .as_deref()
        .map(|b| sha256_hex(b.as_bytes())[..16].to_string());
    let same = |d: &Dispatch| {
        d.is_reservation()
            && d.key == key
            && d.role == role
            && d.purpose == req.purpose
            && d.brief == brief_fp
    };
    let reserve = |request: &str| {
        Some(Dispatch {
            id: DispatchId::new(format!("pending:{request}")).ok()?,
            request: request.to_string(),
            task_id: None,
            run: run.clone(),
            key: key.clone(),
            role: role.clone(),
            purpose: req.purpose,
            tuple: tuple.clone(),
            terminal: None,
            worktree: path_text(&target),
            session_mode: if new_terminal {
                SessionMode::Fresh
            } else {
                SessionMode::Retain
            },
            reason: req.reason,
            started_at: fullops_store::now(),
            bound_session: None,
            sub_of: parent.map(|p| p.id.clone()),
            brief: brief_fp.clone(),
            reported: None,
            settled: None,
            limit_escalated: false,
            extra: Extra::new(),
        })
    };
    let request = ctx.update_state(None, |s| {
        if let Some(found) = s.dispatches.iter().find(|d| same(d)) {
            return found.request.clone();
        }
        let request = new_request();
        s.dispatches.extend(reserve(&request));
        request
    })?;
    let start = StartRequest {
        run: run.clone(),
        spec: spec_of(packet),
        worktree: target.clone(),
        target: match &req.terminal {
            Some(terminal) => StartTarget::Existing {
                terminal: terminal.clone(),
            },
            None => StartTarget::NewTerminal {
                agent: tuple.agent.clone(),
                model: (!tuple.model.is_empty()).then(|| tuple.model.clone()),
                effort: Some(effort.clone()),
            },
        },
        ready_timeout,
        request: request.clone(),
    };
    let drop_reservation = |ctx: &Ctx| {
        ctx.update_state(None, |s| {
            s.dispatches
                .retain(|d| !(d.is_reservation() && d.request == request));
        })
    };
    let unknown = |why: &str| {
        CliError::External(format!(
            "결과를 확인하지 못했다({why}). `fullops dispatch {key}`를 다시 실행하면 같은 요청 ID로 결과를 받는다"
        ))
    };
    let started = match orca.worker_start(&start) {
        Ok(StartOutcome::Started(started)) => started,
        Ok(StartOutcome::Failed(failure)) => {
            drop_reservation(ctx)?;
            let show = |v: &Option<String>| v.clone().unwrap_or_else(|| "-".into());
            return Err(CliError::External(format!(
                "Orca가 worker를 시작하지 못했다. dispatchId: {} / failedStage: {} / lastError: {} / next: {} / residualResources: {}",
                failure
                    .dispatch
                    .as_ref()
                    .map_or("-".into(), |d| d.to_string()),
                show(&failure.failed_stage),
                show(&failure.last_error),
                show(&failure.next),
                if failure.residual.is_empty() {
                    "-".into()
                } else {
                    failure.residual.join(", ")
                }
            )));
        }
        Ok(StartOutcome::Unknown(why)) => return Err(unknown(&why)),
        // 시작되지 않은 것이 확실한 실패: 다음 배정은 새 요청 ID로 간다
        Err(ExtError::NotFound) => {
            drop_reservation(ctx)?;
            return Err(CliError::External("Orca 실행 파일을 찾지 못했다".into()));
        }
        Err(ExtError::Failed { stderr, .. }) => {
            drop_reservation(ctx)?;
            return Err(CliError::External(format!(
                "Orca가 배정을 거절했다: {}",
                stderr.trim()
            )));
        }
        Err(other) => return Err(unknown(&other.to_string())),
    };

    // 9. 상태 쓰기
    let coordinator = ctx.update_state(None, |s| {
        add_run(s);
        // 예약 기록이 그 사이에 정리됐으면 다시 만든다
        if !s.dispatches.iter().any(|d| d.request == request) {
            s.dispatches.extend(reserve(&request));
        }
        if let Some(d) = s
            .dispatches
            .iter_mut()
            .find(|d| d.is_reservation() && d.request == request)
        {
            d.id = started.dispatch.clone();
            d.task_id = Some(started.task.clone());
            d.terminal = started.terminal.clone();
            d.run = started.run.clone();
        }
        if !req.sub {
            let counters = s.tasks.entry(key.clone()).or_default();
            counters.last_tuple = Some(tuple.clone());
            if req.reason == Some(DispatchReason::Limit) {
                counters.limit_redispatch += 1;
            }
        }
        s.runs
            .iter()
            .find(|r| r.id == started.run)
            .and_then(|r| r.coordinator.clone())
    })?;

    // 10. 결과와 다음 할 일
    let next = if orca_wakes(handle.as_deref(), coordinator.as_deref()) {
        "턴을 끝내면 Orca가 깨운다".to_string()
    } else {
        format!("`fullops wait --run {}`", started.run)
    };
    if effort == "ultracode" {
        ctx.note(messages::flow::ULTRA_STARTED);
    }
    ctx.print(
        &format!(
            "배정했다: {key} → {role}\nrun {} / task {} / dispatch {} / worker {}\n다음: {next}",
            started.run,
            started.task,
            started.dispatch,
            started.terminal.as_deref().unwrap_or("터미널 없음")
        ),
        json!({"key": key, "role": role, "run": started.run, "task": started.task, "dispatch": started.dispatch,
               "terminal": started.terminal, "state": started.state, "request": request, "next": next}),
    );
    Ok(EXIT_OK)
}
