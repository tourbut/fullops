//! 실제 파일과 프로세스에서 사실을 읽는 `Facts`(설계 10.2절). 모든 사실은 지연 계산이고 한 번 읽은 것은 다시 읽지 않는다.
//! git·Orca 호출은 이벤트별 상한(설계 7.1절)을 넘으면 `Unknown`을 돌려준다.

use crate::assets;
use fullops_core::config::{PolicyError, mode_changed};
use fullops_core::derive::{is_change_target, is_record_path, landing_of};
use fullops_core::facts::{ActorInput, Facts, resolve_actor};
use fullops_core::model::{
    Accepted, Actor, Caps, ChangeSet, Config, Derived, DispatchId, EventKind, HeadRef, HookInput,
    HostId, Inbox, Known, Landing, Mode, Need, Plan, ReviewEvidence, RoleId, Route, RunId, Seat,
    Session, SessionId, Sha, SkillText, State, TaskKey, Waiter, Waiver, WorkOutcome, caps_of,
    path_text,
};
use fullops_core::rules::NO_SESSION;
use fullops_ext::{Git, GitCli, Orca, OrcaCli};
use fullops_store::Repo;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

const QA_REPORTS: &str = ".fullops-squad/docs/evaluations/qa-reports";
/// 대기 프로세스의 heartbeat를 살아 있다고 보는 시간(초).
const WAITER_FRESH: i64 = 90;
/// S-06이 읽는 스킬 파일의 한도.
const SKILL_LIMIT: u64 = 256 * 1024;

/// `Unknown`이면 그 사유로 돌려준다.
macro_rules! know {
    ($e:expr) => {
        match $e {
            Known::Yes(value) => value,
            Known::Unknown(why) => return Known::Unknown(why),
        }
    };
}

pub struct RealFacts {
    pub repo: Repo,
    pub sid: SessionId,
    pub plugin_root: Option<PathBuf>,
    host: HostId,
    event: EventKind,
    git: GitCli,
    git_used: u32,
    orca_used: u32,
    config: Option<Result<Config, PolicyError>>,
    session: Option<Known<Option<Session>>>,
    state: Option<Known<State>>,
    archive: Option<Known<BTreeSet<TaskKey>>>,
    plan: Option<Known<Plan>>,
    head: Option<Known<Sha>>,
    actor: Option<Known<Actor>>,
    changed: Option<Known<ChangeSet>>,
    /// 터미널 핸들로 묶인 dispatch(7.3절 2번). 효과 실행이 세션 파일에 쓴다.
    pub bind: Option<(DispatchId, Option<String>)>,
    /// 이 호출이 계산한 통합 위치(SHA → 위치). `status.json`의 유도값으로 넘긴다.
    landings: BTreeMap<String, Landing>,
    /// X-03의 Orca 조회가 본 결과가 실패였는가.
    pub settled_failed: bool,
    /// 커밋 → "그 커밋부터 HEAD까지 기록 경로만 바뀌었는가". lint 통과와 리뷰 증거가 같은 답을 쓴다.
    records_since: BTreeMap<String, Known<bool>>,
}

impl RealFacts {
    pub fn new(repo: Repo, host: HostId, input: &HookInput, plugin_root: Option<PathBuf>) -> Self {
        Self {
            git: GitCli::hook(repo.root.clone()),
            repo,
            sid: input.sid.clone(),
            plugin_root,
            host,
            event: input.event,
            git_used: 0,
            orca_used: 0,
            config: None,
            session: None,
            state: None,
            archive: None,
            plan: None,
            head: None,
            actor: None,
            changed: None,
            bind: None,
            landings: BTreeMap::new(),
            settled_failed: false,
            records_since: BTreeMap::new(),
        }
    }

    /// 터미널 핸들로 묶인 dispatch가 있어 세션 파일에 써야 한다.
    pub fn must_bind(&self) -> bool {
        self.bind.is_some()
    }

