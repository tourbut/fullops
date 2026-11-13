//! 묶음 E(리뷰·통합)의 통합 시험. 빌드한 `fullops`를 임시 레포와 임시 bare 원격에서 CLI로 돌린다.
//! 완료 보고는 묶음 C의 `dispatch`·`done`·`wait`가 `state.json`에 남기는 모양(docs/formats.md 4.1절)을
//! `fullops_store::update_state`로 적어 준비한다. C의 명령으로 시작하는 전체 흐름은 `scenarios/`의 파일에 있다.

use fullops_core::model::{
    Completion, Dispatch, DispatchId, Followup, LintPass, LintResult, Message, MessageId,
    ModelTuple, Purpose, Reported, RoleId, Run, RunId, RunOwner, SessionId, SessionMode, Settled,
    SettledHow, Sha, ShaSource, SnapshotState, State, TaskKey, TestLevel, WorkOutcome, path_text,
};
use fullops_store::{CLI_WAIT, Repo};
use fullops_testkit::{Sandbox, Setup, orca_shapes, run_scenario, scenario};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

struct World {
    sb: Sandbox,
    seq: u32,
}

fn out(run: &fullops_testkit::Run) -> Value {
    serde_json::from_str(run.stdout.trim())
        .unwrap_or_else(|e| panic!("JSON이 아닙니다({e}): {} / {}", run.stdout, run.stderr))
}

/// 종료 코드와 출력에 있어야 할 글을 본다.
#[track_caller]
fn expect(run: &fullops_testkit::Run, code: i32, words: &[&str]) {
    let all = format!("{}{}", run.stdout, run.stderr);
    assert_eq!(run.code, Some(code), "{all}");
    for word in words {
        assert!(all.contains(word), "출력에 `{word}`가 없습니다: {all}");
    }
}

impl World {
    fn new() -> Self {
        Self {
            sb: Sandbox::with(&Setup::default()),
            seq: 0,
        }
    }

    fn lead(&self) -> PathBuf {
        self.sb.path("lead")
    }

    fn dev(&self) -> PathBuf {
        self.sb.path("dev")
    }

    fn repo(&self) -> Repo {
        fullops_store::discover(&self.lead()).unwrap().unwrap()
    }

    fn state(&self) -> State {
        fullops_store::read_state(&self.repo()).yes().unwrap()
    }

    fn edit(&self, f: impl FnOnce(&mut State)) {
        fullops_store::update_state(&self.repo(), CLI_WAIT, None, f).unwrap();
    }

    /// lead가 `fullops <args>`를 실행한다.
    fn cli(&self, args: &[&str]) -> fullops_testkit::Run {
        self.sb.cli(&self.lead(), args)
    }

    fn git(&self, who: &str, args: &[&str]) -> String {
        self.sb.git_ok(&self.sb.path(who), args)
    }

    fn commit(&self, who: &str, path: &str, text: &str) -> Sha {
        self.sb.write(&self.sb.path(who).join(path), text);
        self.git(who, &["add", "-A"]);
        self.git(who, &["commit", "-q", "-m", &format!("{path} 변경")]);
        Sha::new(self.git(who, &["rev-parse", "HEAD"])).unwrap()
    }

    /// worker가 코드 파일 하나를 `lines`줄로 쓰고 커밋한다.
    fn work(&self, path: &str, lines: usize) -> Sha {
        let body: String = (0..lines).map(|i| format!("print({i})\n")).collect();
        self.commit("dev", path, &body)
    }

    fn contains(&self, older: &str, newer: &str) -> bool {
        let lead = self.lead();
        self.sb
            .git(&lead, &["merge-base", "--is-ancestor", older, newer])
            .code
            == Some(0)
    }

    /// 묶음 C가 남기는 기록: 정산된 dispatch와 그 보고 메시지. Build면 `done`의 기록과 통합 대기 completion도 둔다.
    fn dispatched(&mut self, key: &str, purpose: Purpose, sha: &Sha, settled: bool) -> String {
        self.seq += 1;
        let n = self.seq;
        let (key, role) = (TaskKey::new(key).unwrap(), RoleId::new("dev").unwrap());
        let run = RunId::new("run_1").unwrap();
        let dispatch = DispatchId::new(format!("ctx_{n}")).unwrap();
        let message = MessageId::new(format!("msg_{n}")).unwrap();
        let (now, worktree) = (fullops_store::now(), path_text(&self.dev()));
        let work = purpose == Purpose::Build;
        let id = message.to_string();
        self.edit(|state| {
            if state.runs.is_empty() {
                state.runs.push(Run {
                    id: run.clone(),
                    owner: RunOwner::Lead,
                    coordinator: None,
                    opened_at: now - 200,
                    closed_at: None,
                    extra: Default::default(),
                });
            }
            state.dispatches.push(Dispatch {
                id: dispatch.clone(),
                request: format!("req_{n:016x}"),
                task_id: Some(format!("task_{n}")),
                run: run.clone(),
                key: key.clone(),
                role: role.clone(),
                purpose,
                tuple: ModelTuple {
                    agent: "claude".into(),
                    model: "test-model".into(),
                    effort: "medium".into(),
                },
                terminal: Some(format!("term_{n}")),
                worktree,
                session_mode: SessionMode::Fresh,
                reason: None,
                started_at: now - 100,
                bound_session: Some(SessionId::new("sid-dev").unwrap()),
                sub_of: None,
                brief: None,
                reported: work.then(|| Reported {
                    branch: "fullops/dev".into(),
                    sha: sha.clone(),
                    lint_head: Some(sha.clone()),
                    followups: vec![Followup {
                        role: RoleId::new("tester").unwrap(),
                        text: "독립 QA".into(),
                        extra: Default::default(),
                    }],
                    at: now - 60,
                    extra: Default::default(),
                }),
                settled: settled.then(|| Settled {
                    how: SettledHow::Done,
                    outcome: Some(WorkOutcome::Succeeded),
                    message: Some(message.clone()),
                    at: now - 50,
                    extra: Default::default(),
                }),
                limit_escalated: false,
                extra: Default::default(),
            });
            if !settled {
                return;
            }
            if work {
                state.completions.push(Completion {
                    id: message.clone(),
                    dispatch: Some(dispatch.clone()),
                    key: Some(key),
                    role: Some(role),
                    branch: Some("fullops/dev".into()),
                    sha: Some(sha.clone()),
                    sha_source: Some(ShaSource::Done),
                    direct: false,
                    outcome: WorkOutcome::Succeeded,
                    received_at: now - 50,
                    hold: None,
                    released_hold: None,
                    landed_at: None,
                    drift: None,
                    extra: Default::default(),
                });
            }
            state.messages.push(Message {
                id: message,
                delivery: None,
                run,
                kind: "worker_done".into(),
                dispatch: Some(dispatch),
                received_at: now - 50,
                acked_at: None,
                extra: Default::default(),
            });
        });
        id
    }

