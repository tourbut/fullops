//! 흐름 명령의 순수 검사와 기록(설계 8.4~8.6절, 6.7절). 묶음 C가 채운다.
//! 시그니처는 설계 10.2절이 정본이다.

use crate::facts::Facts;
use crate::messages::{self, MsgId};
use crate::model::{
    ArchiveLine, BriefAt, Completion, Dispatch, DispatchId, DispatchReason, DispatchRequest, Extra,
    Inbox, Known, Message, MessageNote, Metrics, ModelTuple, OrcaMessage, Parallel, Plan,
    PlanStatus, Purpose, REVIEWER_SLOT, Refusal, RoleId, RoleKind, RouteClass, RunId, RunOwner,
    Seat, Settled, SettledHow, Sha, ShaSource, SnapshotState, State, TaskKey, TaskNewRequest,
    TestLevel, Turn, TurnKind, WorkOutcome, candidates_text, host_of_agent, sha256_hex,
};
use std::collections::BTreeSet;

/// 거절 한 건. 사유 문구는 `messages`가 만든다. `exit`는 2(거절) 또는 3(사람 손 필요)이다.
pub fn refusal(rule: &'static str, args: &[&str], escape: &str, exit: u8) -> Refusal {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    Refusal {
        rule,
        reason: messages::reason(MsgId(rule), &args),
        escape: escape.to_string(),
        exit,
    }
}

/// `dispatch`의 기본값을 채운 결과(설계 8.5절). `check_dispatch`와 `cmd/dispatch.rs`가 같은 값을 쓴다.
#[derive(Debug, Clone, Default)]
pub struct DispatchDefaults {
    pub role: Option<RoleId>,
    pub tuple: Option<ModelTuple>,
    /// 하위 배정이면 호출자에 묶인 dispatch.
    pub parent: Option<Dispatch>,
}

/// `--role`과 tuple의 기본값. 하위 배정은 호출자 dispatch의 것, 그 밖은 route의 것이다.
pub fn dispatch_defaults(req: &DispatchRequest, facts: &mut dyn Facts) -> DispatchDefaults {
    if req.sub {
        let bound = facts.actor().yes().and_then(|a| a.dispatch);
        let parent = match (bound, facts.state()) {
            (Some(id), Known::Yes(state)) => state.dispatch(&id).cloned(),
            _ => None,
        };
        return DispatchDefaults {
            role: req.role.clone().or(parent.as_ref().map(|p| p.role.clone())),
            tuple: req
                .tuple
                .clone()
                .or(parent.as_ref().map(|p| p.tuple.clone())),
            parent,
        };
    }
    let route = facts.route(&req.key).yes().flatten();
    let design = facts.config().ok().and_then(|c| c.design_role().cloned());
    let mut role = req.role.clone();
    if let (None, Some(route)) = (&role, &route) {
        let no_inbox = |facts: &mut dyn Facts, r: &RoleId| facts.inbox(r) == Known::Yes(None);
        let designed = matches!(route.class, RouteClass::Design | RouteClass::Product);
        role = match &route.role {
            Some(r) if designed && no_inbox(facts, r) => design.or(Some(r.clone())),
            Some(r) => Some(r.clone()),
            None if designed => design,
            None => None,
        };
    }
    let slot = tuple_slot(req.purpose, role.as_ref());
    let recorded = || Some(route.as_ref()?.tuples.get(slot.as_deref()?)?.tuple());
    // 후보가 하나뿐이면 그것을 쓴다
    let config = facts.config().ok();
    let only = || match config?.candidates(slot.as_deref()?) {
        [only] => Some(only.tuple()),
        _ => None,
    };
    DispatchDefaults {
        tuple: req.tuple.clone().or_else(recorded).or_else(only),
        role,
        parent: None,
    }
}

/// tuple을 기록하고 후보를 찾는 자리: 리뷰는 `reviewer`, 그 밖은 역할 ID.
pub fn tuple_slot(purpose: Purpose, role: Option<&RoleId>) -> Option<String> {
    match (purpose, role) {
        (Purpose::Review, _) => Some(REVIEWER_SLOT.to_string()),
        (_, Some(role)) => Some(role.to_string()),
        _ => None,
    }
}

/// `--brief` 글의 지문(`dispatches[].brief`). 글은 저장하지 않는다.
pub fn brief_fingerprint(brief: &str) -> String {
    sha256_hex(brief.as_bytes()).chars().take(16).collect()
}

/// 선택형 위임인가(설계 8.5절 D-08). lead의 필수 독립 리뷰(`--purpose review`와 `--sha`)는 아니다.
fn optional_delegation(req: &DispatchRequest, direct_dev: bool) -> bool {
    let mandatory_review = !req.sub && req.purpose == Purpose::Review && req.sha.is_some();
    !mandatory_review
        && (req.sub || direct_dev || matches!(req.purpose, Purpose::Review | Purpose::Research))
}

/// spec에 `Read only: true`를 넣는가: 리뷰·조사, 그리고 방침 `read`에서 허용된 선택형 위임.
pub fn read_only(req: &DispatchRequest, parallel: Parallel, direct_dev: bool) -> bool {
    matches!(req.purpose, Purpose::Review | Purpose::Research)
        || (parallel == Parallel::Read && optional_delegation(req, direct_dev))
}

