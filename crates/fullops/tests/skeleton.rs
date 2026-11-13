//! 뼈대의 통합 시험: 빌드한 바이너리를 실제 hook·CLI처럼 돌린다.

use fullops_testkit::{Sandbox, Setup};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn events(cwd: &std::path::Path) -> Vec<Value> {
    let base = |name: &str, extra: Value| {
        let mut event = json!({ "hook_event_name": name, "session_id": "sid-1", "cwd": cwd });
        event
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        event
    };
    vec![
        base("SessionStart", json!({ "source": "startup" })),
        base(
            "UserPromptSubmit",
            json!({ "prompt": "결제 재시도 버튼을 만들어 줘" }),
        ),
        base(
            "PreToolUse",
            json!({ "tool_name": "Bash", "tool_input": { "command": "orca orchestration worker-start --spec x" } }),
        ),
        base(
            "PreToolUse",
            json!({ "tool_name": "Write", "tool_input": { "file_path": ".fullops-squad/handovers/to_dev.md", "content": "x" } }),
        ),
        base(
            "PreToolUse",
            json!({ "tool_name": "모르는 도구", "tool_input": {} }),
        ),
        base("Stop", json!({})),
        base("StopFailure", json!({ "error": "API Error" })),
        base("모르는 이벤트", json!({})),
    ]
}

#[test]
fn hook_answers_empty_object_and_exit_zero_for_any_input() {
    let sb = Sandbox::with(&Setup::default());
    let plain = sb.init_repo("plain");
    // 활성 레포(주 체크아웃, 역할 워크트리), 비활성 레포, Git 레포가 아닌 폴더
    let places = [sb.path("lead"), sb.path("dev"), plain, sb.home.clone()];
    for host in ["claude", "codex"] {
        for cwd in &places {
            let mut inputs: Vec<Vec<u8>> = events(cwd)
                .iter()
                .map(|e| e.to_string().into_bytes())
                .collect();
            inputs.extend([
                Vec::new(),
                b"{ \"hook_event_name\": ".to_vec(),
                b"[1, 2, 3]".to_vec(),
                b"null".to_vec(),
                vec![0xff, 0xfe, 0x00, 0x80],
                json!({ "hook_event_name": "Stop" })
                    .to_string()
                    .into_bytes(),
                json!({ "hook_event_name": "Stop", "session_id": 7, "cwd": ["x"] })
                    .to_string()
                    .into_bytes(),
            ]);
            for input in inputs {
                let run = sb.fullops(cwd, &["hook", host], &[], &input);
                let shown = String::from_utf8_lossy(&input);
                assert_eq!(
                    (run.stdout.as_str(), run.code),
                    ("{}", Some(0)),
                    "{host} {} 입력 {shown}",
                    cwd.display()
                );
                assert_eq!(run.stderr, "", "hook은 stderr에 쓰지 않는다: {shown}");
            }
        }
    }
    // 호스트 인자가 없거나 틀려도, 설치용 인자가 붙어도 같다
    let stop = events(&sb.path("lead"))[5].to_string();
    for args in [
        vec!["hook"],
        vec!["hook", "grok"],
        vec!["hook", "claude", "--plugin-root", "x", "--in-place"],
        vec!["hook", "claude", "--plugin-root"],
    ] {
        let run = sb.fullops(&sb.path("lead"), &args, &[], stop.as_bytes());
        assert_eq!((run.stdout.as_str(), run.code), ("{}", Some(0)), "{args:?}");
    }
    // 깨진 설정에서도 뼈대의 hook은 죽지 않는다(정책 오류의 차단은 묶음 B가 규칙으로 넣는다)
    sb.write(
        &sb.path("lead").join(".fullops-squad/fullops.toml"),
        "schema = = 2",
    );
    let run = sb.fullops(&sb.path("lead"), &["hook", "claude"], &[], stop.as_bytes());
    assert_eq!((run.stdout.as_str(), run.code), ("{}", Some(0)));
}

