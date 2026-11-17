//! 활성 레포 찾기와 파일 자리(설계 6.1~6.3절, 7.1절 3번). git 프로세스를 띄우지 않는다.

use fullops_core::layout;
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

/// 옛 자리 레포인가(설계 13.6절): `.fullops-squad/fullops.toml`만 있고 `.fullops/fullops.toml`이 없다.
/// 그런 레포는 비활성이고 SessionStart와 CLI가 `fullops migrate --layout`을 안내한다. 이 판정은 여기 하나다.
pub fn legacy_marker(root: &Path) -> bool {
    let marker = |dir: &str| root.join(dir).join("fullops.toml").is_file();
    marker(layout::LEGACY_DIR) && !marker(layout::DIR)
}

/// `cwd`에서 위로 올라가며 찾은 Git 작업 트리의 루트가 옛 자리 레포면 그 루트.
pub fn legacy_root(cwd: &Path) -> Option<PathBuf> {
    let start = clean(&std::path::absolute(cwd).ok()?);
    let root = start.ancestors().find(|dir| dir.join(".git").exists())?;
    legacy_marker(root).then(|| root.to_path_buf())
}

/// `cwd`에서 위로 올라가며 `.git`을 찾고, 그 자리에 활성 표식(`.fullops/fullops.toml`)이 있으면 `Repo`를 돌려준다.
/// `None`은 Git 레포가 아니거나 표식이 없다는 뜻이다. 옛 자리의 표식은 보지 않는다.
pub fn discover(cwd: &Path) -> io::Result<Option<Repo>> {
    let start = clean(&std::path::absolute(cwd)?);
    let Some(root) = start.ancestors().find(|dir| dir.join(".git").exists()) else {
        return Ok(None);
    };
    if !root.join(layout::config_toml()).is_file() {
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
    /// `.fullops/` (커밋되는 파일, 설계 6.1절). 그 아래 자리는 `layout`이 정한다.
    pub fn dir(&self) -> PathBuf {
        self.root.join(layout::DIR)
    }
    /// 활성 표식이자 설정.
    pub fn config_path(&self) -> PathBuf {
        self.root.join(layout::config_toml())
    }
    pub fn plan_path(&self) -> PathBuf {
        self.root.join(layout::plans())
    }
    pub fn inbox_path(&self, role: &RoleId) -> PathBuf {
        self.root.join(layout::inbox(role))
    }
    /// `tasks/<KEY>/<역할>.r<차수>.md`.
    pub fn handover_log_path(&self, key: &TaskKey, role: &RoleId, round: u32) -> PathBuf {
        self.root.join(layout::handover_log(key, role, round))
    }
    pub fn route_path(&self, key: &TaskKey) -> PathBuf {
        self.root.join(layout::route(key))
    }
    /// `tasks/<KEY>/reviews/<head 12자>/`.
    pub fn review_dir(&self, key: &TaskKey, head: &Sha) -> PathBuf {
        self.root.join(layout::review_dir(key, head.short(12)))
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
    /// `explore/` (`find`·`context`의 결과와 덮어쓴 사본 `history/`, 설계 6.2절). 커밋되지 않는다.
    pub fn explore_dir(&self) -> PathBuf {
        self.shared().join("explore")
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
