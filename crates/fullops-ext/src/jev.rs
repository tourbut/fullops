//! Jev 호출(설계 9.3절). `route`·`find`·`context`가 부른다. 전송 스키마(미정-M6)는 `Judge` trait 뒤에 숨긴다.
//! 타입은 설계 10.2절이 정본이고 `JevCurl`의 구현은 묶음 C가 채운다.

use std::collections::BTreeMap;

/// 질문 하나. `instructions`는 무엇을 판단할지 적은 글이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub id: String,
    pub kind: QuestionKind,
    pub instructions: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestionKind {
    /// 선택지 가운데 하나.
    Choice { options: Vec<String> },
    /// 0~1의 수.
    Noul,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Choice {
        id: String,
        choice: String,
        /// 키는 선택지와 정확히 같다.
        probabilities: BTreeMap<String, f64>,
    },
    Noul {
        id: String,
        value: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Judged {
    pub answers: Vec<Answer>,
    pub usage: Usage,
    pub model: String,
    /// 캐시에서 읽은 응답이다.
    pub cached: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// 응답에 비용이 없으면 `None`.
    pub cost: Option<f64>,
}

/// 어떤 실패든 부르는 쪽은 폴백으로 처리한다(`route`는 폴백 분류, `find`는 로컬 후보, `context`는 전부 `keep`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JudgeError {
    /// `FULLOPS_OFFLINE=1` 또는 `FULLOPS_JEV_ENABLED=0`.
    Offline,
    NoKey,
    /// 글 규칙에 걸려 보내지 않았다.
    Unsafe,
    Request(String),
    Response(String),
}

pub trait Judge {
    /// `state`는 JSON 객체 글이다.
    fn ask(&self, state: &str, questions: &[Question]) -> Result<Judged, JudgeError>;
}

/// 호출과 캐시 읽기를 모두 건너뛰는 설정인가(설계 9.3절).
pub fn offline() -> bool {
    let is = |name: &str, value: &str| std::env::var(name).is_ok_and(|v| v == value);
    is("FULLOPS_OFFLINE", "1") || is("FULLOPS_JEV_ENABLED", "0")
}

/// `curl` 프로세스로 Jev를 부르는 구현. 뼈대의 임시 구현은 항상 오프라인이다(부르는 쪽이 폴백한다).
#[derive(Debug, Clone, Default)]
pub struct JevCurl;

impl Judge for JevCurl {
    fn ask(&self, _state: &str, _questions: &[Question]) -> Result<Judged, JudgeError> {
        Err(JudgeError::Offline)
    }
}
