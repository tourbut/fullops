//! `fullops why [--session <sid>] [--last <N>] [--rule <ID>]`: hook 추적과 이벤트별 실행 표식(설계 7.8·8.13절).
//! "기록 없음"은 정상이 아니라 확인 불가다: hook이 돌지 않았는지 이 명령으로는 알 수 없다.

use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::model::{EventKind, SessionId, TraceLine};
use fullops_store::Repo;
use lexopt::prelude::*;
use serde_json::json;

const EVENTS: [EventKind; 5] = [
    EventKind::SessionStart,
    EventKind::UserPromptSubmit,
    EventKind::PreToolUse,
    EventKind::Stop,
    EventKind::StopFailure,
];

/// 실행 표식이 가장 최근에 바뀐 세션.
fn latest(repo: &Repo) -> Option<SessionId> {
    std::fs::read_dir(repo.local().join("sessions"))
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let sid = SessionId::new(name.strip_suffix(".seen")?).ok()?;
            Some((entry.metadata().ok()?.modified().ok()?, sid))
        })
        .max_by_key(|(at, _)| *at)
        .map(|(_, sid)| sid)
}

fn describe(line: &TraceLine) -> String {
    let rules: Vec<String> = line
        .rules
        .iter()
        .map(|r| format!("{}={}", r.id, r.verdict))
        .collect();
    format!(
        "{} {} {} [{}] git {} orca {} {}ms{}",
        line.t,
        line.ev,
        line.actor.as_deref().unwrap_or("-"),
        rules.join(" "),
        line.git,
        line.orca,
        line.ms,
        line.err
            .as_ref()
            .map(|e| format!(" 오류: {e}"))
            .unwrap_or_default()
    )
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let (mut session, mut last, mut rule) = (None, 20usize, None);
    while let Some(arg) = args.next()? {
        match arg {
            Long("session") => session = Some(args.value()?.string()?),
            Long("last") => last = args.value()?.parse()?,
            Long("rule") => rule = Some(args.value()?.string()?),
            other => return Err(other.unexpected().into()),
        }
    }
    let repo = ctx.repo()?;
    // 세션: --session, 없으면 이 체크아웃에서 마지막으로 fullops를 부른 세션, 그것도 없으면 가장 최근에 hook이 돈 세션
    let sid = match session {
        Some(raw) => Some(
            SessionId::from_host(&raw)
                .ok_or_else(|| CliError::Usage("--session 값이 비어 있습니다.".into()))?,
        ),
        None => fullops_store::read_current(repo)
            .yes()
            .flatten()
            .map(|current| current.sid)
            .or_else(|| latest(repo)),
    };
    let Some(sid) = sid else {
        ctx.print(
            "hook 실행 기록이 없습니다(확인 불가: hook이 돌지 않았는지는 알 수 없습니다).",
            json!({"session": null, "events": {}, "trace": []}),
        );
        return Ok(EXIT_OK);
    };
    let all = fullops_store::trace::read_trace(repo, &sid);
    let mut events = serde_json::Map::new();
    let mut human = format!("세션 {sid}\n");
    for event in EVENTS {
        let status = if !repo.seen_path(&sid, event.as_str()).exists() {
            "기록 없음"
        } else if all.iter().any(|l| l.ev == event && l.err.is_some()) {
            "오류"
        } else {
            "실행됨"
        };
        human.push_str(&format!(
            "  {event}: {status}{}\n",
            if status == "기록 없음" {
                "(확인 불가)"
            } else {
                ""
            }
        ));
        events.insert(event.to_string(), json!(status));
    }
    let matches = |line: &&TraceLine| {
        rule.as_ref()
            .is_none_or(|id| line.rules.iter().any(|r| &r.id == id))
    };
    let picked: Vec<&TraceLine> = all.iter().filter(matches).collect();
    let shown = picked
        .get(picked.len().saturating_sub(last)..)
        .unwrap_or_default();
    if shown.is_empty() {
        human.push_str("추적 줄이 없습니다(통과한 호출은 `FULLOPS_TRACE=1`일 때만 남습니다).");
    }
    for line in shown {
        human.push_str(&describe(line));
        human.push('\n');
    }
    ctx.print(
        human.trim_end(),
        json!({"session": sid, "events": events, "trace": shown}),
    );
    Ok(EXIT_OK)
}
