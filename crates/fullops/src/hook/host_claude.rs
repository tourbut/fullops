//! Claude Code 어댑터(설계 7.2절 표의 `claude` 열). `pass`는 뼈대의 완성본이고 `decode`와 `encode`는 묶음 B가 채운다.

use super::{DecodeError, Host};
use fullops_core::model::{EventKind, HookInput, HostId, Verdict};

pub struct Claude;

impl Host for Claude {
    fn id(&self) -> HostId {
        HostId::Claude
    }

    fn decode(&self, _raw: &[u8], _event_arg: Option<&str>) -> Result<HookInput, DecodeError> {
        Err(DecodeError("unimplemented"))
    }

    // 뼈대: 어떤 판정이든 통과의 글을 낸다
    fn encode(&self, _event: EventKind, _v: &Verdict) -> String {
        super::EMPTY.to_string()
    }

    /// 모든 이벤트에서 `{}`다.
    fn pass(&self, _event_arg: Option<&str>) -> &'static str {
        super::EMPTY
    }
}
