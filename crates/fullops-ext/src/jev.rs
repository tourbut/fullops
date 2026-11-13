//! 분류 호출(설계 9.3절). `route`만 부른다. 요청·응답 스키마(미정-M6)는 이 trait 뒤에 숨긴다. 묶음 C가 채운다.

/// 질문 하나. `choices`가 비어 있으면 예/아니오 질문이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub id: String,
    pub text: String,
    /// `(값, 설명)`.
    pub choices: Vec<(String, String)>,
}

/// 답 하나. 선택지(예/아니오 질문은 `yes`)별 확률이다.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub id: String,
    pub probabilities: Vec<(String, f64)>,
}

/// 어떤 실패든 `route`는 폴백으로 처리한다(설계 8.3절 6번).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JudgeError {
    /// `FULLOPS_OFFLINE=1`, `FULLOPS_JEV_ENABLED=0`, 키 없음.
    Disabled,
    Call(String),
    Format(String),
}

pub trait Judge {
    /// `state`는 판단할 글(요청)이다.
    fn ask(&self, state: &str, questions: &[Question]) -> Result<Vec<Answer>, JudgeError>;
}