/// D-01~D-10과 D-13. 걸린 것을 모두 돌려준다. 하위 배정(`req.sub`)이면 D-04·D-08·D-09·D-13만 본다.
/// 워크트리와 snapshot의 git 상태(D-05·D-06의 일부)는 `Facts`에 없어 `cmd/dispatch.rs`가 이어서 본다.
pub fn check_dispatch(req: &DispatchRequest, facts: &mut dyn Facts) -> Vec<Refusal> {
    let mut out = Vec::new();
    let Ok(config) = facts.config().cloned() else {
        return out;
    };
    let Known::Yes(actor) = facts.actor() else {
        return vec![refusal("D-04", &[""], "lead 세션에서 실행한다", 2)];
    };
    let state = facts.state().yes().cloned().unwrap_or_default();
    let found = dispatch_defaults(req, facts);
    let key = req.key.as_str();

    // D-08: 선택형 위임과 병렬 방침
    if optional_delegation(req, actor.direct_dev) {
        let allowed = match config.parallel {
            Parallel::Off => false,
            Parallel::Read => matches!(req.purpose, Purpose::Review | Purpose::Research),
            Parallel::On | Parallel::Ultra => true,
        };
        if !allowed {
            out.push(refusal(
                "D-08",
                &[config.parallel.as_str()],
                "직접 처리한다. 방침은 사람이 `fullops.toml`의 `parallel`로 바꾼다",
                2,
            ));
        }
    }
    let slot = tuple_slot(req.purpose, found.role.as_ref());
    let list = slot.as_deref().map_or(&[][..], |s| config.candidates(s));
    if found.tuple.is_none() {
        let escape = if list.is_empty() {
            "`--agent --model --effort`".to_string()
        } else {
            format!(
                "`--agent --model --effort`. `[models].{}` 후보: {}",
                slot.as_deref().unwrap_or_default(),
                candidates_text(list)
            )
        };
        out.push(refusal("D-09", &[], &escape, 2));
    }
    // D-14: 후보 목록 밖의 tuple은 근거가 있어야 받는다. 하위 배정의 tuple은 호출자가 정한다
    if let (false, Some(tuple), Some(slot)) = (req.sub, &req.tuple, &slot)
        && req.why.as_deref().is_none_or(|w| w.trim().is_empty())
        && let Some(list) = config.off_list(slot, tuple)
    {
        out.push(refusal(
            "D-14",
            &[
                slot,
                &format!("{}/{}/{}", tuple.agent, tuple.model, tuple.effort),
            ],
            &format!(
                "후보에서 고른다({}). 사용자가 정한 조합이면 `--why \"<근거>\"`",
                candidates_text(list)
            ),
            2,
        ));
    }
    // D-10(같은 배정): 성공한 배정을 다시 실행하면 같은 워크트리에 worker가 하나 더 뜬다. 한도 재배정은 D-07이 본다.
    // build와 fix는 같은 워크트리의 같은 작업이라 목적이 달라도 같은 배정이다
    let brief = req.brief.as_deref().map(brief_fingerprint);
    let same = |d: &&Dispatch| {
        !d.is_reservation()
            && d.settled.is_none()
            && d.key == req.key
            && Some(&d.role) == found.role.as_ref()
            && d.is_work()
            && d.is_sub() == req.sub
            && d.brief == brief
    };
    if matches!(req.purpose, Purpose::Build | Purpose::Fix)
        && req.reason != Some(DispatchReason::Limit)
        && let Some(live) = state.dispatches.iter().find(same)
    {
        out.push(refusal(
            "D-10",
            &["same", live.role.as_str(), key, live.id.as_str()],
            &format!(
                "`fullops wait --run {}`로 그 worker의 보고를 받는다. 한도로 멈춘 worker는 `--reason limit`, 끊긴 worker는 그 워크트리에서 `fullops task recover --role {} --key {key}` 뒤 다시 배정, 다른 범위의 하위 배정은 다른 `--brief`",
                live.run, live.role
            ),
            2,
        ));
    }

    if req.sub {
        let own = |run: &RunId| {
            state.runs.iter().any(|r| {
                &r.id == run
                    && matches!((&r.owner, &actor.dispatch), (RunOwner::Dispatch(o), Some(d)) if o == d)
            })
        };
        if let Some(run) = req.run.as_ref().filter(|run| !own(run)) {
            out.push(refusal(
                "D-04",
                &["run", run.as_str()],
                "`--run`을 빼면 자기 Run을 만든다",
                2,
            ));
        }
        let mut sub = |case: &str, escape: &str| out.push(refusal("D-13", &[case], escape, 2));
        if actor.sub {
            sub("a", "직접 처리하거나 호스트 서브에이전트를 쓴다");
        }
        let parent = found.parent.as_ref();
        if parent.is_none_or(|p| p.key != req.key)
            || matches!((&req.role, parent), (Some(r), Some(p)) if r != &p.role)
        {
            sub(
                "b",
                "다른 역할과 다른 과제는 `fullops done --followup \"<역할>: <글>\"`로 lead에게 넘긴다",
            );
        }
        if req.brief.as_deref().is_none_or(|b| b.trim().is_empty()) {
            sub("c", "`--brief \"<맡길 파일 묶음과 완료 조건>\"`");
        }
        if req.terminal.is_some() || req.sha.is_some() || req.reason.is_some() {
            sub("d", "그 인자를 뺀다");
        }
        return out;
    }

    if !actor.is_lead() {
        out.push(refusal("D-04", &[""], "lead 세션에서 실행한다", 2));
    }
    let route = facts.route(&req.key).yes().flatten();
    let review = req.purpose == Purpose::Review;
    let work = matches!(req.purpose, Purpose::Build | Purpose::Fix);

    // D-01
    let routed = match &route {
        Some(r) if review && r.tuples.contains_key("reviewer") => true,
        Some(r) => r.class != RouteClass::Unresolved,
        None => false,
    };
    if !routed {
        out.push(refusal("D-01", &[key], "`fullops route`", 2));
    }

    // D-05: 역할
    let class = route.as_ref().map(|r| r.class);
    match &found.role {
        None => out.push(refusal("D-05", &["none"], "`--role <역할>`", 2)),
        Some(role) if !config.roles.contains_key(role) => out.push(refusal(
            "D-05",
            &["role", role.as_str()],
            "`fullops.toml`의 `[roles]`에 등록된 역할을 준다",
            2,
        )),
        Some(role) => {
            if let (Some(RouteClass::Direct), Some(Some(routed))) =
                (class, route.as_ref().map(|r| r.role.as_ref()))
                && routed != role
            {
                out.push(refusal(
                    "D-05",
                    &["route", routed.as_str(), role.as_str()],
                    "`fullops route --force`로 역할을 바꾸거나 분류된 역할에 배정한다",
                    2,
                ));
            }
        }
    }
    let role = found.role.as_ref();

    // D-02: 인박스
    if let (true, Some(role)) = (work, role) {
        let design_build = req.purpose == Purpose::Build
            && config
                .roles
                .get(role)
                .is_some_and(|r| r.kind == RoleKind::Design)
            && matches!(class, Some(RouteClass::Design | RouteClass::Product));
        let escape = "`fullops task new`";
        match facts.inbox(role) {
            Known::Yes(Some(inbox)) if inbox.key != req.key => out.push(refusal(
                "D-02",
                &["key", role.as_str(), inbox.key.as_str(), key],
                escape,
                2,
            )),
            Known::Yes(Some(_)) => {}
            // 인박스 없이 받는 설계 역할은 route에 남긴 요청 글로 할 일을 안다
            Known::Yes(None) if design_build => {
                if route.as_ref().is_some_and(|r| r.request_file.is_none()) {
                    out.push(refusal(
                        "D-02",
                        &["request", role.as_str(), key],
                        &format!("`fullops route {key} --force --request \"<요청 글>\" --class … --role …`"),
                        2,
                    ));
                }
            }
            Known::Yes(None) => out.push(refusal("D-02", &["none", role.as_str()], escape, 2)),
            Known::Unknown(_) => out.push(refusal("D-02", &["syntax", role.as_str()], escape, 2)),
        }
    }

    // D-03: 다른 키의 통합 대기
    let others: Vec<&Completion> = state
        .pending()
        .filter(|c| c.key.as_ref() != Some(&req.key))
        .collect();
    if let Some(first) = others.first() {
        let name = |c: &&Completion| {
            let key = c.key.as_ref().map_or("키 없음", TaskKey::as_str);
            format!("{key}({})", c.id)
        };
        let listed: Vec<String> = others.iter().map(name).collect();
        let escape = match &first.key {
            Some(key) => format!("`fullops merge {key}` 또는 `fullops hold`"),
            None => format!(
                "`fullops completion set-sha {} <SHA> --key <KEY>` 뒤 `fullops merge <KEY>`",
                first.id
            ),
        };
        out.push(refusal("D-03", &[&listed.join(", ")], &escape, 2));
    }

    // D-06: 리뷰
    if review {
        let escape = format!("`fullops review snapshot {key}`");
        let sha = req.sha.as_deref().and_then(|s| Sha::new(s).ok());
        match &sha {
            None => out.push(refusal("D-06", &["sha"], &escape, 2)),
            Some(sha) => {
                let active = state
                    .snapshots
                    .iter()
                    .any(|s| s.state == SnapshotState::Active && s.key() == key && &s.head == sha);
                if !active {
                    out.push(refusal("D-06", &["snapshot", sha.short(12)], &escape, 2));
                }
            }
        }
        let implementer =
            |d: &Dispatch| d.key == req.key && d.is_work() && d.counts() && d.terminal.is_some();
        if req.terminal.is_some()
            && state
                .dispatches
                .iter()
                .any(|d| implementer(d) && d.terminal == req.terminal)
        {
            out.push(refusal(
                "D-06",
                &["terminal"],
                "`--terminal`을 빼고 새 세션으로 배정한다",
                2,
            ));
        }
    }

    // D-07: 한도 재배정
    if req.reason == Some(DispatchReason::Limit) {
        let counters = state.tasks.get(&req.key);
        if counters.is_some_and(|t| t.limit_redispatch >= 2) {
            out.push(refusal(
                "D-07",
                &["count", key],
                "사용자 결정을 기다린다(`fullops decision list --open`)",
                3,
            ));
        } else if counters.and_then(|t| t.last_tuple.as_ref()) != found.tuple.as_ref() {
            out.push(refusal(
                "D-07",
                &["tuple"],
                "직전과 같은 `--agent --model --effort`로 다시 배정한다",
                2,
            ));
        }
    }

    // D-10: 그 역할의 끝나지 않은 다른 과제
    if let Some(role) = role
        && let Some(busy) = state
            .dispatches
            .iter()
            .find(|d| &d.role == role && d.counts() && d.settled.is_none() && d.key != req.key)
    {
        out.push(refusal(
            "D-10",
            &[role.as_str(), busy.key.as_str()],
            "그 과제가 끝난 뒤 배정하거나 `fullops plan add`로 대기",
            2,
        ));
    }
    out
}

