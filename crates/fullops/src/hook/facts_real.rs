//! 실제 파일과 프로세스에서 사실을 읽는 `Facts`(설계 10.2절). 모든 사실은 지연 계산이고 한 번 읽은 것은 다시 읽지 않는다.
//! git·Orca 호출은 이벤트별 상한(설계 7.1절)을 넘으면 `Unknown`을 돌려준다.

use super::changes::{self, Criteria, GitGate};
use crate::assets;
use fullops_core::config::{PolicyError, mode_changed};
use fullops_core::derive::landing_of;
use fullops_core::facts::{ActorInput, CommitScope, Facts, resolve_actor};
use fullops_core::model::{
    Actor, Caps, ChangeSet, Config, Derived, DispatchId, EventKind, HeadRef, HookInput, HostId,
    Inbox, Known, Landing, Need, Plan, ReviewEvidence, RoleId, Route, RunId, Session, SessionId,
    Sha, SkillText, State, TaskKey, Waiter, Waiver, WorkOutcome, caps_of, path_text,
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
    gate: GitGate,
    orca_used: u32,
    config: Option<Result<Config, PolicyError>>,
    session: Option<Known<Option<Session>>>,
    state: Option<Known<State>>,
    archive: Option<Known<BTreeSet<TaskKey>>>,
    plan: Option<Known<Plan>>,
    head: Option<Known<Sha>>,
    actor: Option<Known<Actor>>,
    changed: Option<Known<ChangeSet>>,
    /// `changed()`가 읽은 기준 시점의 검사 기준. X-02의 `review_need`가 다시 읽지 않는다.
    criteria: Option<Criteria>,
    /// 터미널 핸들로 묶인 dispatch(7.3절 2번). 효과 실행이 세션 파일에 쓴다.
    pub bind: Option<(DispatchId, Option<String>)>,
    /// 이 호출이 계산한 통합 위치(SHA → 위치). `status.json`의 유도값으로 넘긴다.
    landings: BTreeMap<String, Landing>,
    /// X-03의 Orca 조회가 본 결과가 실패였는가.
    pub settled_failed: bool,
}

