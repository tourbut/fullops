//! fake-orca에 넣는 응답과 preamble의 모양(설계 9.2절의 가정).
//! 실제 Orca 계약(미정-M3)이 확정되면 이 파일만 `scenarios/fixtures/orca/`의 표본에 맞춘다.

use serde_json::{Value, json};

fn ok(result: Value) -> Value {
    json!({ "ok": true, "result": result })
}

/// `orchestration run-create`의 응답.
pub fn run_created(run: &str) -> Value {
    ok(json!({ "runId": run }))
}

/// `orchestration worker-start`의 응답.
pub fn worker_started(run: &str, dispatch: &str, task: &str, terminal: &str) -> Value {
    ok(json!({ "runId": run, "dispatchId": dispatch, "taskId": task, "terminal": terminal }))
}

/// `orchestration check`의 응답. `messages`는 `message`로 만든 값들이다.
pub fn checked(messages: Vec<Value>) -> Value {
    ok(json!({ "messages": messages }))
}

/// 메시지 하나. `kind`는 `worker_done`·`question`·`escalation`·`heartbeat`·`status`다.
pub fn message(
    id: &str,
    delivery: &str,
    run: &str,
    kind: &str,
    dispatch: &str,
    task: &str,
    body: &str,
) -> Value {
    json!({
        "id": id, "deliveryId": delivery, "run_id": run, "type": kind, "body": body,
        "payload": { "dispatchId": dispatch, "taskId": task },
    })
}

/// `orchestration worker-show`·`worker-list`의 worker 한 건.
pub fn worker(
    run: &str,
    dispatch: &str,
    task: &str,
    terminal: &str,
    settled: bool,
    liveness: &str,
) -> Value {
    json!({
        "runId": run, "dispatchId": dispatch, "taskId": task, "terminal": terminal,
        "stage": if settled { "settled" } else { "running" },
        "outcome": if settled { json!("succeeded") } else { Value::Null },
        "liveness": liveness,
    })
}

/// Orca가 worker 세션의 첫 프롬프트로 넣는 preamble.
pub fn preamble(dispatch: &str, task: &str) -> String {
    format!(
        "You are a worker in an Orca orchestration run. When finished, report with: \
         orca orchestration send --type worker_done --dispatch-id {dispatch} --task-id {task} \
         --subject \"done\" --body \"<result>\". Ask with --type ask, escalate with --type escalation."
    )
}