/// H-01~H-04. 걸린 것을 모두 돌려준다. 경로는 레포 루트 기준이다.
pub fn check_task_new(req: &TaskNewRequest, facts: &mut dyn Facts) -> Vec<Refusal> {
    let mut out = Vec::new();
    let (Ok(config), Known::Yes(actor)) = (facts.config().cloned(), facts.actor()) else {
        return out;
    };
    let (key, role) = (req.key.as_str(), req.role.as_str());
    let route = facts.route(&req.key).yes().flatten();
    let inbox = facts.inbox(&req.role);
    // 같은 키의 열린 차수가 이미 있다: 새로 쓸 지시서가 없으니 그 차수를 배정하라고만 안내한다
    let open_round = match &inbox {
        Known::Yes(Some(open)) if req.reopen && open.key == req.key => Some(open.round),
        _ => None,
    };
    let own = actor.direct_dev && config.primary_role.as_ref() == Some(&req.role);
    let class = route
        .as_ref()
        .map(|r| r.class)
        .filter(|c| !matches!(c, RouteClass::Unresolved | RouteClass::Untracked));
    match class {
        _ if own => {}
        None => out.push(refusal("H-01", &[key], "`fullops route`", 2)),
        Some(class) => {
            let routed_here = route.as_ref().and_then(|r| r.role.as_ref()) == Some(&req.role);
            let designer = matches!(
                &actor.seat,
                Seat::Role {
                    kind: RoleKind::Design,
                    ..
                }
            );
            let designed = matches!(class, RouteClass::Design | RouteClass::Product);
            // 반려·lint 실패의 수정 차수는 class와 관계없이 lead가 연다(1.24.0의 reopen). 설계 역할은 코드를 고치지 않는다
            let allowed = (actor.is_lead() && class == RouteClass::Direct && routed_here)
                || (designer && designed)
                || (actor.is_lead() && config.design_role().is_none())
                || (actor.is_lead() && req.reopen);
            if !allowed {
                let design = config.design_role().map_or("<설계 역할>", RoleId::as_str);
                out.push(refusal(
                    "H-02",
                    &[key, class.as_str(), role],
                    &format!(
                        "design·product 과제의 지시서는 설계 역할이 쓴다. `fullops dispatch {key} --role {design}`로 설계 역할에 배정한다"
                    ),
                    2,
                ));
            }
        }
    }
    let boxed = inbox != Known::Yes(None);
    if boxed {
        // 다시 열기의 탈출구가 다시 열기를 가리키지 않게 한다
        let escape = match open_round {
            Some(round) => format!(
                "그 차수의 worker가 실행 중이면 `fullops wait`로 보고를 받은 뒤 다시 실행한다. 아직 배정하지 않았거나 실패한 차수면 `fullops dispatch {key} --role {role}{}`",
                if round > 0 { " --purpose fix" } else { "" }
            ),
            None if req.reopen => "worker의 완료 보고를 `fullops wait`로 받은 뒤 다시 실행한다. 아직 배정하지 않은 과제면 본문을 고쳐 `fullops dispatch`".into(),
            None => "아직 배정하지 않은 같은 과제면 본문을 고치고, 완료 보고를 받은 같은 과제의 수정이면 `fullops task reopen`, 다른 과제면 `fullops plan add`".into(),
        };
        out.push(refusal("H-03", &[role], &escape, 2));
    }
    let first_log = crate::layout::handover_log(key, role, 0);
    let logged = facts.path_exists(std::path::Path::new(&first_log)) == Known::Yes(true);
    if logged && !req.reopen {
        out.push(refusal("H-04", &[key, role], "`fullops task reopen`", 2));
    } else if !logged && req.reopen && !boxed {
        out.push(refusal(
            "H-04",
            &["missing", key, role],
            "`fullops task new`",
            2,
        ));
    }
    out
}

