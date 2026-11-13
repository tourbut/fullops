//! UserPromptSubmit 규칙의 `check`(설계 7.5절). 프롬프트 이벤트가 없는 호스트에서는 `decide`가 사용자 입력이 바뀐 첫 호출에서 부른다.

use super::session::role_text;
use super::{Effect, Finding, call, hit};
use crate::facts::Facts;
use crate::model::{BoundBy, DispatchId, HookInput, Known, Mode, TurnKind};

/// Orca가 터미널 세션에 넣는 알림: `You have <N> orchestration message(s). Run …`
const ORCA_WAKE: (&str, &str) = ("You have", "orchestration message");
/// Orca가 채팅 방식 세션에 메시지를 통째로 넣을 때의 첫 줄(실제 글은 미정-M5).
const ORCA_CHAT: &str = "──── From:";
/// Claude Code의 백그라운드 작업 알림(hook의 `prompt`에 실리는지는 미정-M5).
const TASK_NOTIFICATION: &str = "<task-notification>";

/// P-02의 분류. 앞 공백과 줄바꿈을 떼고 위에서부터 먼저 맞는 것을 쓴다.
fn turn_kind(prompt: &str) -> TurnKind {
    let p = prompt.trim_start();
    if (p.starts_with(ORCA_WAKE.0) && p.contains(ORCA_WAKE.1)) || p.starts_with(ORCA_CHAT) {
        TurnKind::OrcaMessage
    } else if p.contains(TASK_NOTIFICATION) {
        TurnKind::TaskNotification
    } else {
        TurnKind::Human
    }
}

/// `<flag>` 뒤 공백 또는 `=`와 `[A-Za-z0-9_.:-]+`.
fn flag_value<'a>(prompt: &'a str, flag: &str) -> Option<&'a str> {
    let mut rest = prompt;
    while let Some(at) = rest.find(flag) {
        let after = rest.get(at + flag.len()..)?;
        let spaced = after.trim_start();
        let value = after
            .strip_prefix('=')
            .or((spaced.len() < after.len()).then_some(spaced));
        if let Some(value) = value {
            let end = value
                .find(|c: char| !(c.is_ascii_alphanumeric() || "_.:-".contains(c)))
                .unwrap_or(value.len());
            if end > 0 {
                return value.get(..end);
            }
        }
        rest = after;
    }
    None
}

/// P-00
pub(super) fn p00(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    call::ensure_session(facts);
    call::effect(Effect::BumpTurn);
    call::effect(Effect::TouchSession);
    call::with(|call| call.turn_bump = true);
    hit("", Vec::new())
}

/// P-01
pub(super) fn p01(input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let prompt = input.prompt.as_deref().unwrap_or_default();
    let Some(id) = flag_value(prompt, "--dispatch-id").and_then(|v| DispatchId::new(v).ok()) else {
        return Finding::None;
    };
    let bound = facts
        .session()
        .yes()
        .and_then(|s| s.dispatch.as_ref())
        .is_some_and(|d| d.id == id);
    if bound {
        return Finding::None;
    }
    call::ensure_session(facts);
    call::effect(Effect::BindDispatch {
        id: id.clone(),
        task: flag_value(prompt, "--task-id").map(str::to_string),
        by: BoundBy::Preamble,
    });
    call::with(|call| call.bound = Some(id.clone()));
    // dev 모드에서는 dispatch 받은 세션에 역할 안내를 준다
    let dev = facts.config().is_ok_and(|c| c.mode == Mode::Dev);
    let mut args = Vec::new();
    if dev
        && facts.caps().context_at_prompt
        && let Known::Yes(actor) = call::actor(facts)
    {
        args.push(role_text(facts, &actor));
    }
    hit(id.as_str(), args)
}

/// P-02
pub(super) fn p02(input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    let prompt = input.prompt.as_deref().unwrap_or_default();
    if flag_value(prompt, "--dispatch-id").is_some() {
        return Finding::None;
    }
    call::effect(Effect::RecordTurn(turn_kind(prompt)));
    hit("", Vec::new())
}

/// P-03
pub(super) fn p03(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let Some(error) = facts.policy_error() else {
        return Finding::None;
    };
    let key = format!("policy:{}", error.kind.as_str());
    // 세션당 한 번. systemMessage는 모델에게 가지 않으므로 Context로만 낸다
    if !facts.caps().context_at_prompt || call::noticed(facts, &key) {
        return Finding::None;
    }
    call::effect(Effect::RecordNotice(key));
    hit(error.kind.as_str(), vec![error.detail])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_preamble_ids_and_turn_kinds() {
        let p = "orca orchestration send --type worker_done --task-id task_1a --dispatch-id ctx_2a3b --outcome succeeded";
        assert_eq!(flag_value(p, "--dispatch-id"), Some("ctx_2a3b"));
        assert_eq!(flag_value(p, "--task-id"), Some("task_1a"));
        assert_eq!(
            flag_value("--dispatch-id=dsp:1.a x", "--dispatch-id"),
            Some("dsp:1.a")
        );
        assert_eq!(
            flag_value("--dispatch-id <id> --dispatch-id d1", "--dispatch-id"),
            Some("d1")
        );
        assert_eq!(flag_value("--dispatch-idx", "--dispatch-id"), None);

        assert_eq!(turn_kind("결제 버튼을 추가해 줘"), TurnKind::Human);
        assert_eq!(
            turn_kind("\nYou have 1 orchestration message. Run `orca orchestration check`.\n"),
            TurnKind::OrcaMessage
        );
        assert_eq!(
            turn_kind("──── From: DISPATCH (worker_done) ────\nSubject"),
            TurnKind::OrcaMessage
        );
        assert_eq!(
            turn_kind("<task-notification>done</task-notification>"),
            TurnKind::TaskNotification
        );
    }
}
