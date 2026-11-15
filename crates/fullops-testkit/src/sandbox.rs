//! 임시 레포·원격(bare)·역할 워크트리·임시 홈과 격리된 환경에서의 프로세스 실행(설계 12.1절).
//! 실제 호스트 CLI, 네트워크, 사용자 홈은 건드리지 않는다.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

/// 끝난 프로세스 하나.
#[derive(Debug, Clone)]
pub struct Run {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    /// 프로세스 생성부터 종료까지의 벽시계.
    pub ms: u128,
}

/// 시나리오의 `[setup]`(설계 12.1절). 활성 레포 하나를 만든다.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Setup {
    /// `coor` 또는 `dev`.
    pub mode: String,
    /// 역할 ID → kind(`worker`·`design`·`tester`·`coordinator`). 역할마다 브랜치 `fullops/<id>`와 워크트리가 생긴다.
    pub roles: BTreeMap<String, String>,
    /// 원격(bare 레포 `origin`)을 만들고 기본 브랜치와 역할 브랜치를 push한다.
    pub remote: bool,
    /// dev 모드의 주 담당 역할. `primary_branch`는 `main`이다.
    pub primary_role: Option<String>,
    /// `legacy` 또는 `split`.
    pub routing: String,
    pub tech_plan_role: Option<String>,
    pub test_level: String,
    /// 병렬 방침. `off`·`read`·`on`·`ultra`.
    pub parallel: String,
    /// hook을 부를 호스트. `claude`·`codex`·`grok`·`agy`. 구동기가 그 호스트의 입력 모양과 등록 인자로 hook을 부른다.
    pub host: String,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            mode: "coor".into(),
            roles: BTreeMap::from([("dev".to_string(), "worker".to_string())]),
            remote: true,
            primary_role: None,
            routing: "legacy".into(),
            tech_plan_role: None,
            test_level: "lite".into(),
            parallel: "on".into(),
            host: "claude".into(),
        }
    }
}

impl Setup {
    /// 이 설정의 `fullops.toml` 글.
    pub fn config_toml(&self) -> String {
        let mut out = format!(
            "schema = 2\napplied_version = \"2.0.0\"\nmode = \"{}\"\n",
            self.mode
        );
        if let Some(role) = &self.primary_role {
            out.push_str(&format!(
                "primary_role = \"{role}\"\nprimary_branch = \"main\"\n"
            ));
        }
        out.push_str(&format!("routing = \"{}\"\n", self.routing));
        if let Some(role) = &self.tech_plan_role {
            out.push_str(&format!("tech_plan_role = \"{role}\"\n"));
        }
        out.push_str(&format!(
            "test_level = \"{}\"\nparallel = \"{}\"\n",
            self.test_level, self.parallel
        ));
        out.push_str("\n[git]\nbase = \"main\"\n");
        if self.remote {
            out.push_str("remote = \"origin\"\n");
        }
        for (id, kind) in &self.roles {
            out.push_str(&format!(
                "\n[roles.{id}]\nbranch = \"fullops/{id}\"\nkind = \"{kind}\"\n"
            ));
        }
        out
    }
}

/// 시험이 쓰는 최소 `PLANS.md`. 열과 절 제목은 1.24.0 템플릿과 같다.
pub const PLANS: &str = "# 현재 작업\n\n| 과제 키 | 목표 | 담당 | 상태 | 지시서·원천 링크 |\n|---|---|---|---|---|\n\n## 위임 범위\n\n";

/// 바이너리 폴더. `FULLOPS_TESTKIT_BIN_DIR`이 있으면 그 폴더이고, 없으면 실행 파일 위쪽의 프로필 폴더(`debug`·`release`)다.
/// 시험 실행 파일은 cargo 버전에 따라 `<프로필>/deps/`에도, `<프로필>/build/<패키지>/<해시>/out/`에도 놓인다.
fn bin_path(name: &str) -> PathBuf {
    let dir = match std::env::var_os("FULLOPS_TESTKIT_BIN_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => {
            let exe = std::env::current_exe().expect("시험 실행 파일 경로");
            let dir = exe.parent().expect("실행 파일의 폴더");
            dir.ancestors()
                .find(|a| a.file_name().is_some_and(|n| n == "debug" || n == "release"))
                .unwrap_or(dir)
                .to_path_buf()
        }
    };
    dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

