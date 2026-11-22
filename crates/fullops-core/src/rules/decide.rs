//! 이벤트 하나의 판정(설계 7.1절 6번, 7.5·7.6절). 규칙 표를 순서대로 평가해 판정 하나와 효과 목록을 만든다.

use super::call::{self, NO_SESSION, Scene};
use super::{Effect, Finding, Outcome, RULES, Strength};
use crate::facts::Facts;
use crate::layout;
use crate::messages::{self, hook};
use crate::model::{
    Actor, EventKind, HookInput, Intent, Known, RoleKind, RuleTrace, Seat, ToolKind, Verdict,
    sha256_hex,
};

/// 인박스 폴더와 과제 폴더(QA 증거가 그 아래에 있다). 이 아래를 고치는 호출이 T-05·T-06·T-10의 후보다.
pub(super) const INBOX: &str = layout::RECORDS[1];
pub(super) const TASKS: &str = layout::RECORDS[2];

/// 이벤트 하나를 판정한다. `scene`은 `Facts`에 없는 호출 문맥이다(레포 루트, Orca 실행 파일 이름 등).
pub fn decide_in(scene: &Scene, input: &HookInput, facts: &mut dyn Facts) -> Outcome {
    call::begin(scene, input);
    let mut trace = Vec::new();
    let verdict = run(input, facts, &mut trace);
    let effects = call::with(|call| std::mem::take(&mut call.effects));
    Outcome {
        verdict,
        effects,
        trace,
    }
}

/// 이벤트 하나를 판정한다. 레포 루트는 `cwd`에서 위로 올라가며 `.git`을 찾아 정하고 Orca 이름은 기본값만 본다.
/// hook 진입점은 문맥을 아는 `decide_in`을 부른다.
pub fn decide(input: &HookInput, facts: &mut dyn Facts) -> Outcome {
    let root = input
        .cwd
        .ancestors()
        .find(|dir| facts.path_exists(&dir.join(".git")) == Known::Yes(true))
        .unwrap_or(&input.cwd)
        .to_path_buf();
    let scene = Scene {
        root,
        ..Scene::default()
    };
    decide_in(&scene, input, facts)
}

fn mark(id: &str, verdict: &str, fp: &str) -> RuleTrace {
    RuleTrace {
        id: id.to_string(),
        verdict: verdict.to_string(),
        fp: (!fp.is_empty()).then(|| fp.to_string()),
    }
}

fn with_escape(reason: String, escape: &str) -> String {
    if escape.is_empty() {
        reason
    } else {
        format!("{reason} 다음: {escape}")
    }
}

/// 세션당 한 번 알릴 것의 키(설계 7.8절). lead에게만 알린다. 이미 알린 것은 뺀다.
pub(super) fn notice_keys(facts: &mut dyn Facts) -> Vec<&'static str> {
    if !call::actor(facts).yes().is_some_and(|a| a.is_lead()) {
        return Vec::new();
    }
    let mut keys = Vec::new();
    match facts.state() {
        Known::Unknown("state_newer") => keys.push("state-newer"),
        Known::Unknown("state_broken") => keys.push("state-broken"),
        _ => {}
    }
    if facts
        .plan()
        .yes()
        .is_some_and(|p| !p.parse_errors.is_empty())
    {
        keys.push("plan-parse");
    }
    keys.retain(|key| !call::noticed(facts, key));
    keys
}

/// PreToolUse의 규칙 후보가 있는가(설계 7.4절 끝). 없으면 설정·HEAD·세션 파일만 읽고 끝난다.
fn candidate(input: &HookInput, facts: &mut dyn Facts) -> bool {
    let Some(tool) = &input.tool else {
        return false;
    };
    if matches!(
        tool.kind,
        ToolKind::SubAgent | ToolKind::Workflow | ToolKind::Ask
    ) {
        return true;
    }
    let relevant = |i: &Intent| !matches!(i, Intent::Other | Intent::Unparsed);
    if call::intents().iter().any(relevant) {
        return true;
    }
    let paths = call::target_paths(input, facts);
    if paths
        .iter()
        .any(|p| p.starts_with(INBOX) || p.starts_with(TASKS))
    {
        return true;
    }
    if facts.policy_error().is_some() {
        return true;
    }
    // 설계 역할의 편집. 설계 역할이 없는 레포에서는 좌석을 보지 않는다(state.json을 읽지 않는다)
    !paths.is_empty()
        && facts.config().is_ok_and(|c| c.design_role().is_some())
        && call::actor(facts).yes().is_some_and(|a| {
            matches!(
                a.seat,
                Seat::Role {
                    kind: RoleKind::Design,
                    ..
                }
            )
        })
}

fn run(input: &HookInput, facts: &mut dyn Facts, trace: &mut Vec<RuleTrace>) -> Verdict {
    // Stop의 공통 전제: 정상 종료가 아니면 전부 통과한다
    let abnormal = |reason: &str| reason != "end_turn";
    if input.event == EventKind::Stop && input.stop_reason.as_deref().is_some_and(abnormal) {
        return Verdict::Pass;
    }
    // 프롬프트 이벤트가 없는 호스트: 사용자 입력의 지문이 바뀐 첫 호출에서 P 규칙을 먼저 평가한다
    let promptless = !facts.caps().prompt_event && input.event != EventKind::UserPromptSubmit;
    let new_fp = input
        .prompt
        .as_deref()
        .filter(|_| promptless)
        .and_then(|p| sha256_hex(p.as_bytes()).get(..16).map(str::to_string))
        .filter(|fp| match facts.session() {
            Known::Yes(session) => session.prompt_fp.as_deref() != Some(fp),
            Known::Unknown(why) => why == NO_SESSION,
        });
    if input.event == EventKind::PreToolUse && new_fp.is_none() && !candidate(input, facts) {
        return Verdict::Pass;
    }
    if let Some(fp) = new_fp {
        call::ensure_session(facts);
        let prompt = HookInput {
            event: EventKind::UserPromptSubmit,
            ..input.clone()
        };
        evaluate(&prompt, facts, trace);
        call::effect(Effect::SetPromptFp(fp));
    }
    evaluate(input, facts, trace)
}

