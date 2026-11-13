//! `fullops gc`: 닫힌 기록 정리(설계 6.8절). 열린 것은 건드리지 않는다.

use super::hold::archive_done;
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::model::{
    ArchiveLine, DecisionId, DecisionStatus, DispatchId, MessageId, RunId, RunOwner, Settled,
    SettledHow, SnapshotState, State, TaskKey,
};
use serde_json::json;
use std::collections::BTreeSet;

const DAY: i64 = 24 * 60 * 60;

/// 정리한 건수: 예약 기록, 하위 dispatch, 과제, 결정, snapshot, dispatch, 메시지, Run.
fn sweep(repo: &fullops_store::Repo, state: &mut State, now: i64) -> [usize; 8] {
    // Orca 응답을 받지 못한 채 24시간 지난 예약 기록
    let total = state.dispatches.len();
    state
        .dispatches
        .retain(|d| !(d.is_reservation() && now - d.started_at > DAY));
    let reservations = total - state.dispatches.len();

    // 하위 배정의 뒷정리: 부모 dispatch가 정산됐으면(또는 없으면) 남은 하위 dispatch를 lost로 닫고 그 부모의 Run을 닫는다
    let orphaned = |owner: &RunOwner| match owner {
        RunOwner::Dispatch(parent) => state.dispatch(parent).is_none_or(|p| p.settled.is_some()),
        RunOwner::Lead => false,
    };
    let orphans: Vec<RunId> = state
        .runs
        .iter()
        .filter(|r| r.closed_at.is_none() && orphaned(&r.owner))
        .map(|r| r.id.clone())
        .collect();
    let mut subs = 0;
    for dispatch in &mut state.dispatches {
        let left = dispatch.settled.is_none() && !dispatch.is_reservation();
        if left && orphans.contains(&dispatch.run) {
            dispatch.settled = Some(Settled {
                how: SettledHow::Lost,
                outcome: None,
                message: None,
                at: now,
                extra: Default::default(),
            });
            subs += 1;
        }
    }
    for run in state.runs.iter_mut().filter(|r| orphans.contains(&r.id)) {
        run.closed_at = Some(now);
    }

    let tasks = archive_done(repo, state).len();

    // 닫힌 지 30일 지난 결정은 archive.jsonl에 줄로 남긴다
    let old: Vec<DecisionId> = state
        .decisions
        .iter()
        .filter(|d| d.status != DecisionStatus::Open)
        .filter(|d| d.closed_at.is_some_and(|at| now - at > 30 * DAY))
        .filter(|d| {
            let line = ArchiveLine::Decision {
                decision: (*d).clone(),
            };
            fullops_store::append_archive(repo, &line).is_ok()
        })
        .map(|d| d.id.clone())
        .collect();
    state.decisions.retain(|d| !old.contains(&d.id));

    let total = state.snapshots.len();
    let closed = |s: SnapshotState| {
        matches!(
            s,
            SnapshotState::Removed | SnapshotState::Abandoned | SnapshotState::Missing
        )
    };
    state.snapshots.retain(|s| !closed(s.state));
    let snapshots = total - state.snapshots.len();

    // 완료 보고를 만들지 않는 정산된 dispatch: 조사·리뷰(그 과제에 남은 일이 없을 때)와 부모가 없어진 하위 배정
    let reported: BTreeSet<&TaskKey> = state
        .completions
        .iter()
        .filter_map(|c| c.key.as_ref())
        .collect();
    let working: BTreeSet<&TaskKey> = state
        .dispatches
        .iter()
        .filter(|d| d.settled.is_none() && !d.is_reservation())
        .map(|d| &d.key)
        .collect();
    let unread: BTreeSet<&DispatchId> = state
        .messages
        .iter()
        .filter(|m| m.acked_at.is_none())
        .filter_map(|m| m.dispatch.as_ref())
        .collect();
    let ids: BTreeSet<&DispatchId> = state.dispatches.iter().map(|d| &d.id).collect();
    let spent: Vec<DispatchId> = state
        .dispatches
        .iter()
        .filter(|d| d.settled.is_some() && !unread.contains(&d.id))
        .filter(|d| match &d.sub_of {
            Some(parent) => !ids.contains(parent),
            None => !d.is_work() && !reported.contains(&d.key) && !working.contains(&d.key),
        })
        .map(|d| d.id.clone())
        .collect();
    state.dispatches.retain(|d| !spent.contains(&d.id));

    // 처리 표시가 있고 dispatch도 완료 보고도 더는 없는 메시지
    let total = state.messages.len();
    let ids: BTreeSet<DispatchId> = state.dispatches.iter().map(|d| d.id.clone()).collect();
    let reports: BTreeSet<MessageId> = state.completions.iter().map(|c| c.id.clone()).collect();
    state.messages.retain(|m| {
        let referenced =
            m.dispatch.as_ref().is_some_and(|d| ids.contains(d)) || reports.contains(&m.id);
        m.acked_at.is_none() || referenced
    });
    let messages = total - state.messages.len();

    // 닫힌 Run 가운데 dispatch도 메시지도 남지 않은 것
    let total = state.runs.len();
    let used: BTreeSet<RunId> = state
        .dispatches
        .iter()
        .map(|d| d.run.clone())
        .chain(state.messages.iter().map(|m| m.run.clone()))
        .collect();
    state
        .runs
        .retain(|r| r.closed_at.is_none() || used.contains(&r.id));
    let runs = total - state.runs.len();

    [
        reservations,
        subs,
        tasks,
        old.len(),
        snapshots,
        spent.len(),
        messages,
        runs,
    ]
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    if let Some(arg) = args.next()? {
        return Err(arg.unexpected().into());
    }
    let repo = ctx.repo()?;
    let now = fullops_store::now();
    let n = ctx.update_state(None, |state| sweep(repo, state, now))?;
    ctx.print(
        &format!(
            "정리: 예약 기록 {}건, 하위 dispatch {}건(lost), 과제 {}건(archive), 결정 {}건(archive), snapshot {}건, dispatch {}건, 메시지 {}건, Run {}건.",
            n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7]
        ),
        json!({
            "reservations": n[0], "sub_dispatches_lost": n[1], "tasks_archived": n[2],
            "decisions_archived": n[3], "snapshots": n[4], "dispatches": n[5],
            "messages": n[6], "runs": n[7],
        }),
    );
    Ok(EXIT_OK)
}
