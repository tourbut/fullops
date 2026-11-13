//! 묶음 F(설치·이행·배포·스킬)의 시나리오(설계 12.2절 N2·N3, Q1~Q6, Q9·Q10, R8).
//! 시나리오 파일(`scenarios/<id>.toml`)을 `rs` 구동기로 돌리고, 파일로 적지 못하는 단언(가짜 호스트가 받은 호출, 홈에 생긴 파일)을 더 본다.
//! 실제 호스트 CLI와 사용자 홈은 건드리지 않는다: 홈은 임시 폴더이고 PATH는 가짜 실행 파일 폴더 하나다.

use fullops_testkit::driver::mismatches;
use fullops_testkit::{Driver, Scenario, StepResult};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load(name: &str) -> Scenario {
    let path = repo_root().join("scenarios").join(format!("{name}.toml"));
    fullops_testkit::scenario::load(&path).unwrap_or_else(|e| panic!("{e}"))
}

/// 호스트 CLI를 찾는 명령은 PATH가 가짜 실행 파일 폴더 하나여야 한다.
/// 이 PC의 실제 `claude`·`codex`·`grok`·`agy`·`npx`가 잡히면 사용자 설정이 바뀐다.
fn guard(name: &str, scenario: &Scenario) {
    for step in &scenario.steps {
        let args = step.list("args");
        let touches_hosts = step.act == "runs_fullops"
            && matches!(args.first().map(String::as_str), Some("install" | "doctor"));
        if !touches_hosts {
            continue;
        }
        let path = step
            .args
            .get("env")
            .and_then(|env| env.get("PATH"))
            .and_then(|v| v.as_str());
        assert_eq!(
            path,
            Some("../bin"),
            "{name}: install·doctor 단계는 env.PATH가 \"../bin\"이어야 한다"
        );
        assert_eq!(
            step.args.get("raw").and_then(|v| v.as_bool()),
            Some(true),
            "{name}: install·doctor 단계는 raw = true여야 한다"
        );
    }
}

/// 시나리오를 끝까지 돌리고 구동기(임시 폴더)와 실행한 단계의 결과를 돌려준다. 기대와 다르면 그 단계에서 실패한다.
fn play(name: &str) -> (Driver, Vec<StepResult>) {
    let scenario = load(name);
    guard(name, &scenario);
    let mut driver = Driver::new(&scenario.setup);
    let mut results = Vec::new();
    for (index, step) in scenario.steps.iter().enumerate() {
        if step.pending.is_some() {
            continue;
        }
        let result = driver
            .step(step)
            .unwrap_or_else(|e| panic!("{name} {}단계: {e}", index + 1));
        if let Some(expect) = &step.expect {
            let diff = mismatches(expect, &result);
            assert!(
                diff.is_empty(),
                "{name} {}단계({}): {}\n출력: {}",
                index + 1,
                step.act,
                diff.join(", "),
                result.output
            );
        }
        results.push(result);
    }
    (driver, results)
}

