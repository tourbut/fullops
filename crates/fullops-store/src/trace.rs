//! `trace/<sid>.jsonl`(설계 6.3절). 묶음 B가 채운다.

use crate::repo::Repo;
use fullops_core::model::{SessionId, TraceLine};

/// 추적 한 줄을 더한다. 실패는 버린다(hook의 판정에 영향을 주지 않는다).
pub fn append_trace(_repo: &Repo, _sid: &SessionId, _line: &TraceLine) {}
