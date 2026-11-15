//! 활성 레포 찾기와 파일 자리(설계 6.1~6.3절, 7.1절 3번). git 프로세스를 띄우지 않는다.

use fullops_core::model::{HeadRef, RoleId, RunId, SessionId, Sha, TaskKey};
use std::io;
use std::path::{Component, Path, PathBuf};

/// 활성 레포의 한 체크아웃.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// 작업 트리 루트.
    pub root: PathBuf,
    /// 이 작업 트리의 Git 디렉터리. 주 체크아웃은 `<root>/.git`, 워크트리는 `<공용>/worktrees/<이름>`.
    pub git_dir: PathBuf,
    /// 모든 워크트리가 공유하는 Git 공용 디렉터리.
    pub common_dir: PathBuf,
}

/// `..`과 `.`을 파일 시스템을 보지 않고 푼다.
fn clean(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// `cwd`에서 위로 올라가며 `.git`을 찾고, 그 자리에 활성 표식이 있으면 `Repo`를 돌려준다.
/// `None`은 Git 레포가 아니거나 표식이 없다는 뜻이다.
pub fn discover(cwd: &Path) -> io::Result<Option<Repo>> {
    let start = clean(&std::path::absolute(cwd)?);
    let Some(root) = start.ancestors().find(|dir| dir.join(".git").exists()) else {
        return Ok(None);
    };
    if !root.join(".fullops-squad").join("fullops.toml").is_file() {
        return Ok(None);
    }
    let dot_git = root.join(".git");
    let git_dir = if dot_git.is_dir() {
        dot_git
    } else {
        let text = std::fs::read_to_string(&dot_git)?;
        let target = text
            .lines()
            .find_map(|line| line.strip_prefix("gitdir:"))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    ".git 파일에 gitdir 줄이 없습니다",
                )
            })?;
        clean(&root.join(target.trim()))
    };
    // 워크트리의 Git 디렉터리에는 공용 디렉터리를 가리키는 commondir 파일이 있다
    let common_dir = match std::fs::read_to_string(git_dir.join("commondir")) {
        Ok(text) => clean(&git_dir.join(text.trim())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => git_dir.clone(),
        Err(e) => return Err(e),
    };
    Ok(Some(Repo {
        root: root.to_path_buf(),
        git_dir,
        common_dir,
    }))
}

/// 현재 브랜치 또는 detached HEAD. `<git-dir>/HEAD` 파일을 읽는다.
pub fn head(repo: &Repo) -> io::Result<HeadRef> {
    let text = std::fs::read_to_string(repo.git_dir.join("HEAD"))?;
    let text = text.trim();
    if let Some(name) = text.strip_prefix("ref:") {
        let name = name.trim();
        return Ok(HeadRef::Branch(
            name.strip_prefix("refs/heads/").unwrap_or(name).to_string(),
        ));
    }
    Sha::new(text)
        .map(HeadRef::Detached)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

impl Repo {
    /// `.fullops-squad/` (커밋되는 파일, 설계 6.1절).
    pub fn squad(&self) -> PathBuf {
        self.root.join(".fullops-squad")
    }
    /// 활성 표식이자 설정.
    pub fn config_path(&self) -> PathBuf {
        self.squad().join("fullops.toml")
    }
    pub fn plan_path(&self) -> PathBuf {
        self.squad().join("PLANS.md")
    }
    pub fn inbox_path(&self, role: &RoleId) -> PathBuf {
        self.squad().join("handovers").join(format!("to_{role}.md"))
    }
    /// `handovers/logs/<KEY>.<역할>.r<차수>.md`.
    pub fn handover_log_path(&self, key: &TaskKey, role: &RoleId, round: u32) -> PathBuf {
        self.squad()
            .join("handovers")
            .join("logs")
            .join(format!("{key}.{role}.r{round}.md"))
    }
    pub fn route_path(&self, key: &TaskKey) -> PathBuf {
        self.squad().join("routes").join(format!("{key}.json"))
    }
    /// `reviews/<KEY>/<head 12자>/`.
    pub fn review_dir(&self, key: &TaskKey, head: &Sha) -> PathBuf {
        self.squad()
            .join("reviews")
            .join(key.as_str())
            .join(head.short(12))
    }

    /// `docs/evaluations/jev/` (탐색 결과와 패킷, 설계 6.1.5절).
    pub fn explore_dir(&self) -> PathBuf {
        self.squad().join("docs").join("evaluations").join("jev")
    }
    /// `<KEY>-packet.json`.
    pub fn packet_path(&self, key: &TaskKey) -> PathBuf {
        self.explore_dir().join(format!("{key}-packet.json"))
    }
    /// `<KEY>-packet-outcomes.json`.
    pub fn packet_outcomes_path(&self, key: &TaskKey) -> PathBuf {
        self.explore_dir()
            .join(format!("{key}-packet-outcomes.json"))
    }

    /// `<git-common-dir>/fullops/` (모든 워크트리가 공유, 설계 6.2절).
    pub fn shared(&self) -> PathBuf {
        self.common_dir.join("fullops")
    }
    pub fn state_path(&self) -> PathBuf {
        self.shared().join("state.json")
    }
    pub fn status_path(&self) -> PathBuf {
        self.shared().join("status.json")
    }
    pub fn archive_path(&self) -> PathBuf {
        self.shared().join("archive.jsonl")
    }
    pub fn turns_path(&self) -> PathBuf {
        self.shared().join("turns.jsonl")
    }
    pub fn waiter_path(&self, run: &RunId) -> PathBuf {
        self.shared()
            .join("waiters")
            .join(format!("{}.json", file_name(run.as_str())))
    }

    /// `<git-dir>/fullops/` (작업 트리별, 설계 6.3절).
    pub fn local(&self) -> PathBuf {
        self.git_dir.join("fullops")
    }
    pub fn session_path(&self, sid: &SessionId) -> PathBuf {
        self.local().join("sessions").join(format!("{sid}.json"))
    }
    /// 그 세션에서 이벤트 종류가 처음 실행됐다는 표식.
    pub fn seen_path(&self, sid: &SessionId, event: &str) -> PathBuf {
        self.local()
            .join("sessions")
            .join(format!("{sid}.seen"))
            .join(event)
    }
    pub fn current_path(&self) -> PathBuf {
        self.local().join("current.json")
    }
    pub fn trace_path(&self, sid: &SessionId) -> PathBuf {
        self.local().join("trace").join(format!("{sid}.jsonl"))
    }
    /// 탐색 색인 `code.json`, `documents.json`, `lock`이 있는 폴더(설계 6.3절).
    pub fn search_dir(&self) -> PathBuf {
        self.local().join("search")
    }
    /// `result.json`, `pass.json`, `logs/`, `lock`이 있는 폴더.
    pub fn lint_dir(&self) -> PathBuf {
        self.local().join("lint")
    }
}

/// Orca가 준 ID를 파일 이름으로 쓸 수 있게 `[A-Za-z0-9_.-]` 밖 글자를 `_`로 바꾼다.
fn file_name(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}