    /// worker의 완료 보고 하나(통합 대기).
    fn report(&mut self, key: &str, sha: &Sha) -> String {
        self.dispatched(key, Purpose::Build, sha, true)
    }

    fn archive(&self) -> Vec<Value> {
        std::fs::read_to_string(self.repo().archive_path())
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn review_dir(&self, key: &str, sha: &Sha) -> PathBuf {
        self.repo().review_dir(&TaskKey::new(key).unwrap(), sha)
    }

    /// worker 워크트리에 그 head의 lint 결과(묶음 D의 `lint/result.json`)를 둔다.
    fn lint_result(&self, sha: &Sha) {
        let base = Sha::new(self.git("lead", &["merge-base", "main", sha.as_str()])).unwrap();
        let result = LintResult {
            v: 1,
            base: base.clone(),
            merge_base: base,
            head: sha.clone(),
            config_sha256: "0".repeat(64),
            test_level: TestLevel::Lite,
            started_at: fullops_store::now(),
            seconds: 0.1,
            commands: Vec::new(),
            findings: Vec::new(),
            errors: 0,
            warnings: 0,
            unrunnable: 0,
            passed: true,
        };
        let dev = fullops_store::discover(&self.dev()).unwrap().unwrap();
        fullops_store::write_json(&dev.lint_dir().join("result.json"), &result).unwrap();
    }

    /// 리뷰어가 `result.json`을 채운다.
    fn review(&self, key: &str, sha: &Sha, reviewer: &str, high_resolved: Option<bool>) {
        let path = self.review_dir(key, sha).join("result.json");
        let mut result: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        result["reviewer_session"] = json!(reviewer);
        result["conclusion"] = json!("accept");
        for file in result["files"].as_array_mut().unwrap() {
            file["review_status"] = json!("reviewed");
        }
        result["findings"] = match high_resolved {
            Some(resolved) => json!([{ "path": "src/big.py", "content": "경계 검사 없음",
                "severity": "high", "resolved": resolved, "start_line": 1, "end_line": 2 }]),
            None => json!([]),
        };
        fullops_store::write_json(&path, &result).unwrap();
    }

    /// `review cleanup`이 묻는 터미널 목록에 "없음"으로 답한다.
    fn no_terminals(&self) {
        self.sb.orca_rule(
            json!({ "when": ["terminal", "list"], "stdout": orca_shapes::terminals(&[]) }),
        );
    }
}

fn subjects(w: &World, who: &str) -> Vec<String> {
    let log = w.git(who, &["log", "--format=%s"]);
    log.lines().map(str::to_string).collect()
}

/// L5 fast-forward: lint를 재사용하고, 병합 기록 커밋·push·처리 표시·archive·동기화까지 한다.
/// B의 I4 가운데 "`merge` 뒤"와 "구현 보고와 리뷰 보고가 함께 있을 때 `merge` 뒤"의 처리 표시를 본다.
#[test]
fn merge_fast_forwards_records_and_acks() {
    let mut w = World::new();
    let plans = format!(
        "{}| A-1 | 결제 재시도 | dev | 대기 | 사용자 지시 |\n| B-2 | 다음 일 | dev | 대기 | 사용자 지시 |\n",
        fullops_testkit::sandbox::PLANS.replace("\n\n## 위임 범위\n\n", "\n")
    );
    w.commit("lead", ".fullops-squad/PLANS.md", &plans);
    w.git("lead", &["push", "-q", "origin", "main"]);
    w.git("dev", &["merge", "-q", "main"]);
    let sha = w.work("src/a.py", 3);
    w.report("A-1", &sha);
    // 같은 과제의 리뷰 보고(completion이 없다)도 미처리로 남아 있다
    w.dispatched("A-1", Purpose::Review, &sha, true);
    // 건드리지 않을 것: 같은 과제의 질문과 다른 과제의 보고
    w.dispatched("B-9", Purpose::Review, &sha, true);
    w.edit(|state| {
        let mut question = state.messages[0].clone();
        question.id = MessageId::new("msg_q").unwrap();
        question.kind = "question".into();
        state.messages.push(question);
    });
    assert_eq!(w.state().messages.len(), 4);

    let run = w.cli(&["merge", "A-1", "--json"]);
    expect(&run, 0, &[]);
    let result = out(&run);
    assert_eq!(
        (
            &result["merge"],
            &result["lint"],
            &result["landing"],
            &result["pushed"]
        ),
        (
            &json!("ff"),
            &json!("reused"),
            &json!("landed"),
            &json!(true)
        )
    );
    // 원격에 들어갔고 병합 기록 커밋이 하나 있다
    assert!(w.contains(sha.as_str(), "origin/main"));
    assert_eq!(subjects(&w, "lead")[0], "fullops: 병합 기록 A-1");
    let plans = std::fs::read_to_string(w.repo().plan_path()).unwrap();
    for text in [
        format!(
            "| A-1 | 결제 재시도 | dev | 완료({}) | 사용자 지시 |",
            sha.short(7)
        ),
        "## 배정 기록".into(),
        "A-1 → dev | claude/test-model/medium | fresh | run run_1 | 재배정 0 | -".into(),
        "## 완료 보고 후속 요청".into(),
        "- [ ] tester: 독립 QA (A-1)".into(),
    ] {
        assert!(
            plans.contains(&text),
            "PLANS.md에 `{text}`가 없습니다:\n{plans}"
        );
    }
    let body = w.git("lead", &["log", "-1", "--format=%b"]);
    assert!(
        body.contains("리뷰 판정: 소규모 변경") && body.contains("재사용"),
        "{body}"
    );
    // M-10: 그 과제의 두 보고는 처리 표시가 돼 과제와 함께 archive로 갔다(처리 표시가 없는 메시지는 state에 남는다).
    // 질문과 다른 과제의 보고는 그대로 미처리다
    let state = w.state();
    assert!(state.completions.is_empty() && state.dispatches.len() == 1);
    let left: Vec<(&str, bool)> = state
        .messages
        .iter()
        .map(|m| (m.id.as_str(), m.acked_at.is_some()))
        .collect();
    assert_eq!(left, [("msg_3", false), ("msg_q", false)]);
    assert_eq!(result["unacked"], json!(["msg_3", "msg_q"]));
    let next = result["next"].as_str().unwrap();
    assert!(
        next.contains("fullops wait --run run_1 --ack msg_3 --ack msg_q --no-wait"),
        "{next}"
    );
    let archived = w.archive();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0]["key"], "A-1");
    assert!(
        archived[0]["landed_at"].is_i64()
            && archived[0]["dispatches"].as_array().unwrap().len() == 2
    );
    // M-11: 깨끗한 역할 워크트리는 기본 브랜치를 받았다. M-13: 준비된 행과 지표가 status.json에 있다
    assert_eq!(
        w.git("dev", &["rev-parse", "HEAD"]),
        w.git("lead", &["rev-parse", "HEAD"])
    );
    assert_eq!(result["ready"], json!(["B-2"]));
    let status: Value = fullops_store::read_json(&w.repo().status_path())
        .yes()
        .flatten()
        .unwrap();
    assert_eq!(status["ready"], json!(["B-2"]));
    assert!(status["metrics"]["at"].as_i64().unwrap() > 0 && status["pending"] == json!([]));
    // 다시 실행하면 통합 대기가 없다
    expect(&w.cli(&["merge", "A-1"]), 2, &["FullOps[M-02]"]);
}

