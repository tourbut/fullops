//! Jev 호출(설계 9.3절). `route`·`find`·`context`가 부른다. 전송 스키마(미정-M6)는 `Judge` trait 뒤에 숨긴다.
//! 타입은 설계 10.2절이 정본이고 `JevCurl`의 구현은 묶음 C의 것이다.

use crate::proc::{ProcStatus, run};
use fullops_core::model::sha256_hex;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

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

/// `curl` 프로세스로 Jev를 부르는 구현(설계 9.3절). 키는 환경 변수, 그다음 작업 디렉터리에서 위로 찾은
/// `.fullops-squad/.env`에서 찾는다. `--env-file`이나 `--repo`를 받는 명령은 `JevCurl::at`으로 자리를 정해 준다.
#[derive(Debug, Clone, Default)]
pub struct JevCurl;

/// 키를 찾을 자리를 정한 `JevCurl`. 순서는 `env_file` → 환경 변수 → `squad_env`다.
#[derive(Debug, Clone, Default)]
pub struct JevCurlAt {
    /// `--env-file`의 값.
    pub env_file: Option<PathBuf>,
    /// 레포의 `.fullops-squad/.env`.
    pub squad_env: Option<PathBuf>,
}

impl JevCurl {
    /// `repo_root`의 `.fullops-squad/.env`와 `--env-file`에서 키를 찾는 구현.
    pub fn at(repo_root: &Path, env_file: Option<PathBuf>) -> JevCurlAt {
        JevCurlAt {
            env_file,
            squad_env: Some(repo_root.join(".fullops-squad").join(".env")),
        }
    }
}

impl Judge for JevCurl {
    fn ask(&self, state: &str, questions: &[Question]) -> Result<Judged, JudgeError> {
        let cwd = std::env::current_dir().unwrap_or_default();
        let squad = cwd
            .ancestors()
            .map(|dir| dir.join(".fullops-squad"))
            .find(|dir| dir.is_dir());
        JevCurlAt {
            env_file: None,
            squad_env: squad.map(|dir| dir.join(".env")),
        }
        .ask(state, questions)
    }
}

/// 제공자: (이름, 키의 환경 변수, 엔드포인트, 모델 이름). 둘 다 있으면 공식 API가 먼저다.
///
/// 전송 스키마(미정-M6)는 TypeSafe의 System One HTTP API다(1.24.0이 쓰던 형식과 같다).
/// 요청: `{"model", "state": <객체>, "questions": {<id>: {"type": "choice", "instructions", "criteria": {<선택지>: null}}
///        | {"type": "noul", "instructions"}}}`. 질문 ID는 모델에게 가지 않고 선택지 이름은 간다.
/// 응답: `{"model", "answers": {<id>: {"type": "choice", "choice", "probabilities", "confidence"} | {"type": "noul", "noul"}},
///        "usage": {"input_tokens", "output_tokens", "cost"?}}`. 공식 API는 `cost`를 주지 않는다.
const PROVIDERS: [(&str, &str, &str, &str); 2] = [
    (
        "typesafe",
        "TYPESAFE_API_KEY",
        "https://api.typesafe.ai/v1/systemone",
        "jev-latest",
    ),
    (
        "openrouter",
        "OPENROUTER_API_KEY",
        "https://openrouter.ai/api/v1/systemone",
        "~typesafe/jev-latest",
    ),
];

/// env 파일의 키 줄만 읽는다. 같은 출처에서는 공식 API 키가 먼저다.
fn key_in(text: &str) -> Option<(usize, String)> {
    // 메모장과 PowerShell 5가 붙이는 BOM은 첫 줄의 이름에 붙어 키를 가린다
    let text = text.trim_start_matches('\u{feff}');
    PROVIDERS.iter().enumerate().find_map(|(index, p)| {
        text.lines()
            .filter_map(|line| line.split_once('='))
            .find(|(name, _)| name.trim() == p.1)
            .and_then(|(_, value)| clean_key(value))
            .map(|key| (index, key))
    })
}

