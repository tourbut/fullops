//! 탐색(묶음 J)의 통합 시험: `scenarios/p*.toml`을 `rs` 구동기로 돌리고, 시나리오 형식으로 적지 못하는 것
//! (파일 내용, 패킷의 `input_sha256`에 맞춘 처리 기록, 동시 실행)을 같은 임시 레포에서 이어서 본다.

use fullops_testkit::driver::mismatches;
use fullops_testkit::{Driver, Run, scenario};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// 시나리오를 끝까지 돌리고 구동기를 돌려준다. `pending` 단계는 건너뛴다.
fn play(name: &str) -> Driver {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scenarios")
        .join(format!("{name}.toml"));
    let scenario = scenario::load(&path).unwrap();
    let mut driver = Driver::new(&scenario.setup);
    for (n, step) in scenario.steps.iter().enumerate() {
        if step.pending.is_some() {
            continue;
        }
        let result = driver
            .step(step)
            .unwrap_or_else(|e| panic!("{name} {}단계: {e}", n + 1));
        let diff = step
            .expect
            .as_ref()
            .map(|expect| mismatches(expect, &result))
            .unwrap_or_default();
        assert!(
            diff.is_empty(),
            "{name} {}단계({}): {}\n출력: {}",
            n + 1,
            step.act,
            diff.join(", "),
            result.output
        );
    }
    driver
}

fn jev(driver: &Driver, who: &str) -> PathBuf {
    driver
        .sb
        .path(who)
        .join(".fullops-squad/docs/evaluations/jev")
}

