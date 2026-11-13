//! `fullops decision`: 사용자 결정 기록(설계 8.10절). 묻기 전에 기록하고, 닫힌 결정은 덮어쓰지 않는다.

use super::hold::{commit, load_state, refused, task_key, value};
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::model::{Decision, DecisionId, DecisionStatus, Origin, Sha, State, TaskKey};
use fullops_ext::GitCli;
use lexopt::prelude::*;
use serde_json::json;

/// SHA에 고정된 결정이 열려 있는 시간(초).
const TTL: i64 = 24 * 60 * 60;

/// 새 결정의 내용. `merge`의 M-04도 이것으로 연다.
#[derive(Default)]
pub(super) struct Ask {
    pub key: Option<TaskKey>,
    pub question: String,
    pub options: Vec<String>,
    pub blocks: Vec<String>,
    pub sha: Option<Sha>,
    pub risks: Vec<String>,
    pub origin: Option<Origin>,
    pub allow: Vec<String>,
    pub repository: Option<String>,
}

/// 결정을 연다. `update_state`의 잠금 안에서 부른다.
pub(super) fn open(state: &mut State, ask: Ask, now: i64) -> DecisionId {
    let id = DecisionId::from_number(state.next_decision);
    state.next_decision += 1;
    state.decisions.push(Decision {
        id: id.clone(),
        status: DecisionStatus::Open,
        key: ask.key,
        question: ask.question,
        options: ask.options,
        blocks: ask.blocks,
        asked_at: now,
        origin: ask.origin.unwrap_or(Origin::User),
        expires_at: ask.sha.as_ref().map(|_| now + TTL),
        sha: ask.sha,
        risks: ask.risks,
        allow: ask.allow,
        answer: None,
        decided_by: None,
        via: None,
        closed_at: None,
        repository: ask.repository,
        drop_reason: None,
        extra: Default::default(),
    });
    id
}

pub(super) fn expired(decision: &Decision, now: i64) -> bool {
    decision.expires_at.is_some_and(|at| now > at)
}

fn origin(text: &str) -> Result<Origin, CliError> {
    Origin::parse(text)
        .ok_or_else(|| CliError::Usage(format!("user 또는 bot이어야 합니다: {text}")))
}

fn sha(ctx: &Ctx, text: &str) -> Result<Sha, CliError> {
    match Sha::new(text) {
        Ok(sha) => Ok(sha),
        Err(_) => commit(&GitCli::cli(ctx.repo()?.root.clone()), text)
            .ok_or_else(|| CliError::Usage(format!("`{text}`를 커밋으로 풀지 못했습니다."))),
    }
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let verb = match args.next()? {
        Some(Value(v)) => v.string()?,
        _ => {
            return Err(CliError::Usage(
                "사용법: fullops decision add|answer|drop|list".into(),
            ));
        }
    };
    match verb.as_str() {
        "add" => add(ctx, args),
        "answer" | "drop" => close(ctx, args, verb == "drop"),
        "list" => list(ctx, args),
        other => Err(CliError::Usage(format!("모르는 하위 명령입니다: {other}"))),
    }
}

fn add(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let mut ask = Ask::default();
    while let Some(arg) = args.next()? {
        match arg {
            Long("question") => ask.question = value(&mut args)?,
            Long("key") => ask.key = Some(task_key(Some(value(&mut args)?))?),
            Long("option") => ask.options.push(value(&mut args)?),
            Long("blocks") => ask.blocks.push(value(&mut args)?),
            Long("sha") => ask.sha = Some(sha(ctx, &value(&mut args)?)?),
            Long("risk") => ask.risks.push(value(&mut args)?),
            Long("origin") => ask.origin = Some(origin(&value(&mut args)?)?),
            Long("allow") => ask.allow.push(value(&mut args)?),
            Long("repository") => ask.repository = Some(value(&mut args)?),
            other => return Err(other.unexpected().into()),
        }
    }
    if ask.question.trim().is_empty() {
        return Err(CliError::Usage("--question이 필요합니다.".into()));
    }
    let now = fullops_store::now();
    let expires = ask.sha.as_ref().map(|_| now + TTL);
    let id = ctx.update_state(None, |state| open(state, ask, now))?;
    ctx.print(
        &format!("결정 {id}를 열었습니다. 답은 `fullops decision answer {id} --answer \"<답>\"`"),
        json!({ "id": id, "expires_at": expires }),
    );
    Ok(EXIT_OK)
}