/// 따옴표를 떼고, curl 설정 글을 깨지 않는 글자뿐인 키만 받는다.
fn clean_key(raw: &str) -> Option<String> {
    let key = raw.trim().trim_matches(['"', '\'']);
    let ok = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    (!key.is_empty() && key.chars().all(ok)).then(|| key.to_string())
}

fn home() -> Option<PathBuf> {
    let name = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn bad(why: &str) -> JudgeError {
    JudgeError::Response(why.to_string())
}

/// 응답 검증(설계 9.3절). 하나라도 어기면 그 호출은 실패다.
fn judged(response: &Value, questions: &[Question], cached: bool) -> Result<Judged, JudgeError> {
    let unit = |p: &f64| p.is_finite() && (0.0..=1.0).contains(p);
    let mut answers = Vec::new();
    for q in questions {
        let answer = &response["answers"][&q.id];
        answers.push(match &q.kind {
            QuestionKind::Choice { options } => {
                let probabilities: BTreeMap<String, f64> = answer["probabilities"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter_map(|(k, v)| Some((k.clone(), v.as_f64()?)))
                    .collect();
                let choice = answer["choice"].as_str().unwrap_or_default();
                let same_keys = probabilities.len() == options.len()
                    && options.iter().all(|o| probabilities.contains_key(o));
                let sum: f64 = probabilities.values().sum();
                let top = probabilities.values().copied().fold(0.0, f64::max);
                if !same_keys
                    || !probabilities.values().all(unit)
                    || !(0.98..=1.02).contains(&sum)
                    || probabilities.get(choice).is_none_or(|p| *p < top - 0.02)
                {
                    return Err(bad("choice 분포가 맞지 않습니다"));
                }
                Answer::Choice {
                    id: q.id.clone(),
                    choice: choice.to_string(),
                    probabilities,
                }
            }
            QuestionKind::Noul => Answer::Noul {
                id: q.id.clone(),
                value: answer["noul"]
                    .as_f64()
                    .filter(unit)
                    .ok_or_else(|| bad("noul 값이 맞지 않습니다"))?,
            },
        });
    }
    let usage = &response["usage"];
    // 공식 문서는 토큰 수가 없을 수 있다고 적는다. 없으면 0으로 보고, 있는데 0 이상의 정수가 아니면 실패다
    let tokens = |name: &str| match &usage[name] {
        Value::Null => Ok(0),
        value => value.as_u64().ok_or_else(|| bad("usage가 맞지 않습니다")),
    };
    let model = response["model"].as_str().filter(|m| !m.is_empty());
    Ok(Judged {
        answers,
        // 캐시에서 읽은 답은 새로 쓴 것이 없다
        usage: if cached {
            Usage::default()
        } else {
            Usage {
                input_tokens: tokens("input_tokens")?,
                output_tokens: tokens("output_tokens")?,
                cost: usage["cost"].as_f64(),
            }
        },
        model: model.ok_or_else(|| bad("model이 없습니다"))?.to_string(),
        cached,
    })
}

impl JevCurlAt {
    /// 키와 그 제공자(`PROVIDERS`의 자리).
    fn key(&self) -> Option<(usize, String)> {
        let read = |path: &Option<PathBuf>| key_in(&std::fs::read_to_string(path.as_ref()?).ok()?);
        let from_env = || {
            PROVIDERS
                .iter()
                .enumerate()
                .find_map(|(index, p)| Some((index, clean_key(&std::env::var(p.1).ok()?)?)))
        };
        read(&self.env_file)
            .or_else(from_env)
            .or_else(|| read(&self.squad_env))
    }

    /// 이 실행이 부를 엔드포인트의 이름(`typesafe`·`openrouter`). 오프라인이거나 키가 없으면 `None`이다
    /// (설계 6.1.5절의 `options.provider`).
    pub fn provider(&self) -> Option<&'static str> {
        if offline() {
            return None;
        }
        self.key()
            .and_then(|(index, _)| PROVIDERS.get(index))
            .map(|p| p.0)
    }
}

impl Judge for JevCurlAt {
    fn ask(&self, state: &str, questions: &[Question]) -> Result<Judged, JudgeError> {
        if offline() {
            return Err(JudgeError::Offline);
        }
        let state: Value = serde_json::from_str(state)
            .map_err(|e| JudgeError::Request(format!("state가 JSON이 아닙니다: {e}")))?;
        let asked: serde_json::Map<String, Value> = questions
            .iter()
            .map(|q| {
                let question = match &q.kind {
                    QuestionKind::Choice { options } => {
                        let criteria: serde_json::Map<String, Value> =
                            options.iter().map(|o| (o.clone(), Value::Null)).collect();
                        json!({"type": "choice", "instructions": q.instructions, "criteria": criteria})
                    }
                    QuestionKind::Noul => json!({"type": "noul", "instructions": q.instructions}),
                };
                (q.id.clone(), question)
            })
            .collect();
        // 캐시 키는 제공자와 무관한 본문이다. 제공자별 모델 이름은 보내기 직전에 넣는다
        let mut body = json!({"model": PROVIDERS[0].3, "state": state, "questions": asked});
        let canonical = body.to_string();
        // 보내기 전에 본문 전체를 글 규칙(설계 8.14절 표의 "글" 행)으로 본다. 탐색과 같은 함수다
        if !fullops_core::explore::sendable_text(&canonical) {
            return Err(JudgeError::Unsafe);
        }
        let cache = home().map(|h| {
            h.join(".fullops")
                .join("cache")
                .join("jev")
                .join(format!("{}.json", sha256_hex(canonical.as_bytes())))
        });
        let bypass = std::env::var("FULLOPS_JEV_CACHE_BYPASS").is_ok_and(|v| v == "1");
        if let (false, Some(path)) = (bypass, &cache)
            && let Ok(text) = std::fs::read_to_string(path)
            && let Ok(response) = serde_json::from_str::<Value>(&text)
            && let Ok(hit) = judged(&response, questions, true)
        {
            return Ok(hit);
        }
        let (index, key) = self.key().ok_or(JudgeError::NoKey)?;
        let (_, _, url, model) = PROVIDERS.get(index).copied().ok_or(JudgeError::NoKey)?;
        body["model"] = json!(model);
        // 키와 본문은 명령행 인자가 아니라 curl 설정 글(표준입력)로 넘긴다. 따옴표 안에서는 `\`와 `"`만 풀어 쓴다
        let quoted = body.to_string().replace('\\', "\\\\").replace('"', "\\\"");
        let config = format!(
            "url = \"{url}\"\nheader = \"Authorization: Bearer {key}\"\nheader = \"Content-Type: application/json\"\ndata-binary = \"{quoted}\"\n"
        );
        // PATH에서 처음 찾은 curl의 전체 경로로 띄운다. 이름만 주면 Windows는 System32의 것을 먼저 잡는다
        let curl = crate::proc::find_program("curl")
            .ok_or_else(|| JudgeError::Request("curl을 찾지 못했습니다".into()))?;
        let argv = [
            curl.to_string_lossy().as_ref(),
            "--config",
            "-",
            "--silent",
            "--show-error",
            "--fail",
            "--connect-timeout",
            "5",
            "--max-time",
            "20",
        ]
        .map(String::from);
        let out = run(
            &argv,
            &std::env::temp_dir(),
            Duration::from_secs(25),
            &[],
            Some(config.as_bytes()),
        );
        match out.status {
            ProcStatus::Exited(0) => {}
            ProcStatus::Exited(code) => {
                return Err(JudgeError::Request(format!("curl 종료 코드 {code}")));
            }
            ProcStatus::TimedOut => return Err(JudgeError::Request("제한 시간 초과".into())),
            ProcStatus::NotFound => {
                return Err(JudgeError::Request("curl을 찾지 못했습니다".into()));
            }
        }
        let response: Value =
            serde_json::from_slice(&out.stdout).map_err(|_| bad("JSON이 아닌 응답"))?;
        let answer = judged(&response, questions, false)?;
        // 캐시를 쓰지 못해도 답은 이미 받았다
        if let Some(path) = cache
            && let Some(dir) = path.parent()
            && std::fs::create_dir_all(dir).is_ok()
        {
            let _ = std::fs::write(path, response.to_string());
        }
        Ok(answer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_rule_and_key_lines() {
        let unsafe_text = |text: &str| !fullops_core::explore::sendable_text(text);
        assert!(unsafe_text("password = hunter2"));
        assert!(unsafe_text("{\"note\":\"API_KEY: abc\"}"));
        assert!(unsafe_text("sk-abcdefghijkl"));
        assert!(unsafe_text("-----BEGIN RSA PRIVATE KEY-----"));
        // 따옴표로 닫은 키(JSON·YAML)도 걸린다. 본문에 다시 실려 `\"`가 된 것도 같다. 값 없는 낱말은 걸리지 않는다
        assert!(unsafe_text("{\"token\":\"x\"}"));
        assert!(unsafe_text(r#"{"text":"{\"password\": \"x\"}"}"#));
        assert!(!unsafe_text("{\"guide\":\"secret 관리 문서\"}"));
        assert!(!unsafe_text("sk-short"));
        assert_eq!(
            key_in("# 주석\nOPENROUTER_API_KEY=or-1\nTYPESAFE_API_KEY = \"ts_2\"\n"),
            Some((0, "ts_2".to_string()))
        );
        assert_eq!(key_in("TYPESAFE_API_KEY=a b\nOTHER=1\n"), None);
        // R2-6: BOM으로 시작하는 파일의 첫 줄도 키로 읽는다
        assert_eq!(
            key_in("\u{feff}TYPESAFE_API_KEY=bom-key\n"),
            Some((0, "bom-key".to_string()))
        );
    }

    #[test]
    fn response_validation_follows_design() {
        let questions = [
            Question {
                id: "scope".into(),
                kind: QuestionKind::Choice {
                    options: vec!["simple".into(), "design".into()],
                },
                instructions: String::new(),
            },
            Question {
                id: "doc".into(),
                kind: QuestionKind::Noul,
                instructions: String::new(),
            },
        ];
        let reply = |simple: f64, design: f64, choice: &str| {
            json!({"model": "jev-1", "usage": {"input_tokens": 3, "output_tokens": 1},
                   "answers": {"scope": {"type": "choice", "choice": choice, "probabilities": {"simple": simple, "design": design}},
                               "doc": {"type": "noul", "noul": 0.7}}})
        };
        let ok = judged(&reply(0.9, 0.1, "simple"), &questions, false).unwrap();
        assert_eq!(
            (ok.usage.input_tokens, ok.usage.cost, ok.answers.len()),
            (3, None, 2)
        );
        // 합이 1이 아니거나, 고른 것이 최댓값이 아니거나, 키가 선택지와 다르면 실패다
        assert!(judged(&reply(0.9, 0.5, "simple"), &questions, false).is_err());
        assert!(judged(&reply(0.1, 0.9, "simple"), &questions, false).is_err());
        let mut wrong = reply(0.9, 0.1, "simple");
        wrong["answers"]["scope"]["probabilities"] = json!({"simple": 1.0});
        assert!(judged(&wrong, &questions, false).is_err());
        let mut usage = reply(0.9, 0.1, "simple");
        usage["usage"]["input_tokens"] = json!(-1);
        assert!(judged(&usage, &questions, false).is_err());
    }
}
