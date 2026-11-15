//! hook 규칙과 CLI 검사가 바깥을 보는 유일한 통로(설계 10.2절)와 호출자 판정(7.3절).

use crate::config::PolicyError;
use crate::model::{
    Actor, BoundBy, Caps, ChangeSet, Config, DispatchId, HeadRef, HostId, Inbox, Known, Landing,
    Mode, Need, Plan, ReviewEvidence, RoleId, Route, RunId, Seat, Session, Sha, SkillText,
    SnapshotState, State, TaskKey, same_path,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// 모든 사실은 지연 계산이고, 읽지 못하면 `Unknown`이다. 호출 상한(설계 7.1절)을 넘는 요청도 `Unknown`이다.
pub trait Facts {
    fn now(&self) -> i64;
    /// hook 인자로 고른 어댑터의 ID. S-01의 pstack 문장과 S-06이 쓴다.
    fn host(&self) -> HostId;
    fn caps(&self) -> Caps;
    fn config(&mut self) -> Result<&Config, PolicyError>;
    /// 설정 오류 또는 세션 중 모드 변경.
    fn policy_error(&mut self) -> Option<PolicyError>;
    fn actor(&mut self) -> Known<Actor>;
    fn session(&mut self) -> Known<&Session>;
    fn state(&mut self) -> Known<&State>;
    fn archive_keys(&mut self) -> Known<&BTreeSet<TaskKey>>;
    /// 통합된 과제 키와 그 가장 늦은 통합 시각.
    fn archive_landed(&mut self) -> Known<&BTreeMap<TaskKey, i64>>;
    fn plan(&mut self) -> Known<&Plan>;
    /// 이 체크아웃의 인박스.
    fn inbox(&mut self, role: &RoleId) -> Known<Option<Inbox>>;
    fn route(&mut self, key: &TaskKey) -> Known<Option<Route>>;
    fn file_text(&mut self, path: &Path) -> Known<Option<String>>;
    fn path_exists(&mut self, path: &Path) -> Known<bool>;
    /// 존재하는 앞부분을 실제 경로로 푼 절대 경로(링크, junction, Windows의 대소문자). 풀지 못하면 받은 그대로다.
    fn real_path(&mut self, path: &Path) -> PathBuf {
        path.to_path_buf()
    }
    /// 세션이 만든 변경(설계 6.6절).
    fn changed(&mut self) -> Known<ChangeSet>;
    fn pass_covers(&mut self, head: &Sha, start: &Sha) -> Known<bool>;
    fn head(&mut self) -> Known<Sha>;
    fn review_need(&mut self, base: &Sha, head: &Sha) -> Known<Need>;
    fn review_evidence(&mut self, head: &Sha) -> Known<ReviewEvidence>;
    fn landing(&mut self, sha: &Sha) -> Known<Landing>;
    /// T-09: `scope`가 담는 qa-reports 항목의 레포 상대 경로.
    fn qa_commit_candidates(&mut self, scope: &CommitScope) -> Known<Vec<String>>;
    fn orca_settled(&mut self, id: &DispatchId) -> Known<bool>;
    fn waiter_alive(&mut self, run: &RunId) -> Known<bool>;
    /// 세션 핸들(설계 4절). 환경 변수 `ORCA_AGENT_SESSION_ID` → `ORCA_TERMINAL_HANDLE`에서 읽는다.
    fn session_handle(&self) -> Option<String>;
    /// S-06: 설치된 사용자 스킬의 본문. 파일이 없으면 `Yes(None)`.
    fn user_skill(&mut self, name: &str) -> Known<Option<SkillText>>;
}

/// T-09의 커밋 대상(설계 7.5절): 이미 인덱스에 든 항목, `tracked`면 추적 중인 항목 전부(`commit -a`, `add -u`),
/// `added`(같은 명령의 `git add` 경로, 레포 상대. `""`은 레포 전체) 아래의 항목은 추적하지 않는 것(`??`)까지.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitScope {
    pub tracked: bool,
    pub added: Vec<String>,
}