fn read(path: &Path) -> Value {
    serde_json::from_slice(
        &std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap()
}

fn cli(driver: &Driver, args: &[&str]) -> Run {
    driver.sb.cli(&driver.sb.path("lead"), args)
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

#[test]
fn p1_task_without_packet_is_never_checked() {
    let d = play("p1");
    // 패킷 없는 과제에서 탐색 폴더도 색인도 생기지 않는다
    assert!(!jev(&d, "lead").exists());
    assert!(!d.sb.path("lead").join(".git/fullops/search").exists());
}

#[test]
fn p2_packet_follows_the_instruction_body_only() {
    let d = play("p2");
    let inbox =
        std::fs::read_to_string(d.sb.path("lead").join(".fullops-squad/handovers/to_dev.md"))
            .unwrap();
    assert_eq!(inbox.matches("<!-- fullops-packet:start -->").count(), 1);
    assert!(
        inbox.contains("- [x] `retry`의 중복 클릭과 연타를 막는다")
            && inbox.contains("status: review")
    );
    let block = inbox.find("<!-- fullops-packet:end -->").unwrap();
    assert!(block < inbox.find("## 완료 보고").unwrap());
    let packet = read(&jev(&d, "lead").join("PAY-12-packet.json"));
    // 본문을 고친 뒤 다시 만든 패킷: find·context는 낡았다고 적히고 쓰이지 않는다
    assert_eq!(
        packet["producers"]["find"],
        json!({ "used": false, "reason": "stale" })
    );
    assert_eq!(packet["partial"], true);
    let seed = &packet["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["path"] == "src/pay/retry.rs")
        .unwrap();
    assert_eq!(seed["categories"], json!(["direct_edit"]));
    // 탐색 결과는 본문 없이 경로·요약·지문만 담는다
    let find = read(&jev(&d, "lead").join("PAY-12-find.json"));
    assert_eq!(
        (find["v"].as_i64(), find["semantic"].as_bool()),
        (Some(1), Some(false))
    );
    assert_eq!(
        find["seed_paths"],
        json!(["docs/pay.md", "src/pay/card.rs", "src/pay/retry.rs"])
    );
    assert!(!find.to_string().contains("중복 클릭"));
    let context = read(&jev(&d, "lead").join("PAY-12-context.json"));
    assert_eq!(context["candidates"][0]["decision"], "keep");
    assert!(strings(&context["required_paths"]).contains(&".fullops-squad/handovers/to_dev.md"));
}

#[test]
fn p3_changed_sources_and_rewritten_commits_block_delivery() {
    let d = play("p3");
    assert_eq!(
        jev(&d, "lead").join("history").read_dir().unwrap().count(),
        2
    );
    let packet = read(&jev(&d, "lead").join("PAY-12-packet.json"));
    let seed = packet["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["path"] == "src/pay/retry.rs")
        .unwrap();
    assert_eq!(seed["source_basis"], "worktree");
}

/// 패킷의 전달 목록을 모두 `completed`로 적은 처리 기록.
fn outcomes(packet: &Value) -> Value {
    let items: Vec<Value> = packet["delivered"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| json!({ "path": d["path"], "category": d["category"], "status": "completed", "reason": "확인했다" }))
        .collect();
    json!({ "v": 1, "key": "PAY-12", "round": 0, "packet_input_sha256": packet["input_sha256"],
            "items": items, "uncertainty_review": "" })
}

/// 처리 기록을 쓰고 완료 검사를 돌린다.
fn completion(d: &Driver, record: &Value) -> Run {
    let path = jev(d, "lead").join("PAY-12-packet-outcomes.json");
    std::fs::write(path, record.to_string()).unwrap();
    cli(
        d,
        &["packet", "check", "PAY-12", "--role", "dev", "--completion"],
    )
}

#[test]
fn p4_p5_completion_needs_an_outcome_for_every_delivered_pair() {
    let d = play("p4");
    let packet = read(&jev(&d, "lead").join("PAY-12-packet.json"));
    assert_eq!(packet["partial"], false, "{}", packet["unknown"]);
    assert_eq!(packet["decisions"][0]["action"], "exclude");
    let full = outcomes(&packet);
    assert!(full["items"].as_array().unwrap().len() >= 8);
    // P4·P5: 전부 기록하면 통과한다
    let run = completion(&d, &full);
    assert_eq!(run.code, Some(0), "{}", run.stderr);

    // J-15 (a) 전달 항목 누락 (b) 같은 쌍이 두 번 (c) 패킷에 없는 쌍 (d) status (e) 빈 reason
    let edit = |change: &dyn Fn(&mut Vec<Value>)| {
        let mut record = full.clone();
        change(record["items"].as_array_mut().unwrap());
        record
    };
    let cases: [(&str, Value); 5] = [
        ("빠졌다", edit(&|items| drop(items.remove(0)))),
        ("두 번 있다", edit(&|items| items.push(items[0].clone()))),
        (
            "패킷의 항목이 아니다",
            edit(&|items| {
                items.push(json!({ "path": "src/none.rs", "category": "direct_edit", "status": "completed", "reason": "x" }))
            }),
        ),
        (
            "completed·no_change",
            edit(&|items| items[0]["status"] = json!("unknown")),
        ),
        (
            "reason이 비었다",
            edit(&|items| items[0]["reason"] = json!("  ")),
        ),
    ];
    for (word, record) in &cases {
        let run = completion(&d, record);
        assert_eq!(run.code, Some(2), "{word}");
        assert!(
            run.stderr.contains("FullOps[J-15]") && run.stderr.contains(word),
            "{word}: {}",
            run.stderr
        );
        assert!(run.stderr.contains("packet-outcomes.json"));
    }
    // no_change와 이유를 적으면 통과한다. 추천은 수정 의무가 아니다
    let run = completion(&d, &edit(&|items| items[0]["status"] = json!("no_change")));
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    // 형식 버전이 다르면 없는 것으로 본다
    let mut old = full.clone();
    old["v"] = json!(2);
    assert!(completion(&d, &old).stderr.contains("FullOps[J-14]"));

    // J-16: partial 패킷(원천이 없는 갱신 ID)은 uncertainty_review를 요구한다
    let build = [
        "packet",
        "build",
        "PAY-12",
        "--role",
        "dev",
        "--seed",
        "src/pay/retry.rs",
        "--update-doc",
        "D99",
        "--force",
    ];
    assert_eq!(cli(&d, &build).code, Some(0));
    let packet = read(&jev(&d, "lead").join("PAY-12-packet.json"));
    assert_eq!(
        packet["unknown"],
        json!([{ "path": null, "source": null, "doc_id": "D99", "reason": "no source" }])
    );
    assert!(
        completion(&d, &full).stderr.contains("FullOps[J-14]"),
        "옛 패킷의 처리 기록"
    );
    let mut record = outcomes(&packet);
    let run = completion(&d, &record);
    assert!(
        run.code == Some(2) && run.stderr.contains("FullOps[J-16]") && !run.stderr.contains("J-15"),
        "{}",
        run.stderr
    );
    record["uncertainty_review"] = json!("D99는 이번 과제에서 만들지 않는 문서임을 확인했다");
    assert_eq!(completion(&d, &record).code, Some(0));
    // P5: 리뷰 head가 패킷을 만든 커밋이면 J-13에 걸리지 않는다
    let run = cli(
        &d,
        &[
            "packet",
            "check",
            "PAY-12",
            "--role",
            "dev",
            "--completion",
            "--head",
            "HEAD",
            "--json",
        ],
    );
    assert_eq!(
        (run.code, run.stdout.trim()),
        (Some(0), r#"{"checked":true,"ok":true}"#)
    );
    play("p5");
}

#[test]
fn p6_only_migrated_routes_require_a_packet() {
    play("p6");
}

#[test]
fn p7_find_ends_with_local_candidates_without_jev() {
    let d = play("p7");
    let find = read(&jev(&d, "lead").join("PAY-12-find.json"));
    assert_eq!(find["options"]["offline"], false);
    assert_eq!(
        (&find["options"]["provider"], &find["error"], &find["model"]),
        (&Value::Null, &json!("no key"), &Value::Null)
    );
    assert_eq!(
        (
            &find["semantic"],
            &find["presented_files"],
            &find["usage"]["calls"]
        ),
        (&json!(false), &json!(0), &json!(0))
    );
    assert_eq!(
        find["existence"],
        json!({ "status": "unknown", "found_probability": null })
    );
    let candidates = find["candidates"].as_array().unwrap();
    assert!(
        !candidates.is_empty()
            && candidates
                .iter()
                .all(|c| c["basis"] == "local" && c["probability"].is_null())
    );
    // 오프라인 결과는 history에 남았다
    let old = jev(&d, "lead")
        .join("history")
        .read_dir()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(read(&old)["error"], "offline");
    assert!(d.sb.curl_calls().is_empty());
}

#[test]
fn p8_secret_files_stay_out_of_the_map_and_results() {
    let d = play("p8");
    let dir = jev(&d, "lead");
    let (code, documents) = (
        read(&dir.join("PAY-12-find.json")),
        read(&dir.join("PAY-12-documents-find.json")),
    );
    // 지도: 코드는 README.md·docs/pay.md·src/empty.txt(빈 파일이라 제외)·src/pay/* 가운데 읽을 수 있는 것
    assert_eq!(code["files"], 4, "{code}");
    assert_eq!(documents["files"], 9, "{documents}");
    for result in [&code, &documents, &read(&dir.join("PAY-12-context.json"))] {
        let text = result.to_string();
        for word in ["hunter2", "prod.env", "secrets.yaml", "blob.rs", "link.rs"] {
            assert!(!text.contains(word), "{word}: {text}");
        }
    }
    assert!(!code.to_string().contains("keys.rs") && !code.to_string().contains(".env"));
    let context = read(&dir.join("PAY-12-context.json"));
    assert_eq!(
        context["refused_paths"],
        json!([{ "path": ".env", "reason": "sensitive path" }])
    );
    assert_eq!(context["candidates"][0]["unsent_reason"], "sensitive text");
    assert_eq!(context["options"]["sources"][".env"], "unavailable");
    // 색인에도 비밀 파일의 요약이 없다
    let index =
        std::fs::read_to_string(d.sb.path("lead").join(".git/fullops/search/code.json")).unwrap();
    assert!(!index.contains("hunter2") && !index.contains("\"path\": \".env\""));
    let keys = read(Path::new(
        &d.sb.path("lead").join(".git/fullops/search/code.json"),
    ));
    let keys = keys["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "src/keys.rs")
        .unwrap();
    assert_eq!(
        (&keys["searchable"], &keys["definitions"]),
        (&json!(false), &json!([]))
    );
    // 요청이 나갔다면 그 본문에도 없다(C의 JevCurl이 합쳐진 뒤에 뜻이 있다)
    assert!(
        d.sb.curl_calls()
            .iter()
            .all(|call| !call.to_string().contains("hunter2"))
    );
}

#[test]
fn p9_p11_index_handles_many_files_and_concurrent_runs() {
    let d = play("p9");
    let lead = d.sb.path("lead");
    for n in 0..300 {
        d.sb.write(
            &lead.join(format!("src/gen/m{n:03}.rs")),
            &format!("pub fn generated_{n}() {{}}\n"),
        );
    }
    d.sb.git_ok(&lead, &["add", "-A"]);
    d.sb.git_ok(&lead, &["commit", "-q", "-m", "파일 300개"]);
    let find = ["find", "PAY-12", "--role", "dev", "--force", "--json"];
    let run = cli(&d, &find);
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let out: Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!(
        (&out["ok"], &out["reused"], &out["result"]["files"]),
        (&json!(true), &json!(false), &json!(304))
    );
    assert_eq!(out["result"]["presented_files"], 0);

    // P11: 색인과 결과를 지우고 두 find를 동시에 돌린다. 둘 다 같은 결과이고 색인이 온전하다
    let d = play("p11");
    let lead = d.sb.path("lead");
    let index = lead.join(".git/fullops/search/code.json");
    let before = read(&index);
    std::fs::remove_file(&index).unwrap();
    std::fs::remove_file(jev(&d, "lead").join("PAY-12-find.json")).unwrap();
    let find = ["find", "PAY-12", "--role", "dev", "--json"];
    let (a, b) = std::thread::scope(|scope| {
        let a = scope.spawn(|| cli(&d, &find));
        let b = scope.spawn(|| cli(&d, &find));
        (a.join().unwrap(), b.join().unwrap())
    });
    let result = |run: &Run| -> Value {
        assert_eq!(run.code, Some(0), "{}", run.stderr);
        let mut out: Value = serde_json::from_str(&run.stdout).unwrap();
        // 먼저 끝난 쪽의 결과를 다른 쪽이 다시 낼 수 있다
        out["result"]["created_at"] = Value::Null;
        out["result"].take()
    };
    assert_eq!(result(&a), result(&b));
    assert_eq!(read(&index), before);
    // 색인은 바뀐 파일의 새 blob을 담는다
    let retry = before["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "src/pay/retry.rs")
        .unwrap();
    assert_eq!(
        retry["blob"],
        d.sb.git_ok(&lead, &["rev-parse", "HEAD:src/pay/retry.rs"])
    );
}

#[test]
fn p10_context_refuses_too_many_paths_and_keeps_everything_offline() {
    let d = play("p10");
    let context = read(&jev(&d, "lead").join("PAY-12-context.json"));
    let candidates = context["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 3);
    assert!(
        candidates
            .iter()
            .all(|c| c["decision"] == "keep" && c["signal"].is_null() && c["sent"] == true)
    );
    assert_eq!(
        (
            &context["semantic"],
            &context["error"],
            &context["usage"]["calls"]
        ),
        (&json!(false), &json!("offline"), &json!(0))
    );
    assert_eq!(candidates[1]["required"], true);
    assert_eq!(
        candidates[0]["span"],
        json!({ "start_line": 1, "end_line": 3 })
    );
}

#[test]
fn p12_packet_in_a_role_worktree() {
    let d = play("p12");
    let packet = read(&jev(&d, "dev").join("PAY-12-packet.json"));
    assert_eq!(packet["options"]["seeds"], json!(["src/pay/retry.rs"]));
}

#[test]
fn p13_stale_results_are_neither_reused_nor_overwritten_silently() {
    let d = play("p13");
    let dir = jev(&d, "lead");
    let mut history: Vec<String> = dir
        .join("history")
        .read_dir()
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    history.sort();
    let count = |prefix: &str| {
        history
            .iter()
            .filter(|name| name.starts_with(prefix))
            .count()
    };
    // --force마다 이전 결과가 남는다: find 2번, context 1번, packet 2번
    assert_eq!(
        (
            count("PAY-12-find."),
            count("PAY-12-context."),
            count("PAY-12-packet.")
        ),
        (2, 1, 2),
        "{history:?}"
    );
    let find = read(&dir.join("PAY-12-find.json"));
    assert_eq!(find["options"]["provider"], "typesafe");
    assert_eq!(find["semantic"], false);
    // 같은 입력으로 다시: 요청을 보내지 않고 있던 결과를 낸다
    let calls = d.sb.curl_calls().len();
    let online = [
        ("FULLOPS_OFFLINE", "0"),
        ("TYPESAFE_API_KEY", "scenario-key"),
    ];
    let find = ["find", "PAY-12", "--role", "dev", "--json"];
    let run = d.sb.fullops(&d.sb.path("lead"), &find, &online, b"");
    let out: Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!((run.code, &out["reused"]), (Some(0), &json!(true)));
    assert_eq!(d.sb.curl_calls().len(), calls);
    // 읽히지 않는 옛 파일(1.24.0이 남긴 것)은 없는 것으로 보고 history로 옮긴 뒤 새로 만든다
    std::fs::write(
        dir.join("PAY-12-context.json"),
        r#"{"version": "context-v2", "task_key": "PAY-12"}"#,
    )
    .unwrap();
    let run = cli(
        &d,
        &[
            "context",
            "PAY-12",
            "--role",
            "dev",
            "--paths",
            "docs/pay.md",
        ],
    );
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert_eq!(read(&dir.join("PAY-12-context.json"))["v"], 1);
    assert_eq!(
        dir.join("history").read_dir().unwrap().count(),
        history.len() + 1
    );
}
