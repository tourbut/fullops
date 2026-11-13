//! 임시 레포·원격(bare)·역할 워크트리와 격리된 환경에서의 프로세스 실행.

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
    pub subagent_level: String,
    /// hook을 부를 호스트. `claude` 또는 `codex`.
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
            subagent_level: "lite".into(),
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
            "test_level = \"{}\"\nsubagent_level = \"{}\"\n",
            self.test_level, self.subagent_level
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

fn bin_path(name: &str) -> PathBuf {
    let exe = std::env::current_exe().expect("시험 실행 파일 경로");
    let profile = exe
        .parent()
        .and_then(Path::parent)
        .expect("target/<profile>/deps 아래에서 실행");
    profile.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

/// 시험 실행 파일 옆(`target/<profile>/`)의 바이너리(`fullops`, `fake-orca`). 없으면 한 번 빌드한다.
pub fn bin(name: &str) -> PathBuf {
    let path = bin_path(name);
    if !path.exists() {
        // shortcut: 기본 target 디렉터리와 debug·release만 다룬다, `--target`으로 시험할 일이 생기면 경로를 인자로 받는다
        let package = if name == "fake-orca" {
            "fullops-testkit"
        } else {
            name
        };
        let mut cargo = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
        cargo.args(["build", "-p", package, "--bin", name]);
        if path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|p| p == "release")
        {
            cargo.arg("--release");
        }
        let built = cargo.status().is_ok_and(|s| s.success());
        assert!(
            built && path.exists(),
            "{name} 바이너리를 만들지 못했습니다: {}",
            path.display()
        );
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
            root,
            setup: None,
        };
        std::fs::create_dir_all(&sb.home).expect("홈 만들기");
        std::fs::create_dir_all(&sb.orca_dir).expect("fake-orca 폴더 만들기");
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

    /// 격리된 환경의 프로세스. 사용자의 홈·git 설정·Orca·API 키가 새어 들지 않는다.
    pub fn command(&self, program: impl AsRef<std::ffi::OsStr>, cwd: &Path) -> Command {
        let mut cmd = Command::new(program);
        cmd.current_dir(cwd)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("GIT_CONFIG_GLOBAL", self.home.join(".gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("ORCA_CLI_COMMAND", bin_path("fake-orca"))
            .env("FAKE_ORCA_DIR", &self.orca_dir)
            .env("FULLOPS_OFFLINE", "1");
        for name in [
            "ORCA_TERMINAL_HANDLE",
            "ORCA_DEV_REPO_ROOT",
            "TYPESAFE_API_KEY",
            "OPENROUTER_API_KEY",
            "FULLOPS_TRACE",
        ] {
            cmd.env_remove(name);
        }
        cmd
    }

    /// stdin을 주고 끝까지 기다린다.
    pub fn run(&self, mut cmd: Command, stdin: &[u8]) -> Run {
        let started = Instant::now();
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("프로세스를 띄우지 못했습니다: {e} ({cmd:?})"));
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
        self.run(cmd, stdin)
    }

    /// `fullops hook <host>`에 이벤트 JSON을 넣는다.
    pub fn hook(&self, host: &str, cwd: &Path, event: &serde_json::Value) -> Run {
        self.fullops(cwd, &["hook", host], &[], event.to_string().as_bytes())
    }

    /// `fullops <args…>`를 CLI로 실행한다.
    pub fn cli(&self, cwd: &Path, args: &[&str]) -> Run {
        self.fullops(cwd, args, &[], b"")
    }

    /// fake-orca 각본에 규칙을 덧붙인다(형식은 `src/bin/fake-orca.rs`).
    pub fn orca_rule(&self, rule: serde_json::Value) {
        bin("fake-orca");
        let path = self.orca_dir.join("script.json");
        let mut script: serde_json::Value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_else(|| serde_json::json!({ "rules": [] }));
        if let Some(rules) = script["rules"].as_array_mut() {
            rules.push(rule);
        }
        self.write(&path, &script.to_string());
    }

    /// fake-orca가 받은 호출 기록(`calls.jsonl`).
    pub fn orca_calls(&self) -> Vec<serde_json::Value> {
        std::fs::read_to_string(self.orca_dir.join("calls.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }
}