/// 시험 실행 파일 옆(`target/<profile>/`)의 바이너리(`fullops`, `fake-orca`, `fake-host`, `fake-curl`).
/// 시험 프로세스마다 이름당 한 번 `cargo build`로 최신인지 맞춘다(패키지 하나만 시험할 때 낡은 바이너리로 돌지 않게 한다).
pub fn bin(name: &str) -> PathBuf {
    static BUILT: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    let path = bin_path(name);
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    if !built.iter().any(|b| b == name) {
        // shortcut: 기본 target 디렉터리와 debug·release만 다룬다, `--target`으로 시험할 일이 생기면 경로를 인자로 받는다
        let package = if name.starts_with("fake-") {
            "fullops-testkit"
        } else {
            name
        };
        let mut cargo = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
        cargo.args(["build", "--quiet", "-p", package, "--bin", name]);
        if path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|p| p == "release")
        {
            cargo.arg("--release");
        }
        let ok = cargo.status().is_ok_and(|s| s.success());
        assert!(
            ok && path.exists(),
            "{name} 바이너리를 만들지 못했습니다: {}",
            path.display()
        );
        built.push(name.to_string());
    }
    path
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// 임시 루트 하나. 버리면 지운다(`FULLOPS_TEST_KEEP=1`이면 남긴다).
pub struct Sandbox {
    /// 임시 루트. 심볼릭 링크와 Windows 접두어를 푼 경로다.
    pub root: PathBuf,
    /// 격리된 홈(`HOME`·`USERPROFILE`).
    pub home: PathBuf,
    /// fake-orca의 각본과 호출 기록 폴더(`FAKE_ORCA_DIR`).
    pub orca_dir: PathBuf,
    /// fake-host의 각본과 호출 기록 폴더(`FAKE_HOST_DIR`).
    pub host_dir: PathBuf,
    /// fake-curl의 각본과 호출 기록 폴더(`FAKE_CURL_DIR`).
    pub curl_dir: PathBuf,
    /// PATH 맨 앞에 놓이는 폴더. `install_fakes`가 가짜 실행 파일을 여기에 둔다.
    pub bin_dir: PathBuf,
    /// `with`로 만들었을 때의 설정.
    pub setup: Option<Setup>,
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if std::env::var_os("FULLOPS_TEST_KEEP").is_none() {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

impl Sandbox {
    /// 임시 루트와 홈만 만든다.
    pub fn empty() -> Self {
        let name = format!(
            "fullops-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let made = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&made);
        std::fs::create_dir_all(&made).expect("임시 폴더 만들기");
        // macOS의 /var → /private/var처럼 git이 내는 경로와 같아지게 푼다
        let root = std::fs::canonicalize(&made).expect("임시 폴더 경로");
        let root = PathBuf::from(
            root.to_string_lossy()
                .trim_start_matches(r"\\?\")
                .to_string(),
        );
        let sb = Self {
            home: root.join("home"),
            orca_dir: root.join("orca"),
            host_dir: root.join("host"),
            curl_dir: root.join("curl"),
            bin_dir: root.join("bin"),
            root,
            setup: None,
        };
        for dir in [
            &sb.home,
            &sb.orca_dir,
            &sb.host_dir,
            &sb.curl_dir,
            &sb.bin_dir,
        ] {
            std::fs::create_dir_all(dir).expect("임시 폴더 만들기");
        }
        sb.write(
            &sb.home.join(".gitconfig"),
            "[user]\n\tname = fullops-test\n\temail = test@example.invalid\n[init]\n\tdefaultBranch = main\n\
             [core]\n\tautocrlf = false\n[commit]\n\tgpgsign = false\n[protocol \"file\"]\n\tallow = always\n",
        );
        sb
    }

    /// 활성 레포를 만든다: 주 체크아웃 `lead`(브랜치 `main`), 원격 `origin.git`, 역할마다 워크트리 `<역할 ID>`.
    pub fn with(setup: &Setup) -> Self {
        let mut sb = Self::empty();
        let lead = sb.init_repo("lead");
        sb.write(
            &lead.join(".fullops-squad/fullops.toml"),
            &setup.config_toml(),
        );
        sb.write(&lead.join(".fullops-squad/PLANS.md"), PLANS);
        sb.git_ok(&lead, &["add", "-A"]);
        sb.git_ok(&lead, &["commit", "-q", "-m", "fullops 설정"]);
        if setup.remote {
            let origin = sb.root.join("origin.git");
            sb.git_ok(
                &sb.root,
                &[
                    "init",
                    "-q",
                    "--bare",
                    "-b",
                    "main",
                    &origin.to_string_lossy(),
                ],
            );
            sb.git_ok(
                &lead,
                &["remote", "add", "origin", &origin.to_string_lossy()],
            );
            sb.git_ok(&lead, &["push", "-q", "-u", "origin", "main"]);
        }
        for role in setup.roles.keys() {
            let branch = format!("fullops/{role}");
            sb.git_ok(
                &lead,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    &branch,
                    &sb.root.join(role).to_string_lossy(),
                ],
            );
            if setup.remote {
                sb.git_ok(&lead, &["push", "-q", "origin", &branch]);
            }
        }
        sb.setup = Some(setup.clone());
        sb
    }

    /// `lead`는 주 체크아웃, 역할 ID는 그 워크트리, 그 밖은 루트 기준 상대 경로다.
    pub fn path(&self, who: &str) -> PathBuf {
        self.root.join(who)
    }

    /// 활성 표식이 없는 Git 레포를 만들고 커밋 하나를 둔다.
    pub fn init_repo(&self, name: &str) -> PathBuf {
        let dir = self.root.join(name);
        std::fs::create_dir_all(&dir).expect("레포 폴더 만들기");
        self.git_ok(&dir, &["init", "-q", "-b", "main"]);
        // 격리 환경 밖에서 도는 git(시험 프로세스 안의 GitCli)도 같은 설정으로 커밋하게 레포 설정에도 적는다
        for (key, value) in [
            ("user.name", "fullops-test"),
            ("user.email", "test@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
        ] {
            self.git_ok(&dir, &["config", key, value]);
        }
        self.write(&dir.join("README.md"), "# 시험 레포\n");
        self.git_ok(&dir, &["add", "-A"]);
        self.git_ok(&dir, &["commit", "-q", "-m", "처음"]);
        dir
    }

    pub fn write(&self, path: &Path, text: &str) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("폴더 만들기");
        }
        std::fs::write(path, text).expect("파일 쓰기");
    }

    /// 격리된 환경의 프로세스. 사용자의 홈·git 설정·Orca·호스트 설정·API 키가 새어 들지 않는다.
    pub fn command(&self, program: impl AsRef<std::ffi::OsStr>, cwd: &Path) -> Command {
        let mut cmd = Command::new(program);
        // 실제 호스트 CLI·npx가 든 폴더는 뺀다. 시험이 FULLOPS_OFFLINE을 풀어도 이 PC의 호스트 설정에 닿지 않는다
        let real_host = |dir: &PathBuf| {
            ["claude", "codex", "grok", "agy", "npx"]
                .iter()
                .any(|name| {
                    ["", ".exe", ".cmd"]
                        .iter()
                        .any(|ext| dir.join(format!("{name}{ext}")).exists())
                })
        };
        let path = std::env::join_paths(
            std::iter::once(self.bin_dir.clone()).chain(
                std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                    .filter(|dir| !real_host(dir)),
            ),
        )
        .expect("PATH 만들기");
        cmd.current_dir(cwd)
            .env("PATH", path)
            .env("FAKE_HOST_DIR", &self.host_dir)
            .env("FAKE_CURL_DIR", &self.curl_dir)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            // doctor가 Orca의 Codex 홈·Claude 프로필을 이 아래에서 찾는다(이 PC의 Orca 폴더를 보지 않게 한다)
            .env("APPDATA", self.home.join("AppData").join("Roaming"))
            .env("GIT_CONFIG_GLOBAL", self.home.join(".gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("ORCA_CLI_COMMAND", bin_path("fake-orca"))
            .env("FAKE_ORCA_DIR", &self.orca_dir)
            .env("FULLOPS_OFFLINE", "1");
        for name in [
            "ORCA_TERMINAL_HANDLE",
            "ORCA_AGENT_SESSION_ID",
            "ORCA_DEV_REPO_ROOT",
            "GROK_HOOK_EVENT",
            "GROK_HOME",
            "CODEX_HOME",
            "CLAUDE_CONFIG_DIR",
            "TYPESAFE_API_KEY",
            "OPENROUTER_API_KEY",
            "FULLOPS_JEV_ENABLED",
            "FULLOPS_JEV_CACHE_BYPASS",
            "FULLOPS_TRACE",
        ] {
            cmd.env_remove(name);
        }
        cmd
    }

    /// stdin을 주고 끝까지 기다린다.
    pub fn run(&self, mut cmd: Command, stdin: &[u8]) -> Run {
        let started = Instant::now();
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // 부하가 큰 Windows에서 방금 만든 exe의 spawn이 드물게 '액세스 거부'(os error 5)로 실패한다. 잠깐 뒤 다시 띄운다
        let mut tries = 0;
        let mut child = loop {
            match cmd.spawn() {
                Ok(child) => break child,
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && tries < 5 => {
                    tries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                Err(e) => panic!("프로세스를 띄우지 못했습니다: {e} ({cmd:?})"),
            }
        };
        if let Some(mut pipe) = child.stdin.take() {
            // 읽지 않고 끝나는 프로세스도 있으므로 쓰기 실패는 버린다
            let _ = pipe.write_all(stdin);
        }
        let out = child.wait_with_output().expect("프로세스 기다리기");
        Run {
            code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            ms: started.elapsed().as_millis(),
        }
    }

    pub fn git(&self, cwd: &Path, args: &[&str]) -> Run {
        let mut cmd = self.command("git", cwd);
        cmd.args(args);
        self.run(cmd, b"")
    }

    /// 실패하면 panic 하고, 성공하면 앞뒤 공백을 뗀 stdout을 돌려준다.
    pub fn git_ok(&self, cwd: &Path, args: &[&str]) -> String {
        let run = self.git(cwd, args);
        assert_eq!(run.code, Some(0), "git {args:?} 실패: {}", run.stderr);
        run.stdout.trim().to_string()
    }

    /// 빌드한 `fullops <args…>`를 `cwd`에서 실행한다. `env`는 격리 환경에 덧붙인다.
    pub fn fullops(&self, cwd: &Path, args: &[&str], env: &[(&str, &str)], stdin: &[u8]) -> Run {
        let mut cmd = self.command(bin("fullops"), cwd);
        cmd.args(args).envs(env.iter().copied());
        // fullops는 PATH의 상대 항목을 보지 않는다. 시나리오가 적은 상대 PATH(`../bin`)는 cwd 기준 절대 경로로 넘긴다
        if let Some((_, path)) = env.iter().rev().find(|(name, _)| *name == "PATH") {
            let dirs = std::env::split_paths(path).map(|dir| cwd.join(dir));
            cmd.env("PATH", std::env::join_paths(dirs).expect("PATH 만들기"));
        }
        self.run(cmd, stdin)
    }

    /// `fullops hook <args…>`에 이벤트 JSON을 넣는다. `args`는 호스트 인자와 등록 문자열의 나머지 인자다(agy는 `--event <이름>`).
    pub fn hook(&self, args: &[&str], cwd: &Path, event: &serde_json::Value) -> Run {
        let args: Vec<&str> = std::iter::once("hook")
            .chain(args.iter().copied())
            .collect();
        self.fullops(cwd, &args, &[], event.to_string().as_bytes())
    }

    /// `fullops <args…>`를 CLI로 실행한다.
    pub fn cli(&self, cwd: &Path, args: &[&str]) -> Run {
        self.fullops(cwd, args, &[], b"")
    }

    fn add_rule(&self, dir: &Path, rule: serde_json::Value) {
        let path = dir.join("script.json");
        let mut script: serde_json::Value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_else(|| serde_json::json!({ "rules": [] }));
        if let Some(rules) = script["rules"].as_array_mut() {
            rules.push(rule);
        }
        self.write(&path, &script.to_string());
    }

    /// fake-orca 각본에 규칙을 덧붙인다(형식은 `fake.rs`).
    pub fn orca_rule(&self, rule: serde_json::Value) {
        bin("fake-orca");
        self.add_rule(&self.orca_dir, rule);
    }

    /// fake-orca의 우편함에 그 Run의 메시지 하나를 넣는다. `check`가 안 읽은 것을 내고 소비형이 읽음으로 바꾼다.
    pub fn orca_mail(&self, run: &str, message: serde_json::Value) {
        bin("fake-orca");
        let line = serde_json::json!({ "run": run, "message": message });
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.orca_dir.join("mail.jsonl"))
            .expect("우편함 열기");
        writeln!(file, "{line}").expect("우편함 쓰기");
    }

    /// fake-orca가 받은 호출 기록(`calls.jsonl`).
    pub fn orca_calls(&self) -> Vec<serde_json::Value> {
        crate::fake::calls(&self.orca_dir)
    }

    /// 가짜 실행 파일을 PATH 앞(`bin_dir`)에 그 이름으로 놓는다. `curl`은 fake-curl, 그 밖(`claude`·`codex`·`grok`·`agy`·`npx`)은 fake-host다.
    pub fn install_fakes(&self, names: &[&str]) {
        for name in names {
            let source = bin(if *name == "curl" {
                "fake-curl"
            } else {
                "fake-host"
            });
            let target = self
                .bin_dir
                .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            std::fs::copy(&source, &target).expect("가짜 실행 파일 복사");
        }
    }

    /// fake-host 각본에 규칙을 덧붙인다. `when`의 첫 낱말은 실행 파일 이름이다.
    pub fn host_rule(&self, rule: serde_json::Value) {
        self.add_rule(&self.host_dir, rule);
    }

    /// fake-host가 받은 호출 기록.
    pub fn host_calls(&self) -> Vec<serde_json::Value> {
        crate::fake::calls(&self.host_dir)
    }

    /// fake-curl 각본에 규칙을 덧붙인다.
    pub fn curl_rule(&self, rule: serde_json::Value) {
        self.add_rule(&self.curl_dir, rule);
    }

    /// fake-curl이 받은 호출 기록. `stdin`에 요청 글 전부가 있다.
    pub fn curl_calls(&self) -> Vec<serde_json::Value> {
        crate::fake::calls(&self.curl_dir)
    }
}