/// 가짜 호스트가 받은 호출을 `<실행 파일> <인자…>` 한 줄씩으로.
fn host_lines(driver: &Driver) -> Vec<String> {
    driver
        .sb
        .host_calls()
        .iter()
        .map(|call| {
            let argv: Vec<&str> = call["argv"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            format!(
                "{} {}",
                call["program"].as_str().unwrap_or(""),
                argv.join(" ")
            )
        })
        .collect()
}

fn count(lines: &[String], needle: &str) -> usize {
    lines.iter().filter(|l| l.contains(needle)).count()
}

fn json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn fixed_bin(driver: &Driver) -> PathBuf {
    let name = format!("fullops{}", std::env::consts::EXE_SUFFIX);
    driver.sb.home.join(".fullops/bin").join(name)
}

/// 스킬 묶음의 수(내장 `deps.toml`).
const SKILL_BUNDLES: usize = 9;

#[test]
fn q1_dry_run_plans_four_hosts_without_touching_anything() {
    let (driver, results) = play("q1");
    let out = &results.last().unwrap().output;
    for line in [
        "[claude] claude plugin marketplace add tourbut/fullops",
        "[claude] claude plugin install fullops@fullops",
        "[claude] claude plugin update fullops@fullops --scope user",
        "[claude] claude plugin install ponytail@ponytail",
        "[claude] claude plugin install mattpocock-skills@claude-plugins-official",
        "[claude] claude plugin install pstack@cursor-plugins",
        "[codex] codex plugin marketplace add tourbut/fullops",
        "[codex] codex plugin add fullops@fullops",
        "[codex] codex plugin add ponytail@ponytail",
        "[grok] 조립: ",
        "[grok] grok plugin install ",
        "[grok] 쓰기: ",
        "[agy] 조립: ",
        "[agy] agy plugin install ",
        "[skills] npx --yes skills@latest add JuliusBrussee/caveman --skill caveman --global --agent claude-code codex grok antigravity-cli --yes",
        "[skills] npx --yes skills@latest add mattpocock/skills --skill * --global --agent codex grok antigravity-cli --yes",
        "[skills] npx --yes skills@latest add DietrichGebert/ponytail --skill * --global --agent grok antigravity-cli --yes",
    ] {
        assert!(out.contains(line), "계획에 없다: {line}\n{out}");
    }
    // pstack은 Claude Code에만, ponytail 플러그인은 grok·agy에 없다
    assert!(
        out.lines()
            .filter(|l| l.contains("pstack"))
            .all(|l| l.starts_with("[claude] "))
    );
    assert!(
        !out.contains("grok plugin install ponytail")
            && !out.contains("agy plugin install ponytail")
    );
    // 신뢰 인자와 npm 전역 설치가 없다(DF-17)
    for banned in ["--trust", "npm ", "install -g", "--help"] {
        assert!(!out.contains(banned), "계획에 `{banned}`가 있다\n{out}");
    }
    // 조립한 패키지와 hook 등록 파일의 경로가 임시 홈 아래다
    let home = driver.sb.home.to_string_lossy().replace('\\', "/");
    let shown = out.replace('\\', "/");
    assert!(shown.contains(&format!("{home}/.fullops/package/2.0.0/grok")));
    assert!(shown.contains(&format!("{home}/.grok/hooks/fullops.json")));
    // 호스트 CLI를 부르지 않았고 파일을 쓰지 않았다
    assert!(driver.sb.host_calls().is_empty());
    let home = &driver.sb.home;
    for path in [
        ".fullops/bin",
        ".fullops/package",
        ".fullops/deps-state.json",
        ".grok",
    ] {
        assert!(!home.join(path).exists(), "{path}가 생겼다");
    }
}

#[test]
fn q2_installs_four_hosts_and_reruns() {
    let (driver, _) = play("q2-install");
    let home = &driver.sb.home;
    let calls = host_lines(&driver);
    // 스킬 명령은 묶음마다 한 번이고 --agent에 대상 호스트가 모여 있다(두 번 실행했다)
    assert_eq!(
        count(&calls, "npx --yes skills@latest add "),
        SKILL_BUNDLES * 2
    );
    assert_eq!(
        count(
            &calls,
            "add JuliusBrussee/caveman --skill caveman --global --agent claude-code codex grok antigravity-cli --yes"
        ),
        2
    );
    // grok·agy에는 조립한 패키지의 경로가 넘어갔고 10.3절 표의 파일이 있다
    let package = home.join(".fullops/package/2.0.0");
    for (program, dir) in [("grok", "grok"), ("agy", "agy")] {
        let installs: Vec<&String> = calls
            .iter()
            .filter(|l| l.starts_with(&format!("{program} plugin install ")))
            .collect();
        assert_eq!(installs.len(), 2, "{calls:#?}");
        let given = installs[0].rsplit(' ').next().unwrap().replace('\\', "/");
        assert_eq!(
            given,
            package.join(dir).to_string_lossy().replace('\\', "/")
        );
    }
    for file in [
        "plugin.json",
        ".mcp.json",
        "skills/fullops-work/SKILL.md",
        "VERSION",
    ] {
        assert!(package.join("grok").join(file).is_file(), "grok/{file}");
    }
    assert!(!package.join("grok/bin").exists() && !package.join("grok/hooks").exists());
    for file in [
        "plugin.json",
        "hooks.json",
        "mcp_config.json",
        "skills/fullops-work/SKILL.md",
        "bin/fullops",
        "VERSION",
    ] {
        assert!(package.join("agy").join(file).is_file(), "agy/{file}");
    }
    assert!(!package.join("agy/hooks/hooks.json").exists());
    // grok의 hook 등록 파일이 임시 홈의 고정 위치 실행본을 가리킨다
    let hook = json(&home.join(".grok/hooks/fullops.json"));
    let command = hook["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert_eq!(Path::new(command), fixed_bin(&driver));
    assert!(fixed_bin(&driver).is_file());
    assert_eq!(
        hook["hooks"]["Stop"][0]["hooks"][0]["args"],
        serde_json::json!(["hook", "grok"])
    );
    // 신뢰 인자가 어디에도 없다(Q5)
    assert_eq!(count(&calls, "trust"), 0);
    assert_eq!(count(&calls, "--help"), 0);
    // 다시 실행: 등록된 마켓플레이스는 등록하지 않고 갱신한다
    assert_eq!(
        count(&calls, "claude plugin marketplace add tourbut/fullops"),
        1
    );
    assert_eq!(count(&calls, "claude plugin marketplace update fullops"), 1);
    assert_eq!(count(&calls, "claude plugin install fullops@fullops"), 2);
    // 설치 기록
    let state = json(&home.join(".fullops/deps-state.json"));
    for host in ["claude", "codex", "grok", "agy"] {
        assert_eq!(state["hosts"][host]["complete"], true, "{host}: {state}");
        assert_eq!(state["hosts"][host]["self_version"], "2.0.0");
    }
    assert_eq!(state["manifest_sha256"].as_str().unwrap().len(), 64);
    // 두 번째 조립이 옛 폴더를 옮겨 뒀고 --prune이 지운다
    let olds = |dir: &Path| {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".old-"))
            .count()
    };
    assert!(olds(&package) >= 2);
    let prune = driver.sb.fullops(
        &driver.sb.path("lead"),
        &["install", "--prune"],
        &[("PATH", "../bin")],
        b"",
    );
    assert_eq!(prune.code, Some(0), "{}", prune.stderr);
    assert_eq!(olds(&package), 0);
    assert!(package.join("grok/plugin.json").is_file());
}