    pub fn actor_label(&self) -> Option<String> {
        match &self.actor {
            Some(Known::Yes(actor)) => Some(actor.label()),
            _ => None,
        }
    }

    /// 띄운 git·Orca 프로세스 수.
    pub fn calls(&self) -> (u32, u32) {
        (self.git_used, self.orca_used)
    }

    /// 역할 ID와 그 브랜치. 설정을 읽지 못했으면 비어 있다.
    pub fn branches(&self) -> Vec<(String, String)> {
        match &self.config {
            Some(Ok(config)) => config
                .roles
                .iter()
                .map(|(id, role)| (id.to_string(), role.branch.clone()))
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn session_missing(&mut self) -> bool {
        matches!(self.session(), Known::Unknown(NO_SESSION))
    }

    fn git_cap(&self) -> u32 {
        match self.event {
            EventKind::SessionStart => 3,
            EventKind::UserPromptSubmit | EventKind::StopFailure => 0,
            EventKind::PreToolUse => 1,
            EventKind::Stop => {
                let waiting = match &self.state {
                    Some(Known::Yes(state)) => state
                        .completions
                        .iter()
                        .filter(|c| {
                            !c.direct
                                && c.outcome == WorkOutcome::Succeeded
                                && c.landed_at.is_none()
                        })
                        .count(),
                    _ => 0,
                };
                6 + 2 * waiting as u32
            }
        }
    }

    /// git을 한 번 부른다. 상한을 넘는 요청은 `Unknown`이다.
    pub fn git<T>(&mut self, call: impl FnOnce(&GitCli) -> Known<T>) -> Known<T> {
        if self.git_used >= self.git_cap() {
            return Known::Unknown("git_cap");
        }
        self.git_used += 1;
        call(&self.git)
    }

    /// Orca를 한 번 부를 수 있는가. 이벤트마다 한 번이다(SessionStart와 UserPromptSubmit은 0).
    pub fn orca(&mut self, timeout: Duration) -> Option<OrcaCli> {
        let cap = match self.event {
            EventKind::SessionStart | EventKind::UserPromptSubmit => 0,
            _ => 1,
        };
        if self.orca_used >= cap {
            return None;
        }
        self.orca_used += 1;
        Some(OrcaCli::new(self.repo.root.clone(), timeout))
    }

    /// `from`이 `head`의 조상이고 그 사이에 기록 경로만 바뀌었는가(설계 6.6절).
    fn only_records_since(&mut self, from: &Sha, head: &Sha) -> Known<bool> {
        if let Some(known) = self.records_since.get(from.as_str()) {
            return *known;
        }
        let known = (|| {
            if !know!(self.git(|g| g.is_ancestor(from.as_str(), head.as_str()))) {
                return Known::Yes(false);
            }
            let names = know!(self.git(|g| g.diff_names(from.as_str(), head.as_str())));
            Known::Yes(names.iter().all(|path| is_record_path(path)))
        })();
        self.records_since.insert(from.to_string(), known);
        known
    }

    /// 브랜치 ref의 SHA를 파일에서 읽는다(느슨한 ref, 없으면 packed-refs). git 프로세스를 띄우지 않는다.
    fn ref_sha(&self, name: &str) -> Option<Sha> {
        let read = |file: &str| std::fs::read_to_string(self.repo.common_dir.join(file)).ok();
        let loose = read(name).and_then(|text| Sha::new(text.trim()).ok());
        loose.or_else(|| {
            read("packed-refs")?.lines().find_map(|line| {
                let (sha, packed) = line.split_once(' ')?;
                (packed == name).then(|| Sha::new(sha).ok()).flatten()
            })
        })
    }

    /// 이 체크아웃의 역할: 역할 좌석이면 그 역할, dev 모드의 주 담당 좌석이면 `primary_role`.
    fn own_role(&mut self, actor: &Actor) -> Option<RoleId> {
        match &actor.seat {
            Seat::Role { id, .. } => Some(id.clone()),
            Seat::Lead => self
                .config()
                .ok()
                .filter(|c| c.mode == Mode::Dev)
                .and_then(|c| c.primary_role.clone()),
            _ => None,
        }
    }

    fn lint_excludes(&self) -> globset::GlobSet {
        let path = self.repo.squad().join("lint").join("lint.json");
        let text = fullops_store::read_text(&path).yes().flatten();
        fullops_core::lint_excludes(text.as_deref(), assets::LINT_JSON)
    }

    fn compute_changed(&mut self) -> Known<ChangeSet> {
        let actor = know!(self.actor());
        let head = know!(self.head());
        let (code, base_ref) = match self.config() {
            Ok(config) => (config.code.clone(), config.base_ref()),
            Err(_) => return Known::Unknown("policy"),
        };
        // 1. 시작점: 역할 인박스의 base, 없으면 세션의 anchor. dev 직접 작업은 항상 anchor다
        let inbox_base = match self.own_role(&actor).filter(|_| !actor.direct_dev) {
            Some(role) => self.inbox(&role).yes().flatten().map(|inbox| inbox.base),
            None => None,
        };
        let anchor = self.session().yes().and_then(|s| s.anchor.clone());
        let Some(mut start) = inbox_base.or(anchor) else {
            return Known::Unknown("no_start");
        };
        if start != head {
            // 2. 바닥: merge-base(HEAD, 통합 기준 ref). 시작점이 바닥의 조상이면 바닥으로 바꾼다
            let floor = self.git(|g| g.merge_base(head.as_str(), &base_ref)).yes();
            let lowered = match &floor {
                Some(floor) if *floor == start => true,
                Some(floor) => {
                    let below = self.git(|g| g.is_ancestor(start.as_str(), floor.as_str()));
                    if below == Known::Yes(true) {
                        start = floor.clone();
                    }
                    below == Known::Yes(true)
                }
                None => false,
            };
            // 3. 시작점이 HEAD의 조상이 아니면(amend·rebase) 바닥을 쓴다
            if !lowered && !know!(self.git(|g| g.is_ancestor(start.as_str(), head.as_str()))) {
                match floor {
                    Some(floor) => start = floor,
                    None => return Known::Unknown("no_floor"),
                }
            }
        }
        // 4. 대상: 커밋된 변경과 미커밋 변경의 합집합 가운데 대상 파일
        let exclude = self.lint_excludes();
        let target = |path: &str| is_change_target(path, &code, &exclude, actor.direct_dev);
        let mut files: BTreeSet<String> = BTreeSet::new();
        if start != head {
            let names = know!(self.git(|g| g.diff_names(start.as_str(), head.as_str())));
            files.extend(names.into_iter().filter(|p| target(p)));
        }
        let status = know!(self.git(|g| g.status(None)));
        let mut dirty = Vec::new();
        for entry in status.iter().filter(|e| target(&e.path)) {
            let modified = std::fs::metadata(self.repo.root.join(&entry.path))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as i64);
            dirty.push((entry.path.clone(), modified));
            files.insert(entry.path.clone());
        }
        Known::Yes(ChangeSet {
            start,
            code: files.into_iter().collect(),
            dirty,
        })
    }

    /// `status.json`에 넘길 유도값: 이 호출이 계산한 통합 위치와, lead가 계산할 수 있으면 준비된 행.
    pub fn derived(&mut self) -> Derived {
        let mut derived = Derived::default();
        if let Some(Known::Yes(state)) = &self.state {
            for c in &state.completions {
                let landing = c
                    .sha
                    .as_ref()
                    .and_then(|sha| self.landings.get(sha.as_str()));
                if let Some(landing) = landing.filter(|l| **l != Landing::Landed) {
                    derived.landing.insert(c.id.to_string(), *landing);
                }
            }
        }
        let lead = matches!(&self.actor, Some(Known::Yes(actor)) if actor.is_lead());
        if let (true, Some(Known::Yes(plan)), Some(Known::Yes(state)), Some(Known::Yes(archive))) =
            (lead, &self.plan, &self.state, &self.archive)
        {
            let inbox = |role: &RoleId| self.repo.inbox_path(role).exists();
            derived.ready = Some(fullops_core::ready(plan, state, archive, &inbox));
        }
        derived
    }
}

impl Facts for RealFacts {
    fn now(&self) -> i64 {
        fullops_store::now()
    }
    fn host(&self) -> HostId {
        self.host
    }
    fn caps(&self) -> Caps {
        caps_of(self.host)
    }
    fn config(&mut self) -> Result<&Config, PolicyError> {
        self.config
            .get_or_insert_with(|| fullops_store::load_config(&self.repo))
            .as_ref()
            .map_err(Clone::clone)
    }
    fn policy_error(&mut self) -> Option<PolicyError> {
        let _ = self.session();
        if let Err(e) = self.config() {
            return Some(e);
        }
        match (&self.config, &self.session) {
            (Some(Ok(config)), Some(Known::Yes(Some(session)))) => {
                mode_changed(config, &session.mode_at_start)
            }
            _ => None,
        }
    }
    fn actor(&mut self) -> Known<Actor> {
        if let Some(actor) = &self.actor {
            return actor.clone();
        }
        let resolved = (|| {
            if self.config().is_err() {
                return Known::Unknown("policy");
            }
            let Ok(head) = fullops_store::head(&self.repo) else {
                return Known::Unknown("head");
            };
            let (_, _) = (self.session().yes().is_some(), self.state().yes().is_some());
            let Some(Ok(config)) = &self.config else {
                return Known::Unknown("policy");
            };
            let session_dispatch = match &self.session {
                Some(Known::Yes(Some(session))) => session.dispatch.as_ref().map(|d| &d.id),
                _ => None,
            };
            let handle = crate::cli::session_handle();
            Known::Yes(resolve_actor(&ActorInput {
                config,
                head: &head,
                worktree: &path_text(&self.repo.root),
                session_dispatch,
                terminal: handle.as_deref(),
                state: match &self.state {
                    Some(Known::Yes(state)) => Some(state),
                    _ => None,
                },
            }))
        })();
        let actor = resolved.map(|resolved| {
            self.bind = resolved.bind.map(|(id, task, _)| (id, task));
            resolved.actor
        });
        self.actor = Some(actor.clone());
        actor
    }
    fn session(&mut self) -> Known<&Session> {
        let session = self
            .session
            .get_or_insert_with(|| fullops_store::read_session(&self.repo, &self.sid));
        match session {
            Known::Yes(Some(session)) => Known::Yes(session),
            Known::Yes(None) => Known::Unknown(NO_SESSION),
            Known::Unknown(why) => Known::Unknown(why),
        }
    }
    fn state(&mut self) -> Known<&State> {
        self.state
            .get_or_insert_with(|| fullops_store::read_state(&self.repo))
            .as_ref()
    }
    fn archive_keys(&mut self) -> Known<&BTreeSet<TaskKey>> {
        self.archive
            .get_or_insert_with(|| fullops_store::archive_keys(&self.repo))
            .as_ref()
    }
    fn plan(&mut self) -> Known<&Plan> {
        self.plan
            .get_or_insert_with(|| fullops_store::read_plan(&self.repo, assets::PLANS_MD))
            .as_ref()
    }
    fn inbox(&mut self, role: &RoleId) -> Known<Option<Inbox>> {
        fullops_store::read_inbox(&self.repo, role)
    }
    fn route(&mut self, key: &TaskKey) -> Known<Option<Route>> {
        fullops_store::read_route(&self.repo, key)
    }
    fn file_text(&mut self, path: &Path) -> Known<Option<String>> {
        fullops_store::read_text(&self.repo.root.join(path))
    }
    fn path_exists(&mut self, path: &Path) -> Known<bool> {
        Known::Yes(self.repo.root.join(path).exists())
    }
    fn changed(&mut self) -> Known<ChangeSet> {
        if self.changed.is_none() {
            self.changed = Some(self.compute_changed());
        }
        self.changed.clone().unwrap_or(Known::Unknown("changed"))
    }
    fn pass_covers(&mut self, head: &Sha, start: &Sha) -> Known<bool> {
        let Some(pass) = know!(fullops_store::read_lint_pass(&self.repo)) else {
            return Known::Yes(false);
        };
        // 통과 뒤에 쌓인 커밋이 기록 경로만 바꿨으면 그 통과를 인정한다
        if pass.head != *head && !know!(self.only_records_since(&pass.head, head)) {
            return Known::Yes(false);
        }
        if pass.base == *start {
            return Known::Yes(true);
        }
        self.git(|g| g.is_ancestor(pass.base.as_str(), start.as_str()))
    }
    fn head(&mut self) -> Known<Sha> {
        if self.head.is_none() {
            let head = match fullops_store::head(&self.repo) {
                Ok(HeadRef::Detached(sha)) => Known::Yes(sha),
                Ok(HeadRef::Branch(name)) => match self.ref_sha(&format!("refs/heads/{name}")) {
                    Some(sha) => Known::Yes(sha),
                    None => self.git(|g| g.rev_parse("HEAD")),
                },
                Err(_) => Known::Unknown("head"),
            };
            self.head = Some(head);
        }
        self.head.clone().unwrap_or(Known::Unknown("head"))
    }
    fn review_need(&mut self, base: &Sha, head: &Sha) -> Known<Need> {
        let numstat = know!(self.git(|g| g.numstat(base.as_str(), head.as_str())));
        // 기준 시점 정책: 시작점 커밋의 fullops.toml. 읽지 못하면 작업 트리의 설정을 쓴다
        let at_base = (base.to_string(), ".fullops-squad/fullops.toml".to_string());
        let (review, code) = match self.git(|g| g.show_blobs(&[at_base])) {
            Known::Yes(blobs) => {
                let text = blobs.into_iter().next().flatten();
                let text = text.and_then(|bytes| String::from_utf8(bytes).ok());
                let baseline = fullops_store::baseline(text.as_deref());
                (baseline.review, baseline.code)
            }
            Known::Unknown(_) => match self.config() {
                Ok(config) => (config.review.clone(), config.code.clone()),
                Err(_) => return Known::Unknown("policy"),
            },
        };
        Known::Yes(fullops_core::review_need(
            &review,
            &numstat,
            &code,
            &self.lint_excludes(),
            base.clone(),
            head.clone(),
            base.clone(),
        ))
    }
    fn review_evidence(&mut self, head: &Sha) -> Known<ReviewEvidence> {
        // reviews/<KEY>/<head 12자>/의 accepted.json과 waiver.json
        let mut found: Vec<(i64, Sha, ReviewEvidence)> = Vec::new();
        let dirs = |dir: PathBuf| std::fs::read_dir(dir).into_iter().flatten().flatten();
        for dir in dirs(self.repo.squad().join("reviews")).flat_map(|key| dirs(key.path())) {
            let accepted = fullops_store::read_json::<Accepted>(&dir.path().join("accepted.json"));
            if let Known::Yes(Some(a)) = accepted {
                found.push((a.at, a.head, ReviewEvidence::Accepted));
            }
            let waiver = fullops_store::read_json::<Waiver>(&dir.path().join("waiver.json"));
            if let Known::Yes(Some(w)) = waiver {
                found.push((w.at, w.head, ReviewEvidence::Waived));
            }
        }
        found.sort_by_key(|(at, _, _)| std::cmp::Reverse(*at));
        // HEAD이거나 HEAD와 기록 경로만 다른 조상의 리뷰를 인정한다. 최근 것부터 셋만 본다
        for (_, reviewed, kind) in found.into_iter().take(3) {
            if reviewed == *head || self.only_records_since(&reviewed, head) == Known::Yes(true) {
                return Known::Yes(kind);
            }
        }
        Known::Yes(ReviewEvidence::None)
    }
    fn landing(&mut self, sha: &Sha) -> Known<Landing> {
        if let Some(landing) = self.landings.get(sha.as_str()) {
            return Known::Yes(*landing);
        }
        let (base_ref, local) = match self.config() {
            Ok(config) => (
                config.base_ref(),
                config
                    .remote
                    .as_ref()
                    .map(|_| format!("refs/heads/{}", config.base)),
            ),
            Err(_) => return Known::Unknown("policy"),
        };
        let on_base = know!(self.git(|g| g.is_ancestor(sha.as_str(), &base_ref)));
        let on_local = match local.filter(|_| !on_base) {
            Some(local) => Some(know!(self.git(|g| g.is_ancestor(sha.as_str(), &local)))),
            None => None,
        };
        let landing = landing_of(on_base, on_local);
        self.landings.insert(sha.to_string(), landing);
        Known::Yes(landing)
    }
    fn qa_commit_candidates(&mut self, include_unstaged: bool) -> Known<Vec<String>> {
        let entries = know!(self.git(|g| g.status(Some(QA_REPORTS))));
        Known::Yes(
            entries
                .into_iter()
                .filter(|e| include_unstaged || !matches!(e.index, ' ' | '?'))
                .map(|e| e.path)
                .collect(),
        )
    }
    fn orca_settled(&mut self, id: &DispatchId) -> Known<bool> {
        let Some(orca) = self.orca(Duration::from_secs(3)) else {
            return Known::Unknown("orca_cap");
        };
        match orca.worker_show(id) {
            Ok(view) => {
                self.settled_failed = view.failed();
                Known::Yes(view.settled())
            }
            Err(_) => Known::Unknown("orca"),
        }
    }
    fn waiter_alive(&mut self, run: &RunId) -> Known<bool> {
        let waiter = know!(fullops_store::read_json::<Waiter>(
            &self.repo.waiter_path(run)
        ));
        Known::Yes(waiter.is_some_and(|w| self.now() - w.heartbeat <= WAITER_FRESH))
    }
    fn session_handle(&self) -> Option<String> {
        crate::cli::session_handle()
    }
    fn user_skill(&mut self, name: &str) -> Known<Option<SkillText>> {
        // `~/.agents/skills`를 먼저 보고 그다음 호스트의 스킬 폴더를 본다. 같은 폴더는 한 번만 본다
        let mut dirs: Vec<PathBuf> = Vec::new();
        let agents = fullops_store::home().map(|home| home.join(".agents").join("skills"));
        for dir in agents
            .into_iter()
            .chain(fullops_ext::skill_roots(self.host))
        {
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
        let found = dirs
            .into_iter()
            .map(|dir| dir.join(name).join("SKILL.md"))
            .find_map(|path| {
                let meta = std::fs::metadata(&path).ok().filter(|m| m.is_file())?;
                Some((path, meta.len()))
            });
        let Some((path, size)) = found else {
            return Known::Yes(None);
        };
        // 256KB를 넘는 파일은 없는 것으로 본다
        if size > SKILL_LIMIT {
            return Known::Yes(None);
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Known::Unknown("io");
        };
        // 본문은 파일이 `---` 줄로 시작하면 둘째 `---` 줄 뒤부터, 아니면 전부다
        let mut lines = text.split_inclusive('\n');
        let fenced = lines.next().is_some_and(|line| line.trim() == "---");
        let body = match lines.position(|line| line.trim() == "---") {
            Some(_) if fenced => lines.collect::<String>(),
            _ => text.clone(),
        };
        Known::Yes(Some(SkillText { path, body }))
    }
}