/// 설계 7.5절의 표에서 규칙 ID마다 (이벤트, 강도)를 읽는다.
fn design_rules() -> BTreeMap<String, (String, String)> {
    let design =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/design.md"))
            .unwrap();
    let section = design
        .split("### 7.5 규칙 표")
        .nth(1)
        .unwrap()
        .split("### 7.6")
        .next()
        .unwrap();
    let mut event = String::new();
    let mut rules = BTreeMap::new();
    for line in section.lines() {
        if let Some(title) = line.strip_prefix("#### ") {
            event = title.split_whitespace().next().unwrap().to_string();
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let Some(id) = cells
            .get(1)
            .filter(|c| c.len() == 4 && c.as_bytes()[1] == b'-')
        else {
            continue;
        };
        // Stop 표는 강도 열이, 그 밖은 결과 열이 강도를 정한다
        let result = cells[4];
        let strength = if result.contains("BlockOnce") {
            "BlockOnce"
        } else if result.contains("BlockAlways") {
            "BlockAlways"
        } else if result.starts_with("Deny") {
            "Deny"
        } else if result.starts_with("`Context`") {
            "Context"
        } else if result.starts_with("`Warn`") {
            "Warn"
        } else {
            "Effect"
        };
        rules.insert(id.to_string(), (event.clone(), strength.to_string()));
    }
    rules
}

#[test]
fn rules_table_matches_design() {
    let sb = Sandbox::empty();
    let run = sb.cli(&sb.home, &["rules", "--json"]);
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let out: Value = serde_json::from_str(&run.stdout).unwrap();
    let printed: BTreeMap<String, (String, String)> = out["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let text = |name: &str| r[name].as_str().unwrap().to_string();
            (text("id"), (text("event"), text("strength")))
        })
        .collect();
    let design = design_rules();
    assert_eq!(design.len(), 32, "설계 표에서 읽은 규칙 수");
    assert_eq!(printed, design);
    // 거부·차단 규칙은 모두 탈출구를 가진다(설계 2.1절 G5)
    for rule in out["rules"].as_array().unwrap() {
        let blocking = matches!(
            rule["strength"].as_str(),
            Some("Deny" | "BlockOnce" | "BlockAlways")
        );
        assert!(
            !blocking || !rule["escape"].as_str().unwrap().is_empty(),
            "{rule}"
        );
    }

    let markdown = sb.cli(&sb.home, &["rules", "--markdown"]);
    assert_eq!(markdown.code, Some(0));
    assert_eq!(markdown.stdout.lines().count(), design.len() + 2);
    assert!(
        markdown
            .stdout
            .contains("| T-01 | PreToolUse | Deny | `fullops dispatch <KEY>` |")
    );
}

#[test]
fn cli_follows_common_contract() {
    let sb = Sandbox::with(&Setup::default());
    let (lead, plain) = (sb.path("lead"), sb.init_repo("plain"));

    // 활성 표식 없음: 종료 코드 2
    let run = sb.cli(&plain, &["plan", "list"]);
    assert_eq!(run.code, Some(2));
    assert!(run.stderr.contains("활성 레포가 아닙니다") && run.stdout.is_empty());
    let run = sb.cli(
        &sb.home,
        &["plan", "list", "--json", "--repo", &plain.to_string_lossy()],
    );
    assert_eq!(run.code, Some(2));
    let out: Value = serde_json::from_str(&run.stdout).expect("--json이면 stdout이 JSON이다");
    assert_eq!(out["ok"], false);
    // --repo로 다른 폴더의 레포를 가리킨다. 아직 구현되지 않은 명령은 종료 코드 1이다
    let run = sb.cli(
        &sb.home,
        &["plan", "list", &format!("--repo={}", lead.display())],
    );
    assert_eq!(run.code, Some(1), "{}", run.stderr);
    assert!(run.stderr.contains("구현되지 않았습니다"));
    // 활성 레포가 아니어도 도는 명령
    assert_eq!(sb.cli(&plain, &["doctor"]).code, Some(1));
    assert_eq!(sb.cli(&plain, &["rules"]).code, Some(0));

    // 사용법 오류: 종료 코드 1
    assert_eq!(sb.cli(&lead, &["없는명령"]).code, Some(1));
    assert_eq!(sb.cli(&lead, &[]).code, Some(1));
    assert_eq!(sb.cli(&lead, &["rules", "--nope"]).code, Some(1));
    assert_eq!(sb.cli(&lead, &["plan", "--repo"]).code, Some(1));
    let version = sb.cli(&lead, &["--version"]);
    assert_eq!(
        (version.code, version.stdout.trim()),
        (Some(0), concat!("fullops ", env!("CARGO_PKG_VERSION")))
    );
    assert!(sb.cli(&lead, &["help"]).stdout.contains("dispatch"));

    // 정책 오류: 복구 명령만 받는다(설계 7.8절)
    let wrong_schema = Setup::default()
        .config_toml()
        .replace("schema = 2", "schema = 1");
    for broken in ["schema = = 2", wrong_schema.as_str()] {
        sb.write(&lead.join(".fullops-squad/fullops.toml"), broken);
        let run = sb.cli(&lead, &["dispatch", "PAY-12"]);
        assert_eq!(run.code, Some(2), "{}", run.stderr);
        assert!(run.stderr.contains("정책 오류") && run.stderr.contains("fullops doctor"));
        let run = sb.cli(&lead, &["dispatch", "PAY-12", "--json"]);
        assert!(
            serde_json::from_str::<Value>(&run.stdout).unwrap()["error"]
                .as_str()
                .unwrap()
                .contains("정책 오류")
        );
        for recovery in [
            "doctor", "mode", "init", "migrate", "upgrade", "status", "why",
        ] {
            let run = sb.cli(&lead, &[recovery]);
            assert!(
                run.code == Some(1) && run.stderr.contains("구현되지 않았습니다"),
                "{recovery}: {}",
                run.stderr
            );
        }
    }
}