impl CommitScope {
    /// `git status --porcelain`의 인덱스 칸과 경로로 그 항목이 커밋에 들어갈 수 있는지 본다.
    pub fn takes(&self, index: char, path: &str) -> bool {
        let under = |p: &String| {
            p.is_empty()
                || path == p
                || path
                    .strip_prefix(p.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        };
        !matches!(index, ' ' | '?')
            || (self.tracked && index != '?')
            || self.added.iter().any(under)
    }
}

/// 호출자 판정의 입력. 모두 git 프로세스 없이 얻는 값이다.
pub struct ActorInput<'a> {
    pub config: &'a Config,
    pub head: &'a HeadRef,
    /// 이 워크트리 루트. `path_text` 표기.
    pub worktree: &'a str,
    /// 세션 파일의 `dispatch.id`.
    pub session_dispatch: Option<&'a DispatchId>,
    /// 세션 핸들(환경 변수 `ORCA_AGENT_SESSION_ID` → `ORCA_TERMINAL_HANDLE`).
    pub terminal: Option<&'a str>,
    /// 읽지 못했으면 `None`. 그때는 2·3번 판정을 건너뛰고 `sub`는 거짓이다.
    pub state: Option<&'a State>,
}

pub struct ResolvedActor {
    pub actor: Actor,
    /// 2번(터미널 핸들)으로 묶였으면 그 dispatch와 Orca task ID. 부르는 쪽이 세션 파일에 쓴다.
    pub bind: Option<(DispatchId, Option<String>, BoundBy)>,
}

/// 설계 7.3절의 순서대로 호출자를 정한다. hook과 CLI가 같은 함수를 쓴다.
pub fn resolve_actor(input: &ActorInput) -> ResolvedActor {
    let mut bind = None;
    let mut dispatch = input.session_dispatch.cloned();
    if dispatch.is_none()
        && let (Some(terminal), Some(state)) =
            (input.terminal.filter(|t| !t.is_empty()), input.state)
    {
        let mut matches = state.dispatches.iter().filter(|d| {
            d.settled.is_none()
                && !d.is_reservation()
                && d.terminal.as_deref() == Some(terminal)
                && same_path(&d.worktree, input.worktree)
        });
        if let (Some(only), None) = (matches.next(), matches.next()) {
            dispatch = Some(only.id.clone());
            bind = Some((only.id.clone(), only.task_id.clone(), BoundBy::Terminal));
        }
    }
    let seat = seat(input);
    let direct_dev = input.config.mode == Mode::Dev && seat == Seat::Lead && dispatch.is_none();
    let sub = match (&dispatch, input.state) {
        (Some(id), Some(state)) => state.dispatch(id).is_some_and(|d| d.is_sub()),
        _ => false,
    };
    ResolvedActor {
        actor: Actor {
            seat,
            dispatch,
            direct_dev,
            sub,
        },
        bind,
    }
}

