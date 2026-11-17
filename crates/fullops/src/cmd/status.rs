//! `fullops status [--fetch]`: `status.json`과 같은 내용을 낸다(설계 8.13절). 통합 대기의 `landing`과 지표를 다시 계산한다.
//! SessionStart 안내가 모델에게 닿지 않는 호스트와 hook이 돌지 않는 세션에서는 이 출력의 첫머리가 S-01을 대신한다.

use crate::assets::PLANS_MD;
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::layout;
use fullops_core::messages::{self, flow::METRICS_CAVEAT};
use fullops_core::model::{
    BriefAt, Derived, Known, Landing, MessageId, Mode, Parallel, RoleId, RunOwner, Seat, Status,
    TaskKey,
};
use fullops_ext::{Git, GitCli};
use lexopt::prelude::*;
use std::collections::BTreeMap;

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let mut fetch = false;
    while let Some(arg) = args.next()? {
        match arg {
            Long("fetch") => fetch = true,
            other => return Err(other.unexpected().into()),
        }
    }
    let repo = ctx.repo()?;
    // 정책 오류에서도 돈다: 설정이 필요한 부분만 건너뛴다
    let config = ctx.config().ok();
    let git = GitCli::cli(repo.root.clone());
    if let (true, Some(remote)) = (fetch, config.and_then(|c| c.remote.as_ref())) {
        git.fetch(remote, config.map_or("", |c| c.base.as_str()))?;
    }
    let state = fullops_store::read_state(repo).yes().unwrap_or_default();

    // 통합 위치(설계 6.6절). `settle`과 같은 계산을 쓴다. 직접 작업의 기록은 통합 대기가 아니라 뺀다
    let mut derived = Derived::default();
    let mut landed: Vec<(MessageId, Option<u32>)> = Vec::new();
    if let Some(config) = config {
        let direct = |id: &str| {
            let same = |c: &&fullops_core::model::Completion| c.id.as_str() == id;
            state
                .completions
                .iter()
                .find(same)
                .is_some_and(|c| c.direct)
        };
        (landed, derived) = super::hold::recompute(repo, &git, config, &state);
        landed.retain(|(id, _)| !direct(id.as_str()));
        derived.landing.retain(|id, _| !direct(id));
    }
    // 지표는 잠금 밖에서 계산한다(두 파일이 영구히 자란다)
    let turns = fullops_store::read_turns(repo).yes().unwrap_or_default();
    let archive = fullops_store::read_archive(repo).yes().unwrap_or_default();
    let mut metrics = fullops_core::metrics(&turns, &state, &archive);
    metrics.at = fullops_store::now();
    derived.metrics = Some(metrics);

    let caller = ctx.caller().ok();
    let lead = caller.as_ref().is_some_and(|c| c.actor.is_lead());
    let plan = fullops_store::read_plan(repo, PLANS_MD)
        .yes()
        .unwrap_or_default();
    if lead {
        let keys = fullops_store::archive_keys(repo).yes().unwrap_or_default();
        derived.ready = Some(fullops_core::ready(&plan, &state, &keys, &|role| {
            repo.inbox_path(role).exists()
        }));
    }
    // 통합이 확인된 보고를 닫는다: landed_at과 drift를 쓰고, 보류를 풀고, 그 보고의 메시지에 처리 표시를 한다
    ctx.update_state(Some(&derived), |s| {
        let now = fullops_store::now();
        for (id, drift) in &landed {
            super::hold::mark_landed(s, id, *drift, now);
        }
    })?;
    let Some(status) = fullops_store::read_json::<Status>(&repo.status_path())
        .yes()
        .flatten()
    else {
        return Err(CliError::Internal("status.json을 읽지 못했다.".into()));
    };
    if ctx.json {
        ctx.print("", serde_json::to_value(&status).unwrap_or_default());
        return Ok(EXIT_OK);
    }

    // 첫머리: 역할, 현재 인박스, 병렬 방침
    let mut out: Vec<String> = Vec::new();
    let actor = caller.as_ref().map(|c| &c.actor);
    let mut inbox_role: Option<RoleId> = None;
    out.push(match actor.map(|a| &a.seat) {
        Some(Seat::Lead) if actor.is_some_and(|a| a.dispatch.is_some()) => {
            "역할: dispatch를 받은 세션(lead 규칙을 적용하지 않는다)".into()
        }
        Some(Seat::Lead) if config.is_some_and(|c| c.mode == Mode::Dev) => {
            inbox_role = config.and_then(|c| c.primary_role.clone());
            "역할: dev 주 담당(직접 구현하고 통합까지 책임진다)".into()
        }
        Some(Seat::Lead) => "역할: lead(coordinator). 분류·배정·병합을 책임진다".into(),
        Some(Seat::Role { id, kind }) => {
            inbox_role = Some(id.clone());
            format!("역할: {id}({kind}). 현재 과제는 인박스 하나이고 끝나면 `fullops done`")
        }
        Some(Seat::Reviewer) => "역할: 읽기 전용 리뷰 사본이다".into(),
        Some(Seat::Other) => "역할: 이 체크아웃에는 역할 규칙이 적용되지 않는다".into(),
        None => "역할: 설정을 읽지 못해 정하지 못했다(`fullops doctor`)".into(),
    });
    if actor.is_some_and(|a| a.sub) {
        out.push("하위 worker다. 커밋·lint·`fullops done`은 부모가 한다. 맡은 범위를 끝내면 preamble의 `worker_done`을 보낸다".into());
    }
    if let Some(role) = &inbox_role {
        out.push(match fullops_store::read_inbox(repo, role) {
            Known::Yes(Some(inbox)) => {
                format!("인박스: {} (과제 {})", layout::inbox(role), inbox.key)
            }
            Known::Yes(None) => "인박스: 없음".into(),
            Known::Unknown(_) => format!(
                "인박스: {}의 front matter를 읽을 수 없다",
                layout::inbox(role)
            ),
        });
    }
    if let Some(config) = config {
        let host = caller
            .as_ref()
            .and_then(|c| c.session.as_ref())
            .map(|s| s.host);
        out.push(messages::parallel_brief(
            config.parallel,
            host,
            BriefAt::Session,
        ));
    } else if let Err(e) = ctx.config() {
        out.push(format!("설정을 쓸 수 없다: {e:?}"));
    }

    // lead 항목: 준비된 행, 열린 결정, 통합 대기·보류, 열린 Run
    if lead {
        let goal = |key: &TaskKey| {
            let row = plan.rows.iter().find(|r| &r.key == key);
            row.map(|r| r.goal.chars().take(60).collect::<String>())
                .unwrap_or_default()
        };
        let ready: Vec<String> = status
            .ready
            .iter()
            .take(10)
            .map(|k| format!("{k}({})", goal(k)))
            .collect();
        if !ready.is_empty() {
            let together =
                config.is_some_and(|c| matches!(c.parallel, Parallel::On | Parallel::Ultra));
            out.push(format!(
                "준비된 행: {}. 계속할지 묻지 않고 배정한다.{}",
                ready.join(", "),
                if together {
                    " 담당 역할이 서로 다른 행은 한 턴에 함께 배정한다."
                } else {
                    ""
                }
            ));
        }
        for d in &status.decisions {
            out.push(format!(
                "열린 결정 {}: {} (막는 과제 {})",
                d.id,
                d.question,
                d.key.as_ref().map_or("-", TaskKey::as_str)
            ));
        }
        let held = status
            .pending
            .iter()
            .filter(|p| p.hold_reason.is_some())
            .count();
        // 이어받을 Run은 lead가 소유한 것뿐이다. worker의 하위 Run을 lead가 받으면 부모 대신 보고를 소비한다
        let runs: Vec<&str> = state
            .runs
            .iter()
            .filter(|r| r.closed_at.is_none() && r.owner == RunOwner::Lead)
            .map(|r| r.id.as_str())
            .collect();
        out.push(format!(
            "통합 대기 {}건, 보류 {held}건, 예약된 동기화 {}건, 열린 Run: {}",
            status.pending.len() - held,
            status.sync_pending.len(),
            if runs.is_empty() {
                "없음".into()
            } else {
                format!(
                    "{} (`fullops wait --run <R>`로 이어받는다)",
                    runs.join(", ")
                )
            }
        ));
    }

    if let (Some(config), Some(actor)) = (config, actor) {
        let left = super::dispatch::leftover(repo, config, &state, actor);
        if !left.is_empty() {
            out.push(format!("이어서 할 일: {}", left.join(", ")));
        }
    }

    for t in &status.tasks {
        out.push(format!(
            "과제 {} | {} | {} | 재작업 {}/{}{}{}",
            t.key,
            t.role.as_ref().map_or("-", RoleId::as_str),
            t.stage,
            t.rounds,
            t.budget,
            t.sha
                .as_ref()
                .map(|s| format!(" | {s}"))
                .unwrap_or_default(),
            if t.held { " | 보류" } else { "" }
        ));
    }
    for d in status.dispatches.iter().filter(|d| !d.settled) {
        out.push(format!(
            "배정 {} → {} | {} | {}/{}{}",
            d.key,
            d.role,
            d.purpose,
            d.agent,
            d.model,
            if d.sub { " | 하위 배정" } else { "" }
        ));
    }
    let landing: BTreeMap<&MessageId, &str> = status
        .pending
        .iter()
        .map(|p| {
            (
                &p.id,
                p.landing.map_or("미확인", |l| match l {
                    Landing::MergeLeft => "병합 전",
                    Landing::PushLeft => "push 전",
                    Landing::Landed => "통합됨",
                }),
            )
        })
        .collect();
    for p in &status.pending {
        out.push(format!(
            "통합 대기 {} | {} | {} | {}{}",
            p.id,
            p.key.as_ref().map_or("키 없음", TaskKey::as_str),
            p.sha.as_deref().unwrap_or("SHA 없음"),
            landing.get(&p.id).copied().unwrap_or("미확인"),
            p.hold_reason
                .as_ref()
                .map(|r| format!(" | 보류: {r}"))
                .unwrap_or_default()
        ));
    }
    let m = &status.metrics;
    out.push(format!(
        "지표: 사람 턴 {}, 완료 과제 {}{}, 이어진 배정 {}(과제 {}), 서브에이전트 호출 {}, 워크플로 실행 {}, 하위 배정 {} — {METRICS_CAVEAT}",
        m.human_turns,
        m.done_tasks,
        m.turns_per_task.map(|t| format!(", 과제당 사람 턴 {t:.1}")).unwrap_or_default(),
        m.unattended_dispatches,
        m.unattended_tasks,
        m.subagent_calls,
        m.workflow_runs,
        m.sub_dispatches
    ));
    println!("{}", out.join("\n"));
    Ok(EXIT_OK)
}