/// M-01과 M-05: lead만 병합하고, lint 증거(`reported.lint_head`, 없으면 worker 워크트리의 `lint/pass.json`)가 SHA를 덮어야 한다.
#[test]
fn merge_requires_lead_and_lint_evidence() {
    let mut w = World::new();
    let sha = w.work("src/a.py", 3);
    let id = w.report("A-1", &sha);
    expect(
        &w.sb.cli(&w.dev(), &["merge", "A-1"]),
        2,
        &["FullOps[M-01]"],
    );
    let no_lint =
        |state: &mut State| state.dispatches[0].reported.as_mut().unwrap().lint_head = None;
    w.edit(no_lint);
    let words = ["FullOps[M-05]", "fullops lint", "--accept-missing-lint"];
    expect(&w.cli(&["merge", "A-1"]), 2, &words);

    // worker 워크트리의 pass.json이 덮으면 통과한다. 그 뒤에 기록 경로만 바꾼 커밋이 있어도 덮는다
    let pass = LintPass {
        v: 1,
        base: sha.clone(),
        head: sha.clone(),
        tree: sha.clone(),
        config_sha256: "0".repeat(64),
        test_level: TestLevel::Lite,
        at: fullops_store::now(),
    };
    let dev = fullops_store::discover(&w.dev()).unwrap().unwrap();
    fullops_store::write_json(&dev.lint_dir().join("pass.json"), &pass).unwrap();
    let log = ".fullops-squad/handovers/logs/A-1.dev.r0.md";
    let recorded = w.commit(
        "dev",
        log,
        "완료 보고
",
    );
    expect(
        &w.cli(&["completion", "set-sha", &id, recorded.as_str()]),
        0,
        &[],
    );
    expect(&w.cli(&["merge", "A-1"]), 0, &[]);

    // 코드가 더 바뀌면 옛 통과 기록은 덮지 못한다. 사유를 적으면 증거 없이 병합한다
    let next = w.work("src/b.py", 3);
    w.report("B-2", &next);
    w.edit(no_lint);
    expect(&w.cli(&["merge", "B-2"]), 2, &["FullOps[M-05]"]);
    let accept = ["merge", "B-2", "--accept-missing-lint", "검사가 없는 레포"];
    expect(&w.cli(&accept), 0, &[]);
    let body = w.git("lead", &["log", "-1", "--format=%b"]);
    assert!(
        body.contains("lint 증거 없이 병합: 검사가 없는 레포"),
        "{body}"
    );

    // 문서만 바꾼 과제는 lint 증거가 필요 없다
    let doc = w.commit(
        "dev",
        "docs/a.md",
        "# 문서
",
    );
    w.report("C-3", &doc);
    w.edit(no_lint);
    expect(&w.cli(&["merge", "C-3"]), 0, &[]);
    assert_eq!(w.archive().len(), 3);
}

/// L5 lint 실패: 병합 결과의 트리가 다르면 lint를 한 번 돌리고, 실패하면 병합 전으로 되돌린다.
#[test]
fn merge_reverts_when_lint_fails_after_merge() {
    let mut w = World::new();
    // 기준 시점 설정에 실패하는 검사를 둔다(lint가 어느 쪽에서 실패하든 M-07은 같다)
    let lint = r#"{"schema_version":1,"commands":[{"name":"fail","run":["git","no-such-subcommand"]}],"exclude":[],"rules":[]}"#;
    w.commit("lead", ".fullops-squad/lint/lint.json", lint);
    w.git("lead", &["push", "-q", "origin", "main"]);
    w.git("dev", &["merge", "-q", "main"]);
    let sha = w.work("src/a.py", 3);
    w.report("A-1", &sha);
    // 기본 브랜치가 따로 나아가 병합 커밋이 생긴다
    w.commit("lead", "src/other.py", "print('lead')\n");
    w.git("lead", &["push", "-q", "origin", "main"]);
    let tip = w.git("lead", &["rev-parse", "HEAD"]);

    let run = w.cli(&["merge", "A-1"]);
    expect(
        &run,
        2,
        &["FullOps[M-07]", "fullops task reopen", "fullops hold"],
    );
    assert_eq!(w.git("lead", &["rev-parse", "HEAD"]), tip);
    assert!(!w.contains(sha.as_str(), "origin/main"));
    assert_eq!(w.state().pending().count(), 1);
}

