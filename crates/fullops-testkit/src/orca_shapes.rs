//! fake-orca에 넣는 응답과 preamble의 모양(설계 9.2절). `scenarios/fixtures/orca/`의 표본과 같은 필드를 쓴다
//! (시험이 표본과 견준다). 실제 응답의 값(미정-M3)이 다르면 표본과 이 파일을 함께 고친다.

use serde_json::{Value, json};

fn ok(result: Value) -> Value {
    json!({ "ok": true, "result": result })
}

/// `orchestration run-create`·`run-use`의 응답.
pub fn run_created(run: &str) -> Value {
    ok(json!({ "runId": run, "objective": "fullops" }))
}

/// `orchestration worker-start`의 성공 응답. `terminal`이 `None`이면 채팅 방식 worker다(영수증에 `terminal`이 없다).
pub fn worker_started(run: &str, dispatch: &str, task: &str, terminal: Option<&str>) -> Value {
    let mut result = json!({ "dispatchId": dispatch, "taskId": task, "runId": run, "state": "ready", "agent": "claude" });
    if let Some(terminal) = terminal {
        result["terminal"] = json!(terminal);
    }
    ok(result)
}

/// `orchestration worker-start`의 실패 응답. 종료 코드 1과 함께 낸다.
pub fn worker_start_failed(run: &str, dispatch: &str, task: &str) -> Value {
    json!({ "ok": false, "result": {
        "dispatchId": dispatch, "taskId": task, "runId": run, "state": "failed", "agent": "claude",
        "failedStage": "terminal_readying", "lastError": "가짜 실패", "residualResources": [], "next": "다시 배정한다",
    } })
}

/// `orchestration check`의 응답. `messages`는 `message`로 만든 값들이다.
pub fn checked(messages: Vec<Value>) -> Value {
    ok(json!({ "messages": messages }))
}

/// 메시지 하나. `kind`는 `worker_done`·`question`·`escalation`·`heartbeat`·`status` 등이다.
/// `outcome`은 `worker_done`의 `payload.outcome`(`succeeded`·`failed`)이다.
pub fn message(
    id: &str,
    kind: &str,
    dispatch: &str,
    task: &str,
    subject: &str,
    body: &str,
    outcome: Option<&str>,
) -> Value {
    let mut payload = json!({ "taskId": task, "dispatchId": dispatch });
    if let Some(outcome) = outcome {
        payload["outcome"] = json!(outcome);
    }
    json!({ "id": id, "type": kind, "from": dispatch, "subject": subject, "body": body, "payload": payload })
}

/// `orchestration worker-show`의 응답 내용(`worker-list`의 `workers[]` 원소로도 쓴다).
/// `liveness`는 `live`·`exited`·`unverifiable (<사유>)`다.
pub fn worker(
    run: &str,
    dispatch: &str,
    task: &str,
    terminal: &str,
    settled: bool,
    liveness: &str,
) -> Value {
    json!({
        "dispatchId": dispatch, "taskId": task, "runId": run, "terminal": terminal,
        "state": if settled { "succeeded" } else { "ready" },
        "stage": if settled { "settled" } else { "input_accepted" },
        "liveness": liveness,
    })
}

/// `terminal list`의 응답. `terminals`는 `(핸들, connected, worktreePath)`다.
pub fn terminals(items: &[(&str, bool, &str)]) -> Value {
    let terminals: Vec<Value> = items
        .iter()
        .map(|(handle, connected, path)| json!({ "handle": handle, "connected": connected, "worktreePath": path }))
        .collect();
    json!({ "id": "req_fake", "ok": true, "result": { "terminals": terminals }, "_meta": { "runtimeId": "fake" } })
}

/// 오류 응답. 종료 코드 1과 함께 낸다.
pub fn error(code: &str, message: &str) -> Value {
    json!({ "ok": false, "error": { "code": code, "message": message } })
}

/// Orca가 worker 세션의 첫 프롬프트로 넣는 preamble(`fixtures/orca/preamble.terminal.txt`의 뼈대). `spec`은 `=== TASK ===` 뒤의 글이다.
pub fn preamble(dispatch: &str, task: &str, spec: &str) -> String {
    format!(
        "You are working inside Orca, a multi-agent IDE. You are a dispatched worker.\nYour task ID is: {task}\n\n\
         === CLI COMMANDS ===\n\n  orca orchestration send --from term_worker --type worker_done --subject \"<short status>\" \
         --body \"<summary>\" --task-id {task} --dispatch-id {dispatch} --outcome succeeded\n\n=== TASK ===\n{spec}\n"
    )
}
