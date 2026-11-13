//! agy(Antigravity CLI) 어댑터(설계 7.2절 표의 `agy` 열). `pass`는 뼈대의 완성본이고 `decode`와 `encode`는 묶음 B가 채운다.
//! agy는 입력에 이벤트 이름이 없어 등록 문자열이 `--event`를 준다. PreToolUse·Stop의 출력에는 `decision`이 필수다.

use super::{DecodeError, Host};
use fullops_core::model::{EventKind, HookInput, HostId, Verdict};

pub struct Agy;

/// PreToolUse의 통과 값. `allow`가 "의견 없음"으로 확인되면(미정-M11) 이 상수 하나를 바꾼다.
const PASS_PRE_TOOL_USE: &str = r#"{"decision":"ask"}"#;
/// Stop(StopFailure로 옮긴 Stop 포함)의 통과 값.
const PASS_STOP: &str = r#"{"decision":""}"#;

impl Host for Agy {
    fn id(&self) -> HostId {
        HostId::Agy
    }

    fn decode(&self, _raw: &[u8], _event_arg: Option<&str>) -> Result<HookInput, DecodeError> {
        Err(DecodeError("unimplemented"))
    }

    // 뼈대: 어떤 판정이든 그 이벤트의 통과 글을 낸다
    fn encode(&self, event: EventKind, _v: &Verdict) -> String {
        let event = match event {
            // 오류로 끝난 Stop을 옮긴 것이라 출력은 Stop의 것이다
            EventKind::StopFailure => EventKind::Stop,
            other => other,
        };
        self.pass(Some(event.as_str())).to_string()
    }

    fn pass(&self, event_arg: Option<&str>) -> &'static str {
        match event_arg {
            Some("PreToolUse") => PASS_PRE_TOOL_USE,
            Some("Stop") => PASS_STOP,
            _ => super::EMPTY,
        }
    }
}
