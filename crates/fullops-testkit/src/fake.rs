//! 가짜 실행 파일 셋(fake-orca, fake-host, fake-curl)이 함께 쓰는 각본 규약(설계 12.1절).
//!
//! 각본 폴더 안:
//! - `script.json`: `{"rules":[{"when":["orchestration","check"],"times":1,"sleep_ms":0,"exit":0,"stdout":{…},"stderr":""}]}`.
//!   `when`의 낱말이 argv에 그 순서로 모두 있으면 맞는다. 위에서부터 처음 맞고 `times`(없으면 무제한)를 다 쓰지 않은 규칙의 응답을 낸다.
//!   `stdout`이 문자열이면 그대로, 그 밖이면 JSON 한 줄로 낸다.
//! - `calls.jsonl`: 호출마다 `{"argv":[…],"cwd":"…","rule":<번호>|null,"stdout":…,"exit":<코드>, …실행 파일별 필드}` 한 줄.

use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

/// 가짜 실행 파일이 낼 응답 하나.
pub struct Reply {
    /// 맞은 각본 규칙의 번호. 각본 밖의 응답이면 `None`.
    pub rule: Option<usize>,
    pub stdout: Value,
    pub stderr: String,
    pub exit: u8,
    pub sleep_ms: u64,
}

impl Reply {
    /// 각본 밖의 응답.
    pub fn plain(stdout: Value, exit: u8) -> Self {
        Self {
            rule: None,
            stdout,
            stderr: String::new(),
            exit,
            sleep_ms: 0,
        }
    }
}

fn matches(when: &Value, argv: &[String]) -> bool {
    let mut rest = argv.iter();
    when.as_array().is_some_and(|words| {
        words
            .iter()
            .all(|w| w.as_str().is_some_and(|w| rest.any(|a| a == w)))
    })
}

/// `calls.jsonl`의 모든 줄.
pub fn calls(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join("calls.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// 각본에서 이 호출에 맞는 규칙의 응답. 없으면 `None`.
pub fn scripted(dir: &Path, argv: &[String]) -> Option<Reply> {
    let text = std::fs::read_to_string(dir.join("script.json")).unwrap_or_default();
    let script: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let past = calls(dir);
    let used = |index: usize| past.iter().filter(|c| c["rule"] == json!(index)).count() as u64;
    let rules = script["rules"].as_array()?;
    let (index, rule) = rules.iter().enumerate().find(|(i, rule)| {
        matches(&rule["when"], argv) && rule["times"].as_u64().is_none_or(|times| used(*i) < times)
    })?;
    Some(Reply {
        rule: Some(index),
        stdout: rule["stdout"].clone(),
        stderr: rule["stderr"].as_str().unwrap_or_default().to_string(),
        exit: rule["exit"].as_u64().unwrap_or(0) as u8,
        sleep_ms: rule["sleep_ms"].as_u64().unwrap_or(0),
    })
}

/// 호출을 `calls.jsonl`에 남기고 응답을 낸다. `extra`는 실행 파일별로 더 남길 필드(JSON 객체)다.
pub fn finish(dir: &Path, argv: &[String], reply: Reply, extra: Value) -> ExitCode {
    let mut record = json!({
        "argv": argv,
        "cwd": std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        "rule": reply.rule,
        "stdout": reply.stdout,
        "exit": reply.exit,
    });
    if let (Some(record), Some(extra)) = (record.as_object_mut(), extra.as_object()) {
        record.extend(extra.clone());
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("calls.jsonl"))
    {
        let _ = file.write_all(format!("{record}\n").as_bytes());
    }
    std::thread::sleep(std::time::Duration::from_millis(reply.sleep_ms));
    match &reply.stdout {
        Value::Null => {}
        Value::String(text) => print!("{text}"),
        other => println!("{other}"),
    }
    eprint!("{}", reply.stderr);
    ExitCode::from(reply.exit)
}

/// 각본 폴더: 환경 변수 `var`가 가리키는 폴더, 없으면 작업 디렉터리다
/// (시험 프로세스 안에서 환경 변수 없이 띄울 때는 작업 디렉터리를 각본 폴더로 둔다).
pub fn script_dir(var: &str) -> Result<std::path::PathBuf, ExitCode> {
    std::env::var_os(var)
        .map(Into::into)
        .map_or_else(std::env::current_dir, Ok)
        .map_err(|e| {
            eprintln!("각본 폴더를 찾지 못했습니다: {e}");
            ExitCode::from(97)
        })
}

/// argv에서 `--이름` 다음 값.
pub fn flag<'a>(argv: &'a [String], name: &str) -> Option<&'a str> {
    argv.iter()
        .position(|a| a == name)
        .and_then(|at| argv.get(at + 1))
        .map(String::as_str)
}
