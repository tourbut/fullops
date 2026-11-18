//! 2.2.0의 모델 후보(`fullops.toml`의 `[models]`. 설계 6.1.1절, 13.7절)의 통합 시험.
//! `migrate --models`가 2.1.x의 `orca-agents.md` 후보를 옮기고, `route`·`dispatch`가 후보를 쓰는지 본다. fake-orca만 쓴다.

use fullops_testkit::{Run, Sandbox, Setup};
use std::path::Path;

fn setup() -> Setup {
    Setup {
        roles: [("dev", "worker"), ("architecture", "design")]
            .iter()
            .map(|(id, kind)| (id.to_string(), kind.to_string()))
            .collect(),
        ..Setup::default()
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn refused(run: &Run, rule: &str) {
    assert_eq!(run.code, Some(2), "{}{}", run.stdout, run.stderr);
    assert!(
        run.stderr.contains(&format!("FullOps[{rule}]")),
        "{}",
        run.stderr
    );
}

const OLD_GUIDE: &str = include_str!("../assets/legacy/orca-agents-2.0.md");

#[test]
fn migrate_models_moves_candidates_and_removes_the_stock_guide() {
    let sb = Sandbox::with(&setup());
    let lead = sb.path("lead");
    let guide = lead.join(".fullops/guide/orca-agents.md");
    let filled = OLD_GUIDE.replace(
        "## 모델 후보\n",
        "## 모델 후보\n\n- `architecture` `claude` `claude-opus-5-5` `high`: 설계\n- `dev` `claude` `claude-sonnet-5-5` `medium`: 일반 구현\n- `dev` `agy` `gemini-3` `high`: 큰 구현\n- `qa` `claude` `m` `low`: 없는 역할\n",
    );
    sb.write(&guide, &filled);
    sb.git_ok(&lead, &["add", "-A"]);
    sb.git_ok(&lead, &["commit", "-m", "2.1 후보"]);
    let before = sb.git_ok(&lead, &["rev-parse", "HEAD"]);

    let dry = sb.cli(&lead, &["migrate", "--models", "--dry-run"]);
    assert_eq!(dry.code, Some(0), "{}{}", dry.stdout, dry.stderr);
    assert!(
        dry.stdout.contains("모델 후보 3개") && dry.stdout.contains("qa"),
        "{}",
        dry.stdout
    );
    assert_eq!(sb.git_ok(&lead, &["rev-parse", "HEAD"]), before);
    assert!(guide.is_file());

    let run = sb.cli(&lead, &["migrate", "--models"]);
    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert_eq!(
        sb.git_ok(&lead, &["log", "--format=%s", "-1"]),
        "fullops: 모델 후보를 fullops.toml로 옮김"
    );
    assert_eq!(sb.git_ok(&lead, &["status", "--porcelain"]), "");
    // 나머지가 기본 글이라 지웠다. 후보는 표로 갔고 agy는 Orca ID로 바뀌었다
    assert!(!guide.exists());
    let toml: toml::Table = read(&lead.join(".fullops/fullops.toml")).parse().unwrap();
    let dev = toml["models"]["dev"].as_array().unwrap();
    assert_eq!(dev.len(), 2);
    assert_eq!(dev[1]["agent"].as_str(), Some("antigravity"));
    assert_eq!(dev[0]["about"].as_str(), Some("일반 구현"));
    assert!(toml["models"].get("qa").is_none());

    // 다시 실행하면 할 일이 없다
    let again = sb.cli(&lead, &["migrate", "--models"]);
    assert_eq!(again.code, Some(0), "{}", again.stderr);
    // upgrade의 필수 적용 models는 통과한다
    let upgrade = sb.cli(&lead, &["upgrade"]);
    assert!(!upgrade.stdout.contains("- [models]"), "{}", upgrade.stdout);
}

#[test]
fn route_and_dispatch_use_the_candidates() {
    let sb = Sandbox::with(&setup());
    let lead = sb.path("lead");
    let config = lead.join(".fullops/fullops.toml");
    sb.write(
        &config,
        &format!(
            "{}\n[models]\narchitecture = [\n  {{ agent = \"claude\", model = \"claude-opus-5-5\", effort = \"high\", about = \"설계\" }},\n]\ndev = [\n  {{ agent = \"claude\", model = \"s\", effort = \"medium\" }},\n  {{ agent = \"claude\", model = \"o\", effort = \"high\" }},\n]\n",
            read(&config)
        ),
    );
    let route = |key: &str, extra: &[&str]| {
        let mut args = vec![
            "route",
            key,
            "--request",
            "결제 설계",
            "--class",
            "design",
            "--role",
            "architecture",
        ];
        args.extend_from_slice(extra);
        sb.cli(&lead, &args)
    };
    let off = [
        "--agent",
        "codex",
        "--model",
        "gpt-6-sol",
        "--effort",
        "low",
    ];
    // 목록 밖 조합은 근거가 있어야 기록한다
    let run = route("M-1", &off);
    refused(&run, "RT-06");
    assert!(
        run.stderr.contains("claude/claude-opus-5-5/high(설계)"),
        "{}",
        run.stderr
    );
    assert!(!lead.join(".fullops/tasks/M-1/route.json").exists());
    let with_why = [&off[..], &["--why", "사용자 지정"]].concat();
    assert_eq!(route("M-1", &with_why).code, Some(0));

    // 후보가 하나인 자리는 모델을 주지 않아도 그 후보로 배정한다
    assert_eq!(route("M-2", &[]).code, Some(0));
    let dry = sb.cli(
        &lead,
        &["dispatch", "M-2", "--role", "architecture", "--dry-run"],
    );
    assert_eq!(dry.code, Some(0), "{}{}", dry.stdout, dry.stderr);
    assert!(
        dry.stdout.contains("claude/claude-opus-5-5/high"),
        "{}",
        dry.stdout
    );
    // dispatch에서 목록 밖 조합은 D-14
    let bad = sb.cli(
        &lead,
        &[
            &["dispatch", "M-2", "--role", "architecture", "--dry-run"][..],
            &off,
        ]
        .concat(),
    );
    refused(&bad, "D-14");

    // 후보가 여럿이면 고르지 않은 배정을 거절하고 후보를 보여 준다
    let direct = sb.cli(
        &lead,
        &[
            "route",
            "M-3",
            "--request",
            "버튼",
            "--class",
            "direct",
            "--role",
            "dev",
        ],
    );
    assert_eq!(direct.code, Some(0), "{}", direct.stderr);
    let task = sb.cli(
        &lead,
        &[
            "task", "new", "--role", "dev", "--key", "M-3", "--goal", "버튼",
        ],
    );
    assert_eq!(task.code, Some(0), "{}", task.stderr);
    let none = sb.cli(&lead, &["dispatch", "M-3", "--dry-run"]);
    refused(&none, "D-09");
    assert!(
        none.stderr.contains("claude/s/medium, claude/o/high"),
        "{}",
        none.stderr
    );
}