/// 좌석은 3~6번으로 정한다. dispatch가 묶여도 3번을 본다: 배정받은 리뷰어가 `Reviewer`가 된다.
fn seat(input: &ActorInput) -> Seat {
    let config = input.config;
    let is_snapshot = input.state.is_some_and(|state| {
        state
            .snapshots
            .iter()
            .any(|s| s.state == SnapshotState::Active && same_path(&s.path, input.worktree))
    });
    if is_snapshot {
        return Seat::Reviewer;
    }
    let HeadRef::Branch(branch) = input.head else {
        return Seat::Other;
    };
    match (config.role_of_branch(branch), config.mode) {
        (Some((_, role)), Mode::Coor) if role.kind == crate::model::RoleKind::Coordinator => {
            Seat::Lead
        }
        (Some((id, _)), Mode::Dev) if config.primary_role.as_ref() == Some(id) => Seat::Lead,
        (Some((id, role)), _) => Seat::Role {
            id: id.clone(),
            kind: role.kind,
        },
        (None, Mode::Coor) => Seat::Lead,
        (None, Mode::Dev) if config.primary_branch.as_deref() == Some(branch.as_str()) => {
            Seat::Lead
        }
        (None, Mode::Dev) => Seat::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn config(mode: Mode) -> Config {
        let role = |kind| Role {
            branch: format!("fullops/{kind}"),
            kind,
            about: None,
        };
        Config {
            schema: 2,
            applied_version: "2.0.0".into(),
            mode,
            primary_role: (mode == Mode::Dev).then(|| RoleId::new("worker").unwrap()),
            primary_branch: (mode == Mode::Dev).then(|| "main".to_string()),
            routing: Routing::Legacy,
            tech_plan_role: None,
            test_level: TestLevel::Lite,
            parallel: Parallel::On,
            base: "main".into(),
            remote: None,
            roles: [RoleKind::Worker, RoleKind::Design, RoleKind::Coordinator]
                .map(|kind| (RoleId::new(kind.as_str()).unwrap(), role(kind)))
                .into(),
            review: ReviewPolicy::default(),
            code: CodeDef::default(),
            wait: WaitConfig::default(),
        }
    }

    fn dispatch(id: &str, terminal: &str, worktree: &str) -> Dispatch {
        Dispatch {
            id: DispatchId::new(id).unwrap(),
            request: "req_0000000000000001".into(),
            task_id: Some("task_1".into()),
            run: RunId::new("run_1").unwrap(),
            key: TaskKey::new("A-1").unwrap(),
            role: RoleId::new("worker").unwrap(),
            purpose: Purpose::Build,
            tuple: ModelTuple {
                agent: "claude".into(),
                model: "m".into(),
                effort: "medium".into(),
            },
            terminal: Some(terminal.into()),
            worktree: worktree.into(),
            session_mode: SessionMode::Fresh,
            reason: None,
            started_at: 1,
            bound_session: None,
            sub_of: None,
            brief: None,
            reported: None,
            settled: None,
            limit_escalated: false,
            round: None,
            extra: Extra::new(),
        }
    }

    fn seat_of(mode: Mode, head: &HeadRef) -> Seat {
        let config = config(mode);
        let input = ActorInput {
            config: &config,
            head,
            worktree: "/w",
            session_dispatch: None,
            terminal: None,
            state: None,
        };
        resolve_actor(&input).actor.seat
    }

    #[test]
    fn seats_follow_branch_and_mode() {
        let branch = |name: &str| HeadRef::Branch(name.into());
        let role = |kind: RoleKind| Seat::Role {
            id: RoleId::new(kind.as_str()).unwrap(),
            kind,
        };
        let detached = HeadRef::Detached(Sha::new("a".repeat(40)).unwrap());

        assert_eq!(seat_of(Mode::Coor, &branch("main")), Seat::Lead);
        assert_eq!(seat_of(Mode::Coor, &branch("feature/x")), Seat::Lead);
        assert_eq!(
            seat_of(Mode::Coor, &branch("fullops/coordinator")),
            Seat::Lead
        );
        assert_eq!(
            seat_of(Mode::Coor, &branch("fullops/worker")),
            role(RoleKind::Worker)
        );
        assert_eq!(
            seat_of(Mode::Coor, &branch("fullops/design")),
            role(RoleKind::Design)
        );
        assert_eq!(seat_of(Mode::Coor, &detached), Seat::Other);

        assert_eq!(seat_of(Mode::Dev, &branch("main")), Seat::Lead);
        assert_eq!(seat_of(Mode::Dev, &branch("fullops/worker")), Seat::Lead);
        assert_eq!(
            seat_of(Mode::Dev, &branch("fullops/design")),
            role(RoleKind::Design)
        );
        assert_eq!(
            seat_of(Mode::Dev, &branch("fullops/coordinator")),
            role(RoleKind::Coordinator)
        );
        assert_eq!(seat_of(Mode::Dev, &branch("feature/x")), Seat::Other);
    }

    #[test]
    fn dispatch_binding_and_review_snapshot() {
        let (coor, dev) = (config(Mode::Coor), config(Mode::Dev));
        let main = HeadRef::Branch("main".into());
        let mut state = State::default();
        state.dispatches = vec![
            dispatch("dsp_1", "term_1", "C:/w/dev"),
            dispatch("dsp_2", "term_2", "C:/w/art"),
        ];
        let input = |config, worktree, session_dispatch, terminal| ActorInput {
            config,
            head: &main,
            worktree,
            session_dispatch,
            terminal,
            state: Some(&state),
        };

        // 묶인 것이 없으면 lead이고, dev 모드의 lead는 직접 작업이다
        let lead = resolve_actor(&input(&dev, "C:/w/dev", None, None));
        assert!(lead.actor.is_lead() && lead.actor.direct_dev && lead.bind.is_none());
        assert!(
            !resolve_actor(&input(&coor, "C:/w/dev", None, None))
                .actor
                .direct_dev
        );

        // 터미널 핸들과 워크트리가 모두 같은 미정산 dispatch가 하나일 때만 묶는다
        let bound = resolve_actor(&input(&dev, r"C:\w\dev\", None, Some("term_1")));
        assert_eq!(
            bound.actor.dispatch.as_ref().map(DispatchId::as_str),
            Some("dsp_1")
        );
        assert_eq!(bound.bind.map(|b| b.2), Some(BoundBy::Terminal));
        assert!(
            bound.actor.seat == Seat::Lead && !bound.actor.is_lead() && !bound.actor.direct_dev
        );
        assert!(
            resolve_actor(&input(&dev, "C:/w/other", None, Some("term_1")))
                .actor
                .is_lead()
        );
        assert!(
            resolve_actor(&input(&dev, "C:/w/dev", None, Some("")))
                .actor
                .is_lead()
        );

        // 세션 파일의 dispatch가 먼저다
        let session = DispatchId::new("dsp_9").unwrap();
        let by_session = resolve_actor(&input(&coor, "C:/w/dev", Some(&session), Some("term_1")));
        assert_eq!(
            (by_session.actor.dispatch, by_session.bind.is_none()),
            (Some(session), true)
        );

        // 등록된 리뷰 사본은 Reviewer다
        state.dispatches[0].settled = Some(Settled {
            how: SettledHow::Done,
            outcome: None,
            message: None,
            at: 2,
            extra: Extra::new(),
        });
        state.snapshots.push(Snapshot {
            review: "A-1@aaaaaaaaaaaa".into(),
            path: "C:/w/.fullops-review-1".into(),
            head: Sha::new("a".repeat(40)).unwrap(),
            base: Sha::new("b".repeat(40)).unwrap(),
            state: SnapshotState::Active,
            implementer_session: None,
            reviewer_dispatch: None,
            owner: RunOwner::Lead,
            created_at: 3,
            hold: None,
            extra: Extra::new(),
        });
        let detached = HeadRef::Detached(Sha::new("a".repeat(40)).unwrap());
        let review = |terminal| ActorInput {
            config: &coor,
            head: &detached,
            worktree: "C:/w/.fullops-review-1",
            session_dispatch: None,
            terminal,
            state: Some(&state),
        };
        assert_eq!(resolve_actor(&review(None)).actor.seat, Seat::Reviewer);
        // dispatch로 배정받은 리뷰어도 Reviewer다. 하위 배정이면 sub가 참이다
        let mut sub_state = state.clone();
        sub_state.dispatches[1].worktree = "C:/w/.fullops-review-1".into();
        sub_state.dispatches[1].sub_of = DispatchId::new("dsp_1").ok();
        let reviewer = resolve_actor(&ActorInput {
            state: Some(&sub_state),
            ..review(Some("term_2"))
        });
        assert_eq!(
            (reviewer.actor.seat.clone(), reviewer.actor.label()),
            (Seat::Reviewer, "reviewer+sub".to_string())
        );
        assert!(reviewer.actor.sub && reviewer.bind.is_some());
        // 정산된 dispatch의 터미널로는 묶지 않는다
        assert!(
            resolve_actor(&ActorInput {
                worktree: "C:/w/dev",
                head: &main,
                ..review(Some("term_1"))
            })
            .actor
            .is_lead()
        );
    }

    #[test]
    fn commit_scope_takes_what_git_would_commit() {
        let shot = "q/A-1/shot.png";
        let scope = |tracked, added: &[&str]| CommitScope {
            tracked,
            added: added.iter().map(|p| p.to_string()).collect(),
        };
        // 인덱스에 든 것은 늘 들어간다. 추적하지 않는 캡처는 `commit -a`로는 들어가지 않는다
        assert!(scope(false, &[]).takes('A', shot));
        assert!(!scope(false, &[]).takes(' ', shot) && !scope(true, &[]).takes('?', shot));
        assert!(scope(true, &[]).takes(' ', shot));
        // `git add`는 그 경로 아래만 넓힌다
        assert!(scope(false, &[""]).takes('?', shot));
        assert!(
            scope(false, &["q/A-1"]).takes('?', shot) && scope(false, &[shot]).takes('?', shot)
        );
        assert!(
            !scope(false, &["q/A"]).takes('?', shot) && !scope(false, &["src"]).takes('?', shot)
        );
    }

    #[test]
    fn sub_open_follows_the_parents_own_run() {
        let id = |s: &str| DispatchId::new(s).unwrap();
        let run = || RunId::new("run_sub").unwrap();
        let mut sub = dispatch("dsp_2", "term_2", "C:/w/dev");
        sub.run = run();
        sub.sub_of = Some(id("dsp_1"));
        let mut state = State::default();
        state.dispatches = vec![dispatch("dsp_1", "term_1", "C:/w/dev"), sub];
        state.runs = vec![Run {
            id: run(),
            owner: RunOwner::Dispatch(id("dsp_1")),
            coordinator: None,
            opened_at: 1,
            closed_at: None,
            extra: Extra::new(),
        }];
        // 부모가 소유한 Run의 정산되지 않은 하위 dispatch만 센다. lead의 Run에 있는 부모 자신은 세지 않는다
        assert!(state.sub_open(&id("dsp_1")) && !state.sub_open(&id("dsp_2")));
        state.dispatches[1].settled = Some(Settled {
            how: SettledHow::Done,
            outcome: None,
            message: None,
            at: 2,
            extra: Extra::new(),
        });
        assert!(!state.sub_open(&id("dsp_1")));
        // 정산돼도 보고 메시지에 처리 표시가 없으면 남아 있다
        state.messages.push(Message {
            id: MessageId::new("msg_1").unwrap(),
            delivery: None,
            run: run(),
            kind: "worker_done".into(),
            dispatch: Some(id("dsp_2")),
            received_at: 3,
            acked_at: None,
            extra: Extra::new(),
        });
        assert!(state.sub_open(&id("dsp_1")));
        state.messages[0].acked_at = Some(4);
        // 예약 기록과 닫힌 Run은 세지 않는다
        state.dispatches[1].settled = None;
        state.dispatches[1].id = id("pending:req_1");
        assert!(!state.sub_open(&id("dsp_1")));
        state.dispatches[1].id = id("dsp_2");
        assert!(state.sub_open(&id("dsp_1")));
        state.runs[0].closed_at = Some(5);
        assert!(!state.sub_open(&id("dsp_1")));
    }
}