fn evaluate(input: &HookInput, facts: &mut dyn Facts, trace: &mut Vec<RuleTrace>) -> Verdict {
    let caps = facts.caps();
    let mut contexts: Vec<String> = Vec::new();
    let mut blocks: Vec<(&'static str, String)> = Vec::new();
    let mut warns: Vec<String> = Vec::new();
    let mut policy: Option<Vec<String>> = None;
    let mut briefed = false;

    for rule in RULES.iter().filter(|r| r.event == input.event) {
        // X-07은 X-00~X-06에서 Block도 Warn도 없을 때만 평가한다
        if rule.id == "X-07" && !(blocks.is_empty() && warns.is_empty()) {
            continue;
        }
        // 호출자를 모르면(설정을 못 읽었다) 대상이 "전부"인 규칙만 평가한다
        let actor = call::actor(facts).yes().unwrap_or(Actor {
            seat: Seat::Other,
            dispatch: None,
            direct_dev: false,
            sub: false,
        });
        if !(rule.applies)(&actor) {
            continue;
        }
        let (fp, args, case, always) = match (rule.check)(input, facts) {
            Finding::None => {
                trace.push(mark(rule.id, "pass", ""));
                continue;
            }
            Finding::Unknown => {
                trace.push(mark(rule.id, "unknown", ""));
                continue;
            }
            Finding::Hit {
                fp,
                args,
                case,
                always,
            } => (fp, args, case, always),
        };
        match rule.strength {
            Strength::Context | Strength::Effect => {
                let kind = if rule.strength == Strength::Context {
                    "context"
                } else {
                    "effect"
                };
                trace.push(mark(rule.id, kind, &fp));
                match rule.id {
                    // 정책 오류면 아래 규칙은 건너뛴다
                    "S-00" => {
                        policy = Some(args);
                        break;
                    }
                    "S-01" => briefed = true,
                    _ if !args.is_empty() => contexts.push(messages::reason(rule.message, &args)),
                    _ => {}
                }
            }
            Strength::Deny => {
                trace.push(mark(rule.id, "deny", &fp));
                let reason = messages::reason(rule.message, &args);
                return Verdict::Deny {
                    rule: rule.id,
                    reason: with_escape(reason, rule.escape(case)),
                };
            }
            Strength::BlockOnce | Strength::BlockAlways => {
                let reason = messages::reason(rule.message, &args);
                let once = rule.strength == Strength::BlockOnce && !always;
                if once && call::blocked_before(facts, rule.id, &fp) {
                    trace.push(mark(rule.id, "warn", &fp));
                    warns.push(hook::passed_once(rule.id, &reason));
                    continue;
                }
                if once {
                    call::ensure_session(facts);
                    call::effect(Effect::RecordBlock {
                        rule: rule.id,
                        fp: fp.clone(),
                    });
                }
                trace.push(mark(rule.id, "block", &fp));
                blocks.push((rule.id, with_escape(reason, rule.escape(case))));
            }
        }
    }

    match input.event {
        EventKind::SessionStart => {
            if let Some(args) = policy {
                return if caps.context_at_start {
                    Verdict::Context(messages::reason(messages::MsgId("S-00"), &args))
                } else {
                    Verdict::Warn(format!("정책 오류: {}", args.join(" ")))
                };
            }
            let (brief, skill) = call::with(|call| (call.brief.take(), call.skill.take()));
            if !caps.context_at_start {
                // 닿지 않는 안내는 만들지 않는다. 사용자에게 한 줄만 낸다
                return if briefed {
                    Verdict::Warn(hook::NO_CONTEXT_AT_START.into())
                } else {
                    Verdict::Pass
                };
            }
            let mut parts: Vec<String> = brief
                .iter()
                .map(|brief| hook::session_context(brief, skill.as_ref()))
                .collect();
            parts.append(&mut contexts);
            if parts.is_empty() {
                Verdict::Pass
            } else {
                Verdict::Context(parts.join("\n\n"))
            }
        }
        EventKind::UserPromptSubmit if !contexts.is_empty() => {
            Verdict::Context(contexts.join("\n\n"))
        }
        EventKind::Stop => {
            let mut blocks = blocks.into_iter();
            if let Some((rule, reason)) = blocks.next() {
                return Verdict::Block {
                    rule,
                    reason,
                    also: blocks.map(|(id, _)| hook::also_line(id)).collect(),
                };
            }
            for key in notice_keys(facts) {
                call::effect(Effect::RecordNotice(key.to_string()));
                warns.push(hook::notice(key).to_string());
            }
            if warns.is_empty() {
                Verdict::Pass
            } else {
                Verdict::Warn(warns.join(" "))
            }
        }
        _ => Verdict::Pass,
    }
}