/// L5 충돌: 종료 코드 3으로 멈추고 `--abort`와 `--continue`로 이어 간다.
#[test]
fn merge_conflict_stops_then_continues() {
    let mut w = World::new();
    w.commit("lead", "src/c.py", "v0\n");
    w.git("lead", &["push", "-q", "origin", "main"]);
    w.git("dev", &["merge", "-q", "main"]);
    w.commit("lead", "src/c.py", "lead\n");
    w.git("lead", &["push", "-q", "origin", "main"]);
    let sha = w.commit("dev", "src/c.py", "dev\n");
    w.report("A-1", &sha);

    expect(
        &w.cli(&["merge", "A-1"]),
        3,
        &["FullOps[M-06]", "--continue", "--abort", "src/c.py"],
    );
    expect(
        &w.cli(&["merge", "A-1"]),
        2,
        &["FullOps[M-03]", "--continue"],
    );
    expect(&w.cli(&["merge", "A-1", "--abort"]), 0, &[]);
    assert_eq!(w.git("lead", &["status", "--porcelain"]), "");
    expect(&w.cli(&["merge", "A-1"]), 3, &["FullOps[M-06]"]);
    // 해결하지 않고 이어 가면 다시 멈춘다
    expect(
        &w.cli(&["merge", "A-1", "--continue"]),
        3,
        &["FullOps[M-06]"],
    );
    w.git("lead", &["checkout", "--theirs", "src/c.py"]);
    w.git("lead", &["add", "src/c.py"]);
    let run = w.cli(&["merge", "A-1", "--continue", "--json"]);
    expect(&run, 0, &[]);
    assert_eq!(out(&run)["landing"], "landed");
    assert!(w.contains(sha.as_str(), "origin/main") && w.state().completions.is_empty());
}

/// L5 push 실패(M-09): 종료 코드 4, 완료 보고는 통합 대기로 남고, 다시 실행하면 push만 한다.
#[test]
fn merge_push_failure_then_rerun_pushes_only() {
    let mut w = World::new();
    let sha = w.work("src/a.py", 3);
    w.report("A-1", &sha);
    let origin = w.git("lead", &["remote", "get-url", "origin"]);
    let missing = w.sb.root.join("missing.git");
    w.git(
        "lead",
        &["remote", "set-url", "origin", &missing.to_string_lossy()],
    );

    expect(&w.cli(&["merge", "A-1"]), 4, &["push", "다시 실행"]);
    assert_eq!(w.state().pending().count(), 1);
    let records = |w: &World| {
        let all = subjects(w, "lead");
        all.iter()
            .filter(|s| *s == "fullops: 병합 기록 A-1")
            .count()
    };
    assert_eq!(records(&w), 1);

    w.git("lead", &["remote", "set-url", "origin", &origin]);
    let run = w.cli(&["merge", "A-1", "--json"]);
    expect(&run, 0, &[]);
    assert_eq!(out(&run)["merge"], "already");
    assert_eq!(records(&w), 1);
    assert!(w.contains(sha.as_str(), "origin/main") && w.state().completions.is_empty());
}

/// `merge`가 M-06 뒤나 M-09 앞뒤에서 죽은 자리를 그대로 만들고 다시 실행한다.
/// 프로세스를 시각에 맞춰 죽이는 대신, 그 단계까지만 끝난 저장소 상태(날것 git으로 만든다)에서 이어 가는지를 본다.
#[test]
fn merge_resumes_from_where_it_died() {
    let mut w = World::new();
    // M-06 직후: 기본 브랜치에 병합만 돼 있고 기록 커밋·push·상태 기록이 없다
    let first = w.work("src/a.py", 3);
    w.report("A-1", &first);
    w.git("lead", &["merge", "-q", "--no-edit", first.as_str()]);
    let run = w.cli(&["merge", "A-1", "--json"]);
    expect(&run, 0, &[]);
    assert_eq!(
        (&out(&run)["merge"], &out(&run)["landing"]),
        (&json!("already"), &json!("landed"))
    );
    assert_eq!(subjects(&w, "lead")[0], "fullops: 병합 기록 A-1");
    assert!(w.contains(first.as_str(), "origin/main"));

    // M-09 앞: 병합과 기록 커밋까지 끝났고 push가 남았다(`--no-push`가 같은 자리에서 멈춘다)
    let second = w.work("src/b.py", 3);
    let id = w.report("B-2", &second);
    let run = w.cli(&["merge", "B-2", "--no-push", "--json"]);
    expect(&run, 0, &["fullops merge B-2"]);
    assert_eq!(out(&run)["landing"], "push_left");
    assert_eq!(w.state().pending().count(), 1);
    let status: Value = fullops_store::read_json(&w.repo().status_path())
        .yes()
        .flatten()
        .unwrap();
    assert_eq!(
        status["pending"][0],
        json!({ "id": id, "key": "B-2", "sha": second.short(7),
        "landing": "push_left", "hold_reason": null })
    );
    // M-09 뒤: push는 됐는데 상태를 쓰기 전에 죽었다
    w.git("lead", &["push", "-q", "origin", "main"]);
    let run = w.cli(&["merge", "B-2", "--json"]);
    expect(&run, 0, &[]);
    assert_eq!(out(&run)["landing"], "landed");
    let all = subjects(&w, "lead");
    assert_eq!(
        all.iter()
            .filter(|s| *s == "fullops: 병합 기록 B-2")
            .count(),
        1
    );
    assert!(w.state().completions.is_empty() && w.archive().len() == 2);
}