/// `ready()`가 보지 못하는 이어 갈 일의 안내 줄들(X-07, `status`, `wait`, `merge`가 함께 쓴다).
/// `inboxes`는 lead 체크아웃의 인박스, 그 역할의 kind, 그 차수의 완료 기록이 역할 쪽에 있는가다(dev 주 담당 자신의 인박스는 뺀다).
/// - 배정되지 않은 지시서: 설계 역할이 쓴 인박스(`from`이 lead가 아니다)인데 그 차수가 끝나지 않았고
///   그 키·역할의 마지막 성공 보고 뒤에 시작됐거나 아직 끝나지 않은 Build·Fix dispatch가 없다.
///   lead가 쓰고 있는 인박스는 X-04와 같이 세지 않는다. tester의 지시서는 같은 키의 구현이 끝나 병합된 뒤에 낸다.
/// - 실패한 채 남은 과제: 가장 늦은 보고가 실패이고 그 뒤의 Build·Fix dispatch가 없다.
/// 둘 다 `PLANS.md`에서 미룬(보류·사람 대기) 과제는 뺀다(X-07의 탈출구).
pub fn leftover(state: &State, plan: &Plan, inboxes: &[(Inbox, RoleKind, bool)]) -> Vec<String> {
    let work = |key: &TaskKey| {
        let key = key.clone();
        state
            .dispatches
            .iter()
            .filter(move |d| d.key == key && d.counts() && d.is_work())
    };
    // 보고의 개수로 차수를 세지 않는다: 보관은 앞 차수의 기록을 지우고 한도 재배정은 한 차수에 보고를 둘 남긴다
    let assigned = |inbox: &Inbox, finished: bool| {
        let since = state
            .completions
            .iter()
            .filter(|c| c.key.as_ref() == Some(&inbox.key) && c.role.as_ref() == Some(&inbox.role))
            .filter(|c| c.outcome == WorkOutcome::Succeeded)
            .map(|c| c.received_at)
            .max();
        inbox.from == "lead"
            || finished
            || work(&inbox.key).any(|d| {
                d.role == inbox.role
                    && (d.settled.is_none() || since.is_none_or(|at| d.started_at > at))
            })
    };
    let put_off = |key: &TaskKey| {
        plan.rows.iter().any(|r| {
            &r.key == key && matches!(r.status, PlanStatus::Held(_) | PlanStatus::WaitHuman(_))
        })
    };
    let mut out = Vec::new();
    for (inbox, kind, _) in inboxes
        .iter()
        .filter(|(inbox, _, finished)| !assigned(inbox, *finished) && !put_off(&inbox.key))
    {
        let building = work(&inbox.key).any(|d| d.settled.is_none())
            || state.pending().any(|c| c.key.as_ref() == Some(&inbox.key))
            || inboxes.iter().any(|(other, kind, finished)| {
                other.key == inbox.key && *kind != RoleKind::Tester && !assigned(other, *finished)
            });
        if *kind != RoleKind::Tester || !building {
            out.push(messages::flow::unassigned_inbox(&inbox.key, &inbox.role));
        }
    }
    let keys: BTreeSet<&TaskKey> = state
        .completions
        .iter()
        .filter_map(|c| c.key.as_ref())
        .collect();
    for key in keys {
        let Some(last) = crate::derive::last_failed(key, state) else {
            continue;
        };
        let resumed = work(key).any(|d| d.settled.is_none() || d.started_at > last.received_at);
        let fix = last
            .dispatch
            .as_ref()
            .and_then(|id| state.dispatch(id))
            .is_some_and(|d| d.purpose == Purpose::Fix);
        if !resumed && !put_off(key) {
            out.push(messages::flow::failed_task(key, last.role.as_ref(), fix));
        }
    }
    out
}

/// 본문에서 `SHA`·`commit`·`커밋` 뒤(사이에 `:`·`=`·공백 허용)의 7~40자 hex를 모은다.
pub fn body_shas(body: &str) -> Vec<String> {
    let lower = body.to_ascii_lowercase();
    let mut found = Vec::new();
    for word in ["sha", "commit", "커밋"] {
        for (at, _) in lower.match_indices(word) {
            let rest = lower.get(at + word.len()..).unwrap_or_default();
            let rest = rest.trim_start_matches([':', '=', ' ', '\t']);
            let hex: String = rest.chars().take_while(char::is_ascii_hexdigit).collect();
            let ends = rest
                .chars()
                .nth(hex.len())
                .is_none_or(|c| !c.is_ascii_alphanumeric());
            if (7..=40).contains(&hex.len()) && ends && !found.contains(&hex) {
                found.push(hex);
            }
        }
    }
    found
}

/// 본문의 `[완료]` 또는 `[설계]` 뒤 첫 토큰이 과제 키 문법에 맞으면 그것.
fn body_key(body: &str) -> Option<TaskKey> {
    ["[완료]", "[설계]"].iter().find_map(|mark| {
        let (_, rest) = body.split_once(mark)?;
        TaskKey::new(rest.split_whitespace().next()?).ok()
    })
}