#[test]
fn q2_conflicting_marketplace_stops_only_that_host() {
    let (driver, _) = play("q2-conflict");
    let calls = host_lines(&driver);
    // Codex에는 목록 조회만 갔다
    let codex: Vec<&String> = calls.iter().filter(|l| l.starts_with("codex ")).collect();
    assert_eq!(codex, ["codex plugin marketplace list --json"]);
    assert!(count(&calls, "claude plugin install fullops@fullops") == 1);
    // 스킬 명령의 --agent에 codex가 없다
    let skills: Vec<&String> = calls.iter().filter(|l| l.starts_with("npx ")).collect();
    assert_eq!(skills.len(), SKILL_BUNDLES);
    assert!(skills.iter().all(|l| !l.contains(" codex")), "{skills:#?}");
    let state = json(&driver.sb.home.join(".fullops/deps-state.json"));
    assert_eq!(state["hosts"]["codex"]["complete"], false);
    assert!(
        !state["hosts"]["codex"]["failed"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(state["hosts"]["claude"]["complete"], true);
}

#[test]
fn q2_failed_skill_command_marks_its_hosts_and_continues() {
    let (driver, _) = play("q2-skill-fail");
    let calls = host_lines(&driver);
    assert_eq!(count(&calls, "npx --yes skills@latest add "), SKILL_BUNDLES);
    let state = json(&driver.sb.home.join(".fullops/deps-state.json"));
    for host in ["claude", "codex", "grok", "agy"] {
        assert_eq!(state["hosts"][host]["complete"], false, "{host}");
        let failed = state["hosts"][host]["failed"].as_array().unwrap();
        assert_eq!(failed.len(), 1);
        assert!(failed[0].as_str().unwrap().contains("typesafe-ai/skills"));
    }
}

#[test]
fn q2_without_package_folder_skips_grok_and_agy_plugins_only() {
    let (driver, _) = play("q2-no-package");
    let calls = host_lines(&driver);
    assert_eq!(count(&calls, "grok plugin install"), 0);
    assert_eq!(count(&calls, "agy plugin install"), 0);
    assert_eq!(count(&calls, "claude plugin install fullops@fullops"), 1);
    assert_eq!(count(&calls, "npx --yes skills@latest add "), SKILL_BUNDLES);
    assert!(driver.sb.home.join(".grok/hooks/fullops.json").is_file());
    assert!(!driver.sb.home.join(".fullops/package").exists());
}

#[test]
fn q3_missing_host_is_skipped() {
    let (driver, _) = play("q3");
    let calls = host_lines(&driver);
    assert_eq!(count(&calls, "codex plugin add fullops@fullops"), 1);
    assert!(calls.iter().all(|l| !l.starts_with("grok ")));
    let state = json(&driver.sb.home.join(".fullops/deps-state.json"));
    assert_eq!(state["hosts"]["codex"]["complete"], true);
    assert!(state["hosts"].get("grok").is_none());
}

#[test]
fn q4_offline_does_runtime_stage_only() {
    let (driver, _) = play("q4");
    assert!(driver.sb.host_calls().is_empty());
    assert!(fixed_bin(&driver).is_file());
    let record = json(&driver.sb.home.join(".fullops/install.json"));
    assert_eq!(record["version"], "2.0.0");
    assert_eq!(record["previous"], "0.0.1");
    assert_eq!(record["sha256"].as_str().unwrap().len(), 64);
    assert!(!driver.sb.home.join(".fullops/deps-state.json").exists());
}

#[test]
fn q5_no_trust_no_npx_and_single_backup() {
    let (driver, _) = play("q5-no-npx");
    let calls = host_lines(&driver);
    assert!(calls.iter().all(|l| !l.starts_with("npx ")));
    assert!(count(&calls, "grok plugin install ") == 1 && count(&calls, "trust") == 0);
    let state = json(&driver.sb.home.join(".fullops/deps-state.json"));
    assert_eq!(state["hosts"]["claude"]["complete"], false);
    assert_eq!(state["hosts"]["claude"]["failed"], serde_json::json!([]));

    let (driver, _) = play("q5-backup");
    let backups = driver.sb.home.join(".fullops/skill-backups/claude/caveman");
    let copies: Vec<_> = std::fs::read_dir(&backups).unwrap().flatten().collect();
    assert_eq!(copies.len(), 1);
    let kept = std::fs::read_to_string(copies[0].path().join("SKILL.md")).unwrap();
    assert!(kept.contains("사용자가 고친 스킬"));
    assert_eq!(copies[0].file_name().to_string_lossy().len(), 12);
}

#[test]
fn q6_check_and_doctor_report_missing_skill_and_stale_record() {
    let (driver, results) = play("q6");
    // 플러그인 목록을 읽은 확인: 없는 것과 꺼진 것만 낸다
    let listed = results
        .iter()
        .find(|r| r.output.contains("claude: plugin "))
        .map(|r| r.output.as_str())
        .unwrap_or_default();
    assert!(listed.contains("claude: plugin pstack@cursor-plugins"));
    assert!(!listed.contains("plugin fullops@fullops") && !listed.contains("plugin ponytail@"));
    // --check와 doctor는 설치 명령을 부르지 않는다
    let calls = host_lines(&driver);
    assert_eq!(count(&calls, "claude plugin install fullops@fullops"), 1);
    assert_eq!(count(&calls, "npx "), SKILL_BUNDLES - 2);
}

#[test]
fn q10_remove_hooks_deletes_only_the_fullops_file() {
    let scenario = load("q10");
    guard("q10", &scenario);
    let mut driver = Driver::new(&scenario.setup);
    let hooks = driver.sb.home.join(".grok/hooks");
    let mut calls_before_remove = 0;
    for step in &scenario.steps {
        let removing = step.list("args").contains(&"--remove-hooks".to_string());
        if removing {
            // 같은 폴더의 다른 도구 파일은 건드리지 않는다
            driver.sb.write(&hooks.join("orca-status.json"), "{}\n");
            assert!(hooks.join("fullops.json").is_file());
            calls_before_remove = driver.sb.host_calls().len();
        }
        let result = driver.step(step).unwrap();
        if let Some(expect) = &step.expect {
            let diff = mismatches(expect, &result);
            assert!(diff.is_empty(), "{}: {diff:?}\n{}", step.act, result.output);
        }
        if step.list("args").contains(&"--check".to_string()) {
            // 플러그인 목록의 이름만으로 로컬 경로로 설치한 fullops 플러그인을 알아본다
            assert!(
                !result.output.contains("grok: plugin fullops"),
                "{}",
                result.output
            );
            assert!(result.output.contains("grok: skill caveman"));
        }
        if removing {
            assert!(!hooks.join("fullops.json").exists());
            assert!(hooks.join("orca-status.json").is_file());
            assert_eq!(driver.sb.host_calls().len(), calls_before_remove);
        }
    }
    // 그 뒤의 확인도 설치 명령과 제거 명령을 부르지 않았다
    let calls = host_lines(&driver);
    assert_eq!(count(&calls, "uninstall"), 0);
    assert_eq!(count(&calls, "grok plugin install "), 1);
}

/// N2: 실행 중인 hook이 있어도 실행본을 바꿀 수 있고, 그 hook은 옛 실행본으로 끝까지 돈다.
#[test]
fn n2_runtime_swap_while_a_hook_is_running() {
    let (driver, _) = play("n2");
    let sb = &driver.sb;
    let fixed = fixed_bin(&driver);
    let lead = sb.path("lead");
    let before = std::fs::read(&fixed).unwrap();
    // 고정 위치의 실행본으로 hook 하나를 띄워 stdin을 열어 둔다(입력을 기다리는 동안 실행 중이다)
    let mut hook = sb.command(&fixed, &lead);
    hook.args(["hook", "claude"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    // 방금 복사한 실행 파일은 백신이 잠깐 잡고 있을 수 있어 몇 번 다시 띄운다
    let mut running = (0..50)
        .find_map(|_| {
            hook.spawn()
                .inspect_err(|_| std::thread::sleep(std::time::Duration::from_millis(200)))
                .ok()
        })
        .expect("고정 위치 실행본을 띄우지 못했다");
    // 플러그인이 갱신됐다: 새 VERSION의 통합 패키지에서 실행본 단계를 다시 한다
    sb.write(&sb.root.join("pkg/VERSION"), "2.0.1\n");
    let swapped = sb.fullops(
        &lead,
        &["install", "--only", "runtime"],
        &[("PATH", "../bin")],
        b"",
    );
    assert_eq!(swapped.code, Some(0), "{}", swapped.stderr);
    // 옛 실행본은 이름만 바뀌어 남았다
    let olds: Vec<_> = std::fs::read_dir(fixed.parent().unwrap())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("fullops.old-"))
        .collect();
    assert_eq!(olds.len(), 1);
    assert_eq!(std::fs::read(olds[0].path()).unwrap(), before);
    // 실행 중이던 hook은 옛 실행본으로 끝까지 돌아 통과를 낸다
    use std::io::Write;
    let event = serde_json::json!({"hook_event_name": "Stop", "session_id": "sid-n2", "cwd": lead});
    running
        .stdin
        .take()
        .unwrap()
        .write_all(event.to_string().as_bytes())
        .unwrap();
    let done = running.wait_with_output().unwrap();
    assert_eq!(done.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&done.stdout)
            .trim_start()
            .starts_with('{')
    );
    // 다음 호출부터 새 실행본이다
    let record = json(&sb.home.join(".fullops/install.json"));
    assert_eq!(record["version"], "2.0.1");
    assert_eq!(record["previous"], "2.0.0");
    let version = {
        let mut cmd = sb.command(&fixed, &lead);
        cmd.arg("--version");
        sb.run(cmd, b"")
    };
    assert_eq!(version.code, Some(0));
    // --rollback은 가장 최근 옛 파일로 되돌린다
    let back = sb.fullops(
        &lead,
        &["install", "--rollback"],
        &[("PATH", "../bin")],
        b"",
    );
    assert_eq!(back.code, Some(0), "{}", back.stderr);
    assert_eq!(
        json(&sb.home.join(".fullops/install.json"))["version"],
        "2.0.0"
    );
    assert_eq!(std::fs::read(&fixed).unwrap(), before);
}

fn all_files(dir: &Path) -> Vec<String> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                out.push(
                    path.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn sorted_keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .map(|o| o.keys().map(String::as_str).collect())
        .unwrap_or_default();
    keys.sort();
    keys
}

/// Q9: `xtask dist`가 통합 패키지와 디렉터리판을 만든다. 호스트 CLI의 검증은 돌리지 않는다(`--no-validate`).
#[test]
fn q9_dist_builds_native_and_directory_packages() {
    let sb = fullops_testkit::Sandbox::empty();
    let prebuilt = sb.root.join("prebuilt");
    let binaries = [
        "fullops.exe",
        "fullops-darwin-arm64",
        "fullops-darwin-x64",
        "fullops-linux-x64",
    ];
    for name in binaries {
        sb.write(&prebuilt.join(name), name);
    }
    let out_root = sb.root.join("out");
    let mut cmd = sb.command(fullops_testkit::bin("xtask"), &sb.root);
    cmd.arg("dist")
        .arg("--root")
        .arg(&out_root)
        .arg("--prebuilt")
        .arg(&prebuilt)
        .arg("--no-validate")
        .env("PATH", &sb.bin_dir);
    let run = sb.run(cmd, b"");
    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);

    let native = out_root.join("dist/native/fullops");
    let directory = out_root.join("dist/directory/fullops");
    let version = env!("CARGO_PKG_VERSION");
    let packaged = std::fs::read_to_string(native.join("VERSION")).unwrap();
    assert_eq!(packaged.trim(), version);

    // 네 매니페스트
    let claude = json(&native.join(".claude-plugin/plugin.json"));
    assert_eq!(claude["name"], "fullops");
    assert_eq!(claude["version"], version);
    assert_eq!(claude["skills"], "./skills/");
    assert!(
        claude.get("hooks").is_none(),
        "Claude Code는 기본 자리(hooks/hooks.json)를 쓴다"
    );
    let wanted = serde_json::json!([
        {"name": "ponytail", "marketplace": "ponytail"},
        {"name": "mattpocock-skills", "marketplace": "claude-plugins-official"},
        {"name": "pstack", "marketplace": "cursor-plugins"},
    ]);
    assert_eq!(claude["dependencies"], wanted);
    let codex = json(&native.join(".codex-plugin/plugin.json"));
    assert_eq!(codex["hooks"], "./hooks/codex.json");
    let grok = json(&native.join("hosts/grok/plugin.json"));
    assert_eq!(sorted_keys(&grok), ["description", "name", "version"]);
    let agy = json(&native.join("hosts/agy/plugin.json"));
    assert_eq!(
        sorted_keys(&agy),
        ["description", "displayName", "name", "version"]
    );

    // hook 등록 파일 셋: 타임아웃과 매처가 7.1·7.2절의 표와 같다
    let timeouts = [
        ("SessionStart", 10),
        ("UserPromptSubmit", 5),
        ("PreToolUse", 8),
        ("Stop", 20),
        ("StopFailure", 15),
    ];
    let hooks = json(&native.join("hooks/hooks.json"));
    for (event, seconds) in timeouts {
        let handler = &hooks["hooks"][event][0]["hooks"][0];
        assert_eq!(handler["timeout"], seconds, "{event}");
        assert_eq!(handler["command"], "${CLAUDE_PLUGIN_ROOT}/bin/fullops");
        assert_eq!(
            handler["args"],
            serde_json::json!(["hook", "claude", "--plugin-root", "${CLAUDE_PLUGIN_ROOT}"])
        );
        assert_eq!(handler["onFailure"], "continue");
        assert!(handler.get("shell").is_none());
    }
    assert_eq!(
        hooks["hooks"]["PreToolUse"][0]["matcher"],
        "Bash|PowerShell|Write|Edit|MultiEdit|Agent|Task|Workflow"
    );
    assert!(hooks["hooks"]["SessionStart"][0].get("matcher").is_none());
    let codex_hooks = json(&native.join("hooks/codex.json"));
    assert_eq!(
        sorted_keys(&codex_hooks["hooks"]),
        ["PreToolUse", "SessionStart", "Stop", "UserPromptSubmit"]
    );
    assert_eq!(
        codex_hooks["hooks"]["PreToolUse"][0]["matcher"],
        "Bash|apply_patch|spawn_agent"
    );
    let start = &codex_hooks["hooks"]["SessionStart"][0]["hooks"][0];
    assert_eq!(start["additionalContextLimit"], 20000);
    assert_eq!(start["timeout"], 10);
    let posix = start["command"].as_str().unwrap();
    assert!(posix.starts_with("exec sh -c '") && posix.contains("$HOME/.fullops/bin/fullops"));
    let windows = start["commandWindows"].as_str().unwrap();
    assert!(windows.contains(r"%USERPROFILE%\.fullops\bin\fullops.exe"));
    assert!(windows.contains(r"${PLUGIN_ROOT}\bin\fullops.exe") && !windows.contains("$HOME"));
    let agy_hooks = json(&native.join("hosts/agy/hooks.json"));
    assert_eq!(
        sorted_keys(&agy_hooks["fullops"]),
        ["PreToolUse", "SessionStart", "Stop"]
    );
    assert_eq!(
        agy_hooks["fullops"]["Stop"][0]["command"],
        "\"bin/fullops\" hook agy --event Stop --in-place"
    );
    assert_eq!(agy_hooks["fullops"]["Stop"][0]["timeout"], 20);
    assert_eq!(
        agy_hooks["fullops"]["PreToolUse"][0]["matcher"],
        "run_command|write_to_file|replace_file_content|multi_replace_file_content|invoke_subagent"
    );
    assert_eq!(
        agy_hooks["fullops"]["PreToolUse"][0]["hooks"][0]["command"],
        "\"bin/fullops\" hook agy --event PreToolUse --in-place"
    );

    // MCP 파일 셋: Context7 원격 HTTP
    let url = "https://mcp.context7.com/mcp";
    for file in [".mcp.json", "hosts/grok/.mcp.json"] {
        let server = &json(&native.join(file))["mcpServers"]["context7"];
        assert_eq!(server["url"], url, "{file}");
        assert!(server.get("command").is_none());
    }
    assert_eq!(
        json(&native.join("hosts/agy/mcp_config.json")),
        serde_json::json!({"mcpServers": {"context7": {"url": url}}})
    );

    // 바이너리와 런처, 의존 목록
    for name in binaries.iter().chain(&["fullops"]) {
        assert!(native.join("bin").join(name).is_file(), "bin/{name}");
    }
    let launcher = std::fs::read_to_string(native.join("bin/fullops")).unwrap();
    assert!(launcher.starts_with("#!/bin/sh\n") && !launcher.contains('\r'));
    assert_eq!(
        std::fs::read(native.join("deps.toml")).unwrap(),
        std::fs::read(repo_root().join("plugin/deps.toml")).unwrap()
    );
    assert!(native.join("INSTALL.md").is_file() && native.join("releases/2.0.0.md").is_file());

    // 디렉터리판: Codex 파일, hosts/, 의존 선언이 없고 hook이 --in-place다
    for gone in [".codex-plugin", "hooks/codex.json", "hosts"] {
        assert!(!directory.join(gone).exists(), "디렉터리판에 {gone}");
    }
    let dir_manifest = json(&directory.join(".claude-plugin/plugin.json"));
    assert!(dir_manifest.get("dependencies").is_none());
    assert!(dir_manifest["userConfig"].get("context7_api_key").is_some());
    let dir_hooks = json(&directory.join("hooks/hooks.json"));
    assert_eq!(
        dir_hooks["hooks"]["Stop"][0]["hooks"][0]["args"],
        serde_json::json!(["hook", "claude", "--in-place"])
    );
    let dir_mcp = json(&directory.join(".mcp.json"));
    assert_eq!(dir_mcp["mcpServers"]["context7"]["url"], url);

    for package in [&native, &directory] {
        let files = all_files(package);
        // 스킬 아홉 개가 하위 폴더째 있다
        for file in all_files(&repo_root().join("plugin/skills")) {
            assert!(files.contains(&format!("skills/{file}")), "skills/{file}");
        }
        assert!(files.contains(&"skills/fullops-motion/templates/README.md".to_string()));
        let skills: std::collections::BTreeSet<&str> = files
            .iter()
            .filter_map(|f| f.strip_prefix("skills/")?.split('/').next())
            .collect();
        assert_eq!(skills.len(), 9, "{skills:?}");
        // G4: 배포본의 스크립트는 render.mjs 하나뿐이다
        let scripts: Vec<&String> = files
            .iter()
            .filter(|f| {
                [".py", ".js", ".cjs", ".mjs"]
                    .iter()
                    .any(|ext| f.ends_with(ext))
            })
            .collect();
        assert_eq!(scripts, ["skills/fullops-motion/render.mjs"]);
        // SHA256SUMS가 모든 파일을 덮고 값이 맞다
        let sums = std::fs::read_to_string(package.join("SHA256SUMS")).unwrap();
        let listed: Vec<(&str, &str)> = sums.lines().filter_map(|l| l.split_once("  ")).collect();
        assert_eq!(listed.len(), files.len() - 1);
        for (hash, file) in listed {
            let bytes = std::fs::read(package.join(file)).unwrap();
            assert_eq!(hash, sha256(&bytes), "{file}");
        }
    }

    // 루트의 마켓플레이스 카탈로그 둘이 통합 패키지 폴더를 가리킨다
    let claude_market = json(&out_root.join(".claude-plugin/marketplace.json"));
    assert_eq!(claude_market["name"], "fullops");
    assert_eq!(claude_market["plugins"][0]["name"], "fullops");
    assert_eq!(
        claude_market["plugins"][0]["source"],
        "./dist/native/fullops"
    );
    let codex_market = json(&out_root.join(".agents/plugins/marketplace.json"));
    assert_eq!(
        codex_market["plugins"][0]["source"]["path"],
        "./dist/native/fullops"
    );
    assert!(!out_root.join(".grok-plugin").exists());
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 스킬 문서와 INSTALL.md: Python 표기가 없고, 가리키는 명령과 스킬 이름이 실제와 같다(설계 18.2절 F의 확인).
#[test]
fn skill_docs_name_real_commands_and_skills() {
    let sb = fullops_testkit::Sandbox::empty();
    let help = sb.cli(&sb.root, &["help"]).stdout;
    let commands: std::collections::BTreeSet<&str> = help
        .lines()
        .filter_map(|l| l.strip_prefix("  "))
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    assert!(commands.contains("dispatch") && commands.contains("install"));
    let skills_dir = repo_root().join("plugin/skills");
    let mut known: std::collections::BTreeSet<String> = std::fs::read_dir(&skills_dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(known.len(), 9, "{known:?}");
    // 의존 목록이 설치하는 스킬의 이름. OCR CLI와 그 delegate 스킬은 목록에 없다
    let deps = std::fs::read_to_string(repo_root().join("plugin/deps.toml")).unwrap();
    for name in [
        "caveman",
        "typesafe-ai",
        "diagram-design",
        "frontend-design",
        "ui-ux-pro-max",
        "impeccable",
        "web-design-guidelines",
    ] {
        assert!(deps.contains(&format!("names = [\"{name}\"]")), "{name}");
        known.insert(name.to_string());
    }
    for banned in ["open-code-review", "\"ocr\"", "npm "] {
        assert!(
            !deps.to_lowercase().contains(banned),
            "deps.toml에 {banned}"
        );
    }

    let mut docs: Vec<PathBuf> = all_files(&skills_dir)
        .into_iter()
        .filter(|f| f.ends_with("SKILL.md") && !f.starts_with("fullops-motion/"))
        .map(|f| skills_dir.join(f))
        .collect();
    assert_eq!(docs.len(), 8);
    docs.push(repo_root().join("plugin/INSTALL.md"));
    for doc in docs {
        let text = std::fs::read_to_string(&doc).unwrap();
        let name = doc.display();
        for banned in [
            "python3",
            " python ",
            ".py",
            "subagent_level",
            "squad/fullops.json",
        ] {
            assert!(!text.contains(banned), "{name}: `{banned}`");
        }
        // `fullops <명령>`은 실제 명령이어야 한다
        let mut words = text
            .split(|c: char| c.is_whitespace() || c == '`' || c == '(' || c == ')')
            .peekable();
        while let Some(word) = words.next() {
            let is_program =
                word == "fullops" || word.ends_with("/fullops") || word.ends_with("fullops.exe");
            let Some(next) = words.peek() else { break };
            let is_command = !next.is_empty() && next.chars().all(|c| c.is_ascii_lowercase());
            if is_program && is_command {
                assert!(
                    commands.contains(next) || *next == "hook",
                    "{name}: `fullops {next}`는 없는 명령이다"
                );
            }
        }
        // 가리키는 스킬 이름이 있다
        for word in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
            let looks_like_skill = (word.starts_with("fullops-") && word != "fullops-squad")
                || word == "setup-fullops"
                || word == "update-fullops";
            if looks_like_skill {
                assert!(known.contains(word), "{name}: 스킬 `{word}`가 없다");
            }
        }
        // Windows 경로 표기(설계 10.3절): 플러그인 폴더의 실행본을 적는 문서는 .exe 경로를 함께 적는다
        if text.contains("<플러그인 폴더>/bin/fullops") {
            assert!(text.contains(r"<플러그인 폴더>\bin\fullops.exe"), "{name}");
        }
        assert!(
            text.contains(r"%USERPROFILE%\.fullops\bin\fullops.exe"),
            "{name}: Windows의 고정 위치 실행본 경로가 없다"
        );
    }
}

fn toml_of(path: &Path) -> toml::Table {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// N3: Python판 레포를 전환했다가 되돌린다. 전환 직후와 되돌린 뒤의 레포를 본다.
#[test]
fn n3_migrate_round_trip() {
    let scenario = load("n3");
    guard("n3", &scenario);
    let mut driver = Driver::new(&scenario.setup);
    let lead = driver.sb.path("lead");
    let squad = lead.join(".fullops-squad");
    let mut migrated = false;
    for (index, step) in scenario.steps.iter().enumerate() {
        if step.pending.is_some() {
            continue;
        }
        let result = driver
            .step(step)
            .unwrap_or_else(|e| panic!("n3 {}단계: {e}", index + 1));
        if let Some(expect) = &step.expect {
            let diff = mismatches(expect, &result);
            assert!(
                diff.is_empty(),
                "n3 {}단계: {diff:?}\n{}",
                index + 1,
                result.output
            );
        }
        let args = step.list("args");
        if args == ["migrate", "--from-python", "--dry-run"] {
            // --dry-run은 아무것도 바꾸지 않는다
            assert!(squad.join("fullops.json").is_file() && !squad.join("fullops.toml").exists());
            assert!(result.output.contains("만들 fullops.toml"));
        }
        if args == ["migrate", "--from-python"] && !migrated {
            migrated = true;
            // 표식이 바뀌었다
            assert!(!squad.join("fullops.json").exists());
            assert!(squad.join("fullops.json.py-1.24.0").is_file());
            let config = toml_of(&squad.join("fullops.toml"));
            assert_eq!(config["schema"].as_integer(), Some(2));
            assert_eq!(config["mode"].as_str(), Some("coor"));
            assert_eq!(config["test_level"].as_str(), Some("lite"));
            assert_eq!(config["parallel"].as_str(), Some("on"));
            assert_eq!(config["routing"].as_str(), Some("legacy"));
            assert_eq!(config["git"]["remote"].as_str(), Some("origin"));
            assert_eq!(
                config["roles"]["architecture"]["kind"].as_str(),
                Some("design")
            );
            assert_eq!(config["roles"]["dev"]["kind"].as_str(), Some("worker"));
            assert_eq!(
                config["roles"]["dev"]["about"].as_str(),
                Some("서버 코드 구현과 테스트")
            );
            assert_eq!(config["review"]["min_code_lines"].as_integer(), Some(300));
            assert_eq!(config["review"]["always"][0].as_str(), Some("db/**"));
            // 보고서: 옮긴 값, 읽지 못한 줄, 사람이 고친 delegation.md
            for line in [
                "subagent_level lite → parallel = \"on\"",
                "읽기 전용만 허용하려면 `read`, 끄려면 `off`로 고친다",
                "읽지 못한 표식 줄: - 형식이 다른 줄",
                "병렬 방침(8.15절)에 맞게 고친다",
                "열린 결정 1건, 보류 1건",
                "선행 과제 A-1는 끝난 것으로 넣었다",
            ] {
                assert!(
                    result.output.contains(line),
                    "보고서에 없다: {line}\n{}",
                    result.output
                );
            }
            assert!(!result.output.contains("이 줄은 라우팅 기준이 아니다"));
            let delegation = std::fs::read_to_string(squad.join("rules/delegation.md")).unwrap();
            assert!(delegation.contains("이 레포가 고쳐 쓴 규칙이다"));
            // 관리 블록 셋, .gitignore 줄, 없는 템플릿, 빈 인박스 파일
            let agents = std::fs::read_to_string(lead.join("AGENTS.md")).unwrap();
            assert!(agents.contains("<!-- fullops:start -->") && agents.contains("fullops status"));
            assert!(!agents.contains("fullops-squad:start") && agents.starts_with("# 안내"));
            for name in ["CLAUDE.md", "GEMINI.md"] {
                let text = std::fs::read_to_string(lead.join(name)).unwrap();
                assert!(text.contains("<!-- fullops:start -->"), "{name}");
            }
            let ignore = std::fs::read_to_string(squad.join(".gitignore")).unwrap();
            assert!(ignore.lines().any(|l| l == "docs/evaluations/jev/history/"));
            assert!(squad.join("FULLOPS.md").is_file() && squad.join("lint/lint.json").is_file());
            assert!(!squad.join("handovers/to_dev.md").exists());
            // 상태: 열린 결정과 보류만 가져왔고, 선행 과제는 끝난 것으로 들어갔다
            let state = json(&lead.join(".git/fullops/state.json"));
            let decisions = state["decisions"].as_array().unwrap();
            assert_eq!(decisions.len(), 1);
            assert_eq!(decisions[0]["id"], "D001");
            assert_eq!(decisions[0]["blocks"], serde_json::json!(["B-2"]));
            assert_eq!(decisions[0]["asked_at"], 1_793_502_000);
            assert_eq!(state["next_decision"], 2);
            assert_eq!(state["completions"][0]["key"], "C-3");
            assert_eq!(state["completions"][0]["hold"]["reason"], "검수 대기");
            let archive = std::fs::read_to_string(lead.join(".git/fullops/archive.jsonl")).unwrap();
            assert!(archive.contains("\"key\":\"A-1\"") && archive.contains("\"landed_at\""));
            // Python판 상태 폴더는 그대로이고 사본이 생겼다
            assert!(lead.join(".git/fullops-decisions/D001.json").is_file());
            assert!(
                lead.join(".git/fullops/py-backup/fullops-decisions/D002.json")
                    .is_file()
            );
        }
    }
    assert!(migrated);
    // 되돌린 뒤: Python판 표식과 블록, 빈 인박스 파일이 돌아왔고 새 표식은 이름이 바뀌어 남았다
    assert!(squad.join("fullops.json").is_file() && !squad.join("fullops.json.py-1.24.0").exists());
    assert!(
        !squad.join("fullops.toml").exists() && squad.join("fullops.toml.rolled-back").is_file()
    );
    let agents = std::fs::read_to_string(lead.join("AGENTS.md")).unwrap();
    assert!(
        agents.contains("<!-- fullops-squad:start -->")
            && !agents.contains("<!-- fullops:start -->")
    );
    assert!(!lead.join("CLAUDE.md").exists());
    assert_eq!(
        std::fs::read_to_string(squad.join("handovers/to_dev.md")).unwrap(),
        ""
    );
    let ignore = std::fs::read_to_string(squad.join(".gitignore")).unwrap();
    assert!(!ignore.contains("docs/evaluations/jev/history/"));
    assert!(lead.join(".git/fullops/state.json").is_file());
}

/// R8: `subagent_level`을 `parallel`로 옮긴다. 값이 없던 레포도 `on`이 되고 보고서가 그 사실을 낸다.
#[test]
fn r8_migrate_moves_subagent_level_to_parallel() {
    for (name, parallel) in [
        ("off", "off"),
        ("lite", "on"),
        ("full", "on"),
        ("missing", "on"),
    ] {
        let (driver, results) = play(&format!("r8-{name}"));
        let config = toml_of(&driver.sb.path("lead").join(".fullops-squad/fullops.toml"));
        assert_eq!(config["parallel"].as_str(), Some(parallel), "{name}");
        let report = &results.last().unwrap().output;
        let off_notice = report.contains("1.24.0에서는 하위 위임이 꺼져 있었다");
        assert_eq!(off_notice, name == "missing", "{name}: {report}");
        if parallel == "on" {
            assert!(report.contains("읽기 전용만 허용하려면 `read`, 끄려면 `off`로 고친다"));
        }
    }
}

/// `init`: 없는 파일만 만들고, 다시 실행하면 이어서 하며, `--parallel`과 원격 역할 브랜치를 다룬다.
#[test]
fn init_mode_and_upgrade() {
    let sb = fullops_testkit::Sandbox::empty();
    let repo = sb.init_repo("fresh");
    let origin = sb.root.join("origin.git");
    let origin_text = origin.to_string_lossy().into_owned();
    sb.git_ok(
        &sb.root,
        &["init", "-q", "--bare", "-b", "main", &origin_text],
    );
    sb.git_ok(&repo, &["remote", "add", "origin", &origin_text]);
    sb.git_ok(&repo, &["push", "-q", "-u", "origin", "main"]);
    let squad = repo.join(".fullops-squad");
    let init = [
        "init",
        "--roles",
        "coor,designer,dev",
        "--kind",
        "designer=design",
        "--kind",
        "coor=coordinator",
    ];

    // --dry-run은 쓰지 않는다
    let dry: Vec<&str> = init.iter().copied().chain(["--dry-run"]).collect();
    let planned = sb.cli(&repo, &dry);
    assert_eq!(planned.code, Some(0), "{}", planned.stderr);
    assert!(planned.stdout.contains("origin/fullops/dev") && !squad.exists());

    let made = sb.cli(&repo, &init);
    assert_eq!(made.code, Some(0), "{}", made.stderr);
    let config = toml_of(&squad.join("fullops.toml"));
    assert_eq!(config["parallel"].as_str(), Some("on"));
    assert_eq!(config["test_level"].as_str(), Some("lite"));
    assert_eq!(config["mode"].as_str(), Some("coor"));
    assert_eq!(config["roles"]["designer"]["kind"].as_str(), Some("design"));
    assert_eq!(
        config["roles"]["dev"]["branch"].as_str(),
        Some("fullops/dev")
    );
    for file in [
        "FULLOPS.md",
        "PLANS.md",
        "project.md",
        "orca-agents.md",
        ".gitignore",
        ".env.example",
        "handovers/_TEMPLATE.md",
        "handovers/logs/.gitkeep",
        "lint/lint.json",
        "lint/README.md",
        "review/rule.json",
        "rules/delegation.md",
        "rules/common/testing.md",
        "docs/deliverables/README.md",
        "docs/agents/document-writing.md",
    ] {
        assert!(squad.join(file).is_file(), "{file}");
    }
    let delegation = std::fs::read_to_string(squad.join("rules/delegation.md")).unwrap();
    assert!(delegation.contains("병렬 진행") && delegation.contains("--brief"));
    for name in ["AGENTS.md", "CLAUDE.md", "GEMINI.md"] {
        let text = std::fs::read_to_string(repo.join(name)).unwrap();
        assert!(
            text.contains("<!-- fullops:start -->") && text.contains("fullops status"),
            "{name}"
        );
    }
    // 원격 역할 브랜치가 생겼다. 커밋은 하지 않았다
    let heads = sb.git_ok(&repo, &["ls-remote", "--heads", "origin"]);
    for branch in ["fullops/coor", "fullops/designer", "fullops/dev"] {
        assert!(heads.contains(&format!("refs/heads/{branch}")), "{branch}");
    }
    assert!(
        sb.git_ok(&repo, &["status", "--porcelain"])
            .contains(".fullops-squad/")
    );
    assert_eq!(sb.git_ok(&repo, &["rev-list", "--count", "HEAD"]), "1");

    // 다시 실행: 사람이 고친 파일은 그대로이고 역할과 방침만 더한다
    sb.write(&squad.join("project.md"), "# 이 레포의 정본\n");
    let mut agents = std::fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    agents.insert_str(0, "# 우리 안내\n\n");
    sb.write(&repo.join("AGENTS.md"), &agents);
    let again = sb.cli(&repo, &["init", "--roles", "ops", "--parallel", "ultra"]);
    assert_eq!(again.code, Some(0), "{}", again.stderr);
    assert_eq!(
        std::fs::read_to_string(squad.join("project.md")).unwrap(),
        "# 이 레포의 정본\n"
    );
    assert_eq!(
        std::fs::read_to_string(repo.join("AGENTS.md")).unwrap(),
        agents
    );
    let config = toml_of(&squad.join("fullops.toml"));
    assert_eq!(config["parallel"].as_str(), Some("ultra"));
    assert_eq!(config["roles"]["ops"]["kind"].as_str(), Some("worker"));
    assert!(config["roles"].get("dev").is_some());
    let unknown = sb.cli(&repo, &["init", "--roles", "dev", "--parallel", "max"]);
    assert_eq!(unknown.code, Some(1));

    // doctor의 병렬 방침 행: ultra면 사용량과 권한 모드를 알린다
    let doctor = sb.fullops(&repo, &["doctor"], &[("PATH", "../bin")], b"");
    assert_eq!(doctor.code, Some(0), "{}", doctor.stderr);
    assert!(
        doctor
            .stdout
            .contains("울트라코드로 띄운다. 사용량이 빨리 준다")
    );
    assert!(
        doctor
            .stdout
            .contains("auto나 bypass가 아니면 Workflow 실행마다 승인 창에서 멈춘다")
    );

    // mode: 등록 역할만 주 담당이 된다. dev → coor는 없다
    assert_eq!(
        sb.cli(&repo, &["mode", "dev", "--primary-role", "nobody"])
            .code,
        Some(1)
    );
    assert_eq!(sb.cli(&repo, &["mode", "coor"]).code, Some(1));
    let switched = sb.cli(&repo, &["mode", "dev", "--primary-role", "dev"]);
    assert_eq!(switched.code, Some(0), "{}", switched.stderr);
    let config = toml_of(&squad.join("fullops.toml"));
    assert_eq!(config["mode"].as_str(), Some("dev"));
    assert_eq!(config["primary_role"].as_str(), Some("dev"));
    assert_eq!(config["primary_branch"].as_str(), Some("main"));
    assert_eq!(config["parallel"].as_str(), Some("ultra"));

    // upgrade: 적용 버전이 낮으면 릴리스 노트와 필수 적용을 내고, 통과한 --verify만 버전을 올린다
    let path = squad.join("fullops.toml");
    let lowered = std::fs::read_to_string(&path).unwrap().replace(
        "applied_version = \"2.0.0\"",
        "applied_version = \"1.24.0\"",
    );
    sb.write(&path, &lowered);
    std::fs::remove_file(repo.join("AGENTS.md")).unwrap();
    let notes = sb.cli(&repo, &["upgrade"]);
    assert_eq!(notes.code, Some(0), "{}", notes.stderr);
    assert!(notes.stdout.contains("릴리스 2.0.0") && notes.stdout.contains("[managed-block]"));
    assert_eq!(sb.cli(&repo, &["upgrade", "--verify"]).code, Some(2));
    assert_eq!(toml_of(&path)["applied_version"].as_str(), Some("1.24.0"));
    assert_eq!(sb.cli(&repo, &["init"]).code, Some(0));
    let verified = sb.cli(&repo, &["upgrade", "--verify"]);
    assert_eq!(
        verified.code,
        Some(0),
        "{}{}",
        verified.stdout,
        verified.stderr
    );
    assert_eq!(
        toml_of(&path)["applied_version"].as_str(),
        Some(env!("CARGO_PKG_VERSION"))
    );
}
