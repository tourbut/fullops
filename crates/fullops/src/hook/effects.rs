//! 판정 뒤의 효과 실행(설계 7.1절 8번). 실패는 추적에만 남긴다. 묶음 B가 채운다.

use super::facts_real::RealFacts;
use fullops_core::Effect;

pub fn run(_facts: &mut RealFacts, _effects: &[Effect]) {}