/// 메시지 한 건의 종류별 처리와 completion 기록 규칙(설계 8.6절 6단계). 모든 종류의 메시지를 받는다.
/// `run`은 호출할 때 준 `--run`이다(메시지에는 Run이 없다). `at`은 받은 시각이다(core는 시계를 읽지 않는다).
/// `resolve`는 짧은 hex를 커밋으로 풀고 `branch_tip`은 역할 브랜치의 로컬 ref를 읽고
/// `is_ancestor(older, newer)`는 조상 관계를 답한다. 이미 기록한 메시지는 다시 기록하지 않는다.
pub fn record_message(
    state: &mut State,
    run: &RunId,
    msg: &OrcaMessage,
    at: i64,
    resolve: &mut dyn FnMut(&str) -> Option<Sha>,
    branch_tip: &mut dyn FnMut(&RoleId) -> Option<Sha>,
    is_ancestor: &mut dyn FnMut(&Sha, &Sha) -> bool,
) -> MessageNote {
    let mut note = MessageNote::default();
    if matches!(msg.kind.as_str(), "heartbeat" | "status") {
        note.absorbed = true;
        return note;
    }
    if state.messages.iter().any(|m| m.id == msg.id) {
        return note;
    }
    let found = state
        .dispatches
        .iter()
        .position(|d| !d.is_reservation() && Some(&d.id) == msg.dispatch_id.as_ref())
        .or_else(|| {
            let task = msg.task_id.as_deref()?;
            state
                .dispatches
                .iter()
                .position(|d| d.task_id.as_deref() == Some(task))
        });
    state.messages.push(Message {
        id: msg.id.clone(),
        delivery: None,
        run: run.clone(),
        kind: msg.kind.clone(),
        dispatch: found
            .and_then(|i| state.dispatches.get(i))
            .map(|d| d.id.clone())
            .or(msg.dispatch_id.clone()),
        received_at: at,
        acked_at: None,
        extra: Extra::new(),
    });
    let body = msg.body.as_deref().unwrap_or_default();
    match msg.kind.as_str() {
        "escalation" => {
            if let Some(d) = found.and_then(|i| state.dispatches.get_mut(i)) {
                // 보고로 끝난 배정을 뒤늦은 escalation이 되살리면 끝난 차수를 다시 배정하게 된다
                if d.settled.as_ref().is_none_or(|s| s.how != SettledHow::Done) {
                    d.settled = Some(Settled {
                        how: SettledHow::Escalated,
                        outcome: None,
                        message: None,
                        at,
                        extra: Extra::new(),
                    });
                }
                let limit = |s: &String| s.starts_with("[fullops:limit]");
                d.limit_escalated |= msg.subject.as_ref().is_some_and(limit);
            }
            return note;
        }
        "worker_done" => {}
        _ => return note,
    }

    let outcome = match msg.outcome.as_deref() {
        Some("failed") => WorkOutcome::Failed,
        _ => WorkOutcome::Succeeded,
    };
    let dispatch = found.and_then(|i| state.dispatches.get_mut(i));
    let Some(dispatch) = dispatch else {
        // dispatch를 찾지 못한 보고: 본문에서 키와 SHA를 찾는다
        let key = body_key(body);
        if key.is_none() {
            note.notes.push(format!(
                "과제 키를 찾지 못했다. `fullops completion set-sha {} <SHA> --key <KEY>`",
                msg.id
            ));
        }
        let (sha, sha_source) = sha_of(
            None,
            body,
            None,
            resolve,
            branch_tip,
            is_ancestor,
            &mut note.notes,
        );
        return complete(
            state, msg, None, key, None, sha, sha_source, outcome, at, note,
        );
    };
    dispatch.settled = Some(Settled {
        how: SettledHow::Done,
        outcome: Some(outcome),
        message: Some(msg.id.clone()),
        at,
        extra: Extra::new(),
    });
    // 리뷰·조사 보고와 하위 배정의 보고는 completion을 만들지 않는다
    if !dispatch.is_work() || dispatch.is_sub() {
        return note;
    }
    let dispatch = dispatch.clone();
    match &dispatch.reported {
        None => note
            .notes
            .push("`fullops done` 기록이 없는 보고다. lint 증거를 확인한다".into()),
        // lint_head는 `--no-checks`뿐 아니라 코드 변경이 없을 때도 빈다. 실패 보고는 lint를 따지지 않는다
        Some(r) if r.lint_head.is_none() && outcome != WorkOutcome::Failed => note.notes.push(
            "lint 통과 기록이 없는 보고다(코드 변경이 없거나 `--no-checks`). 병합 때 `fullops merge`가 확인한다".into(),
        ),
        Some(_) => {}
    }
    let (sha, sha_source) = sha_of(
        dispatch.reported.as_ref().map(|r| r.sha.clone()),
        body,
        Some(&dispatch.role),
        resolve,
        branch_tip,
        is_ancestor,
        &mut note.notes,
    );
    complete(
        state,
        msg,
        Some(&dispatch),
        Some(dispatch.key.clone()),
        Some(dispatch.role.clone()),
        sha,
        sha_source,
        outcome,
        at,
        note,
    )
}

/// 완료 SHA를 정하는 순서(설계 8.6절 5번): `done` 기록 → 본문 → 역할 브랜치 tip.
fn sha_of(
    reported: Option<Sha>,
    body: &str,
    role: Option<&RoleId>,
    resolve: &mut dyn FnMut(&str) -> Option<Sha>,
    branch_tip: &mut dyn FnMut(&RoleId) -> Option<Sha>,
    is_ancestor: &mut dyn FnMut(&Sha, &Sha) -> bool,
    notes: &mut Vec<String>,
) -> (Option<Sha>, Option<ShaSource>) {
    if let Some(sha) = reported {
        return (Some(sha), Some(ShaSource::Done));
    }
    let mut named: Vec<Sha> = Vec::new();
    for hex in body_shas(body) {
        if let Some(sha) = resolve(&hex).filter(|sha| !named.contains(sha)) {
            named.push(sha);
        }
    }
    let tip = role.and_then(branch_tip);
    // 하나면 그것, 여럿이면 나머지를 모두 조상으로 가지는 하나다
    let newest = named.iter().find(|&sha| {
        named
            .iter()
            .all(|other| other == sha || is_ancestor(other, sha))
    });
    if let Some(sha) = newest {
        return (Some(sha.clone()), Some(ShaSource::Body));
    }
    if tip.is_some() {
        notes.push("보고에 SHA가 없어 브랜치 tip을 썼다".into());
        return (tip, Some(ShaSource::BranchTip));
    }
    (None, None)
}

