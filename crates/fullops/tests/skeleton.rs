//! 뼈대의 통합 시험: 빌드한 바이너리를 실제 hook·CLI처럼 돌린다.

use fullops_testkit::host_shapes::{HOSTS, pass_output};
use fullops_testkit::{Sandbox, Setup};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn fixture_dir(host: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scenarios/fixtures/host")
        .join(host)
}

/// 그 호스트의 입력 표본 전부: `(파일 이름, JSON)`.
fn fixtures(host: &str) -> Vec<(String, Value)> {
    let mut list: Vec<(String, Value)> = std::fs::read_dir(fixture_dir(host))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).unwrap();
            (name, serde_json::from_str(&text).unwrap())
        })
        .collect();
    list.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(list.len() >= 10, "{host} 표본이 모자란다");
    list
}

/// 표본 파일 이름에서 fullops의 이벤트 이름을 읽는다. agy의 등록 문자열이 `--event`로 주는 값이다.
fn event_of(file: &str) -> Option<&'static str> {
    Some(match file {
        f if f.starts_with("session_start") => "SessionStart",
        f if f.starts_with("user_prompt_submit") => "UserPromptSubmit",
        f if f.starts_with("pre_tool_use") => "PreToolUse",
        f if f.starts_with("stop_failure") => "StopFailure",
        f if f.starts_with("stop") => "Stop",
        _ => return None,
    })
}

/// 표본의 자리 채움 경로를 시험 폴더로 바꾼다.
fn at(mut input: Value, cwd: &Path) -> Value {
    if input.get("cwd").is_some() {
        input["cwd"] = json!(cwd);
    }
    if input.get("workspacePaths").is_some() {
        input["workspacePaths"] = json!([cwd]);
    }
    input
}

