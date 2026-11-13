//! 묶음 D의 통합 시험: `fullops lint`와 `fullops doc`을 실제 프로세스로 돌린다.
//! 시나리오 파일(`scenarios/g3.toml`, `scenarios/lint-*.toml`)은 판정과 사람용 출력을, 이 파일의 나머지는 결과 파일과 문서 내용을 본다.

use fullops_core::model::sha256_hex;
use fullops_testkit::{Run, Sandbox, Setup, run_scenario, scenario};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const LINT_TEMPLATE: &str = include_str!("../assets/repo/lint/lint.json");
const INDEX_TEMPLATE: &str = include_str!("../assets/repo/docs/deliverables/README.md");

fn scenarios_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios")
}

fn run_file(name: &str) {
    let scenario = scenario::load(&scenarios_dir().join(format!("{name}.toml")))
        .unwrap_or_else(|e| panic!("{e}"));
    if let Err(e) = run_scenario(&scenario) {
        panic!("{e}");
    }
}

macro_rules! scenario_tests {
    ($($name:ident => $file:literal),+ $(,)?) => {
        $(#[test] fn $name() { run_file($file); })+
        const SCENARIOS: &[&str] = &[$($file),+];
    };
}

scenario_tests! {
    g3_branch_cannot_loosen_lint_config => "g3",
    lint_size_001 => "lint-size-001",
    lint_size_002 => "lint-size-002",
    lint_dep_001 => "lint-dep-001",
    lint_anti_002 => "lint-anti-002",
    lint_anti_003 => "lint-anti-003",
    lint_anti_004 => "lint-anti-004",
    lint_anti_005 => "lint-anti-005",
    lint_sec_001 => "lint-sec-001",
    lint_doc_001 => "lint-doc-001",
    lint_doc_002 => "lint-doc-002",
    lint_doc_003 => "lint-doc-003",
    lint_lint_000 => "lint-lint-000",
    lint_lint_001 => "lint-lint-001",
    lint_lint_002 => "lint-lint-002",
    lint_regex_rules => "lint-rules",
    lint_command_selection => "lint-commands",
}

/// LINT-002의 검출 사례. 명령이 실제로 120초를 넘겨야 해서 평소에는 돌리지 않는다.
#[test]
#[ignore = "2분 넘게 걸린다. cargo test -p fullops --test lint_doc -- --ignored"]
fn lint_lint_002_slow() {
    run_file("lint-lint-002-slow");
}

#[test]
fn every_builtin_code_has_a_scenario() {
    for code in [
        "SIZE-001", "SIZE-002", "DEP-001", "ANTI-002", "ANTI-003", "ANTI-004", "ANTI-005",
        "SEC-001", "DOC-001", "DOC-002", "DOC-003", "LINT-000", "LINT-001", "LINT-002",
    ] {
        let file = format!("lint-{}", code.to_lowercase());
        assert!(
            SCENARIOS.contains(&file.as_str()),
            "{code}의 시나리오가 없다"
        );
    }
    // 폴더에 있는 이 묶음의 시나리오가 빠짐없이 돈다
    for entry in std::fs::read_dir(scenarios_dir()).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".toml") else {
            continue;
        };
        if stem.starts_with("lint-") && stem != "lint-lint-002-slow" {
            assert!(SCENARIOS.contains(&stem), "{name}이 시험 목록에 없다");
        }
    }
}

// ---- lint: 결과 파일 ----

fn commit(sb: &Sandbox, dir: &Path, message: &str) -> String {
    sb.git_ok(dir, &["add", "-A"]);
    sb.git_ok(dir, &["commit", "-q", "-m", message]);
    sb.git_ok(dir, &["rev-parse", "HEAD"])
}