impl RealFacts {
    pub fn new(repo: Repo, host: HostId, input: &HookInput, plugin_root: Option<PathBuf>) -> Self {
        Self {
            gate: GitGate::hook(repo.root.clone()),
            repo,
            sid: input.sid.clone(),
            plugin_root,
            host,
            event: input.event,
            orca_used: 0,
            config: None,
            session: None,
            state: None,
            archive: None,
            plan: None,
            head: None,
            actor: None,
            changed: None,
            criteria: None,
            bind: None,
            landings: BTreeMap::new(),
            settled_failed: false,
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
        (self.gate.used, self.orca_used)
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

    /// git 호출 창구. 상한은 `state.json`의 통합 대기 건수에 따라 달라지므로 쓸 때마다 다시 넣는다.
    fn gate(&mut self) -> &mut GitGate {
        self.gate.cap = self.git_cap();
        &mut self.gate
    }

    /// git을 한 번 부른다. 상한을 넘는 요청은 `Unknown`이다.
    pub fn git<T>(&mut self, call: impl FnOnce(&GitCli) -> Known<T>) -> Known<T> {
        self.gate().ask(call)
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

    /// 브랜치 ref의 SHA를 파일에서 읽는다(느슨한 ref, 없으면 packed-refs). git 프로세스를 띄우지 않는다.
    pub fn ref_sha(&self, name: &str) -> Option<Sha> {
        let read = |file: &str| std::fs::read_to_string(self.repo.common_dir.join(file)).ok();
        let loose = read(name).and_then(|text| Sha::new(text.trim()).ok());
        loose.or_else(|| {
            read("packed-refs")?.lines().find_map(|line| {
                let (sha, packed) = line.split_once(' ')?;
                (packed == name).then(|| Sha::new(sha).ok()).flatten()
            })
        })
    }

    fn lint_excludes(&self) -> globset::GlobSet {
        let path = self.repo.squad().join("lint").join("lint.json");
        let text = fullops_store::read_text(&path).yes().flatten();
        fullops_core::lint_excludes(text.as_deref(), assets::LINT_JSON)
    }

    fn compute_changed(&mut self) -> Known<ChangeSet> {
        let actor = know!(self.actor());
        let head = know!(self.head());
        let Ok(config) = self.config().cloned() else {
            return Known::Unknown("policy");
        };
        let anchor = self.session().yes().and_then(|s| s.anchor.clone());
        // 시작점 후보가 없는 세션(세션 파일 없이 온 Stop)은 판정하지 않는다
        let Some(candidate) = changes::start_candidate(&self.repo, &config, &actor, anchor) else {
            return Known::Unknown("no_start");
        };
        // lead: 병합으로 들어온 완료 보고의 변경은 뺀다. 통합 위치는 X-06과 같은 답을 쓴다
        let merged = match self.state().yes().cloned().filter(|_| actor.is_lead()) {
            Some(state) => changes::merged_in(&state, &mut |sha| self.landing(sha)),
            None => Vec::new(),
        };
        let repo = self.repo.clone();
        let found = know!(changes::changed(
            self.gate(),
            &repo,
            &config,
            &actor,
            &head,
            Some(candidate),
            &merged,
        ));
        self.criteria = found.criteria;
        Known::Yes(found.set)
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
    fn real_path(&mut self, path: &Path) -> PathBuf {
        // 아직 없는 뒷부분은 그대로 붙인다: 새로 만드는 파일도 실제 폴더 기준으로 본다
        let mut tail = Vec::new();
        let mut head = path;
        loop {
            if let Ok(real) = std::fs::canonicalize(head) {
                return tail.iter().rev().fold(real, |path, name| path.join(name));
            }
            match (head.parent(), head.file_name()) {
                (Some(parent), Some(name)) => {
                    tail.push(name);
                    head = parent;
                }
                _ => return path.to_path_buf(),
            }
        }
    }
    fn changed(&mut self) -> Known<ChangeSet> {
        if self.changed.is_none() {
            self.changed = Some(self.compute_changed());
        }
        self.changed.clone().unwrap_or(Known::Unknown("changed"))
    }
    fn pass_covers(&mut self, head: &Sha, start: &Sha) -> Known<bool> {
        let repo = self.repo.clone();
        changes::pass_covers(self.gate(), &repo, head, Some(start)).map(|pass| pass.is_some())
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
        // 기준 시점 정책은 `changed()`가 시작점 커밋에서 읽어 둔 것이다. 없으면 작업 트리의 설정을 쓴다
        let criteria = match self.criteria.take() {
            Some(criteria) => criteria,
            None => match self.config() {
                Ok(config) => Criteria {
                    review: config.review.clone(),
                    code: config.code.clone(),
                    exclude: self.lint_excludes(),
                },
                Err(_) => return Known::Unknown("policy"),
            },
        };
        let need = fullops_core::review_need(
            &criteria.review,
            &numstat,
            &criteria.code,
            &criteria.exclude,
            base.clone(),
            head.clone(),
            base.clone(),
        );
        self.criteria = Some(criteria);
        Known::Yes(need)
    }
    fn review_evidence(&mut self, head: &Sha) -> Known<ReviewEvidence> {
        // reviews/<KEY>/<head 12자>/의 accepted.json과 waiver.json
        let mut found: Vec<(i64, Sha, ReviewEvidence)> = Vec::new();
        let dirs = |dir: PathBuf| std::fs::read_dir(dir).into_iter().flatten().flatten();
        for dir in dirs(self.repo.squad().join("reviews")).flat_map(|key| dirs(key.path())) {
            // 유효한 수락 기록만 센다: 두 해시가 지금의 result.json·lint.json과 같다
            if let Some(a) = crate::cmd::accepted(&dir.path()) {
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
            if reviewed == *head
                || self.gate().only_records_since(&reviewed, head) == Known::Yes(true)
            {
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
    fn qa_commit_candidates(&mut self, scope: &CommitScope) -> Known<Vec<String>> {
        let entries = know!(self.git(|g| g.status(Some(QA_REPORTS))));
        Known::Yes(
            entries
                .into_iter()
                .filter(|e| scope.takes(e.index, &e.path))
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
        Self::skill_of(self.host, name)
    }
}

impl RealFacts {
    /// 사용자 스킬의 글. 레포가 필요 없어서 비활성 레포의 SessionStart(S-06)도 쓴다.
    pub(super) fn skill_of(host: HostId, name: &str) -> Known<Option<SkillText>> {
        // `~/.agents/skills`를 먼저 보고 그다음 호스트의 스킬 폴더를 본다. 같은 폴더는 한 번만 본다
        let mut dirs: Vec<PathBuf> = Vec::new();
        let agents = fullops_store::home().map(|home| home.join(".agents").join("skills"));
        for dir in agents.into_iter().chain(fullops_ext::skill_roots(host)) {
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
