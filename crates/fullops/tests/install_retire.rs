//! `fullops install`의 배포본 보관(P1), 하향 거절(P2), 스킬 되돌리기(P9), 옛 판 정리(P4)(설계 10.3절, `docs/install-and-retire.md` 4절).
//! 홈은 임시 폴더이고 호스트 CLI는 가짜 실행 파일이다(`bundle_f.rs`와 같다). 실제 홈과 호스트는 건드리지 않는다.

use fullops_testkit::{Run, Sandbox};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const HOSTS: [&str; 5] = ["claude", "codex", "grok", "agy", "npx"];

fn json_of(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

/// 통합 패키지를 흉내 낸 폴더를 만들고 `install.json`이 그 폴더를 가리키게 한다(플러그인 루트가 된다).
fn package(sb: &Sandbox, name: &str, version: &str, installed: &str) -> PathBuf {
    let pkg = sb.root.join(name);
    sb.write(&pkg.join("VERSION"), &format!("{version}\n"));
    sb.write(&pkg.join("bin/fullops"), "#!/bin/sh\n");
    sb.write(
        &pkg.join("skills/fullops-work/SKILL.md"),
        "---\nname: fullops-work\n---\n",
    );
    sb.write(
        &pkg.join("hosts/grok/plugin.json"),
        "{\"name\":\"fullops\",\"version\":\"2.0.0\",\"description\":\"시험\"}\n",
    );
    let record = json!({"version": installed, "sha256": "", "source": pkg.to_string_lossy(), "at": 0, "previous": null});
    sb.write(&sb.home.join(".fullops/install.json"), &record.to_string());
    pkg
}

fn run(sb: &Sandbox, args: &[&str], online: bool) -> Run {
    let offline = if online { "0" } else { "1" };
    sb.fullops(&sb.home, args, &[("FULLOPS_OFFLINE", offline)], b"")
}

fn calls(sb: &Sandbox) -> Vec<String> {
    sb.host_calls()
        .iter()
        .map(|c| {
            let argv: Vec<&str> = c["argv"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            format!("{} {}", c["program"].as_str().unwrap_or(""), argv.join(" "))
        })
        .collect()
}

fn names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// P1·P2: 배포본을 `~/.fullops/package/<버전>/source`에 보관하고, 낮은 버전으로는 명시 인자 없이 바꾸지 않는다.
#[test]
fn p1_keeps_package_and_p2_refuses_downgrade() {
    let sb = Sandbox::empty();
    sb.install_fakes(&HOSTS);
    let pkg = package(&sb, "npx-cache", "2.0.0", "0.0.1");
    let record = || json_of(&sb.home.join(".fullops/install.json"));
    let kept = sb.home.join(".fullops/package/2.0.0/source");

    let plan = run(&sb, &["install", "--only", "runtime", "--dry-run"], false);
    assert_eq!(plan.code, Some(0), "{}", plan.stderr);
    assert!(
        plan.stdout.contains("[runtime] 배포본 보관 → "),
        "{}",
        plan.stdout
    );
    assert!(!kept.exists());

    let first = run(&sb, &["install", "--only", "runtime"], false);
    assert_eq!(first.code, Some(0), "{}", first.stderr);
    assert_eq!(Path::new(record()["source"].as_str().unwrap()), kept);
    assert!(kept.join("VERSION").is_file() && kept.join("hosts/grok/plugin.json").is_file());
    // 같은 내용이면 다시 복사하지 않는다(옛 폴더가 생기지 않는다)
    assert_eq!(
        run(&sb, &["install", "--only", "runtime"], false).code,
        Some(0)
    );
    assert_eq!(names(&sb.home.join(".fullops/package/2.0.0")), ["source"]);
    // npx 캐시가 지워져도 보관한 배포본이 플러그인 루트다
    std::fs::remove_dir_all(&pkg).unwrap();
    let again = run(&sb, &["install", "--only", "runtime"], false);
    assert_eq!(again.code, Some(0), "{}", again.stderr);
    assert_eq!(Path::new(record()["source"].as_str().unwrap()), kept);
    let planned = run(&sb, &["install", "--hosts", "grok", "--dry-run"], true);
    assert!(
        planned.stdout.contains("[grok] 조립: "),
        "{}",
        planned.stdout
    );

    // P2: 기록(2.0.0)보다 낮은 배포본은 거절하고 아무것도 바꾸지 않는다
    package(&sb, "older", "1.9.0", "2.0.0");
    let before = std::fs::read_to_string(sb.home.join(".fullops/install.json")).unwrap();
    for args in [
        &["install", "--only", "runtime"][..],
        &["install", "--dry-run"][..],
    ] {
        let refused = run(&sb, args, false);
        assert_eq!(refused.code, Some(2), "{args:?}: {}", refused.stderr);
        assert!(
            refused.stderr.contains("--allow-downgrade"),
            "{}",
            refused.stderr
        );
    }
    assert_eq!(
        std::fs::read_to_string(sb.home.join(".fullops/install.json")).unwrap(),
        before
    );
    assert!(!sb.home.join(".fullops/package/1.9.0").exists());
    let forced = run(
        &sb,
        &["install", "--only", "runtime", "--allow-downgrade"],
        false,
    );
    assert_eq!(forced.code, Some(0), "{}", forced.stderr);
    assert_eq!(record()["version"], "1.9.0");
    assert_eq!(record()["previous"], "2.0.0");
    // 되돌린 뒤 기록의 버전(2.0.0)이 아닌 폴더라도 기록의 source를 담고 있으면 --prune이 지우지 않는다
    assert_eq!(run(&sb, &["install", "--rollback"], false).code, Some(0));
    assert_eq!(record()["version"], "2.0.0");
    assert_eq!(run(&sb, &["install", "--prune"], false).code, Some(0));
    assert!(
        sb.home
            .join(".fullops/package/1.9.0/source/VERSION")
            .is_file()
    );
}

/// P9: 백업에 원래 자리를 적고, `--restore-skills`는 계획만 내다가 `--apply`로 그 자리에 되돌린다.
#[test]
fn p9_restore_skills_plans_then_applies() {
    let sb = Sandbox::empty();
    sb.install_fakes(&HOSTS);
    package(&sb, "pkg", "2.0.0", "0.0.1");
    let skill = sb.home.join(".claude/skills/caveman");
    sb.write(&skill.join("SKILL.md"), "사용자가 고친 스킬\n");
    let installed = run(&sb, &["install", "--hosts", "claude"], true);
    assert_eq!(installed.code, Some(0), "{}", installed.stderr);
    let backups = sb.home.join(".fullops/skill-backups/claude/caveman");
    let copies = names(&backups);
    assert_eq!(copies.len(), 1);
    let meta = json_of(&backups.join(&copies[0]).join(".fullops-origin.json"));
    assert_eq!(
        Path::new(meta["origins"][0].as_str().unwrap()),
        skill.as_path()
    );
    // 설치 도구가 덮어썼다고 본다. 원래 자리를 적지 않은 옛 백업도 하나 둔다
    sb.write(&skill.join("SKILL.md"), "새로 설치된 스킬\n");
    let legacy = sb
        .home
        .join(".fullops/skill-backups/codex/old-one/abcdefabcdef");
    sb.write(&legacy.join("SKILL.md"), "옛 백업\n");

    let plan = run(&sb, &["install", "--restore-skills"], false);
    assert_eq!(plan.code, Some(0), "{}", plan.stderr);
    assert!(plan.stdout.contains("되돌림: "), "{}", plan.stdout);
    assert!(plan.stdout.contains("원래 자리 모름"), "{}", plan.stdout);
    let now = std::fs::read_to_string(skill.join("SKILL.md")).unwrap();
    assert_eq!(now, "새로 설치된 스킬\n");
    assert_eq!(
        run(&sb, &["install", "--restore-skills", "--dry-run"], false).code,
        Some(1)
    );
    assert_eq!(run(&sb, &["install", "--apply"], false).code, Some(1));

    let applied = run(&sb, &["install", "--restore-skills", "--apply"], false);
    assert_eq!(applied.code, Some(0), "{}", applied.stderr);
    let now = std::fs::read_to_string(skill.join("SKILL.md")).unwrap();
    assert_eq!(now, "사용자가 고친 스킬\n");
    assert!(!skill.join(".fullops-origin.json").exists());
    // 덮어쓴 내용도 백업에 남아 되돌림을 되돌릴 수 있다. 원래 자리를 모르는 백업은 그대로다
    assert_eq!(names(&backups).len(), 2);
    assert!(legacy.join("SKILL.md").is_file());
    let again = run(&sb, &["install", "--restore-skills", "--apply"], false);
    assert!(again.stdout.contains("이미 같다"), "{}", again.stdout);
}

/// 옛 판의 자리를 홈에 만든다. 정리 대상이 아닌 것(사람 설정, 공용 스킬)도 함께 둔다.
fn old_home(sb: &Sandbox) -> Vec<PathBuf> {
    let home = &sb.home;
    let mut moved = vec![
        home.join(".claude/fullops-deps.json"),
        home.join(".codex/fullops-skill-backups/1/caveman/SKILL.md"),
        home.join(".grok/fullops-deps.json"),
        home.join(".cache/fullops-squad/jev/a.json"),
    ];
    if cfg!(windows) {
        moved.push(home.join("AppData/Roaming/orca/claude-profiles/p1/home/fullops-deps.json"));
    }
    for path in &moved {
        sb.write(path, "{}\n");
    }
    sb.write(
        &home.join(".fullops-squad/runtime/fullops-squad/scripts/hook_runtime.py"),
        "#\n",
    );
    sb.write(&home.join(".claude/settings.json"), "{}\n");
    sb.write(&home.join(".agents/skills/caveman/SKILL.md"), "공용\n");
    moved
}

fn lists(sb: &Sandbox, claude: Value) {
    // 마켓플레이스 목록 규칙이 먼저다(`grok plugin list`의 낱말이 `grok plugin marketplace list`에도 맞는다)
    sb.host_rule(json!({"when": ["grok", "plugin", "marketplace", "list"], "stdout": [{"name": "fullops-squad", "source": {"url": "https://github.com/tourbut/Fullops-Squad-plugin"}}]}));
    sb.host_rule(json!({"when": ["claude", "plugin", "list"], "stdout": claude}));
    sb.host_rule(json!({"when": ["codex", "plugin", "list"], "stdout": {"installed": []}}));
    sb.host_rule(json!({"when": ["agy", "plugin", "list"], "stdout": []}));
}

/// P4: 계획만 → 레포 목록·옛 표식이 있으면 거절 → 실행(옮김, grok 제거, runtime 마지막) → 되돌림.
#[test]
fn p4_retire_python_moves_and_restores() {
    let sb = Sandbox::empty();
    sb.install_fakes(&HOSTS);
    let moved = old_home(&sb);
    lists(&sb, json!([]));
    // grok의 옛 플러그인은 제거 명령 뒤의 목록에서 사라진다(계획 1번, 거절 2번, 실행 전 1번)
    sb.host_rule(json!({"when": ["grok", "plugin", "list"], "times": 4, "stdout": [{"name": "fullops-squad", "marketplace": "fullops-squad"}]}));
    sb.host_rule(json!({"when": ["grok", "plugin", "list"], "stdout": []}));
    let repo = sb.init_repo("old-repo");
    let worktree = sb.root.join("old-wt");
    sb.git_ok(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "role",
            &worktree.to_string_lossy(),
        ],
    );
    let marker = worktree.join(".fullops-squad/fullops.json");
    sb.write(&marker, "{}\n");
    let runtime = sb.home.join(".fullops-squad/runtime/fullops-squad");
    let untouched = || {
        moved.iter().all(|p| p.is_file())
            && runtime.is_dir()
            && !calls(&sb)
                .iter()
                .any(|c| c.contains("uninstall") || c.contains(" remove "))
    };

    let plan = run(&sb, &["install", "--retire-python"], true);
    assert_eq!(plan.code, Some(0), "{}", plan.stderr);
    for line in [
        "레포 목록이 없어",
        "[grok] 실행: grok plugin uninstall fullops-squad --confirm",
        "[grok] 실행: grok plugin marketplace remove https://github.com/tourbut/Fullops-Squad-plugin",
        "[옮김] ",
        "[옮김·마지막] ",
        "fullops-meter@fullops-squad`를 남기므로",
        "npm 전역 패키지",
    ] {
        assert!(
            plan.stdout.contains(line),
            "계획에 없다: {line}\n{}",
            plan.stdout
        );
    }
    assert!(untouched());

    let no_repos = run(&sb, &["install", "--retire-python", "--apply"], true);
    assert_eq!(no_repos.code, Some(2), "{}", no_repos.stderr);
    let repos = repo.to_string_lossy().into_owned();
    let left = run(
        &sb,
        &["install", "--retire-python", "--repos", &repos, "--apply"],
        true,
    );
    assert_eq!(left.code, Some(2), "{}", left.stderr);
    assert!(left.stderr.contains("옛 표식이 남았다"), "{}", left.stderr);
    assert!(untouched());

    std::fs::remove_file(&marker).unwrap();
    let done = run(
        &sb,
        &["install", "--retire-python", "--repos", &repos, "--apply"],
        true,
    );
    assert_eq!(done.code, Some(0), "{}\n{}", done.stdout, done.stderr);
    let host = calls(&sb);
    assert!(host.contains(&"grok plugin uninstall fullops-squad --confirm".to_string()));
    assert!(
        host.contains(
            &"grok plugin marketplace remove https://github.com/tourbut/Fullops-Squad-plugin"
                .to_string()
        )
    );
    // Claude Code·Codex 제거와 npm은 부르지 않고, 어떤 명령에도 --help가 없다
    assert!(host.iter().all(|c| !c.contains("--help")), "{host:#?}");
    assert!(
        host.iter()
            .filter(|c| c.starts_with("claude ") || c.starts_with("codex "))
            .all(|c| c.ends_with("list --json")),
        "{host:#?}"
    );
    assert!(moved.iter().all(|p| !p.exists()) && !runtime.exists());
    assert!(sb.home.join(".claude/settings.json").is_file());
    assert!(sb.home.join(".agents/skills/caveman/SKILL.md").is_file());
    let retired = sb.home.join(".fullops/py-retired");
    let at = names(&retired).pop().unwrap();
    let record = json_of(&retired.join(&at).join("moved.json"));
    assert_eq!(record.as_array().unwrap().len(), moved.len() + 1);
    // runtime이 마지막이다
    let last = record.as_array().unwrap().last().unwrap()["from"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        last.replace('\\', "/")
            .ends_with(".fullops-squad/runtime/fullops-squad")
    );

    let plan = run(
        &sb,
        &["install", "--retire-python", "--restore", &at],
        false,
    );
    assert_eq!(plan.code, Some(0), "{}", plan.stderr);
    assert!(moved.iter().all(|p| !p.exists()));
    assert_eq!(
        run(
            &sb,
            &["install", "--retire-python", "--restore", "../x"],
            false
        )
        .code,
        Some(1)
    );
    let back = run(
        &sb,
        &["install", "--retire-python", "--restore", &at, "--apply"],
        false,
    );
    assert_eq!(back.code, Some(0), "{}", back.stderr);
    assert!(moved.iter().all(|p| p.is_file()) && runtime.is_dir());
}

/// P4: Claude Code에 옛 플러그인이 남아 있으면 runtime은 옮기지 않고 사람 손이 필요하다고 끝낸다(종료 코드 3).
#[test]
fn p4_keeps_runtime_while_old_plugin_remains() {
    let sb = Sandbox::empty();
    sb.install_fakes(&HOSTS);
    old_home(&sb);
    lists(
        &sb,
        json!([{"id": "fullops-squad@fullops-squad"}, {"id": "fullops-meter@fullops-squad"}]),
    );
    sb.host_rule(json!({"when": ["grok", "plugin", "list"], "stdout": []}));
    let repo = sb.init_repo("new-repo");
    let done = run(
        &sb,
        &[
            "install",
            "--retire-python",
            "--repos",
            &repo.to_string_lossy(),
            "--apply",
            "--remove-meter",
        ],
        true,
    );
    assert_eq!(done.code, Some(3), "{}\n{}", done.stdout, done.stderr);
    assert!(
        done.stdout.contains("runtime을 옮기지 않았다"),
        "{}",
        done.stdout
    );
    assert!(
        done.stdout
            .contains("claude plugin marketplace remove fullops-squad")
    );
    assert!(
        sb.home
            .join(".fullops-squad/runtime/fullops-squad")
            .is_dir()
    );
    assert!(!sb.home.join(".claude/fullops-deps.json").exists());
    // grok에 옛 판이 없으면 grok 명령도 부르지 않는다(마켓플레이스만 남은 것은 지운다)
    let host = calls(&sb);
    assert!(!host.iter().any(|c| c.contains("uninstall")), "{host:#?}");
}

/// P4: 기본(meter를 남긴다)에서는 `fullops-meter@fullops-squad`가 남아도 runtime을 옮긴다(meter는 runtime을 쓰지 않는다).
#[test]
fn p4_meter_left_does_not_block_runtime() {
    let sb = Sandbox::empty();
    sb.install_fakes(&HOSTS);
    old_home(&sb);
    lists(&sb, json!([{"id": "fullops-meter@fullops-squad"}]));
    sb.host_rule(json!({"when": ["grok", "plugin", "list"], "stdout": []}));
    sb.write(
        &sb.home.join(".claude/settings.json"),
        "{\"enabledPlugins\":{\"fullops-meter@fullops-squad\":true}}\n",
    );
    let repo = sb.init_repo("new-repo");
    let repos = repo.to_string_lossy().into_owned();
    let plan = run(&sb, &["install", "--retire-python"], true);
    assert!(
        plan.stdout.contains("[claude] 옛 판 플러그인 없음"),
        "{}",
        plan.stdout
    );
    let done = run(
        &sb,
        &["install", "--retire-python", "--repos", &repos, "--apply"],
        true,
    );
    assert_eq!(done.code, Some(0), "{}\n{}", done.stdout, done.stderr);
    assert!(
        !sb.home
            .join(".fullops-squad/runtime/fullops-squad")
            .exists()
    );
}

/// P4: 지금 환경의 목록이 비어도 다른 홈(Codex 홈의 `config.toml`, Orca Claude 프로필의 `settings.json`)에 옛 판이 남았으면
/// runtime과 그 홈의 `fullops-deps.json`을 옮기지 않는다(종료 코드 3). 지운 뒤 다시 실행하면 옮긴다.
#[test]
fn p4_old_plugin_in_another_home_keeps_runtime() {
    let sb = Sandbox::empty();
    sb.install_fakes(&HOSTS);
    old_home(&sb);
    lists(&sb, json!([]));
    sb.host_rule(json!({"when": ["grok", "plugin", "list"], "stdout": []}));
    let codex = sb.home.join(".codex");
    sb.write(&codex.join("fullops-deps.json"), "{}\n");
    sb.write(
        &codex.join("config.toml"),
        "[plugins.\"fullops-squad@fullops-squad\"]\nenabled = true\n",
    );
    let mut stuck = vec![codex.clone()];
    if cfg!(windows) {
        let profile = sb.home.join("AppData/Roaming/orca/claude-profiles/p1/home");
        sb.write(
            &profile.join("settings.json"),
            "{\"enabledPlugins\":{\"fullops-squad@fullops-squad\":true}}\n",
        );
        stuck.push(profile);
    }
    let runtime = sb.home.join(".fullops-squad/runtime/fullops-squad");
    let repo = sb.init_repo("new-repo");
    let repos = repo.to_string_lossy().into_owned();

    let plan = run(&sb, &["install", "--retire-python"], true);
    assert!(
        plan.stdout.contains("[홈] ") && plan.stdout.contains("fullops-deps.json은 남긴다"),
        "{}",
        plan.stdout
    );
    let done = run(
        &sb,
        &["install", "--retire-python", "--repos", &repos, "--apply"],
        true,
    );
    assert_eq!(done.code, Some(3), "{}\n{}", done.stdout, done.stderr);
    assert!(runtime.is_dir());
    assert!(stuck.iter().all(|h| h.join("fullops-deps.json").is_file()));
    assert!(!sb.home.join(".claude/fullops-deps.json").exists());

    sb.write(
        &codex.join("config.toml"),
        "[plugins.\"ponytail@ponytail\"]\n",
    );
    for h in &stuck[1..] {
        sb.write(&h.join("settings.json"), "{\"enabledPlugins\":{}}\n");
    }
    let again = run(
        &sb,
        &["install", "--retire-python", "--repos", &repos, "--apply"],
        true,
    );
    assert_eq!(again.code, Some(0), "{}\n{}", again.stdout, again.stderr);
    assert!(!runtime.exists());
    assert!(stuck.iter().all(|h| !h.join("fullops-deps.json").exists()));
}

/// P4: 호스트 CLI가 PATH에 없어도 그 호스트의 홈(`~/.grok`)이 있으면 옛 판이 남았는지 모른다고 보고 runtime을 남긴다.
#[test]
fn p4_missing_cli_with_home_keeps_runtime() {
    let sb = Sandbox::empty();
    sb.install_fakes(&["claude", "codex", "agy", "npx"]);
    old_home(&sb);
    lists(&sb, json!([]));
    let runtime = sb.home.join(".fullops-squad/runtime/fullops-squad");
    let repo = sb.init_repo("new-repo");
    let repos = repo.to_string_lossy().into_owned();
    let args = ["install", "--retire-python", "--repos", &repos, "--apply"];
    let done = run(&sb, &args, true);
    assert_eq!(done.code, Some(3), "{}\n{}", done.stdout, done.stderr);
    assert!(runtime.is_dir());
    // grok을 쓴 적이 없는 PC(홈이 없다)는 막지 않는다
    std::fs::remove_dir_all(sb.home.join(".grok")).unwrap();
    let again = run(&sb, &args, true);
    assert_eq!(again.code, Some(0), "{}\n{}", again.stdout, again.stderr);
    assert!(!runtime.exists());
}

/// `--prune`은 스킬 백업을 지우지 않는다(`--restore-skills`의 원본이고, 되돌림 전 내용도 여기에만 있다).
#[test]
fn prune_keeps_skill_backups() {
    let sb = Sandbox::empty();
    package(&sb, "pkg", "2.0.0", "2.0.0");
    let backup = sb
        .home
        .join(".fullops/skill-backups/claude/caveman/abcdefabcdef/SKILL.md");
    sb.write(&backup, "백업\n");
    assert_eq!(run(&sb, &["install", "--prune"], false).code, Some(0));
    assert!(backup.is_file());
}

/// grok은 플러그인 신뢰를 사람에게 맡긴다. 신뢰가 필요해 멈추면 할 일을 알리고,
/// 사람이 `--trust`로 설치한 뒤 다시 실행하면 같은 출처를 또 설치하지 않고 남은 단계를 마친다.
#[test]
fn grok_trust_is_left_to_the_human_and_a_rerun_finishes() {
    let sb = Sandbox::empty();
    sb.install_fakes(&HOSTS);
    let pkg = package(&sb, "npx-cache", "2.0.0", "2.0.0");
    sb.write(
        &pkg.join("hosts/grok/.mcp.json"),
        "{}
",
    );
    let target = [".fullops", "package", "2.0.0", "grok"]
        .iter()
        .fold(sb.home.clone(), |p, c| p.join(c));
    let target = target.to_string_lossy();
    sb.host_rule(json!({"when": ["grok", "plugin", "marketplace", "list"], "stdout": []}));
    sb.host_rule(json!({"when": ["grok", "plugin", "list"], "times": 1, "stdout": []}));
    sb.host_rule(json!({"when": ["grok", "plugin", "list"], "stdout": [{"name": "fullops", "source": target}]}));
    sb.host_rule(json!({"when": ["grok", "plugin", "install", &*target], "times": 1, "exit": 1,
        "stderr": format!("To proceed, re-run with --trust:\n  grok plugin install {target} --trust\n")}));

    let first = run(&sb, &["install", "--hosts", "grok"], true);
    assert_eq!(
        first.code,
        Some(4),
        "{}
{:?}",
        first.stdout,
        calls(&sb)
    );
    assert!(
        first.stderr.contains("`fullops install --hosts grok`"),
        "{}",
        first.stderr
    );
    assert!(!sb.home.join(".grok/hooks/fullops.json").exists());
    // 조립을 다시 해도 옛 폴더가 쌓이지 않는다
    let again = run(&sb, &["install", "--hosts", "grok"], true);
    assert_eq!(again.code, Some(0), "{}\n{}", again.stdout, again.stderr);
    assert!(
        again.stderr.contains("이미 설치돼 있어"),
        "{}",
        again.stderr
    );
    assert_eq!(
        names(&sb.home.join(".fullops/package/2.0.0")),
        ["grok", "source"]
    );
    let installs = calls(&sb)
        .into_iter()
        .filter(|c| c.starts_with("grok plugin install") && c.contains(&*target))
        .count();
    assert_eq!(installs, 1, "{:?}", calls(&sb));
    assert!(calls(&sb).iter().all(|c| !c.contains("--trust")));
}
