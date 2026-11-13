//! 가짜 Orca CLI. 시험이 `ORCA_CLI_COMMAND`로 가리킨다. 각본대로 응답하고 받은 호출을 기록한다.
//!
//! 규약 (`FAKE_ORCA_DIR` 폴더 안):
//! - `script.json`: `{"rules":[{"when":["orchestration","check"],"times":1,"sleep_ms":0,"exit":0,
//!   "stdout":{…},"stderr":""}]}`. `when`의 낱말이 argv에 그 순서로 모두 있으면 맞는다.
//!   위에서부터 처음 맞고 `times`(없으면 무제한)를 다 쓰지 않은 규칙의 응답을 낸다.
//!   `stdout`이 문자열이면 그대로, 그 밖이면 JSON으로 낸다.
//! - `calls.jsonl`: 호출마다 `{"argv":[…],"cwd":"…","terminal":"…"|null,"rule":<번호>|null,"stdout":…}` 한 줄.
//! - 맞는 규칙이 없으면 `{"ok":false,"error":{"code":"unscripted",…}}`와 종료 코드 1을 낸다.

use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

fn matches(when: &Value, argv: &[String]) -> bool {
    let mut rest = argv.iter();
    when.as_array().is_some_and(|words| {
        words
            .iter()
            .all(|w| w.as_str().is_some_and(|w| rest.any(|a| a == w)))
    })
}

fn main() -> ExitCode {
    let Some(dir) = std::env::var_os("FAKE_ORCA_DIR").map(PathBuf::from) else {
        eprintln!("fake-orca: FAKE_ORCA_DIR이 없습니다");
        return ExitCode::from(97);
    };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
    let script: Value = serde_json::from_str(&read("script.json")).unwrap_or(Value::Null);
    let calls: Vec<Value> = read("calls.jsonl")
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let used = |index: usize| calls.iter().filter(|c| c["rule"] == json!(index)).count() as u64;

    let rules = script["rules"].as_array().cloned().unwrap_or_default();
    let hit = rules.iter().enumerate().find(|(i, rule)| {
        matches(&rule["when"], &argv) && rule["times"].as_u64().is_none_or(|times| used(*i) < times)
    });
    let (index, stdout, stderr, exit, sleep_ms) = match hit {
        Some((i, rule)) => (
            Some(i),
            rule["stdout"].clone(),
            rule["stderr"].as_str().unwrap_or_default().to_string(),
            rule["exit"].as_u64().unwrap_or(0) as u8,
            rule["sleep_ms"].as_u64().unwrap_or(0),
        ),
        None => (
            None,
            json!({"ok": false, "error": {"code": "unscripted", "message": format!("각본에 없는 호출: {argv:?}")}}),
            String::new(),
            1,
            0,
        ),
    };

    let record = json!({
        "argv": argv,
        "cwd": std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        "terminal": std::env::var("ORCA_TERMINAL_HANDLE").ok(),
        "rule": index,
        "stdout": stdout,
    });
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("calls.jsonl"))
    {
        let _ = file.write_all(format!("{record}\n").as_bytes());
    }

    std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
    match &stdout {
        Value::Null => {}
        Value::String(text) => print!("{text}"),
        other => println!("{other}"),
    }
    eprint!("{stderr}");
    ExitCode::from(exit)
}