#[allow(clippy::too_many_arguments)]
fn complete(
    state: &mut State,
    msg: &OrcaMessage,
    dispatch: Option<&Dispatch>,
    key: Option<TaskKey>,
    role: Option<RoleId>,
    sha: Option<Sha>,
    sha_source: Option<ShaSource>,
    outcome: WorkOutcome,
    at: i64,
    mut note: MessageNote,
) -> MessageNote {
    let body = msg.body.as_deref().unwrap_or_default();
    let branch = dispatch
        .and_then(|d| d.reported.as_ref())
        .map(|r| r.branch.clone())
        .or_else(|| {
            let (_, rest) = body.split_once("브랜치")?;
            rest.split_whitespace().next().map(str::to_string)
        });
    if branch.is_none() {
        note.notes.push("보고에 브랜치가 없다".into());
    }
    if !state.completions.iter().any(|c| c.id == msg.id) {
        state.completions.push(Completion {
            id: msg.id.clone(),
            dispatch: dispatch.map(|d| d.id.clone()),
            key: key.clone(),
            role: role.clone(),
            branch,
            sha: sha.clone(),
            sha_source,
            direct: false,
            outcome,
            received_at: at,
            hold: None,
            released_hold: None,
            landed_at: None,
            drift: None,
            extra: Extra::new(),
        });
    }
    note.completion = Some(crate::model::CompletionNote {
        key,
        role,
        sha,
        sha_source,
        outcome: Some(outcome),
    });
    note
}

/// 지표(설계 6.7절). `status`와 `merge`(M-13)가 잠금 밖에서 부른다. `at`은 부르는 쪽이 계산한 시각으로 채운다.
pub fn metrics(turns: &[Turn], state: &State, archive: &[ArchiveLine]) -> Metrics {
    let count = |kind: TurnKind| turns.iter().filter(|t| t.kind == kind).count() as u32;
    let humans: Vec<i64> = turns
        .iter()
        .filter(|t| t.kind == TurnKind::Human)
        .map(|t| t.at)
        .collect();
    let since = humans.iter().copied().min().unwrap_or(0);
    let archived = || {
        archive.iter().filter_map(|line| match line {
            ArchiveLine::Task(task) => Some(task),
            ArchiveLine::Decision { .. } => None,
        })
    };

    let mut done: BTreeSet<&TaskKey> = BTreeSet::new();
    done.extend(
        state
            .completions
            .iter()
            .filter(|c| c.landed_at.is_some_and(|at| at >= since))
            .filter_map(|c| c.key.as_ref()),
    );
    done.extend(
        archived()
            .filter(|t| t.landed_at.is_some_and(|at| at >= since))
            .map(|t| &t.key),
    );

    // 사건: 배정 시작(키, 시각, Build·Fix인가)과 완료 보고 수신. 하위 배정과 예약 기록은 사건이 아니다
    let mut starts: Vec<(i64, &TaskKey, bool)> = state
        .dispatches
        .iter()
        .filter(|d| d.counts())
        .map(|d| (d.started_at, &d.key, d.is_work()))
        .collect();
    let mut events: Vec<i64> = state.completions.iter().map(|c| c.received_at).collect();
    for task in archived() {
        events.push(task.received_at);
        for d in task.dispatches.iter().filter(|d| !d.sub) {
            let work = matches!(d.purpose, Purpose::Build | Purpose::Fix);
            starts.push((d.started_at, &task.key, work));
        }
    }
    starts.sort();
    let mut unattended: Vec<&TaskKey> = Vec::new();
    for (index, (started, key, work)) in starts.iter().enumerate() {
        let before = starts
            .get(..index)
            .unwrap_or_default()
            .iter()
            .map(|s| s.0)
            .chain(events.iter().copied().filter(|at| at <= started))
            .max();
        if let (true, Some(before)) = (*work, before)
            && !humans.iter().any(|at| (before..=*started).contains(at))
        {
            unattended.push(key);
        }
    }
    let unattended_dispatches = unattended.len() as u32;
    let unattended_tasks = unattended.into_iter().collect::<BTreeSet<_>>().len() as u32;

    let sub_dispatches = state
        .dispatches
        .iter()
        .filter(|d| d.is_sub() && !d.is_reservation())
        .count()
        + archived()
            .flat_map(|t| &t.dispatches)
            .filter(|d| d.sub)
            .count();
    let done_tasks = done.len() as u32;
    Metrics {
        at: 0,
        since,
        human_turns: humans.len() as u32,
        done_tasks,
        turns_per_task: (done_tasks > 0).then(|| humans.len() as f64 / done_tasks as f64),
        unattended_dispatches,
        unattended_tasks,
        subagent_calls: count(TurnKind::Subagent),
        workflow_runs: count(TurnKind::Workflow),
        sub_dispatches: sub_dispatches as u32,
    }
}

/// Orca `worker-start`에 넘길 `--effort` 값(설계 8.5절 8단계). 기록에는 항상 tuple의 값을 쓴다.
/// 방침이 `ultra`이고 새 터미널의 Claude Code worker면 `ultracode`다. 미정-M13이 바꾸는 곳은 이 함수 하나다.
pub fn orca_effort(tuple: &ModelTuple, parallel: Parallel, new_terminal: bool) -> String {
    if parallel == Parallel::Ultra && tuple.agent == "claude" && new_terminal {
        "ultracode".into()
    } else {
        tuple.effort.clone()
    }
}

/// spec의 리뷰 조각(설계 8.5절).
#[derive(Debug, Clone, Copy)]
pub struct ReviewSpec<'a> {
    pub sha: &'a str,
    pub implementer: &'a str,
    /// lead 체크아웃의 `tasks/<KEY>/reviews/<head 12자>` 절대 경로.
    pub folder: &'a str,
}