/// L2: 대규모 변경은 결정을 한 번 열고(종료 코드 3), 답이 생략이면 면제 기록 뒤에 병합한다.
#[test]
fn large_change_asks_once_then_waives() {
    let mut w = World::new();
    let sha = w.work("src/big.py", 400);
    w.report("A-1", &sha);

    let need = out(&w.cli(&["review", "need", "A-1", "--json"]));
    assert_eq!(
        (&need["required"], &need["action"], &need["code_lines"]),
        (&json!(true), &json!("ask"), &json!(400))
    );
    let run = w.cli(&["merge", "A-1", "--json"]);
    expect(&run, 3, &["FullOps[M-04]", "fullops decision answer D001"]);
    assert_eq!(out(&run)["human"]["decision"], "D001");
    let asked = w.state().decisions[0].clone();
    assert_eq!(
        (asked.sha.as_ref(), asked.blocks.as_slice()),
        (Some(&sha), &["A-1".to_string()][..])
    );
    assert_eq!(asked.options, ["진행", "생략"]);
    assert_eq!(asked.expires_at, Some(asked.asked_at + 86_400));
    // 다시 실행해도 같은 결정이다
    expect(&w.cli(&["merge", "A-1"]), 3, &["D001"]);
    assert_eq!(w.state().decisions.len(), 1);
    // 답이 없는 결정으로는 면제하지 못한다
    let waive = [
        "review",
        "waive",
        "A-1",
        "--decision",
        "D001",
        "--reason",
        "사용자가 생략을 지시했다",
    ];
    expect(&w.cli(&waive), 2, &["FullOps[RV-00]"]);

    expect(
        &w.cli(&[
            "decision",
            "answer",
            "D001",
            "--answer",
            "생략",
            "--sha",
            sha.as_str(),
        ]),
        0,
        &[],
    );
    expect(
        &w.cli(&["merge", "A-1"]),
        2,
        &["FullOps[M-04]", "fullops review waive A-1 --decision D001"],
    );
    expect(&w.cli(&waive), 0, &[]);
    expect(&w.cli(&waive), 2, &["FullOps[RV-00]"]);
    let waiver: Value = serde_json::from_str(
        &std::fs::read_to_string(w.review_dir("A-1", &sha).join("waiver.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        (
            &waiver["decision"],
            &waiver["decided_by"],
            &waiver["need"]["code_lines"]
        ),
        (&json!("D001"), &json!("user"), &json!(400))
    );

    expect(&w.cli(&["merge", "A-1"]), 0, &[]);
    assert!(w.contains(sha.as_str(), "origin/main"));
    // 면제 기록은 병합 기록 커밋에 들어갔다
    let files = w.git("lead", &["show", "--name-only", "--format=", "HEAD"]);
    assert!(files.contains("waiver.json"), "{files}");
}

/// L2 진행 → 수락 리뷰, L3(구현자와 같은 세션·미해결 high), M-12의 snapshot 정리.
#[test]
fn accepted_review_lets_large_change_merge() {
    let mut w = World::new();
    let sha = w.work("src/big.py", 400);
    w.lint_result(&sha);
    w.report("A-1", &sha);
    expect(&w.cli(&["merge", "A-1"]), 3, &["D001"]);
    expect(
        &w.cli(&[
            "decision",
            "answer",
            "D001",
            "--answer",
            "진행",
            "--sha",
            sha.as_str(),
        ]),
        0,
        &[],
    );
    expect(
        &w.cli(&["merge", "A-1"]),
        2,
        &["FullOps[M-04]", "fullops review snapshot A-1"],
    );

    let run = w.cli(&["review", "snapshot", "A-1", "--json"]);
    expect(&run, 0, &[]);
    let snapshot = PathBuf::from(out(&run)["snapshot"].as_str().unwrap());
    let dir = w.review_dir("A-1", &sha);
    assert!(snapshot.join("src/big.py").is_file());
    for name in ["targets.json", "result.json", "report.md", "lint.json"] {
        assert!(dir.join(name).is_file(), "{name}");
    }
    let state = w.state();
    assert_eq!(state.snapshots[0].state, SnapshotState::Active);
    assert_eq!(
        state.snapshots[0].implementer_session.as_deref(),
        Some("sid-dev")
    );
    assert_eq!(
        fullops_core::stage_of(
            &TaskKey::new("A-1").unwrap(),
            &state,
            &Default::default(),
            true
        ),
        fullops_core::model::Stage::Reviewing
    );
    // 있는 기록은 덮어쓰지 않는다. 끝나지 않은 리뷰로는 병합하지 못한다
    expect(
        &w.cli(&["review", "snapshot", "A-1"]),
        2,
        &["FullOps[RV-00]"],
    );
    expect(
        &w.cli(&["merge", "A-1"]),
        2,
        &["FullOps[M-04]", "fullops review check A-1"],
    );
    expect(
        &w.cli(&["review", "check", "A-1"]),
        2,
        &["FullOps[RV-03]", "FullOps[RV-04]", "FullOps[RV-06]"],
    );

    // L3: 구현자와 같은 세션의 리뷰, 미해결 high
    w.review("A-1", &sha, "sid-dev", Some(false));
    let run = w.cli(&["review", "check", "A-1", "--json"]);
    expect(&run, 2, &["FullOps[RV-04]", "FullOps[RV-07]"]);
    assert_eq!(out(&run)["refused"].as_array().unwrap().len(), 2);
    assert!(!dir.join("accepted.json").exists());
    expect(
        &w.cli(&["merge", "A-1"]),
        2,
        &["FullOps[M-04]", "fullops task reopen"],
    );

    // snapshot을 건드리면 검사에 걸린다(읽기 전용 리뷰)
    w.sb.write(
        &snapshot.join("scratch.txt"),
        "x
",
    );
    w.review("A-1", &sha, "sid-reviewer", Some(true));
    expect(&w.cli(&["review", "check", "A-1"]), 2, &["FullOps[RV-05]"]);
    std::fs::remove_file(snapshot.join("scratch.txt")).unwrap();

    // 다른 세션이 검토하고 high를 해결했다
    w.review("A-1", &sha, "sid-reviewer", Some(true));
    expect(&w.cli(&["review", "check", "A-1"]), 0, &["accepted.json"]);
    let accepted: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("accepted.json")).unwrap()).unwrap();
    assert_eq!(
        (&accepted["head"], &accepted["reviewer_session"]),
        (&json!(sha.as_str()), &json!("sid-reviewer"))
    );
    // 수락 뒤에 결과를 고치면 수락이 무효다
    let result = std::fs::read_to_string(dir.join("result.json")).unwrap();
    std::fs::write(dir.join("result.json"), format!("{result}\n")).unwrap();
    expect(&w.cli(&["merge", "A-1"]), 2, &["FullOps[M-04]"]);
    std::fs::write(dir.join("result.json"), result).unwrap();

    w.no_terminals();
    expect(&w.cli(&["merge", "A-1"]), 0, &[]);
    assert!(w.contains(sha.as_str(), "origin/main"));
    let files = w.git("lead", &["show", "--name-only", "--format=", "HEAD"]);
    assert!(
        files.contains("accepted.json") && files.contains("result.json"),
        "{files}"
    );
    // M-12: 끝난 리뷰의 snapshot을 지웠다
    assert!(!snapshot.exists());
    assert_eq!(w.state().snapshots[0].state, SnapshotState::Removed);
}