#[test]
fn hook_passes_every_input_on_every_host() {
    let sb = Sandbox::with(&Setup::default());
    let plain = sb.init_repo("plain");
    // 활성 레포(주 체크아웃, 역할 워크트리), 비활성 레포, Git 레포가 아닌 폴더
    let places = [sb.path("lead"), sb.path("dev"), plain, sb.home.clone()];
    let junk: Vec<Vec<u8>> = vec![
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
        json!({ "hook_event_name": "모르는 이벤트", "session_id": "s", "cwd": sb.path("lead") })
            .to_string()
            .into_bytes(),
        json!({ "conversationId": "", "workspacePaths": [] })
            .to_string()
            .into_bytes(),
    ];
    let mut checked = 0;
    for host in HOSTS {
        // 그 호스트의 표본 전부. agy는 등록 문자열이 주는 --event와 함께다
        for (name, sample) in fixtures(host) {
            let event = event_of(&name);
            let args: Vec<&str> = match (host, event) {
                ("agy", Some(event)) => vec!["hook", host, "--event", event],
                _ => vec!["hook", host],
            };
            let expected = match host {
                "agy" => pass_output(host, event.unwrap_or("")),
                _ => "{}",
            };
            for cwd in &places {
                let input = at(sample.clone(), cwd).to_string();
                // 프로세스의 작업 디렉터리는 쓰지 않는다: 늘 홈에서 띄운다
                let run = sb.fullops(&sb.home, &args, &[], input.as_bytes());
                assert_eq!(
                    (run.stdout.as_str(), run.code),
                    (expected, Some(0)),
                    "{host} {name} {}",
                    cwd.display()
                );
                assert_eq!(run.stderr, "", "hook은 stderr에 쓰지 않는다: {host} {name}");
                checked += 1;
            }
        }
        // 깨진 입력과 다른 호스트의 표본에도 통과의 글은 인자만으로 정해진다
        let mut odd = junk.clone();
        for other in HOSTS.iter().filter(|other| **other != host) {
            let (_, sample) = &fixtures(other)[0];
            odd.push(
                at(sample.clone(), &sb.path("lead"))
                    .to_string()
                    .into_bytes(),
            );
        }
        for input in &odd {
            let events: &[Option<&str>] = if host == "agy" {
                &[
                    Some("SessionStart"),
                    Some("PreToolUse"),
                    Some("Stop"),
                    Some("PreInvocation"),
                    None,
                ]
            } else {
                &[None]
            };
            for event in events {
                let mut args = vec!["hook", host];
                args.extend(event.iter().flat_map(|e| ["--event", e]));
                let run = sb.fullops(&sb.path("lead"), &args, &[], input);
                let shown = String::from_utf8_lossy(input);
                assert_eq!(
                    (run.stdout.as_str(), run.code, run.stderr.as_str()),
                    (pass_output(host, event.unwrap_or("")), Some(0), ""),
                    "{host} {event:?} 입력 {shown}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 300, "돌린 입력 수: {checked}");

    let stop = at(
        fixtures("claude")
            .into_iter()
            .find(|(n, _)| n == "stop.json")
            .unwrap()
            .1,
        &sb.path("lead"),
    )
    .to_string();
    // 호스트 인자가 없거나 넷이 아니면 `{}`다. 설치용 인자가 붙거나 값이 빠져도 죽지 않는다
    for (args, expected) in [
        (vec!["hook"], "{}"),
        (vec!["hook", "cursor"], "{}"),
        (vec!["hook", "cursor", "--event", "PreToolUse"], "{}"),
        (
            vec!["hook", "claude", "--plugin-root", "x", "--in-place"],
            "{}",
        ),
        (vec!["hook", "claude", "--plugin-root"], "{}"),
        (vec!["hook", "grok", "--event", "PreToolUse"], "{}"),
        (vec!["hook", "agy", "--event"], "{}"),
        (
            vec!["hook", "agy", "--in-place", "--event", "Stop"],
            r#"{"decision":""}"#,
        ),
        (
            vec![
                "hook",
                "agy",
                "--event",
                "PreToolUse",
                "--plugin-root",
                "${PLUGIN_ROOT}",
            ],
            r#"{"decision":"ask"}"#,
        ),
    ] {
        let run = sb.fullops(&sb.path("lead"), &args, &[], stop.as_bytes());
        assert_eq!(
            (run.stdout.as_str(), run.code),
            (expected, Some(0)),
            "{args:?}"
        );
    }
    // grok이 다른 호스트의 등록 파일을 읽어 실행한 경우: 그 등록은 `{}`만 낸다. agy의 통과 글은 그대로다
    let grok = [("GROK_HOOK_EVENT", "pre_tool_use")];
    for (args, expected) in [
        (vec!["hook", "claude"], "{}"),
        (vec!["hook", "codex"], "{}"),
        (vec!["hook", "grok"], "{}"),
        (
            vec!["hook", "agy", "--event", "PreToolUse"],
            r#"{"decision":"ask"}"#,
        ),
    ] {
        let run = sb.fullops(&sb.path("lead"), &args, &grok, stop.as_bytes());
        assert_eq!(
            (run.stdout.as_str(), run.code),
            (expected, Some(0)),
            "{args:?}"
        );
    }
    // 인자 없이 실행: stdout은 비고 종료 코드 0이다(실행 파일만 부르는 호스트에서도 통과가 된다)
    let bare = sb.fullops(&sb.path("lead"), &[], &[], stop.as_bytes());
    assert_eq!((bare.stdout.as_str(), bare.code), ("", Some(0)));
    assert!(bare.stderr.contains("사용법") && bare.stderr.contains("hook <claude|codex|grok|agy>"));
    // 깨진 설정에서도 뼈대의 hook은 죽지 않는다(정책 오류의 차단은 묶음 B가 규칙으로 넣는다)
    sb.write(
        &sb.path("lead").join(".fullops-squad/fullops.toml"),
        "schema = = 2",
    );
    for host in HOSTS {
        let run = sb.fullops(&sb.path("lead"), &["hook", host], &[], stop.as_bytes());
        assert_eq!((run.stdout.as_str(), run.code), ("{}", Some(0)), "{host}");
    }
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
        // Stop 표는 강도 열이, 그 밖은 결과 열이 강도를 정한다. 두 값이 적힌 X-01은 BlockOnce다
        let result = cells[4];
        let strength = if result.contains("BlockOnce") {
            "BlockOnce"
        } else if result.contains("BlockAlways") {
            "BlockAlways"
        } else if result.starts_with("Deny") {
            "Deny"
        } else if result.starts_with("`Context`") {
            "Context"
        } else {
            assert!(
                result.starts_with("효과"),
                "{id}: 강도를 읽지 못했다: {result}"
            );
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
    assert_eq!(out["ok"], true);
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
    assert_eq!(design.len(), 34, "설계 표에서 읽은 규칙 수");
    assert_eq!(printed, design);
    assert_eq!(
        out["rules"].as_array().unwrap().len(),
        34,
        "ID가 겹치지 않는다"
    );
    for rule in out["rules"].as_array().unwrap() {
        // 거부·차단 규칙은 모두 탈출구를 가지고(설계 2.1절 G5) 안내·효과 규칙은 가지지 않는다
        let blocking = matches!(
            rule["strength"].as_str(),
            Some("Deny" | "BlockOnce" | "BlockAlways")
        );
        let escapes = rule["escapes"].as_array().unwrap();
        assert_eq!(!escapes.is_empty(), blocking, "{rule}");
        assert!(
            escapes
                .iter()
                .all(|e| !e["escape"].as_str().unwrap().is_empty())
        );
        assert!(!rule["origin"].as_str().unwrap().is_empty());
        // 경우가 여럿인 규칙은 설계가 든 셋이다
        let cases: Vec<&str> = escapes
            .iter()
            .map(|e| e["case"].as_str().unwrap())
            .collect();
        let expected: &[&str] = match rule["id"].as_str().unwrap() {
            "T-05" => &["a", "b", "c", "d"],
            "T-08" => &["e", "f", "g"],
            "X-05" => &["a", "b"],
            _ if blocking => &[""],
            _ => &[],
        };
        assert_eq!(cases, expected, "{rule}");
    }
    let escape_of = |id: &str| {
        let rule = out["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap();
        rule["escapes"].to_string()
    };
    assert!(
        escape_of("T-08").contains("`parallel`") && !escape_of("T-08").contains("subagent_level")
    );
    assert!(escape_of("X-05").contains("--ack <메시지 ID> --no-wait"));
    assert!(escape_of("T-03").contains("orca orchestration send --to dispatch:<ID>"));

    let markdown = sb.cli(&sb.home, &["rules", "--markdown"]);
    assert_eq!(markdown.code, Some(0));
    assert_eq!(markdown.stdout.lines().count(), design.len() + 2);
    assert!(
        markdown
            .stdout
            .contains("| T-01 | PreToolUse | Deny | `fullops dispatch <KEY>` |")
    );
    assert!(
        markdown
            .stdout
            .contains("| S-06 | SessionStart | Context | - |")
    );
    assert!(
        markdown
            .stdout
            .contains("| T-13 | PreToolUse | Effect | - |")
    );
}

#[test]
fn cli_follows_common_contract() {
    let sb = Sandbox::with(&Setup::default());
    let (lead, plain) = (sb.path("lead"), sb.init_repo("plain"));
    let json_of = |run: &fullops_testkit::Run| -> Value {
        serde_json::from_str(&run.stdout)
            .unwrap_or_else(|e| panic!("--json이면 stdout이 JSON 객체 하나다: {e}: {}", run.stdout))
    };

    // 활성 표식 없음: 종료 코드 2. 문구는 stderr에, 결과 자리인 stdout은 비운다
    let run = sb.cli(&plain, &["plan", "list"]);
    assert_eq!(run.code, Some(2));
    assert!(run.stderr.contains("활성 레포가 아닙니다") && run.stdout.is_empty());
    let run = sb.cli(
        &sb.home,
        &["plan", "list", "--json", "--repo", &plain.to_string_lossy()],
    );
    assert_eq!(run.code, Some(2));
    let out = json_of(&run);
    assert_eq!(
        (&out["ok"], &out["error"]["kind"]),
        (&json!(false), &json!("inactive"))
    );
    assert!(
        out["error"]["message"]
            .as_str()
            .unwrap()
            .contains("활성 레포가 아닙니다")
    );
    // --repo로 다른 폴더의 레포를 가리킨다. 아직 구현되지 않은 명령은 종료 코드 1이다
    let run = sb.cli(
        &sb.home,
        &["plan", "list", &format!("--repo={}", lead.display())],
    );
    assert_eq!(run.code, Some(1), "{}", run.stderr);
    assert!(run.stderr.contains("아직 구현되지 않았습니다"));
    // 명령 표의 모든 명령이 등록돼 있다(설계 8.2절). --json은 명령 이름 뒤 어디에나 올 수 있다
    for command in [
        "route",
        "find",
        "context",
        "packet",
        "task",
        "plan",
        "dispatch",
        "wait",
        "done",
        "merge",
        "hold",
        "resume",
        "settle",
        "completion",
        "sync",
        "review",
        "decision",
        "lint",
        "doc",
        "env",
        "gc",
        "status",
        "why",
        "mode",
        "upgrade",
        "init",
        "migrate",
        "install",
        "doctor",
    ] {
        let run = sb.cli(&lead, &[command, "x", "--json", "y"]);
        let out = json_of(&run);
        assert_eq!(run.code, Some(1), "{command}: {}", run.stderr);
        assert_eq!(out["error"]["kind"], "internal", "{command}");
        assert!(
            out["error"]["message"]
                .as_str()
                .unwrap()
                .contains("아직 구현되지 않았습니다")
        );
        assert!(
            sb.cli(&lead, &["help"])
                .stdout
                .contains(&format!("  {command} ")),
            "{command}"
        );
    }
    // 활성 표식 없이 도는 명령: init·install·doctor·migrate·rules
    for anywhere in ["init", "install", "doctor", "migrate"] {
        assert_eq!(sb.cli(&plain, &[anywhere]).code, Some(1), "{anywhere}");
    }
    assert_eq!(sb.cli(&plain, &["rules"]).code, Some(0));
    assert_eq!(sb.cli(&plain, &["status"]).code, Some(2));

    // 사용법 오류: 종료 코드 1. 인자 없는 실행만 0이다
    assert_eq!(sb.cli(&lead, &["없는명령"]).code, Some(1));
    let run = sb.cli(&lead, &["없는명령", "--json"]);
    assert_eq!(
        (run.code, &json_of(&run)["error"]["kind"]),
        (Some(1), &json!("usage"))
    );
    assert_eq!(sb.cli(&lead, &[]).code, Some(0));
    assert_eq!(sb.cli(&lead, &["rules", "--nope"]).code, Some(1));
    assert_eq!(sb.cli(&lead, &["plan", "--repo"]).code, Some(1));
    let version = sb.cli(&lead, &["--version"]);
    assert_eq!(
        (version.code, version.stdout.trim()),
        (Some(0), concat!("fullops ", env!("CARGO_PKG_VERSION")))
    );
    assert!(sb.cli(&lead, &["help"]).stdout.contains("dispatch"));

    // 정책 오류: 종료 코드 2이고 복구 명령만 받는다(설계 7.8절. T-00의 통과 목록과 같다)
    let wrong_schema = Setup::default()
        .config_toml()
        .replace("schema = 2", "schema = 1");
    for broken in ["schema = = 2", wrong_schema.as_str()] {
        sb.write(&lead.join(".fullops-squad/fullops.toml"), broken);
        let run = sb.cli(&lead, &["dispatch", "PAY-12"]);
        assert_eq!(run.code, Some(2), "{}", run.stderr);
        assert!(run.stderr.contains("정책 오류") && run.stderr.contains("fullops doctor"));
        let run = sb.cli(&lead, &["dispatch", "PAY-12", "--json"]);
        let out = json_of(&run);
        assert_eq!(out["error"]["kind"], "policy");
        assert!(
            out["error"]["message"]
                .as_str()
                .unwrap()
                .contains("정책 오류")
        );
        for recovery in [
            "doctor", "mode", "init", "migrate", "upgrade", "status", "why", "install",
        ] {
            let run = sb.cli(&lead, &[recovery]);
            assert!(
                run.code == Some(1) && run.stderr.contains("아직 구현되지 않았습니다"),
                "{recovery}: {}",
                run.stderr
            );
        }
        assert_eq!(sb.cli(&lead, &["rules"]).code, Some(0));
    }
}