/// spec 문자열의 재료(설계 8.5절). 경로는 `path_text` 표기다.
#[derive(Debug, Clone, Copy)]
pub struct SpecInput<'a> {
    pub key: &'a TaskKey,
    pub purpose: Purpose,
    pub role: &'a RoleId,
    /// 대상 워크트리의 인박스 절대 경로. 인박스 없이 배정하면 `None`.
    pub handover: Option<&'a str>,
    pub base: &'a str,
    pub test_level: TestLevel,
    pub parallel: Parallel,
    /// 설계 역할 배정의 요청 파일 경로.
    pub request: Option<&'a str>,
    pub review: Option<ReviewSpec<'a>>,
    pub read_only: bool,
    /// 하위 배정이면 부모 dispatch ID와 `--brief`의 글.
    pub sub: Option<(&'a DispatchId, &'a str)>,
    /// tuple의 `agent`(Orca 에이전트 ID).
    pub agent: &'a str,
    /// 고정 위치 실행본의 절대 경로. 홈을 모르면 `fullops`.
    pub fixed_bin: &'a str,
}

/// worker에게 가는 spec 한 문단(설계 8.5절). 사고 지시는 넣지 않는다.
pub fn spec(input: &SpecInput) -> String {
    let host = host_of_agent(input.agent);
    let command = messages::spec_command(host, input.fixed_bin);
    let purpose = match input.purpose {
        Purpose::Build => "implementation",
        other => other.as_str(),
    };
    let mut out = format!(
        "Task key: {}; Purpose: {purpose}; Role: {}",
        input.key, input.role
    );
    let mut field = |name: &str, value: &str| out.push_str(&format!("; {name}: {value}"));
    if let Some(handover) = input.handover {
        field("Handover", handover);
    }
    field("Base", input.base);
    field("Test level", input.test_level.as_str());
    field("Parallel", input.parallel.as_str());
    if let Some(request) = input.request {
        field("Request", request);
    }
    if let Some(review) = input.review {
        field("Review SHA", review.sha);
        field("Implementer session", review.implementer);
        field("Reviewer session", "separate");
        field("Review folder", review.folder);
    }
    if input.read_only {
        field("Read only", "true");
    }
    if let Some((parent, brief)) = input.sub {
        field("Sub of", parent.as_str());
        field("Scope", brief);
    }
    let at = BriefAt::Spec {
        command: &command,
        key: input.key,
        sub: input.sub.is_some(),
    };
    let end = if input.sub.is_some() {
        "커밋·lint·fullops done은 하지 않는다(부모가 한다). 맡은 범위를 끝내면 바꾼 파일 목록과 확인한 것을 본문에 적어 preamble의 worker_done을 한 번 보낸다".to_string()
    } else if input.review.is_some() {
        // 리뷰어는 좌석이 Reviewer라 `done`을 하지 못한다(R-01). 설계 8.9절의 끝맺음을 적는다
        format!(
            "끝나면 결과를 Review folder에 쓰고 본문 `[리뷰] {} | SHA <head> | 결론 accept|reject | high <n>`으로 preamble의 worker_done을 한 번 보낸다",
            input.key
        )
    } else {
        format!(
            "끝나면 {command} done을 실행하고 출력된 본문으로 preamble의 worker_done을 한 번 보낸다"
        )
    };
    format!(
        "{out}. {} {end}",
        messages::parallel_brief(input.parallel, host, at)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_parsers_follow_the_report_shape() {
        let sha = "4f0c1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b";
        let body = format!(
            "[완료] PAY-12 | 브랜치 fullops/dev | SHA {sha} | 검증: lint 4f0c1a2 ERROR 0 WARNING 1 | 후속: 없음"
        );
        assert_eq!(body_shas(&body), [sha]);
        assert_eq!(
            body_key(&body).as_ref().map(TaskKey::as_str),
            Some("PAY-12")
        );
        // 구분 글자와 짧은 hex, 낱말 속의 hex는 받지 않는다
        assert_eq!(
            body_shas("commit: abc1234, 커밋=DEF5678 sha abc12 commitabcdefg1"),
            ["abc1234", "def5678"]
        );
        assert_eq!(body_key("[실패] PAY-12 | 사유: x"), None);
    }

    #[test]
    fn body_with_several_shas_takes_the_one_that_contains_the_rest() {
        let sha = |c: char| Sha::new(c.to_string().repeat(40)).unwrap();
        let (old, new) = (sha('a'), sha('b'));
        let body = format!("커밋 {old}, SHA {new}");
        let pick = |linear: bool| {
            sha_of(
                None,
                &body,
                None,
                &mut |hex| Sha::new(hex).ok(),
                &mut |_| None,
                &mut |older, newer| linear && older == &old && newer == &new,
                &mut Vec::new(),
            )
        };
        assert_eq!(pick(true), (Some(new.clone()), Some(ShaSource::Body)));
        // 서로 조상이 아니면 본문에서 고르지 않는다(역할이 없어 tip도 없다)
        assert_eq!(pick(false), (None, None));
    }

    /// lint 기록이 없는 보고의 알림은 까닭을 `--no-checks`로 단정하지 않고, 실패 보고에는 내지 않는다.
    #[test]
    fn report_without_a_lint_record_is_not_called_no_checks() {
        let notes = |outcome: &str| {
            let mut state = State::default();
            let dispatch = serde_json::json!({
                "id": "ctx_1", "request": "req_0000000000000001", "task_id": "task_1", "run": "run_1",
                "key": "A-1", "role": "dev", "purpose": "build",
                "tuple": {"agent": "claude", "model": "m", "effort": "medium"},
                "terminal": "term_1", "worktree": "/w", "session_mode": "fresh", "started_at": 10,
                "reported": {"branch": "fullops/dev", "sha": "a".repeat(40), "lint_head": null, "followups": [], "at": 11},
            });
            state
                .dispatches
                .push(serde_json::from_value(dispatch).unwrap());
            let msg = OrcaMessage {
                id: crate::model::MessageId::new("msg_1").unwrap(),
                kind: "worker_done".into(),
                from: None,
                subject: None,
                body: None,
                task_id: None,
                dispatch_id: DispatchId::new("ctx_1").ok(),
                outcome: Some(outcome.into()),
            };
            let run = RunId::new("run_1").unwrap();
            let note = record_message(
                &mut state,
                &run,
                &msg,
                20,
                &mut |_| None,
                &mut |_| None,
                &mut |_, _| false,
            );
            note.notes.join(" ")
        };
        let text = notes("succeeded");
        assert!(
            text.contains("코드 변경이 없거나 `--no-checks`") && !text.contains("lint 없이 끝낸")
        );
        assert_eq!(notes("failed"), "");
    }

    #[test]
    fn metrics_count_dispatches_that_follow_without_a_human_turn() {
        let line = |json: &str| serde_json::from_str::<ArchiveLine>(json).unwrap();
        let turn = |at: i64, kind: TurnKind| Turn {
            at,
            sid: crate::model::SessionId::new("s").unwrap(),
            kind,
        };
        // A-1: 10에 배정, 20에 보고. B-2: 30에 배정(사람 턴 없이 이어짐). C-3: 50에 배정(40에 사람 턴)
        let archive = [
            line(
                r#"{"key":"A-1","role":"dev","sha":null,"received_at":20,"landed_at":25,"rounds":0,"dispatches":[{"purpose":"build","started_at":10,"reason":null,"sub":false},{"purpose":"build","started_at":12,"reason":null,"sub":true}],"direct":false}"#,
            ),
            line(
                r#"{"key":"B-2","role":"dev","sha":null,"received_at":35,"landed_at":null,"rounds":1,"dispatches":[{"purpose":"build","started_at":30,"reason":null,"sub":false}],"direct":false}"#,
            ),
            line(
                r#"{"key":"C-3","role":"dev","sha":null,"received_at":60,"landed_at":61,"rounds":0,"dispatches":[{"purpose":"review","started_at":45,"reason":null,"sub":false},{"purpose":"fix","started_at":50,"reason":null,"sub":false}],"direct":false}"#,
            ),
        ];
        let turns = [
            turn(5, TurnKind::Human),
            turn(40, TurnKind::Human),
            turn(41, TurnKind::Subagent),
        ];
        let m = metrics(&turns, &State::default(), &archive);
        assert_eq!((m.human_turns, m.since, m.done_tasks), (2, 5, 2));
        assert_eq!(m.turns_per_task, Some(1.0));
        // 첫 배정은 세지 않는다. C-3의 fix는 바로 앞 사건(45의 리뷰 배정) 뒤에 사람 턴이 없어 이어진 것이다
        assert_eq!((m.unattended_dispatches, m.unattended_tasks), (2, 2));
        assert_eq!(
            (m.subagent_calls, m.workflow_runs, m.sub_dispatches),
            (1, 0, 1)
        );
    }

    #[test]
    fn leftover_lists_design_written_inboxes_and_holds_the_tester_back() {
        let inbox = |role: &str, from: &str| Inbox {
            key: TaskKey::new("F-1").unwrap(),
            role: RoleId::new(role).unwrap(),
            goal: "g".into(),
            base: Sha::new("a".repeat(40)).unwrap(),
            route: RouteClass::Design,
            from: from.into(),
            round: 0,
            budget: 2,
            test_level: TestLevel::Lite,
            parallel: Parallel::On,
        };
        let boxes = [
            (inbox("dev", "architecture"), RoleKind::Worker, false),
            (inbox("qa", "architecture"), RoleKind::Tester, false),
        ];
        let state = |settled: &str| -> State {
            serde_json::from_str(&format!(
                r#"{{"v":1,"dispatches":[{{"id":"ctx_1","request":"r","task_id":"t","run":"run_1","key":"F-1","role":"dev","purpose":"build",
                "tuple":{{"agent":"claude","model":"m","effort":"medium"}},"terminal":null,"worktree":"w","session_mode":"fresh","reason":null,
                "started_at":10,"bound_session":null,"sub_of":null,"brief":null,"reported":null,"settled":{settled}}}]}}"#
            ))
            .unwrap()
        };
        let listed = |state: &State, boxes: &[(Inbox, RoleKind, bool)]| -> Vec<bool> {
            let out = leftover(state, &Plan::default(), boxes);
            ["--role dev", "--role qa"]
                .iter()
                .map(|role| out.iter().any(|line| line.contains(role)))
                .collect()
        };
        // 구현 지시서가 먼저고 tester의 지시서는 구현이 끝난 뒤다
        assert_eq!(listed(&State::default(), &boxes), [true, false]);
        assert_eq!(listed(&state("null"), &boxes), [false, false]);
        let done = r#"{"how":"done","outcome":"succeeded","message":"msg_1","at":20}"#;
        assert_eq!(listed(&state(done), &boxes), [false, true]);
        // 구현 보고가 아직 병합되지 않았으면(그 차수는 역할 브랜치에서 끝났다) tester의 지시서는 기다린다
        let mut unmerged = state(done);
        unmerged.completions = serde_json::from_str(
            r#"[{"id":"msg_1","dispatch":"ctx_1","key":"F-1","role":"dev","branch":null,"sha":null,"sha_source":null,
            "outcome":"succeeded","received_at":20,"hold":null,"released_hold":null,"landed_at":null,"drift":null}]"#,
        )
        .unwrap();
        let mut finished = boxes.clone();
        finished[0].2 = true;
        assert_eq!(listed(&unmerged, &finished), [false, false]);
        // 설계 역할이 쓴 다음 차수 지시서는 앞 차수의 배정으로 배정된 것이 아니다
        let next = [(
            Inbox {
                round: 1,
                ..inbox("dev", "architecture")
            },
            RoleKind::Worker,
            false,
        )];
        assert_eq!(listed(&unmerged, &next), [true, false]);
        let mut fixing = unmerged.clone();
        let mut again = fixing.dispatches[0].clone();
        again.id = DispatchId::new("ctx_2").unwrap();
        again.started_at = 30;
        again.settled = None;
        fixing.dispatches.push(again);
        assert_eq!(listed(&fixing, &next), [false, false]);
        // lead가 쓰고 있는 인박스는 세지 않는다
        let own = [(inbox("dev", "lead"), RoleKind::Worker, false)];
        assert_eq!(listed(&State::default(), &own), [false, false]);
    }

    #[test]
    fn orca_effort_is_ultracode_only_for_new_claude_terminals() {
        let tuple = |agent: &str| ModelTuple {
            agent: agent.into(),
            model: "m".into(),
            effort: "medium".into(),
        };
        assert_eq!(
            orca_effort(&tuple("claude"), Parallel::Ultra, true),
            "ultracode"
        );
        assert_eq!(
            orca_effort(&tuple("claude"), Parallel::Ultra, false),
            "medium"
        );
        assert_eq!(
            orca_effort(&tuple("codex"), Parallel::Ultra, true),
            "medium"
        );
        assert_eq!(orca_effort(&tuple("claude"), Parallel::On, true), "medium");
    }
}