fn lint_dir(sb: &Sandbox, dir: &Path) -> PathBuf {
    PathBuf::from(sb.git_ok(dir, &["rev-parse", "--absolute-git-dir"])).join("fullops/lint")
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn stdout_json(run: &Run) -> Value {
    serde_json::from_str(&run.stdout).unwrap_or_else(|e| panic!("{e}: {run:?}"))
}

#[test]
fn lint_writes_result_and_pass_from_the_default_start() {
    let sb = Sandbox::with(&Setup::default());
    let dev = sb.path("dev");
    let dir = lint_dir(&sb, &dev);
    let floor = sb.git_ok(&dev, &["rev-parse", "refs/remotes/origin/main"]);
    sb.write(&dev.join("src/a.py"), "\"\"\"결제.\"\"\"\nvalue = 1\n");
    let head = commit(&sb, &dev, "구현");

    // 인박스도 세션도 없으면 통합 기준 ref와의 merge-base가 시작점이다. 기준 시점에 lint.json이 없어 내장 템플릿을 쓴다
    let out = sb.root.join("out/lint.json");
    let run = sb.cli(&dev, &["lint", "--out", &out.to_string_lossy()]);
    assert_eq!(run.code, Some(0), "{run:?}");
    assert!(run.stdout.contains("통과: head") && run.stdout.contains("WARNING LINT-000"));
    let result = read_json(&dir.join("result.json"));
    assert_eq!(result, read_json(&out));
    let template_sha = sha256_hex(LINT_TEMPLATE.as_bytes());
    for (field, want) in [
        ("v", json!(1)),
        ("base", json!(floor)),
        ("merge_base", json!(floor)),
        ("head", json!(head)),
        ("config_sha256", json!(template_sha)),
        ("test_level", json!("lite")),
        ("commands", json!([])),
        ("errors", json!(0)),
        ("warnings", json!(1)),
        ("unrunnable", json!(0)),
        ("passed", json!(true)),
    ] {
        assert_eq!(result[field], want, "{field}");
    }
    assert_eq!(result["findings"][0]["code"], "LINT-000");
    assert_eq!(result["findings"][0]["severity"], "WARNING");
    assert!(
        result["started_at"]
            .as_i64()
            .is_some_and(|at| at > 1_700_000_000)
    );
    let pass = read_json(&dir.join("pass.json"));
    let tree = sb.git_ok(&dev, &["rev-parse", "HEAD^{tree}"]);
    assert_eq!(
        (&pass["v"], &pass["base"], &pass["head"], &pass["tree"]),
        (&json!(1), &json!(floor), &json!(head), &json!(tree))
    );
    assert_eq!(
        (&pass["config_sha256"], &pass["test_level"]),
        (&json!(template_sha), &json!("lite"))
    );
    assert!(pass["at"].as_i64().is_some());

    // ERROR가 있으면 미통과다: 종료 코드 2, pass.json이 없다. --json은 결과를 그대로 낸다
    sb.write(
        &dev.join("src/a.py"),
        "\"\"\"결제.\"\"\"\nvalue = eval(\"1\")\n",
    );
    let head = commit(&sb, &dev, "금지 표현");
    let run = sb.cli(&dev, &["lint", "--json"]);
    assert_eq!(run.code, Some(2), "{run:?}");
    let printed = stdout_json(&run);
    assert_eq!(
        (&printed["ok"], &printed["passed"]),
        (&json!(false), &json!(false))
    );
    assert_eq!(printed["head"], json!(head));
    assert_eq!(printed["findings"][0]["code"], "ANTI-002");
    assert_eq!(printed["findings"][0]["line"], 2);
    assert!(!dir.join("pass.json").exists());
    assert_eq!(read_json(&dir.join("result.json"))["errors"], 1);

    // 작업 트리가 깨끗하지 않으면 거절한다
    sb.write(&dev.join("src/b.py"), "\"\"\"미커밋.\"\"\"\n");
    let run = sb.cli(&dev, &["lint"]);
    assert_eq!(run.code, Some(2), "{run:?}");
    assert!(run.stderr.contains("FullOps[LINT]") && run.stderr.contains("src/b.py"));
    std::fs::remove_file(dev.join("src/b.py")).unwrap();

    // 인박스가 있으면 그 base가 시작점이다: 앞 커밋의 금지 표현은 검사 범위 밖이 된다
    sb.write(
        &dev.join(".fullops-squad/handovers/to_dev.md"),
        &format!(
            "---\nid: handover-dev\ntitle: PAY-12 — 결제\nstatus: draft\nupdated: 2026-11-14\nowner: dev\nsummary: 결제\n\
             key: PAY-12\nrole: dev\ngoal: 결제\nbase: {head}\nroute: direct\nfrom: lead\nround: 0\nbudget: 2\n\
             test_level: lite\nparallel: on\n---\n# PAY-12 — 결제\n"
        ),
    );
    commit(&sb, &dev, "지시서");
    let run = sb.cli(&dev, &["lint"]);
    assert_eq!(run.code, Some(0), "{run:?}");
    assert_eq!(read_json(&dir.join("result.json"))["base"], json!(head));
    assert!(dir.join("pass.json").exists());

    // 잘못된 인자와 풀리지 않는 기준은 실행 실패다
    for args in [
        &["lint", "--level", "bogus"][..],
        &["lint", "--from", "no-such-ref"],
        &["lint", "--bogus"],
    ] {
        assert_eq!(sb.cli(&dev, args).code, Some(1), "{args:?}");
    }
}

#[test]
fn lint_keeps_logs_and_reports_setup_problems() {
    let sb = Sandbox::with(&Setup {
        remote: false,
        roles: Default::default(),
        ..Setup::default()
    });
    let lead = sb.path("lead");
    let dir = lint_dir(&sb, &lead);
    let config = r#"{"schema_version": 1, "commands": [{"name": "ver sion/1", "run": ["git", "--version"]}]}"#;
    sb.write(&lead.join(".fullops-squad/lint/lint.json"), config);
    // 기준 시점의 fullops.toml이 읽히지 않는다
    let toml = std::fs::read_to_string(lead.join(".fullops-squad/fullops.toml")).unwrap();
    sb.write(&lead.join(".fullops-squad/fullops.toml"), "schema = \n");
    let base = commit(&sb, &lead, "기준");
    sb.write(&lead.join(".fullops-squad/fullops.toml"), &toml);
    sb.write(&lead.join("notes/a.txt"), "메모\n");
    commit(&sb, &lead, "변경");

    // 출력 전체는 logs/<이름>.log에, 끝 줄은 결과에 남는다. 기준 시점 설정을 못 읽으면 기본값으로 돌고 한 줄 알린다
    let run = sb.cli(&lead, &["lint", "--from", &base]);
    assert_eq!(run.code, Some(0), "{run:?}");
    assert!(
        run.stderr
            .contains("기준 시점 설정을 읽지 못해 기본값을 썼다"),
        "{run:?}"
    );
    let result = read_json(&dir.join("result.json"));
    let command = &result["commands"][0];
    assert_eq!(
        (
            &command["status"],
            &command["exit_code"],
            &command["kind"],
            &command["cwd"]
        ),
        (&json!("passed"), &json!(0), &json!("lint"), &json!("."))
    );
    assert!(
        command["output_tail"]
            .as_str()
            .unwrap()
            .starts_with("git version")
    );
    let log = PathBuf::from(command["log"].as_str().unwrap());
    assert_eq!(log, dir.join("logs/ver_sion_1.log"));
    assert!(
        std::fs::read_to_string(&log)
            .unwrap()
            .starts_with("git version")
    );
    assert_eq!(
        result["config_sha256"],
        json!(sha256_hex(config.as_bytes()))
    );
    assert_eq!(result["test_level"], "lite");

    // 다른 lint가 잠금을 쥐고 있으면 실행 실패다. 이전 결과는 그대로다
    let lock = std::fs::File::create(dir.join("lock")).unwrap();
    lock.lock().unwrap();
    let run = sb.cli(&lead, &["lint", "--from", &base]);
    assert_eq!(run.code, Some(1), "{run:?}");
    assert!(run.stderr.contains("lint가 실행 중"));
    assert!(dir.join("pass.json").exists());
    drop(lock);

    // 기준 시점의 lint.json을 쓸 수 없으면 실행 실패다
    for broken in [
        r#"{"schema_version": 2}"#,
        r#"{"schema_version": 1, "commands": [{"name": "out", "run": ["git"], "cwd": "../.."}]}"#,
        "깨진 글",
    ] {
        sb.write(&lead.join(".fullops-squad/lint/lint.json"), broken);
        let base = commit(&sb, &lead, "깨진 설정");
        sb.write(&lead.join("notes/a.txt"), &format!("{broken}\n"));
        commit(&sb, &lead, "변경");
        let run = sb.cli(&lead, &["lint", "--from", &base]);
        assert_eq!(run.code, Some(1), "{broken}: {run:?}");
        assert!(run.stderr.contains("lint.json을 쓸 수 없습니다"), "{run:?}");
        assert!(!dir.join("pass.json").exists() && !dir.join("result.json").exists());
    }
}

// ---- doc ----

fn squad(sb: &Sandbox) -> PathBuf {
    sb.path("lead").join(".fullops-squad")
}

/// 내장 템플릿의 인덱스에서 그 ID 행의 상태 칸을 바꾼다.
fn index_with(statuses: &[(&str, &str)]) -> String {
    INDEX_TEMPLATE
        .lines()
        .map(|line| {
            match statuses
                .iter()
                .find(|(id, _)| line.starts_with(&format!("| {id} |")))
            {
                Some((_, status)) => line.replace("| 미작성 |", &format!("| {status} |")),
                None => line.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn doc(fields: &str) -> String {
    format!("---\n{fields}---\n\n# 본문\n")
}

/// 1.24.0이 쓴 꼴의 문서들(JSON 따옴표 값, 목록, `statuses` 객체, 주석, BOM과 CRLF)에서 판정이 1.24.0과 같다.
/// 기대값은 py-baseline(v1.24.0)의 `deliverables.py --repo <레포> [--strict]` 출력과 견줘 확인한 것이다.
#[test]
fn doc_check_judges_like_1_24_0() {
    let sb = Sandbox::with(&Setup {
        remote: false,
        roles: Default::default(),
        ..Setup::default()
    });
    let lead = sb.path("lead");
    let base = squad(&sb);
    let put = |path: &str, text: &str| sb.write(&base.join(path), text);
    put(
        "docs/deliverables/README.md",
        &index_with(&[
            ("D01", "draft"),
            ("D02", "review"),
            ("D03", "approved"),
            ("D04", "범위 밖"),
            ("D05", "draft"),
            ("D06", "review"),
            ("D07", "draft"),
            ("D08", "draft"),
            ("D09", "approved"),
            ("D10", "review"),
            ("D11", "draft"),
            ("D12", "approved"),
        ]),
    );
    // D01: BOM과 CRLF가 있어도 형식이 같다
    put(
        "docs/planning/business-plan.md",
        &format!(
            "\u{feff}{}",
            doc("id: D01\ntitle: 사업계획서\nstatus: draft\nupdated: 2026-09-24\nowner: arch\ntasks: [JUMP-3]\nupstream: [D02]\ndownstream: [D05, D10]\nsummary: 한 줄 요약\n")
                .replace('\n', "\r\n")
        ),
    );
    // D02(폴더 원천): JSON으로 감싼 값은 통과, 상태가 다른 문서는 걸린다. Markdown이 아닌 파일은 보지 않는다
    put(
        "docs/planning/product-specs/a/login.md",
        &doc(
            "id: D02\ntitle: \"로그인: 소셜\"\nstatus: review\nupdated: 2026-09-24\nowner: plan\ntasks: [\"A-1, 둘\", \"B-2\"]\nsummary: \"요약, 쉼표\"\n",
        ),
    );
    put(
        "docs/planning/product-specs/b.md",
        &doc(
            "id: D02\ntitle: 결제\nstatus: draft\nupdated: 2026-09-24\nowner: plan\nsummary: 요약\n",
        ),
    );
    put("docs/planning/product-specs/c.txt", "메모\n");
    // D03: 필수 필드가 없고 둘째 원천이 없다
    put(
        "docs/design-docs/architecture.md",
        &doc(
            "id: D03\ntitle: 아키텍처설계서\nstatus: approved\nupdated: 2026-09-24\nsummary: 요약\n",
        ),
    );
    // D05: 주석이 있어 정해진 형식과 다르다
    put(
        "docs/design-docs/interface-design.md",
        &doc(
            "id: D05\ntitle: 인터페이스설계서\nstatus: draft  # 검토 전\nupdated: 2026-09-24\nowner: arch\nsummary: 요약\n",
        ),
    );
    // D06·D07·D09가 함께 가진 원천: 상태는 statuses[ID]다
    put(
        "docs/design-docs/data-model.md",
        &doc(
            "id: [D06, D07, D09]\ntitle: 데이터 모델\nstatus: draft\nstatuses: {\"D06\": \"review\", \"D07\": \"draft\", \"D09\": \"draft\"}\nupdated: 2026-09-24\nowner: arch\nsummary: 엔티티와 DB와 CRUD\n",
        ),
    );
    // D08: front matter가 없다
    put("docs/generated/db-schema.md", "# 테이블\n");
    // D10: 다른 ID, 날짜 형식, 목록이 아닌 tasks
    put(
        "docs/design-docs/module-design.md",
        &doc(
            "id: D11\ntitle: 프로그램설계서\nstatus: review\nupdated: 2026/09/24\nowner: dev\ntasks: A-1\nsummary: 요약\n",
        ),
    );
    // D11: JSON 문자열이 깨졌다
    put(
        "docs/operations/user-guide.md",
        &doc(
            "id: D11\ntitle: \"a\\qb\"\nstatus: draft\nupdated: 2026-09-24\nowner: ops\nsummary: 요약\n",
        ),
    );
    // D12: 갖지 않은 ID의 statuses
    put(
        "docs/operations/ops-guide.md",
        &doc(
            "id: D12\ntitle: 운영자설명서\nstatus: approved\nstatuses: {\"D13\": \"draft\"}\nupdated: 2026-09-24\nowner: ops\nsummary: 요약\n",
        ),
    );
    // 조립 문서의 링크
    put(
        "docs/deliverables/D01_사업계획서.md",
        "[원천](../planning/business-plan.md#개요) [없음](../planning/none.md) [웹](https://example.invalid/x) [앵커](#절)\n",
    );

    let warnings = [
        "D02 docs/planning/product-specs/b.md: 인덱스 상태(review)와 문서 상태(draft)가 다름",
        "D03 docs/design-docs/architecture.md: 필수 필드 없음: owner",
        "D05 docs/design-docs/interface-design.md: 정해진 형식과 다름(필드 순서·목록 표기·주석)",
        "D08 docs/generated/db-schema.md: front matter 없음",
        "D09 docs/design-docs/data-model.md: 인덱스 상태(approved)와 문서 상태(draft)가 다름",
        "D10 docs/design-docs/module-design.md: id가 D10가 아님: D11",
        "D10 docs/design-docs/module-design.md: updated는 YYYY-MM-DD: 2026/09/24",
        "D10 docs/design-docs/module-design.md: tasks는 [a, b] 목록",
        "D11 docs/operations/user-guide.md: front matter 없음",
        "D12 docs/operations/ops-guide.md: statuses는 소유한 산출물 ID와 draft/review/approved 상태의 객체여야 함",
    ];
    let errors = [
        "D01_사업계획서.md 링크 없음: ../planning/none.md",
        "D03 원천 없음: docs/design-docs/tech-stack.md",
    ];

    let run = sb.cli(&lead, &["doc", "check", "--json"]);
    assert_eq!(run.code, Some(2), "{run:?}");
    let out = stdout_json(&run);
    assert_eq!(out["errors"], json!(errors));
    assert_eq!(out["warnings"], json!(warnings));
    assert_eq!(
        (&out["ok"], &out["checked"], &out["unwritten"]),
        (&json!(false), &json!(13), &json!(1))
    );

    // 사람용 출력의 끝 줄, --strict(front matter 문제도 오류), --id(한 행만)
    let run = sb.cli(&lead, &["doc", "check"]);
    assert!(
        run.stdout.contains(&format!("경고: {}", warnings[0])),
        "{run:?}"
    );
    assert!(
        run.stdout
            .trim_end()
            .ends_with("검사: 13 / 미작성: 1 / 문제: 2 / 경고: 10"),
        "{run:?}"
    );
    let strict = stdout_json(&sb.cli(&lead, &["doc", "check", "--strict", "--json"]));
    assert_eq!(strict["errors"].as_array().map(Vec::len), Some(12));
    assert_eq!(strict["warnings"], json!([]));
    let run = sb.cli(&lead, &["doc", "check", "--id", "D02"]);
    assert_eq!(run.code, Some(0), "{run:?}");
    assert!(
        run.stdout
            .starts_with("D02: review / 원천: docs/planning/product-specs/\n"),
        "{run:?}"
    );
    assert!(
        run.stdout
            .trim_end()
            .ends_with("검사: D02 / 미작성: 0 / 문제: 0 / 경고: 1"),
        "{run:?}"
    );

    // 없는 ID와 D01–D13이 아닌 인덱스는 실행 실패다
    assert_eq!(
        sb.cli(&lead, &["doc", "check", "--id", "D99"]).code,
        Some(1)
    );
    put(
        "docs/deliverables/README.md",
        "| D01 | 착수 | 사업계획서 | `a.md` | draft |\n",
    );
    assert_eq!(sb.cli(&lead, &["doc", "check"]).code, Some(1));
    assert_eq!(sb.cli(&lead, &["doc"]).code, Some(1));
}

fn is_date(text: &str) -> bool {
    text.len() == 10
        && text.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

/// `updated` 줄을 고정 글로 바꾼 문서 글.
fn stamped(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let date = text
        .lines()
        .find_map(|l| l.strip_prefix("updated: "))
        .unwrap_or_else(|| panic!("updated가 없다: {text}"));
    assert!(is_date(date), "{date}");
    text.replace(&format!("updated: {date}"), "updated: <오늘>")
}

#[test]
fn doc_stamp_rewrites_front_matter_in_fixed_order() {
    let sb = Sandbox::with(&Setup {
        remote: false,
        roles: Default::default(),
        ..Setup::default()
    });
    let lead = sb.path("lead");
    let base = squad(&sb);
    sb.write(
        &base.join("docs/deliverables/README.md"),
        &index_with(&[("D03", "review")]),
    );
    let stamp = |args: &[&str]| {
        let args: Vec<&str> = ["doc", "stamp"]
            .into_iter()
            .chain(args.iter().copied())
            .collect();
        sb.cli(&lead, &args)
    };

    // 일반 문서: 처음 쓸 때는 owner와 summary가 필요하다. 제목은 본문의 첫 제목이고 본문은 그대로다
    let note = base.join("docs/notes/meeting.md");
    sb.write(&note, "# 회의록\n\n내용\n");
    let run = stamp(&["--path", "docs/notes/meeting.md"]);
    assert_eq!(run.code, Some(1), "{run:?}");
    assert!(run.stderr.contains("--owner, --summary"));
    let run = stamp(&[
        "--path",
        "docs/notes/meeting.md",
        "--owner",
        "lead",
        "--summary",
        "주간 회의: 결제",
    ]);
    assert_eq!(run.code, Some(0), "{run:?}");
    assert_eq!(run.stdout.trim(), ".fullops-squad/docs/notes/meeting.md");
    assert_eq!(
        stamped(&note),
        "---\ntitle: 회의록\nstatus: draft\nupdated: <오늘>\nowner: lead\nsummary: \"주간 회의: 결제\"\n---\n\n# 회의록\n\n내용\n"
    );
    // 다시 쓰면 과제 키가 tasks에 쌓이고 상태가 바뀐다. 레포 루트 기준 경로도 받는다
    stamp(&[
        "--path",
        "docs/notes/meeting.md",
        "--task",
        "PAY-12",
        "--status",
        "review",
    ]);
    let run = stamp(&[
        "--path",
        ".fullops-squad/docs/notes/meeting.md",
        "--task",
        "57",
    ]);
    assert_eq!(run.code, Some(0), "{run:?}");
    stamp(&["--path", "docs/notes/meeting.md", "--task", "57"]);
    assert_eq!(
        stamped(&note),
        "---\ntitle: 회의록\nstatus: review\nupdated: <오늘>\nowner: lead\ntasks: [PAY-12, 57]\nsummary: \"주간 회의: 결제\"\n---\n\n# 회의록\n\n내용\n"
    );

    // 1.24.0이 쓴 문서: 값을 그대로 읽어 정해진 순서로 다시 쓴다. 모르는 키는 뒤에 이름순으로 둔다
    let old = base.join("docs/notes/old.md");
    sb.write(
        &old,
        "---\nzeta: 끝\nsummary: \"요약, 쉼표\"\nowner: arch # 담당\ntitle: 옛 문서\ntasks: A-1\nalpha: [x, y]\nupdated: 2025-01-02\nstatus: approved\n---\n본문 첫 줄\n",
    );
    assert_eq!(stamp(&["--path", "docs/notes/old.md"]).code, Some(0));
    assert_eq!(
        stamped(&old),
        "---\ntitle: 옛 문서\nstatus: approved\nupdated: <오늘>\nowner: arch\ntasks: [A-1]\nsummary: \"요약, 쉼표\"\nalpha: [x, y]\nzeta: 끝\n---\n본문 첫 줄\n"
    );

    // 산출물 원천 문서: 없으면 만든다. id와 제목은 인덱스에서, 상태는 인덱스의 값에서 온다
    let run = stamp(&[
        "--path",
        "docs/design-docs/architecture.md",
        "--owner",
        "arch",
        "--summary",
        "구조",
    ]);
    assert_eq!(run.code, Some(0), "{run:?}");
    assert_eq!(
        stamped(&base.join("docs/design-docs/architecture.md")),
        "---\nid: D03\ntitle: 아키텍처설계서\nstatus: review\nupdated: <오늘>\nowner: arch\nsummary: 구조\n---\n\n# 아키텍처설계서\n"
    );
    // 산출물 셋이 함께 가진 원천: id는 목록, 상태는 ID별 statuses다
    let run = stamp(&[
        "--path",
        "docs/design-docs/data-model.md",
        "--owner",
        "arch",
        "--summary",
        "모델",
        "--status",
        "review",
    ]);
    assert_eq!(run.code, Some(0), "{run:?}");
    assert_eq!(
        stamped(&base.join("docs/design-docs/data-model.md")),
        "---\nid: [D06, D07, D09]\ntitle: data-model\nstatus: draft\nstatuses: {\"D06\": \"review\", \"D07\": \"review\", \"D09\": \"review\"}\nupdated: <오늘>\nowner: arch\nsummary: 모델\n---\n\n# data-model\n"
    );

    // stamp가 쓴 문서는 lint의 DOC-002·DOC-003에 걸리지 않고 doc check의 front matter 검사를 통과한다
    let from = sb.git_ok(&lead, &["rev-parse", "HEAD"]);
    commit(&sb, &lead, "문서");
    let run = sb.cli(&lead, &["lint", "--from", &from]);
    assert_eq!(run.code, Some(2), "{run:?}");
    // 인덱스 README는 front matter가 없어 DOC-003에 걸린다. 그 밖의 문서는 걸리지 않는다
    let docs: Vec<&str> = run
        .stdout
        .lines()
        .filter(|l| l.contains(" DOC-00"))
        .collect();
    assert_eq!(docs.len(), 1, "{run:?}");
    assert!(docs[0].starts_with("ERROR DOC-003 .fullops-squad/docs/deliverables/README.md"));
    let check = stdout_json(&sb.cli(
        &lead,
        &["doc", "check", "--strict", "--id", "D03", "--json"],
    ));
    assert_eq!(
        check["errors"],
        json!(["D03 원천 없음: docs/design-docs/tech-stack.md"])
    );

    // 받지 않는 경로와 값, 읽을 수 없는 front matter
    for args in [
        &["--path", "../outside.md", "--owner", "a", "--summary", "b"][..],
        &[
            "--path",
            "docs/notes/data.json",
            "--owner",
            "a",
            "--summary",
            "b",
        ],
        &["--path", "docs/notes/meeting.md", "--status", "done"],
        &["--owner", "a"],
    ] {
        assert_eq!(stamp(args).code, Some(1), "{args:?}");
    }
    sb.write(
        &base.join("docs/notes/broken.md"),
        "---\ntitle: \"a\\qb\"\n---\n",
    );
    let run = stamp(&[
        "--path",
        "docs/notes/broken.md",
        "--owner",
        "a",
        "--summary",
        "b",
    ]);
    assert_eq!(run.code, Some(1), "{run:?}");
    assert!(run.stderr.contains("front matter를 읽을 수 없습니다"));
}