/// `answer`와 `drop`. 이미 닫힌 결정은 덮어쓰지 않고 `already`로 답한다.
fn close(ctx: &Ctx, mut args: lexopt::Parser, drop: bool) -> Result<u8, CliError> {
    let (mut id, mut text, mut by, mut via, mut given) = (None, None, None, None, None);
    while let Some(arg) = args.next()? {
        match arg {
            Long("answer") if !drop => text = Some(value(&mut args)?),
            Long("reason") if drop => text = Some(value(&mut args)?),
            Long("by") if !drop => by = Some(value(&mut args)?),
            Long("via") if !drop => via = Some(origin(&value(&mut args)?)?),
            Long("sha") if !drop => given = Some(sha(ctx, &value(&mut args)?)?),
            Value(v) if id.is_none() => id = Some(v.string()?),
            other => return Err(other.unexpected().into()),
        }
    }
    let (Some(id), Some(text)) = (id, text.filter(|t| !t.trim().is_empty())) else {
        return Err(CliError::Usage(if drop {
            "사용법: fullops decision drop <ID> --reason \"<사유>\"".into()
        } else {
            "사용법: fullops decision answer <ID> --answer \"<답>\" [--by <사람>] [--via user|bot] [--sha <SHA>]".into()
        }));
    };
    let state = load_state(ctx)?;
    let Some(decision) = state.decisions.iter().find(|d| d.id.as_str() == id) else {
        return Err(refused("DC-00", "", &[&id]));
    };
    let now = fullops_store::now();
    let already = |d: &Decision| {
        ctx.print(
            &format!(
                "결정 {id}는 이미 닫혔습니다(답: {}). 기록을 바꾸지 않았습니다.",
                d.answer.as_deref().unwrap_or("없음")
            ),
            json!({ "already": true, "id": d.id, "status": d.status, "answer": d.answer }),
        );
        Ok(EXIT_OK)
    };
    if decision.status != DecisionStatus::Open {
        return already(decision);
    }
    if !drop {
        if let Some(fixed) = &decision.sha
            && given.as_ref() != Some(fixed)
        {
            return Err(refused("DC-01", "", &[&id, fixed.as_str()]));
        }
        if expired(decision, now) {
            return Err(refused("DC-02", "", &[&id]));
        }
        let who = by.as_deref().unwrap_or("");
        if !decision.allow.is_empty() && !decision.allow.iter().any(|a| a == who) {
            return Err(refused(
                "DC-03",
                "",
                &[&id, who, &decision.allow.join(", ")],
            ));
        }
    }
    // 잠금 안에서 다시 본다: 그 사이 다른 쪽이 먼저 닫았으면 덮어쓰지 않는다
    let closed = ctx.update_state(None, |state| {
        let decision = state.decisions.iter_mut().find(|d| d.id.as_str() == id)?;
        if decision.status == DecisionStatus::Open {
            decision.closed_at = Some(now);
            if drop {
                decision.status = DecisionStatus::Dropped;
                decision.drop_reason = Some(text.clone());
            } else {
                decision.status = DecisionStatus::Answered;
                decision.answer = Some(text.clone());
                decision.decided_by = by.clone();
                decision.via = Some(via.unwrap_or(Origin::User));
            }
            return None;
        }
        Some(decision.clone())
    })?;
    if let Some(decision) = closed {
        return already(&decision);
    }
    let status = if drop { "dropped" } else { "answered" };
    ctx.print(
        &format!("결정 {id}를 닫았습니다({status}): {text}"),
        json!({ "id": id, "status": status, "answer": (!drop).then_some(&text) }),
    );
    Ok(EXIT_OK)
}

fn list(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let mut only_open = false;
    while let Some(arg) = args.next()? {
        match arg {
            Long("open") => only_open = true,
            other => return Err(other.unexpected().into()),
        }
    }
    let now = fullops_store::now();
    let decisions: Vec<Decision> = load_state(ctx)?
        .decisions
        .into_iter()
        .filter(|d| !only_open || d.status == DecisionStatus::Open)
        .collect();
    let lines: Vec<String> = decisions
        .iter()
        .map(|d| {
            let mut line = format!("{} [{}] {}", d.id, d.status, d.question);
            if let Some(key) = &d.key {
                line.push_str(&format!(" (과제 {key})"));
            }
            if !d.options.is_empty() {
                line.push_str(&format!(" 선택지: {}", d.options.join("/")));
            }
            if d.status == DecisionStatus::Open && expired(d, now) {
                line.push_str(" 만료됨: 다시 묻는다");
            }
            if let Some(answer) = &d.answer {
                line.push_str(&format!(" 답: {answer}"));
            }
            line
        })
        .collect();
    let human = if lines.is_empty() {
        "결정이 없습니다.".to_string()
    } else {
        lines.join("\n")
    };
    ctx.print(&human, json!({ "decisions": decisions }));
    Ok(EXIT_OK)
}
