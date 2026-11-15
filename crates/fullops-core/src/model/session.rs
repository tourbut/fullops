//! 작업 트리별 Git 디렉터리의 파일 형식(설계 6.3절)과 `turns.jsonl`(6.2.2절).

use super::{DispatchId, EventKind, Extra, HostId, ModeTuple, SessionId, Sha, TaskKey, words};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 이 바이너리가 아는 세션 파일의 `v`.
pub const SESSION_V: u32 = 1;

words!(BoundBy { Preamble = "preamble", Terminal = "terminal" });
words!(
    /// `turns.jsonl`의 `kind`. 사람 턴으로 세는 것은 `Human`뿐이다. `Subagent`·`Workflow`는 T-13이 쓴다.
    TurnKind { Human = "human", OrcaMessage = "machine:orca-message", TaskNotification = "machine:task-notification",
               Subagent = "subagent", Workflow = "workflow" }
);

/// `sessions/<sid>.json`. 모르는 필드는 보존한다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub v: u32,
    pub sid: SessionId,
    /// hook 인자로 고른 어댑터의 ID.
    pub host: HostId,
    pub started_at: i64,
    pub last_seen: i64,
    /// 세션 파일을 만들 때의 세션 핸들(`ORCA_AGENT_SESSION_ID` → `ORCA_TERMINAL_HANDLE`).
    pub terminal: Option<String>,
    /// `[mode, primary_role, primary_branch]`.
    pub mode_at_start: ModeTuple,
    /// 첫 SessionStart의 HEAD. 한 번만 쓴다. 커밋이 없는 레포에서는 `null`.
    pub anchor: Option<Sha>,
    #[serde(default)]
    pub turn: u32,
    /// 프롬프트 이벤트가 없는 호스트(agy)만 쓴다. 마지막으로 본 사용자 입력의 sha256 앞 16자.
    #[serde(default)]
    pub prompt_fp: Option<String>,
    #[serde(default)]
    pub dispatch: Option<SessionDispatch>,
    #[serde(default)]
    pub routed: Vec<TaskKey>,
    /// 규칙 ID → 한 번 막기 기록(설계 7.6절).
    #[serde(default)]
    pub blocks: BTreeMap<String, BlockMark>,
    /// 세션당 한 번 낸 안내·경고의 키.
    #[serde(default)]
    pub noticed: Vec<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionDispatch {
    pub id: DispatchId,
    /// Orca task ID.
    pub task: Option<String>,
    pub bound_by: BoundBy,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockMark {
    pub fp: String,
    pub turn: u32,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Session {
    /// 합치기 연산. 이미 있으면 그대로 둔다.
    pub fn notice(&mut self, key: &str) -> bool {
        let fresh = !self.noticed.iter().any(|k| k == key);
        if fresh {
            self.noticed.push(key.to_string());
        }
        fresh
    }

    /// 합치기 연산. 이미 있으면 그대로 둔다.
    pub fn add_routed(&mut self, key: &TaskKey) {
        if !self.routed.contains(key) {
            self.routed.push(key.clone());
        }
    }

    /// 새 dispatch를 묶고 `blocks`를 비운다(설계 6.3절).
    pub fn bind(&mut self, dispatch: SessionDispatch) {
        self.dispatch = Some(dispatch);
        self.blocks.clear();
    }
}

/// `current.json`. CLI가 자기 세션을 찾는 데 쓴다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Current {
    pub sid: SessionId,
    pub terminal: Option<String>,
    pub at: i64,
}

/// `turns.jsonl`의 한 줄. 본문은 쓰지 않는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turn {
    pub at: i64,
    pub sid: SessionId,
    pub kind: TurnKind,
}

/// `waiters/<run>.json`. `fullops wait`가 30초마다 `heartbeat`를 갱신한다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Waiter {
    pub pid: u32,
    pub sid: Option<SessionId>,
    pub heartbeat: i64,
}

/// `trace/<sid>.jsonl`의 한 줄. 명령 문자열·프롬프트·파일 내용·환경 변수 값은 쓰지 않는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceLine {
    pub t: i64,
    pub v: u32,
    /// 실행본 버전.
    pub bin: String,
    pub host: HostId,
    pub ev: EventKind,
    /// `Actor::label()`. 판정하지 못했으면 `null`.
    pub actor: Option<String>,
    /// Intent 변형 이름.
    pub intents: Vec<String>,
    pub rules: Vec<RuleTrace>,
    /// git 프로세스 수.
    pub git: u32,
    /// Orca 프로세스 수.
    pub orca: u32,
    pub ms: u64,
    /// 오류 종류. `lock_busy`, `timeout`, `panic`, `io` 등.
    pub err: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleTrace {
    pub id: String,
    /// `context`·`warn`·`deny`·`block`·`effect`·`unknown`.
    pub verdict: String,
    pub fp: Option<String>,
}
