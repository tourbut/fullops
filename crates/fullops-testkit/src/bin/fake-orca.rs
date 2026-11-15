//! 가짜 Orca CLI. 시험이 `ORCA_CLI_COMMAND`로 가리킨다. 응답의 형태는 `scenarios/fixtures/orca/`의 표본을 따른다.
//!
//! `FAKE_ORCA_DIR` 폴더의 각본 규약은 `fullops_testkit::fake`에 있다. 각본에 맞는 규칙이 없을 때의 기본 동작:
//! - `orchestration worker-start … --retry-request <ID>`: 같은 요청 ID로 이미 답한 적이 있으면 그 답을 다시 낸다
//!   (새 worker가 뜨지 않는다. 기록에 `"replayed": true`). 이 검사는 각본보다 먼저다.
//! - `orchestration check`: 우편함 `mail.jsonl`(`{"run":"<Run>","message":{…}}` 줄)에서 그 Run의 안 읽은 메시지를 낸다.
//!   `--peek`는 읽음으로 바꾸지 않고, 소비형은 돌려준 묶음을 읽음으로 바꾼다(`mail.read`에 ID를 적는다).
//!   `--wait`인데 없으면 `timedOut: true`, `--types`가 있으면 그 종류만, `--ack`면 빈 묶음을 낸다.
//! - 그 밖: `{"ok":false,"error":{"code":"unscripted",…}}`와 종료 코드 1.
//!
//! 기록에는 `"terminal"`(`ORCA_TERMINAL_HANDLE`)과 `"stdin"`(`worker-start`의 spec)이 더 남는다.

use fullops_testkit::fake::{Reply, calls, finish, flag, script_dir, scripted};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::Path;
use std::process::ExitCode;

fn mailbox(dir: &Path, argv: &[String]) -> Reply {
    if flag(argv, "--ack").is_some() {
        return Reply::plain(json!({"ok": true, "result": {"messages": []}}), 0);
    }
    let read = std::fs::read_to_string(dir.join("mail.read")).unwrap_or_default();
    let run = flag(argv, "--run");
    let types: Option<Vec<&str>> = flag(argv, "--types").map(|t| t.split(',').collect());
    let unread: Vec<Value> = std::fs::read_to_string(dir.join("mail.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|mail| run.is_none_or(|run| mail["run"] == run))
        .map(|mail| mail["message"].clone())
        .filter(|m| !read.lines().any(|id| m["id"] == id))
        .filter(|m| {
            types
                .as_ref()
                .is_none_or(|types| types.contains(&m["type"].as_str().unwrap_or("status")))
        })
        .collect();
    if !argv.iter().any(|a| a == "--peek")
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("mail.read"))
    {
        for message in &unread {
            let _ = writeln!(file, "{}", message["id"].as_str().unwrap_or_default());
        }
    }
    let mut result = json!({ "messages": unread });
    if unread.is_empty() && argv.iter().any(|a| a == "--wait") {
        result["timedOut"] = json!(true);
    }
    Reply::plain(json!({"ok": true, "result": result}), 0)
}

fn main() -> ExitCode {
    let dir = match script_dir("FAKE_ORCA_DIR") {
        Ok(dir) => dir,
        Err(code) => return code,
    };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let is = |a: &str, b: &str| {
        argv.first().is_some_and(|x| x == a) && argv.get(1).is_some_and(|x| x == b)
    };
    let mut stdin = String::new();
    if flag(&argv, "--spec-file") == Some("-") {
        let _ = std::io::stdin().read_to_string(&mut stdin);
    }
    let request = flag(&argv, "--retry-request").filter(|_| is("orchestration", "worker-start"));
    let replay = request.and_then(|request| {
        calls(&dir)
            .into_iter()
            .find(|call| call["request"] == request && call["rule"].is_number())
    });
    let replayed = replay.is_some();
    let reply = match replay {
        Some(call) => Reply::plain(
            call["stdout"].clone(),
            call["exit"].as_u64().unwrap_or(0) as u8,
        ),
        None => scripted(&dir, &argv).unwrap_or_else(|| {
            if is("orchestration", "check") {
                mailbox(&dir, &argv)
            } else {
                let message = format!("각본에 없는 호출: {argv:?}");
                Reply::plain(
                    json!({"ok": false, "error": {"code": "unscripted", "message": message}}),
                    1,
                )
            }
        }),
    };
    let extra = json!({
        "terminal": std::env::var("ORCA_TERMINAL_HANDLE").ok(),
        "request": request,
        "replayed": replayed,
        "stdin": stdin,
    });
    finish(&dir, &argv, reply, extra)
}
