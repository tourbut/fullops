//! 각본 파일의 답을 돌려주는 `Judge` 구현. 묶음 J가 `JevCurl`(묶음 C)이 합쳐지기 전에 `&dyn Judge`를 받는 함수의 시험에 쓴다.
//!
//! 각본은 JSON 배열이고 `ask`를 부를 때마다 앞에서부터 하나씩 쓴다. 다 쓰면 `JudgeError::Offline`이다.
//! - 답: `{"answers":[{"id":"scope","choice":"direct","probabilities":{"direct":0.9,"design":0.1}},{"id":"relevant","value":0.8}],
//!   "usage":{"input_tokens":10,"output_tokens":2,"cost":0.001},"model":"jev-test","cached":false}`.
//!   `choice`가 있으면 Choice, `value`가 있으면 Noul이다. `usage`·`model`·`cached`는 생략할 수 있다.
//! - 실패: `{"error":"offline"|"no_key"|"unsafe"|"request"|"response"}`.

use fullops_ext::{Answer, Judge, JudgeError, Judged, Question, Usage};
use serde_json::Value;
use std::cell::RefCell;
use std::path::Path;

pub struct ScriptJudge {
    replies: RefCell<std::collections::VecDeque<Value>>,
    /// 받은 호출의 `(state, 질문 ID 목록)`.
    pub asked: RefCell<Vec<(String, Vec<String>)>>,
}

impl ScriptJudge {
    pub fn new(replies: Vec<Value>) -> Self {
        Self {
            replies: RefCell::new(replies.into()),
            asked: RefCell::new(Vec::new()),
        }
    }

    /// 각본 파일(JSON 배열)을 읽는다.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text)
            .map(Self::new)
            .map_err(|e| format!("{}: {e}", path.display()))
    }
}

fn answer(value: &Value) -> Option<Answer> {
    let id = value["id"].as_str()?.to_string();
    Some(match value["choice"].as_str() {
        Some(choice) => Answer::Choice {
            id,
            choice: choice.to_string(),
            probabilities: value["probabilities"]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| Some((k.clone(), v.as_f64()?)))
                .collect(),
        },
        None => Answer::Noul {
            id,
            value: value["value"].as_f64()?,
        },
    })
}

impl Judge for ScriptJudge {
    fn ask(&self, state: &str, questions: &[Question]) -> Result<Judged, JudgeError> {
        self.asked.borrow_mut().push((
            state.to_string(),
            questions.iter().map(|q| q.id.clone()).collect(),
        ));
        let Some(reply) = self.replies.borrow_mut().pop_front() else {
            return Err(JudgeError::Offline);
        };
        if let Some(error) = reply["error"].as_str() {
            return Err(match error {
                "no_key" => JudgeError::NoKey,
                "unsafe" => JudgeError::Unsafe,
                "request" => JudgeError::Request("각본의 요청 실패".into()),
                "response" => JudgeError::Response("각본의 응답 실패".into()),
                _ => JudgeError::Offline,
            });
        }
        let answers = reply["answers"]
            .as_array()
            .map(|items| items.iter().map(answer).collect::<Option<Vec<_>>>())
            .unwrap_or_default()
            .ok_or_else(|| JudgeError::Response("각본의 answers 형식 오류".into()))?;
        Ok(Judged {
            answers,
            usage: Usage {
                input_tokens: reply["usage"]["input_tokens"].as_u64().unwrap_or(0),
                output_tokens: reply["usage"]["output_tokens"].as_u64().unwrap_or(0),
                cost: reply["usage"]["cost"].as_f64(),
            },
            model: reply["model"].as_str().unwrap_or("jev-test").to_string(),
            cached: reply["cached"] == true,
        })
    }
}
