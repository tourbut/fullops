//! Orca CLI 호출과 응답 해석(설계 9.2절). 응답 해석은 이 파일 한 곳에서 한다. 뼈대(묶음 A)의 완성본이다.
//! 표본은 `scenarios/fixtures/orca/`에 있다. 응답의 실제 값(미정-M3)이 다르면 이 파일만 고친다.
//! 쓰지 않는 것: `task-create`·`--task`, `terminal send`, `dispatch --inject`, `worker-start`의 `--parent`·`--worktree new-child`.

use crate::ExtError;
use crate::proc::{ProcResult, ProcStatus, run};
use fullops_core::model::{DispatchId, MessageId, OrcaMessage, RunId, path_text};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `orchestration worker-start`의 인자(설계 8.5절 8단계). `--task`는 쓰지 않는다(Task는 spec으로 Orca가 만든다).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRequest {
    pub run: RunId,
    /// stdin으로 준다(`--spec-file -`).
    pub spec: String,
    /// `--worktree path:<경로>`.
    pub worktree: PathBuf,
    pub target: StartTarget,
    /// `--timeout-ms`. 프로세스의 제한 시간은 여기에 30초를 더한 값이다.
    pub ready_timeout: Duration,
    /// `--retry-request`. `state.json`의 `dispatches[].request`.
    pub request: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartTarget {
    /// `--agent <Orca 에이전트 ID> [--model <모델> [--effort <effort>]]`. `effort`는 `model`이 있을 때만 붙는다.
    NewTerminal {
        agent: String,
        model: Option<String>,
        effort: Option<String>,
    },
    /// `--terminal <핸들>`. Orca가 `--agent`와 함께 받지 않는다.
    Existing { terminal: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartOutcome {
    /// 종료 코드 0. `state`가 `ready`든 `outcome_unknown`이든 성공이다(다시 보내면 일이 두 번 간다).
    Started(Started),
    /// 종료 코드가 0이 아니고 `result`가 있다. 예약 기록을 지우고 Orca의 `next`를 그대로 보여 준다.
    Failed(StartFailure),
    /// 결과를 모른다(시간 초과, 연결 끊김, 형식 오류). 예약 기록을 남기고 같은 요청 ID로 다시 부른다.
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    pub dispatch: DispatchId,
    pub task: String,
    pub run: RunId,
    /// 채팅 방식으로 뜬 worker는 터미널이 없다.
    pub terminal: Option<String>,
    /// 기록과 출력에만 쓴다.
    pub state: String,
    pub run_created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartFailure {
    pub dispatch: Option<DispatchId>,
    pub failed_stage: Option<String>,
    pub last_error: Option<String>,
    pub next: Option<String>,
    pub residual: Vec<String>,
}

/// `orchestration check`를 부르는 방법(설계 8.6절).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckMode {
    /// `--peek`: 읽음 표시 없이 받는다.
    Peek,
    /// 소비형: 돌려받은 묶음이 Orca에서 읽음이 된다. `state.json`에 기록한 뒤에만 부른다.
    Consume,
    /// `--peek --wait --types … --timeout-ms …`. 프로세스의 제한 시간은 `timeout`에 15초를 더한 값이다.
    PeekWait {
        types: &'static [&'static str],
        timeout: Duration,
    },
    /// `--ack <deliveryId>`. 옛 Orca가 `deliveryId`를 줬을 때만 쓴다.
    AckLegacy(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub messages: Vec<OrcaMessage>,
    /// 옛 Orca만 준다.
    pub delivery_id: Option<String>,
    pub timed_out: bool,
}

/// `worker-show`·`worker-list`의 worker 한 건(설계 9.2절).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerView {
    pub dispatch: DispatchId,
    pub state: String,
    /// `worker-list`에는 없다.
    pub stage: Option<String>,
    /// `worker-list`에서 `state`와 다를 때만 온다. `worker-show`에는 없다.
    pub outcome: Option<String>,
    /// 첫 낱말만(`worker-show`는 `unverifiable (<사유>)`처럼 괄호가 붙는다). `exited`일 때만 죽은 것으로 본다.
    pub liveness: String,
    pub terminal: Option<String>,
}

impl WorkerView {
    /// 정산됨: `state`가 `succeeded`·`failed`이거나 `stage`가 `settled`·`released`다(정확한 조합은 미정-M3).
    pub fn settled(&self) -> bool {
        matches!(self.state.as_str(), "succeeded" | "failed")
            || matches!(self.stage.as_deref(), Some("settled" | "released"))
    }

    /// 정산된 worker의 결과. `state == "failed"`면 실패, 아니면 성공이다.
    pub fn failed(&self) -> bool {
        self.outcome.as_deref() == Some("failed") || self.state == "failed"
    }
}

pub trait Orca {
    /// `--objective`는 필수다.
    fn run_create(&self, objective: &str) -> Result<RunId, ExtError>;
    /// `Err`는 시작되지 않은 것이 확실한 실패다(실행 파일 없음, Orca가 `error`로 거절).
    fn worker_start(&self, req: &StartRequest) -> Result<StartOutcome, ExtError>;
    /// `run`이 `None`이면 `--run` 없이 부른다(T-11이 본 날것 `check`를 그대로 미리 볼 때).
    fn check(&self, run: Option<&RunId>, mode: CheckMode) -> Result<CheckResult, ExtError>;
    fn worker_show(&self, dispatch: &DispatchId) -> Result<WorkerView, ExtError>;
    fn worker_list(&self, run: &RunId) -> Result<Vec<WorkerView>, ExtError>;
    fn send_escalation(
        &self,
        dispatch: &DispatchId,
        task: Option<&str>,
        subject: &str,
        body: &str,
    ) -> Result<(), ExtError>;
    /// 그 워크트리에서 `connected`인 터미널의 핸들.
    fn terminals_in(&self, worktree: &Path) -> Result<Vec<String>, ExtError>;
    fn run_use(&self, run: &RunId) -> Result<(), ExtError>;
}

/// Orca 실행 파일을 부르는 구현. 시험에서는 `ORCA_CLI_COMMAND`가 fake-orca를 가리킨다.
pub struct OrcaCli {
    /// 설계 9.2절의 순서로 찾은 실행 파일. 실패해도 다른 실행 파일로 바꾸지 않는다.
    pub program: String,
    pub cwd: PathBuf,
    /// 호출당 제한 시간. `wait`는 15초, `dispatch`는 `ready_timeout_seconds` + 30초, hook은 규칙마다 다르다(설계 9.2절 표).
    pub timeout: Duration,
}

/// 실행 파일 이름: `ORCA_CLI_COMMAND` → `orca-dev`(`ORCA_DEV_REPO_ROOT`가 있을 때) → `orca-ide`(Linux) → `orca`.
pub fn orca_program() -> String {
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    if let Some(command) = env("ORCA_CLI_COMMAND") {
        command
    } else if env("ORCA_DEV_REPO_ROOT").is_some() {
        "orca-dev".into()
    } else if cfg!(target_os = "linux") {
        "orca-ide".into()
    } else {
        "orca".into()
    }
}

fn text(value: &Value, name: &str) -> Option<String> {
    value.get(name)?.as_str().map(str::to_string)
}

fn need(value: &Value, name: &str) -> Result<String, ExtError> {
    text(value, name).ok_or_else(|| ExtError::Parse(format!("{name} 필드가 없습니다")))
}

fn id<T: std::str::FromStr>(value: &Value, name: &str) -> Result<T, ExtError> {
    need(value, name)?
        .parse()
        .map_err(|_| ExtError::Parse(format!("{name} 값을 읽을 수 없습니다")))
}

/// stdout의 JSON 객체. `orchestration`은 한 줄, `terminal`은 여러 줄이다.
fn envelope(out: &ProcResult) -> Option<Value> {
    serde_json::from_slice::<Value>(&out.stdout)
        .ok()
        .filter(Value::is_object)
}

/// 성공한 호출의 `result`. 실패의 종류는 오류 문구나 `error.code`로 가르지 않는다.
fn result(out: &ProcResult) -> Result<Value, ExtError> {
    let code = match out.status {
        ProcStatus::NotFound => return Err(ExtError::NotFound),
        ProcStatus::TimedOut => return Err(ExtError::TimedOut),
        ProcStatus::Exited(code) => code,
    };
    let Some(body) = envelope(out) else {
        return Err(if code == 0 {
            ExtError::Parse("JSON이 아닌 출력".into())
        } else {
            ExtError::Failed {
                code,
                stderr: out.stderr_text(),
            }
        });
    };
    if code == 0 && body["ok"] == true {
        return Ok(body["result"].clone());
    }
    let error = &body["error"];
    Err(ExtError::Failed {
        code,
        stderr: format!(
            "{}: {}",
            error["code"].as_str().unwrap_or("orca"),
            error["message"].as_str().unwrap_or_default()
        ),
    })
}

fn message(value: &Value) -> Result<OrcaMessage, ExtError> {
    // payload가 객체가 아니면 세 값은 없는 것으로 본다
    let payload = value.get("payload").filter(|p| p.is_object());
    let from_payload = |name: &str| payload.and_then(|p| text(p, name));
    Ok(OrcaMessage {
        id: id::<MessageId>(value, "id")?,
        kind: text(value, "type").unwrap_or_else(|| "status".into()),
        from: text(value, "from"),
        subject: text(value, "subject"),
        body: text(value, "body"),
        task_id: from_payload("taskId"),
        dispatch_id: from_payload("dispatchId").and_then(|d| d.parse().ok()),
        outcome: from_payload("outcome"),
    })
}

fn worker(value: &Value) -> Result<WorkerView, ExtError> {
    Ok(WorkerView {
        dispatch: id(value, "dispatchId")?,
        state: need(value, "state")?,
        stage: text(value, "stage"),
        outcome: text(value, "outcome"),
        liveness: text(value, "liveness")
            .and_then(|l| l.split_whitespace().next().map(str::to_string))
            .unwrap_or_default(),
        terminal: text(value, "terminal"),
    })
}

impl OrcaCli {
    /// 환경에서 실행 파일을 찾아 만든다.
    pub fn new(cwd: PathBuf, timeout: Duration) -> Self {
        Self {
            program: orca_program(),
            cwd,
            timeout,
        }
    }

    fn call(&self, args: &[&str], timeout: Duration, stdin: Option<&[u8]>) -> ProcResult {
        let argv: Vec<String> = std::iter::once(self.program.as_str())
            .chain(args.iter().copied())
            .chain(["--json"])
            .map(String::from)
            .collect();
        run(&argv, &self.cwd, timeout, &[], stdin)
    }

    fn ask(&self, args: &[&str]) -> Result<Value, ExtError> {
        result(&self.call(args, self.timeout, None))
    }
}

impl Orca for OrcaCli {
    fn run_create(&self, objective: &str) -> Result<RunId, ExtError> {
        let result = self.ask(&["orchestration", "run-create", "--objective", objective])?;
        id(&result, "runId")
    }

    fn worker_start(&self, req: &StartRequest) -> Result<StartOutcome, ExtError> {
        let worktree = format!("path:{}", path_text(&req.worktree));
        let timeout_ms = req.ready_timeout.as_millis().to_string();
        let mut args = vec![
            "orchestration",
            "worker-start",
            "--run",
            req.run.as_str(),
            "--spec-file",
            "-",
            "--worktree",
            &worktree,
        ];
        match &req.target {
            StartTarget::NewTerminal {
                agent,
                model,
                effort,
            } => {
                args.extend(["--agent", agent]);
                if let Some(model) = model {
                    args.extend(["--model", model]);
                    if let Some(effort) = effort {
                        args.extend(["--effort", effort]);
                    }
                }
            }
            StartTarget::Existing { terminal } => args.extend(["--terminal", terminal]),
        }
        args.extend(["--timeout-ms", &timeout_ms, "--retry-request", &req.request]);
        let out = self.call(
            &args,
            req.ready_timeout + Duration::from_secs(30),
            Some(req.spec.as_bytes()),
        );
        let code = match out.status {
            ProcStatus::NotFound => return Err(ExtError::NotFound),
            ProcStatus::TimedOut => return Ok(StartOutcome::Unknown("제한 시간 초과".into())),
            ProcStatus::Exited(code) => code,
        };
        let Some(body) = envelope(&out) else {
            return Ok(StartOutcome::Unknown("JSON이 아닌 출력".into()));
        };
        let receipt = &body["result"];
        if code == 0 {
            // 영수증에는 dispatchId·taskId·runId가 항상 있다. 없으면 결과를 모르는 것으로 본다
            let started = || -> Result<Started, ExtError> {
                Ok(Started {
                    dispatch: id(receipt, "dispatchId")?,
                    task: need(receipt, "taskId")?,
                    run: id(receipt, "runId")?,
                    terminal: text(receipt, "terminal"),
                    state: text(receipt, "state").unwrap_or_default(),
                    run_created: receipt["runCreated"] == true,
                })
            };
            return Ok(match started() {
                Ok(started) => StartOutcome::Started(started),
                Err(e) => StartOutcome::Unknown(e.to_string()),
            });
        }
        if receipt.is_object() {
            return Ok(StartOutcome::Failed(StartFailure {
                dispatch: text(receipt, "dispatchId").and_then(|d| d.parse().ok()),
                failed_stage: text(receipt, "failedStage"),
                last_error: text(receipt, "lastError"),
                next: text(receipt, "next"),
                residual: receipt["residualResources"]
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
            }));
        }
        result(&out).map(|_| StartOutcome::Unknown("형식 오류".into()))
    }

    fn check(&self, run: Option<&RunId>, mode: CheckMode) -> Result<CheckResult, ExtError> {
        let (types, timeout_ms);
        let mut args = vec!["orchestration", "check"];
        if let Some(run) = run {
            args.extend(["--run", run.as_str()]);
        }
        let mut limit = self.timeout;
        match &mode {
            CheckMode::Peek => args.push("--peek"),
            CheckMode::Consume => {}
            CheckMode::PeekWait {
                types: wanted,
                timeout,
            } => {
                types = wanted.join(",");
                timeout_ms = timeout.as_millis().to_string();
                args.extend([
                    "--peek",
                    "--wait",
                    "--types",
                    &types,
                    "--timeout-ms",
                    &timeout_ms,
                ]);
                limit = *timeout + Duration::from_secs(15);
            }
            CheckMode::AckLegacy(delivery) => args.extend(["--ack", delivery]),
        }
        let result = result(&self.call(&args, limit, None))?;
        // 취소와 연결 끊김은 실패로 본다
        for flag in ["cancelled", "connectionLost"] {
            if result[flag] == true {
                return Err(ExtError::Failed {
                    code: 0,
                    stderr: flag.to_string(),
                });
            }
        }
        Ok(CheckResult {
            messages: result["messages"]
                .as_array()
                .map(|items| items.iter().map(message).collect::<Result<_, _>>())
                .transpose()?
                .unwrap_or_default(),
            delivery_id: text(&result, "deliveryId"),
            timed_out: result["timedOut"] == true,
        })
    }

    fn worker_show(&self, dispatch: &DispatchId) -> Result<WorkerView, ExtError> {
        worker(&self.ask(&[
            "orchestration",
            "worker-show",
            "--dispatch",
            dispatch.as_str(),
        ])?)
    }

    fn worker_list(&self, run: &RunId) -> Result<Vec<WorkerView>, ExtError> {
        let mut workers = Vec::new();
        let mut cursor: Option<String> = None;
        // 100건씩 이어 읽는다. 끝나지 않는 응답에 묶이지 않게 쪽 수를 끊는다
        for _ in 0..50 {
            let mut args = vec!["orchestration", "worker-list", "--run", run.as_str()];
            if let Some(cursor) = &cursor {
                args.extend(["--cursor", cursor]);
            }
            let page = self.ask(&args)?;
            for item in page["workers"].as_array().into_iter().flatten() {
                workers.push(worker(item)?);
            }
            cursor = text(&page, "nextCursor");
            if cursor.is_none() {
                break;
            }
        }
        Ok(workers)
    }

    fn send_escalation(
        &self,
        dispatch: &DispatchId,
        task: Option<&str>,
        subject: &str,
        body: &str,
    ) -> Result<(), ExtError> {
        let mut args = vec![
            "orchestration",
            "send",
            "--type",
            "escalation",
            "--subject",
            subject,
            "--body",
            body,
            "--dispatch-id",
            dispatch.as_str(),
        ];
        if let Some(task) = task {
            args.extend(["--task-id", task]);
        }
        self.ask(&args).map(drop)
    }

    fn terminals_in(&self, worktree: &Path) -> Result<Vec<String>, ExtError> {
        let selector = format!("path:{}", path_text(worktree));
        let result = self.ask(&["terminal", "list", "--worktree", &selector])?;
        Ok(result["terminals"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|t| t["connected"] == true)
            .filter_map(|t| text(t, "handle"))
            .collect())
    }

    fn run_use(&self, run: &RunId) -> Result<(), ExtError> {
        self.ask(&["orchestration", "run-use", "--id", run.as_str()])
            .map(drop)
    }
}
