//! 실제 파일과 프로세스에서 사실을 읽는 `Facts`(설계 10.2절). 지연 계산과 호출 상한은 묶음 B가 채운다.

use fullops_core::config::PolicyError;
use fullops_core::facts::Facts;
use fullops_core::model::{
    Actor, Caps, ChangeSet, Config, DispatchId, HostId, Inbox, Known, Landing, Need, Plan,
    ReviewEvidence, RoleId, Route, RunId, Session, Sha, SkillText, State, TaskKey, caps_of,
};
use fullops_store::Repo;
use std::collections::BTreeSet;
use std::path::Path;

const TODO: &str = "unimplemented";

pub struct RealFacts {
    pub repo: Repo,
    host: HostId,
    config: Option<Result<Config, PolicyError>>,
}

impl RealFacts {
    pub fn new(repo: Repo, host: HostId) -> Self {
        Self {
            repo,
            host,
            config: None,
        }
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
        self.config().err()
    }
    fn actor(&mut self) -> Known<Actor> {
        Known::Unknown(TODO)
    }
    fn session(&mut self) -> Known<&Session> {
        Known::Unknown(TODO)
    }
    fn state(&mut self) -> Known<&State> {
        Known::Unknown(TODO)
    }
    fn archive_keys(&mut self) -> Known<&BTreeSet<TaskKey>> {
        Known::Unknown(TODO)
    }
    fn plan(&mut self) -> Known<&Plan> {
        Known::Unknown(TODO)
    }
    fn inbox(&mut self, role: &RoleId) -> Known<Option<Inbox>> {
        fullops_store::read_inbox(&self.repo, role)
    }
    fn route(&mut self, key: &TaskKey) -> Known<Option<Route>> {
        fullops_store::read_route(&self.repo, key)
    }
    fn file_text(&mut self, path: &Path) -> Known<Option<String>> {
        fullops_store::read_text(path)
    }
    fn path_exists(&mut self, path: &Path) -> Known<bool> {
        Known::Yes(path.exists())
    }
    fn changed(&mut self) -> Known<ChangeSet> {
        Known::Unknown(TODO)
    }
    fn pass_covers(&mut self, _head: &Sha, _start: &Sha) -> Known<bool> {
        Known::Unknown(TODO)
    }
    fn head(&mut self) -> Known<Sha> {
        Known::Unknown(TODO)
    }
    fn review_need(&mut self, _base: &Sha, _head: &Sha) -> Known<Need> {
        Known::Unknown(TODO)
    }
    fn review_evidence(&mut self, _head: &Sha) -> Known<ReviewEvidence> {
        Known::Unknown(TODO)
    }
    fn landing(&mut self, _sha: &Sha) -> Known<Landing> {
        Known::Unknown(TODO)
    }
    fn qa_commit_candidates(&mut self, _include_unstaged: bool) -> Known<Vec<String>> {
        Known::Unknown(TODO)
    }
    fn orca_settled(&mut self, _id: &DispatchId) -> Known<bool> {
        Known::Unknown(TODO)
    }
    fn waiter_alive(&mut self, _run: &RunId) -> Known<bool> {
        Known::Unknown(TODO)
    }
    fn session_handle(&self) -> Option<String> {
        crate::cli::session_handle()
    }
    fn user_skill(&mut self, _name: &str) -> Known<Option<SkillText>> {
        Known::Unknown(TODO)
    }
}