/// L4: 수정 커밋 뒤에는 이전 SHA의 리뷰로 병합하지 못한다. 새 SHA를 리뷰하면 옛 리뷰를 대체로 잇는다.
#[test]
fn review_of_old_sha_does_not_cover_fix_commit() {
    let mut w = World::new();
    let old = w.work("src/big.py", 400);
    w.lint_result(&old);
    let id = w.report("A-1", &old);
    expect(&w.cli(&["review", "snapshot", "A-1"]), 0, &[]);
    w.review("A-1", &old, "sid-reviewer", None);
    expect(&w.cli(&["review", "check", "A-1"]), 0, &[]);

    // worker가 수정 커밋을 더했고 완료 보고의 SHA가 새 커밋이 됐다
    let new = w.work("src/big.py", 401);
    w.lint_result(&new);
    expect(
        &w.cli(&["completion", "set-sha", &id, &new.as_str()[..10]]),
        0,
        &[],
    );
    w.edit(|state| {
        // worker가 수정 뒤 다시 `done`을 했다
        let reported = state.dispatches[0].reported.as_mut().unwrap();
        (reported.sha, reported.lint_head) = (new.clone(), Some(new.clone()));
    });
    let completion = w.state().completions[0].clone();
    assert_eq!(
        (completion.sha.as_ref(), completion.sha_source),
        (Some(&new), Some(ShaSource::Manual))
    );
    let main = w.git("lead", &["rev-parse", "main"]);
    let run = w.cli(&["merge", "A-1"]);
    assert_ne!(run.code, Some(0));
    expect(&run, 3, &["FullOps[M-04]"]);
    assert_eq!(w.git("lead", &["rev-parse", "main"]), main);
    // 옛 head로는 기록 검사도 맞지 않는다
    expect(
        &w.cli(&["review", "check", "A-1", "--sha", old.as_str()]),
        2,
        &["FullOps[RV-01]"],
    );

    // 새 SHA의 리뷰를 수락하고 옛 리뷰를 대체로 잇는다
    w.no_terminals();
    expect(
        &w.cli(&[
            "review",
            "cleanup",
            "A-1",
            "--sha",
            old.as_str(),
            "--finished",
        ]),
        0,
        &[],
    );
    expect(&w.cli(&["review", "snapshot", "A-1"]), 0, &[]);
    let supersede = [
        "review",
        "supersede",
        "A-1",
        "--old",
        old.short(12),
        "--new",
        new.short(12),
    ];
    expect(&w.cli(&supersede), 2, &["FullOps[RV-00]"]);
    w.review("A-1", &new, "sid-reviewer", None);
    expect(&w.cli(&["review", "check", "A-1"]), 0, &[]);
    expect(&w.cli(&supersede), 0, &[]);
    let record: Value = serde_json::from_str(
        &std::fs::read_to_string(w.review_dir("A-1", &old).join("superseded.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(record["by"], new.as_str());
    expect(&w.cli(&["merge", "A-1"]), 0, &[]);
    assert!(w.contains(new.as_str(), "origin/main"));
}

/// 결정 검증(설계 8.10절): SHA 불일치, 만료, 허용 밖, 늦은 답.
#[test]
fn decision_answer_is_validated() {
    let w = World::new();
    let sha = "a".repeat(40);
    let add = [
        "decision",
        "add",
        "--question",
        "배포할까요?",
        "--key",
        "A-1",
        "--option",
        "진행",
        "--option",
        "생략",
        "--blocks",
        "A-1",
        "--sha",
        &sha,
        "--allow",
        "owner",
        "--json",
    ];
    let run = w.cli(&add);
    expect(&run, 0, &[]);
    assert_eq!(out(&run)["id"], "D001");
    let answer = |extra: &[&str]| {
        let mut args = vec!["decision", "answer", "D001", "--answer", "진행"];
        args.extend(extra);
        w.cli(&args)
    };
    // SHA가 없거나 다르다
    expect(&answer(&["--by", "owner"]), 2, &["FullOps[DC-01]"]);
    expect(
        &answer(&["--by", "owner", "--sha", &"b".repeat(40)]),
        2,
        &["FullOps[DC-01]"],
    );
    // 허용 밖의 사람
    expect(
        &answer(&["--by", "stranger", "--sha", &sha]),
        2,
        &["FullOps[DC-03]"],
    );
    expect(&answer(&["--sha", &sha]), 2, &["FullOps[DC-03]"]);
    // 만료
    let expires = w.state().decisions[0].expires_at.unwrap();
    w.edit(|state| state.decisions[0].expires_at = Some(fullops_store::now() - 1));
    expect(
        &answer(&["--by", "owner", "--sha", &sha]),
        2,
        &["FullOps[DC-02]"],
    );
    expect(
        &w.cli(&["decision", "list", "--open"]),
        0,
        &["D001", "만료"],
    );
    // 거절은 기록을 바꾸지 않는다
    let open = w.state().decisions[0].clone();
    assert_eq!(
        (open.status.as_str(), open.answer, open.closed_at),
        ("open", None, None)
    );
    w.edit(|state| state.decisions[0].expires_at = Some(expires));

    expect(
        &answer(&["--by", "owner", "--via", "bot", "--sha", &sha]),
        0,
        &[],
    );
    let closed = w.state().decisions[0].clone();
    assert_eq!(
        (
            closed.status.as_str(),
            closed.answer.as_deref(),
            closed.decided_by.as_deref(),
            closed.via.map(|v| v.as_str())
        ),
        ("answered", Some("진행"), Some("owner"), Some("bot"))
    );
    // 늦은 답은 덮어쓰지 않는다
    let late = w.cli(&[
        "decision", "answer", "D001", "--answer", "생략", "--by", "owner", "--sha", &sha, "--json",
    ]);
    expect(&late, 0, &[]);
    assert_eq!(
        (&out(&late)["already"], &out(&late)["answer"]),
        (&json!(true), &json!("진행"))
    );
    assert_eq!(w.state().decisions[0].answer.as_deref(), Some("진행"));

    // 버린 결정, 목록, 모르는 ID
    expect(
        &w.cli(&[
            "decision",
            "add",
            "--question",
            "둘째",
            "--repository",
            "o/r",
            "--origin",
            "bot",
        ]),
        0,
        &["D002"],
    );
    expect(
        &w.cli(&["decision", "drop", "D002", "--reason", "필요 없어졌다"]),
        0,
        &[],
    );
    assert_eq!(
        out(&w.cli(&["decision", "list", "--open", "--json"]))["decisions"],
        json!([])
    );
    let all = out(&w.cli(&["decision", "list", "--json"]));
    assert_eq!(all["decisions"].as_array().unwrap().len(), 2);
    assert_eq!(
        (
            &all["decisions"][1]["drop_reason"],
            &all["decisions"][1]["repository"]
        ),
        (&json!("필요 없어졌다"), &json!("o/r"))
    );
    expect(
        &w.cli(&["decision", "answer", "D999", "--answer", "진행"]),
        2,
        &["FullOps[DC-00]"],
    );
    expect(&w.cli(&["decision", "add"]), 1, &[]);
}

/// J4: 보류는 보고를 처리한 것으로 표시하고 통합 대기에서 뺀다. 보류된 SHA가 날것 병합·push로 원격에 들어가면
/// `settle`이 닫는다(B의 I4 가운데 `hold` 뒤와 통합 확인 뒤의 처리 표시).
#[test]
fn hold_resume_and_settle() {
    let mut w = World::new();
    let first = w.work("src/a.py", 3);
    let id = w.report("A-1", &first);
    expect(
        &w.cli(&["hold", "A-1", "--reason", "배포 창을 기다린다"]),
        1,
        &[],
    );
    let hold = [
        "hold",
        "A-1",
        "--reason",
        "배포 창을 기다린다",
        "--owner",
        "lead",
        "--resume",
        "금요일 배포 뒤",
    ];
    expect(&w.cli(&hold), 0, &[]);
    let state = w.state();
    let held = state.completions[0].hold.clone().unwrap();
    assert_eq!(
        (
            held.reason.as_str(),
            held.owner.as_str(),
            held.resume.as_str()
        ),
        ("배포 창을 기다린다", "lead", "금요일 배포 뒤")
    );
    assert!(state.pending().next().is_none() && state.messages[0].acked_at.is_some());
    expect(&w.cli(&hold), 2, &["FullOps[M-02]"]);
    expect(
        &w.cli(&["merge", "A-1"]),
        2,
        &["FullOps[M-02]", "fullops resume A-1"],
    );

    // 재개하면 사유를 남긴 채 통합 대기로 돌아간다
    expect(&w.cli(&["resume", "A-1"]), 0, &[]);
    let state = w.state();
    assert!(state.completions[0].hold.is_none() && state.completions[0].released_hold.is_some());
    assert_eq!(state.pending().count(), 1);
    expect(&w.cli(&["resume", "A-1"]), 2, &["FullOps[HD-01]"]);
    expect(&w.cli(&hold), 0, &[]);

    // 통합 전의 settle은 아무것도 닫지 않는다
    let run = w.cli(&["settle", "--json"]);
    expect(&run, 0, &[]);
    assert_eq!(
        (&out(&run)["landed"], &out(&run)["pending"][&id]),
        (&json!([]), &json!("merge_left"))
    );

    // 리뷰 worker가 아직 도는 동안, 사람이 역할 브랜치 tip(보고 SHA 뒤의 커밋 포함)을 날것으로 병합해 push했다
    w.dispatched("A-1", Purpose::Review, &first, false);
    w.work("src/a.py", 5);
    w.git("lead", &["merge", "-q", "--no-edit", "fullops/dev"]);
    w.git("lead", &["push", "-q", "origin", "main"]);
    let run = w.cli(&["settle", "--json"]);
    expect(&run, 0, &[]);
    assert_eq!(
        (&out(&run)["landed"], &out(&run)["archived"]),
        (&json!([id]), &json!([]))
    );
    let done = w.state().completions[0].clone();
    assert!(done.landed_at.is_some() && done.hold.is_none());
    assert_eq!(
        (done.released_hold.map(|h| h.owner), done.drift),
        (Some("lead".to_string()), Some(1))
    );

    // 남은 dispatch가 정산되면 과제가 archive로 간다
    w.edit(|state| state.dispatches.retain(|d| d.settled.is_some()));
    assert_eq!(
        out(&w.cli(&["settle", "--json"]))["archived"],
        json!(["A-1"])
    );
    assert!(w.state().completions.is_empty() && w.archive()[0]["key"] == "A-1");
    expect(
        &w.cli(&["completion", "set-sha", "msg_9", first.as_str()]),
        2,
        &["FullOps[HD-02]"],
    );

    // 보류하지 않은 보고도 날것 병합·push 뒤의 settle이 처리 표시한다
    let third = w.work("src/c.py", 2);
    let id = w.report("C-3", &third);
    w.dispatched("C-3", Purpose::Review, &third, false);
    w.git("lead", &["merge", "-q", "--no-edit", third.as_str()]);
    w.git("lead", &["push", "-q", "origin", "main"]);
    assert_eq!(out(&w.cli(&["settle", "--json"]))["landed"], json!([id]));
    let state = w.state();
    let report = state.messages.iter().find(|m| m.id.as_str() == id).unwrap();
    assert!(report.acked_at.is_some() && state.pending().next().is_none());
}

/// M-11과 `fullops sync`: 더러운 역할 워크트리는 예약으로 남기고, 깨끗해지면 반영한다.
#[test]
fn sync_reserves_dirty_worktree() {
    let mut w = World::new();
    let sha = w.work("src/a.py", 3);
    w.report("A-1", &sha);
    w.sb.write(&w.dev().join("wip.txt"), "작업 중\n");
    let run = w.cli(&["merge", "A-1", "--json"]);
    expect(&run, 0, &[]);
    let tip = Sha::new(w.git("lead", &["rev-parse", "main"])).unwrap();
    assert_eq!(out(&run)["synced"], json!([]));
    let dev = RoleId::new("dev").unwrap();
    assert_eq!(w.state().sync_pending.get(&dev), Some(&tip));

    expect(&w.cli(&["sync"]), 2, &["FullOps[M-11]", "dev"]);
    assert_ne!(w.git("dev", &["rev-parse", "HEAD"]), tip.as_str());
    std::fs::remove_file(w.dev().join("wip.txt")).unwrap();
    expect(&w.cli(&["sync", "--json"]), 0, &["dev"]);
    assert_eq!(w.git("dev", &["rev-parse", "HEAD"]), tip.as_str());
    assert!(w.state().sync_pending.is_empty());
    expect(&w.cli(&["sync", "--role", "dev"]), 0, &[]);
    expect(&w.cli(&["sync", "--self"]), 0, &[]);
}

/// `review cleanup`·`prune`: 확인하지 못했거나 남은 파일이 있으면 지우지 않고 사유를 적는다.
#[test]
fn snapshot_cleanup_holds_when_unsure() {
    let mut w = World::new();
    let sha = w.work("src/a.py", 3);
    w.report("A-1", &sha);
    let run = w.cli(&["review", "snapshot", "A-1", "--json"]);
    expect(&run, 0, &[]);
    let snapshot = PathBuf::from(out(&run)["snapshot"].as_str().unwrap());
    let finished = ["review", "cleanup", "A-1", "--finished"];
    expect(&w.cli(&["review", "cleanup", "A-1"]), 1, &[]);
    // lint 결과가 없고, snapshot 뒤에 리뷰 규칙이 바뀌었다
    w.sb.write(
        &w.lead().join(".fullops-squad/review/rule.json"),
        r#"{"exclude":[],"rules":[]}"#,
    );
    expect(
        &w.cli(&["review", "check", "A-1"]),
        2,
        &["FullOps[RV-08]", "FullOps[RV-09]"],
    );

    // Orca가 답하지 않으면 지우지 않는다
    expect(&w.cli(&finished), 2, &["FullOps[RV-00]", "터미널"]);
    assert!(snapshot.is_dir() && w.state().snapshots[0].hold.is_some());
    // 미추적 파일이 있으면 지우지 않는다
    w.no_terminals();
    w.sb.write(&snapshot.join("note.txt"), "리뷰어의 메모\n");
    expect(&w.cli(&finished), 2, &["FullOps[RV-00]", "미추적"]);
    assert_eq!(w.state().snapshots[0].state, SnapshotState::Active);
    expect(&w.cli(&["review", "prune"]), 0, &["A-1@", "정리 보류"]);
    std::fs::remove_file(snapshot.join("note.txt")).unwrap();
    expect(
        &w.cli(&[
            "review",
            "cleanup",
            "A-1",
            "--abandon",
            "--reason",
            "리뷰를 중단했다",
        ]),
        0,
        &[],
    );
    assert!(!snapshot.exists());
    assert_eq!(w.state().snapshots[0].state, SnapshotState::Abandoned);
    assert!(w.review_dir("A-1", &sha).join("abandoned.json").is_file());

    // 경로와 Git 등록이 모두 사라진 snapshot은 prune이 닫는다
    let next = w.work("src/a.py", 4);
    expect(
        &w.cli(&["review", "snapshot", "A-1", "--sha", next.as_str()]),
        0,
        &[],
    );
    let lost = PathBuf::from(&w.state().snapshots[1].path);
    std::fs::remove_dir_all(&lost).unwrap();
    assert_eq!(
        out(&w.cli(&["review", "prune", "--json"]))["active"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    w.git("lead", &["worktree", "prune"]);
    assert_eq!(
        out(&w.cli(&["review", "prune", "--json"]))["active"],
        json!([])
    );
    assert_eq!(w.state().snapshots[1].state, SnapshotState::Missing);
}

/// `gc`: 24시간 지난 예약 기록, 부모가 정산된 하위 dispatch, 30일 지난 결정, 닫힌 snapshot.
#[test]
fn gc_sweeps_closed_records() {
    let mut w = World::new();
    let sha = w.work("src/a.py", 3);
    w.report("A-1", &sha);
    w.dispatched("A-1", Purpose::Build, &sha, false);
    expect(&w.cli(&["review", "snapshot", "A-1"]), 0, &[]);
    expect(
        &w.cli(&["decision", "add", "--question", "오래된 결정"]),
        0,
        &[],
    );
    expect(
        &w.cli(&["decision", "drop", "D001", "--reason", "끝"]),
        0,
        &[],
    );
    expect(
        &w.cli(&["decision", "add", "--question", "열린 결정"]),
        0,
        &[],
    );
    let now = fullops_store::now();
    w.edit(|state| {
        // 하위 배정: 정산된 부모(ctx_1)가 소유한 Run에 남은 dispatch(ctx_2)
        let sub = RunId::new("run_sub").unwrap();
        state.runs.push(Run {
            id: sub.clone(),
            owner: RunOwner::Dispatch(DispatchId::new("ctx_1").unwrap()),
            coordinator: None,
            opened_at: now,
            closed_at: None,
            extra: Default::default(),
        });
        state.dispatches[1].sub_of = Some(DispatchId::new("ctx_1").unwrap());
        state.dispatches[1].run = sub;
        // 예약 기록 둘: 하루 넘은 것과 방금 것
        for (request, age) in [("req_old", 90_000), ("req_new", 10)] {
            let mut reservation = state.dispatches[0].clone();
            reservation.id = DispatchId::new(format!("pending:{request}")).unwrap();
            reservation.request = request.into();
            reservation.started_at = now - age;
            reservation.settled = None;
            state.dispatches.push(reservation);
        }
        state.decisions[0].closed_at = Some(now - 31 * 86_400);
        state.snapshots[0].state = SnapshotState::Missing;
    });

    let run = w.cli(&["gc", "--json"]);
    expect(&run, 0, &[]);
    let n = out(&run);
    assert_eq!(
        (
            &n["reservations"],
            &n["sub_dispatches_lost"],
            &n["decisions_archived"],
            &n["snapshots"],
            &n["tasks_archived"]
        ),
        (&json!(1), &json!(1), &json!(1), &json!(1), &json!(0))
    );
    let state = w.state();
    let ids: Vec<&str> = state.dispatches.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["ctx_1", "ctx_2", "pending:req_new"]);
    assert_eq!(
        state.dispatches[1].settled.as_ref().map(|s| s.how),
        Some(SettledHow::Lost)
    );
    assert!(
        state
            .runs
            .iter()
            .any(|r| r.id.as_str() == "run_sub" && r.closed_at.is_some())
    );
    assert_eq!((state.decisions.len(), state.snapshots.len()), (1, 0));
    assert_eq!(w.archive()[0]["decision"]["id"], "D001");
    // 열린 것은 그대로다
    assert_eq!(
        (
            state.pending().count(),
            state.decisions[0].question.as_str()
        ),
        (1, "열린 결정")
    );
    assert_eq!(out(&w.cli(&["gc", "--json"]))["reservations"], 0);
}

/// 맡은 시나리오 파일(J4, L1~L5와 결정 검증·다시 실행)이 `rs` 구동기에서 돈다. 다른 묶음에 기대는 단계는 보류로 적힌다.
#[test]
fn scenario_files_run() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios");
    let names = [
        "j4",
        "l1",
        "l2",
        "l2-decision",
        "l3",
        "l4",
        "l5",
        "l5-merge-commit",
        "l5-lint-fail",
        "l5-conflict",
        "l5-push-fail",
        "l5-rerun",
    ];
    for name in names {
        let scenario = scenario::load(&dir.join(format!("{name}.toml"))).unwrap();
        let results = run_scenario(&scenario).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(results.len(), scenario.steps.len(), "{name}");
    }
}
