//! git 호출(설계 9.1절). 얇은 프로세스 호출이고 뼈대(묶음 A)의 완성본이다. 모든 묶음이 이 구현에 기댄다.
//! 읽기는 `Known`, 쓰기는 `Result`다. 쓰기는 CLI만 한다. hook은 fetch하지 않는다.

use crate::ExtError;
use crate::proc::{ProcResult, ProcStatus, run};
use fullops_core::model::{FileStat, Known, Sha, TreeEntry};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `git status --porcelain`의 한 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    /// 인덱스 칸(X).
    pub index: char,
    /// 작업 트리 칸(Y).
    pub worktree: char,
    /// 레포 기준 상대 경로(`/` 구분).
    pub path: String,
}

/// `git worktree list --porcelain`의 한 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    /// detached면 `None`. `refs/heads/`를 뗀 이름이다.
    pub branch: Option<String>,
    pub head: Sha,
}

/// `git diff --name-status`의 한 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameStatus {
    /// `A`·`M`·`D`·`R`·`C`·`T` 등 첫 글자.
    pub status: char,
    pub path: String,
    /// 이름 변경·복사의 옛 경로.
    pub old_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMode {
    FfOnly,
    NoEdit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeOutcome {
    Done,
    Conflict,
}

pub trait Git {
    fn is_ancestor(&self, older: &str, newer: &str) -> Known<bool>;
    fn merge_base(&self, a: &str, b: &str) -> Known<Sha>;
    /// `rev-parse --verify`. 커밋으로 풀려면 `<rev>^{commit}`을 준다.
    fn rev_parse(&self, rev: &str) -> Known<Sha>;
    /// `diff --name-only`. 이름 변경은 삭제와 추가로 낸다.
    fn diff_names(&self, from: &str, to: &str) -> Known<Vec<String>>;
    /// `diff --numstat`. 이름 변경은 삭제와 추가로 낸다.
    fn numstat(&self, from: &str, to: &str) -> Known<Vec<FileStat>>;
    /// `status --porcelain --untracked-files=all`.
    fn status(&self, pathspec: Option<&str>) -> Known<Vec<StatusEntry>>;
    /// `(rev, path)` 묶음 조회(`cat-file --batch` 하나). 없는 blob은 `None`.
    fn show_blobs(&self, items: &[(String, String)]) -> Known<Vec<Option<Vec<u8>>>>;
    fn worktrees(&self) -> Known<Vec<Worktree>>;
    /// 탐색. `ls-tree -r -z` 한 번.
    fn ls_tree(&self, rev: &str) -> Known<Vec<TreeEntry>>;
    /// 탐색. 크기를 먼저 묻고(`cat-file --batch-check`) `cat-file --batch`로 읽는다. `(크기, 앞 max 바이트)`. blob이 아니거나 없으면 `None`.
    fn blob_prefixes(&self, oids: &[String], max: usize) -> Known<Vec<Option<(u64, Vec<u8>)>>>;
    /// 탐색. `grep -F -l -z --full-name`. 리터럴 가운데 하나라도 든 파일의 경로.
    fn grep_files(&self, rev: &str, literals: &[String]) -> Known<Vec<String>>;
    /// 바뀐 경로만 골라 `git commit --only`. 바뀐 것이 없으면 `None`.
    fn commit_only(&self, paths: &[String], message: &str) -> Result<Option<Sha>, ExtError>;
    fn merge(&self, cwd: &Path, rev: &str, mode: MergeMode) -> Result<MergeOutcome, ExtError>;
    /// force를 쓰지 않는다.
    fn push(&self, remote: &str, refspec: &str) -> Result<(), ExtError>;
    fn fetch(&self, remote: &str, branch: &str) -> Result<(), ExtError>;
    /// `diff --name-status -z`.
    fn name_status(&self, from: &str, to: &str) -> Known<Vec<NameStatus>>;
    /// `diff -U0`의 글. lint가 추가된 줄을 뽑는다.
    fn diff_u0(&self, from: &str, to: &str) -> Known<String>;
    fn mv(&self, from: &str, to: &str) -> Result<(), ExtError>;
    fn merge_abort(&self, cwd: &Path) -> Result<(), ExtError>;
    /// `git reset --keep`. M-07만 쓴다.
    fn reset_keep(&self, cwd: &Path, rev: &str) -> Result<(), ExtError>;
    /// 이전 값이 `old`일 때만 바꾼다.
    fn update_ref(&self, name: &str, new: &Sha, old: &Sha) -> Result<(), ExtError>;
    fn worktree_add_detached(&self, path: &Path, rev: &str) -> Result<(), ExtError>;
    /// force를 쓰지 않는다.
    fn worktree_remove(&self, path: &Path) -> Result<(), ExtError>;
}

/// git 실행 파일을 부르는 구현. `timeout`은 호출당 제한 시간이다.
pub struct GitCli {
    pub repo_root: PathBuf,
    pub timeout: Duration,
}

/// `fetch`·`push`·`merge`의 제한 시간.
const SLOW: Duration = Duration::from_secs(120);
/// `blob_prefixes`가 `cat-file --batch` 한 번에 읽는 blob 크기의 합.
const BATCH_BYTES: u64 = 16 * 1024 * 1024;

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// NUL로 끝나는 항목들.
fn records(bytes: &[u8]) -> impl Iterator<Item = String> + '_ {
    bytes.split(|b| *b == 0).filter(|r| !r.is_empty()).map(text)
}

fn fail(out: ProcResult) -> ExtError {
    match out.status {
        ProcStatus::NotFound => ExtError::NotFound,
        ProcStatus::TimedOut => ExtError::TimedOut,
        ProcStatus::Exited(code) => ExtError::Failed {
            code,
            stderr: out.stderr_text(),
        },
    }
}

impl GitCli {
    /// hook용. 호출당 2초.
    pub fn hook(repo_root: PathBuf) -> Self {
        Self {
            repo_root,
            timeout: Duration::from_secs(2),
        }
    }

    /// CLI용. 호출당 30초(`fetch`·`push`·`merge`는 120초).
    pub fn cli(repo_root: PathBuf) -> Self {
        Self {
            repo_root,
            timeout: Duration::from_secs(30),
        }
    }

    fn call(
        &self,
        cwd: &Path,
        args: &[&str],
        timeout: Duration,
        stdin: Option<&[u8]>,
        offline_guard: bool,
    ) -> ProcResult {
        let argv: Vec<String> = ["git", "-c", "core.quotepath=false"]
            .iter()
            .chain(args)
            .map(|a| a.to_string())
            .collect();
        let mut env = vec![
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("LC_ALL", "C"),
        ];
        // 오프라인에서는 탐색이 부르는 git이 Git 프로토콜로 나가지 않게 한다(설계 9.3절)
        if offline_guard && crate::jev::offline() {
            env.push(("GIT_ALLOW_PROTOCOL", ""));
        }
        run(&argv, cwd, timeout, &env, stdin)
    }

    fn read(&self, args: &[&str]) -> Known<Vec<u8>> {
        let out = self.call(&self.repo_root, args, self.timeout, None, false);
        if out.ok() {
            Known::Yes(out.stdout)
        } else {
            Known::Unknown("git")
        }
    }

    fn write(&self, cwd: &Path, args: &[&str], timeout: Duration) -> Result<ProcResult, ExtError> {
        let out = self.call(cwd, args, timeout, None, false);
        if out.ok() { Ok(out) } else { Err(fail(out)) }
    }

    fn slow(&self) -> Duration {
        self.timeout.max(SLOW)
    }

    /// `cat-file --batch`의 출력을 요청 수만큼 읽는다. `(종류, 크기, 내용)`이고 없는 객체는 `None`이다.
    fn batch(
        &self,
        names: &[String],
        offline_guard: bool,
    ) -> Known<Vec<Option<(String, Vec<u8>)>>> {
        if names.is_empty() {
            return Known::Yes(Vec::new());
        }
        if names.iter().any(|n| n.contains('\n')) {
            return Known::Unknown("git_name");
        }
        let stdin = names.join("\n") + "\n";
        let out = self.call(
            &self.repo_root,
            &["cat-file", "--batch"],
            self.timeout,
            Some(stdin.as_bytes()),
            offline_guard,
        );
        if !out.ok() {
            return Known::Unknown("git");
        }
        let mut rest = out.stdout.as_slice();
        let mut items = Vec::with_capacity(names.len());
        for _ in names {
            let Some(end) = rest.iter().position(|b| *b == b'\n') else {
                return Known::Unknown("git_format");
            };
            let header = text(&rest[..end]);
            rest = &rest[end + 1..];
            let mut fields = header.rsplitn(3, ' ');
            let (last, kind) = (fields.next(), fields.next());
            let size = match (last, kind) {
                (Some("missing"), _) | (Some("ambiguous"), _) => {
                    items.push(None);
                    continue;
                }
                (Some(size), Some(_)) => size.parse::<usize>().ok(),
                _ => None,
            };
            let Some(size) = size.filter(|size| rest.len() > *size) else {
                return Known::Unknown("git_format");
            };
            items.push(Some((
                kind.unwrap_or_default().to_string(),
                rest[..size].to_vec(),
            )));
            rest = &rest[size + 1..];
        }
        Known::Yes(items)
    }

    /// `cat-file --batch-check`로 크기만 읽는다. blob이 아니거나 없으면 `None`이다.
    fn blob_sizes(&self, oids: &[String]) -> Known<Vec<Option<u64>>> {
        if oids.is_empty() {
            return Known::Yes(Vec::new());
        }
        if oids.iter().any(|n| n.contains('\n')) {
            return Known::Unknown("git_name");
        }
        let stdin = oids.join("\n") + "\n";
        let out = self.call(
            &self.repo_root,
            &["cat-file", "--batch-check"],
            self.timeout,
            Some(stdin.as_bytes()),
            true,
        );
        if !out.ok() {
            return Known::Unknown("git");
        }
        let sizes: Vec<Option<u64>> = text(&out.stdout)
            .lines()
            .map(|line| {
                let mut fields = line.rsplitn(3, ' ');
                let (size, kind) = (fields.next(), fields.next());
                size.filter(|_| kind == Some("blob"))
                    .and_then(|size| size.parse().ok())
            })
            .collect();
        match sizes.len() == oids.len() {
            true => Known::Yes(sizes),
            false => Known::Unknown("git_format"),
        }
    }
}

impl Git for GitCli {
    fn is_ancestor(&self, older: &str, newer: &str) -> Known<bool> {
        let args = ["merge-base", "--is-ancestor", older, newer];
        match self
            .call(&self.repo_root, &args, self.timeout, None, false)
            .status
        {
            ProcStatus::Exited(0) => Known::Yes(true),
            ProcStatus::Exited(1) => Known::Yes(false),
            _ => Known::Unknown("git"),
        }
    }

    fn merge_base(&self, a: &str, b: &str) -> Known<Sha> {
        match self.read(&["merge-base", a, b]) {
            Known::Yes(out) => {
                Sha::new(text(&out).trim()).map_or(Known::Unknown("git_format"), Known::Yes)
            }
            Known::Unknown(why) => Known::Unknown(why),
        }
    }

    fn rev_parse(&self, rev: &str) -> Known<Sha> {
        match self.read(&["rev-parse", "--verify", "--quiet", "--end-of-options", rev]) {
            Known::Yes(out) => {
                Sha::new(text(&out).trim()).map_or(Known::Unknown("git_format"), Known::Yes)
            }
            Known::Unknown(why) => Known::Unknown(why),
        }
    }

    fn diff_names(&self, from: &str, to: &str) -> Known<Vec<String>> {
        self.read(&["diff", "--name-only", "--no-renames", "-z", from, to])
            .map(|out| records(&out).collect())
    }

    fn numstat(&self, from: &str, to: &str) -> Known<Vec<FileStat>> {
        self.read(&["diff", "--numstat", "--no-renames", "-z", from, to])
            .map(|out| {
                records(&out)
                    .filter_map(|record| {
                        let mut parts = record.splitn(3, '\t');
                        // 바이너리 파일은 줄 수가 `-`다
                        let mut count = || parts.next().map(|n| n.parse().unwrap_or(0));
                        let (added, deleted) = (count()?, count()?);
                        Some(FileStat {
                            added,
                            deleted,
                            path: parts.next()?.to_string(),
                        })
                    })
                    .collect()
            })
    }

    fn status(&self, pathspec: Option<&str>) -> Known<Vec<StatusEntry>> {
        let mut args = vec!["status", "--porcelain", "-z", "--untracked-files=all"];
        if let Some(pathspec) = pathspec {
            args.extend(["--", pathspec]);
        }
        self.read(&args).map(|out| {
            let mut entries = Vec::new();
            let mut items = records(&out);
            while let Some(item) = items.next() {
                let mut chars = item.chars();
                let (Some(index), Some(worktree)) = (chars.next(), chars.next()) else {
                    continue;
                };
                // 이름 변경·복사는 옛 경로가 다음 항목으로 온다
                if matches!(index, 'R' | 'C') || matches!(worktree, 'R' | 'C') {
                    items.next();
                }
                entries.push(StatusEntry {
                    index,
                    worktree,
                    path: item.get(3..).unwrap_or_default().to_string(),
                });
            }
            entries
        })
    }

    fn show_blobs(&self, items: &[(String, String)]) -> Known<Vec<Option<Vec<u8>>>> {
        let names: Vec<String> = items
            .iter()
            .map(|(rev, path)| format!("{rev}:{path}"))
            .collect();
        self.batch(&names, false).map(|found| {
            found
                .into_iter()
                .map(|item| {
                    item.filter(|(kind, _)| kind == "blob")
                        .map(|(_, bytes)| bytes)
                })
                .collect()
        })
    }

    fn worktrees(&self) -> Known<Vec<Worktree>> {
        self.read(&["worktree", "list", "--porcelain", "-z"])
            .map(|out| {
                let mut list = Vec::new();
                let (mut path, mut head, mut branch) = (None, None, None);
                // 항목은 NUL, 워크트리는 빈 항목으로 끝난다
                for field in out.split(|b| *b == 0).map(text).chain([String::new()]) {
                    if let Some(value) = field.strip_prefix("worktree ") {
                        path = Some(PathBuf::from(value));
                    } else if let Some(value) = field.strip_prefix("HEAD ") {
                        head = Sha::new(value).ok();
                    } else if let Some(value) = field.strip_prefix("branch ") {
                        branch = Some(
                            value
                                .strip_prefix("refs/heads/")
                                .unwrap_or(value)
                                .to_string(),
                        );
                    } else if field.is_empty() {
                        // bare 저장소 항목은 HEAD가 없어 건너뛴다
                        if let (Some(path), Some(head)) = (path.take(), head.take()) {
                            list.push(Worktree {
                                path,
                                branch: branch.take(),
                                head,
                            });
                        }
                        branch = None;
                    }
                }
                list
            })
    }

    fn ls_tree(&self, rev: &str) -> Known<Vec<TreeEntry>> {
        let out = self.call(
            &self.repo_root,
            &["ls-tree", "-r", "-z", rev],
            self.timeout,
            None,
            true,
        );
        if !out.ok() {
            return Known::Unknown("git");
        }
        Known::Yes(
            records(&out.stdout)
                .filter_map(|record| {
                    let (meta, path) = record.split_once('\t')?;
                    let mut meta = meta.split(' ');
                    Some(TreeEntry {
                        mode: meta.next()?.to_string(),
                        kind: meta.next()?.to_string(),
                        oid: meta.next()?.to_string(),
                        path: path.to_string(),
                    })
                })
                .collect(),
        )
    }

    fn blob_prefixes(&self, oids: &[String], max: usize) -> Known<Vec<Option<(u64, Vec<u8>)>>> {
        // 크기를 먼저 묻고 합이 BATCH_BYTES를 넘지 않는 묶음으로 나눠 읽는다. 한 번에 읽으면 대상 blob 전체가 메모리에 오른다
        // shortcut: 혼자 BATCH_BYTES를 넘는 blob은 여전히 통째로 읽는다, 그런 파일이 흔해지면 출력을 흘려 읽는 실행기로 바꾼다
        let sizes = match self.blob_sizes(oids) {
            Known::Yes(sizes) => sizes,
            Known::Unknown(why) => return Known::Unknown(why),
        };
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut total = 0;
        for (at, size) in sizes.iter().enumerate() {
            let Some(size) = size else { continue };
            if groups.is_empty() || total + size > BATCH_BYTES {
                groups.push(Vec::new());
                total = 0;
            }
            total += size;
            if let Some(group) = groups.last_mut() {
                group.push(at);
            }
        }
        let mut out: Vec<Option<(u64, Vec<u8>)>> = vec![None; oids.len()];
        for group in groups {
            let names: Vec<String> = group
                .iter()
                .filter_map(|at| oids.get(*at).cloned())
                .collect();
            let found = match self.batch(&names, true) {
                Known::Yes(found) => found,
                Known::Unknown(why) => return Known::Unknown(why),
            };
            for (at, item) in group.into_iter().zip(found) {
                if let (Some(slot), Some((_, mut bytes))) = (out.get_mut(at), item) {
                    let size = bytes.len() as u64;
                    bytes.truncate(max);
                    // 잘라 낸 뒤쪽의 자리도 돌려준다
                    bytes.shrink_to_fit();
                    *slot = Some((size, bytes));
                }
            }
        }
        Known::Yes(out)
    }

    fn grep_files(&self, rev: &str, literals: &[String]) -> Known<Vec<String>> {
        if literals.is_empty() {
            return Known::Yes(Vec::new());
        }
        let mut args = vec!["grep", "-F", "-l", "-z", "--full-name"];
        for literal in literals {
            args.extend(["-e", literal]);
        }
        args.push(rev);
        let out = self.call(&self.repo_root, &args, self.timeout, None, true);
        match out.status {
            // 출력은 `<rev>:<경로>`다
            ProcStatus::Exited(0) => Known::Yes(
                records(&out.stdout)
                    .map(|r| {
                        r.strip_prefix(rev)
                            .and_then(|r| r.strip_prefix(':'))
                            .unwrap_or(&r)
                            .to_string()
                    })
                    .collect(),
            ),
            ProcStatus::Exited(1) => Known::Yes(Vec::new()),
            _ => Known::Unknown("git"),
        }
    }

    fn commit_only(&self, paths: &[String], message: &str) -> Result<Option<Sha>, ExtError> {
        // 새 파일은 `commit --only`가 받지 않으므로 바뀐 경로만 골라 먼저 인덱스에 넣는다
        if paths.is_empty() {
            return Ok(None);
        }
        let mut args = vec![
            "status",
            "--porcelain",
            "-z",
            "--untracked-files=all",
            "--no-renames",
            "--",
        ];
        args.extend(paths.iter().map(String::as_str));
        let out = self.write(&self.repo_root, &args, self.timeout)?;
        let entries: Vec<String> = records(&out.stdout).collect();
        let path = |entry: &'_ String| entry.get(3..).map(str::to_string);
        let changed: Vec<String> = entries.iter().filter_map(path).collect();
        if changed.is_empty() {
            return Ok(None);
        }
        // 이미 인덱스에 넣은 삭제(git mv의 옛 경로)는 add가 받지 않으므로 작업 트리 칸이 빈 항목은 뺀다
        let unstaged: Vec<String> = entries
            .iter()
            .filter(|entry| entry.get(1..2) != Some(" "))
            .filter_map(path)
            .collect();
        let run = |head: &[&str], paths: &[String]| {
            let args: Vec<&str> = head
                .iter()
                .copied()
                .chain(paths.iter().map(String::as_str))
                .collect();
            self.write(&self.repo_root, &args, self.timeout)
        };
        if !unstaged.is_empty() {
            run(&["add", "-A", "--"], &unstaged)?;
        }
        run(
            &["commit", "--quiet", "--only", "-m", message, "--"],
            &changed,
        )?;
        match self.rev_parse("HEAD") {
            Known::Yes(sha) => Ok(Some(sha)),
            Known::Unknown(why) => Err(ExtError::Parse(why.to_string())),
        }
    }

    fn merge(&self, cwd: &Path, rev: &str, mode: MergeMode) -> Result<MergeOutcome, ExtError> {
        let flag = match mode {
            MergeMode::FfOnly => "--ff-only",
            MergeMode::NoEdit => "--no-edit",
        };
        let out = self.call(
            cwd,
            &["merge", "--quiet", flag, rev],
            self.slow(),
            None,
            false,
        );
        if out.ok() {
            return Ok(MergeOutcome::Done);
        }
        // 충돌이면 병합이 진행 중으로 남는다
        let merging = self.call(
            cwd,
            &["rev-parse", "--verify", "--quiet", "MERGE_HEAD"],
            self.timeout,
            None,
            false,
        );
        if merging.ok() {
            Ok(MergeOutcome::Conflict)
        } else {
            Err(fail(out))
        }
    }

    fn push(&self, remote: &str, refspec: &str) -> Result<(), ExtError> {
        self.write(
            &self.repo_root,
            &["push", "--quiet", remote, refspec],
            self.slow(),
        )
        .map(drop)
    }

    fn fetch(&self, remote: &str, branch: &str) -> Result<(), ExtError> {
        self.write(
            &self.repo_root,
            &["fetch", "--quiet", remote, branch],
            self.slow(),
        )
        .map(drop)
    }

    fn name_status(&self, from: &str, to: &str) -> Known<Vec<NameStatus>> {
        self.read(&["diff", "--name-status", "-z", from, to])
            .map(|out| {
                let mut list = Vec::new();
                let mut items = records(&out);
                while let Some(code) = items.next() {
                    let status = code.chars().next().unwrap_or('M');
                    let renamed = matches!(status, 'R' | 'C');
                    let old_path = if renamed { items.next() } else { None };
                    let Some(path) = items.next() else { break };
                    list.push(NameStatus {
                        status,
                        path,
                        old_path,
                    });
                }
                list
            })
    }

    fn diff_u0(&self, from: &str, to: &str) -> Known<String> {
        self.read(&["diff", "-U0", "--no-color", "--no-ext-diff", from, to])
            .map(|out| text(&out))
    }

    fn mv(&self, from: &str, to: &str) -> Result<(), ExtError> {
        // git mv는 대상 폴더를 만들어 주지 않는다
        if let Some(dir) = self.repo_root.join(to).parent() {
            std::fs::create_dir_all(dir).map_err(|e| ExtError::Failed {
                code: 1,
                stderr: e.to_string(),
            })?;
        }
        self.write(&self.repo_root, &["mv", from, to], self.timeout)
            .map(drop)
    }

    fn merge_abort(&self, cwd: &Path) -> Result<(), ExtError> {
        self.write(cwd, &["merge", "--abort"], self.timeout)
            .map(drop)
    }

    fn reset_keep(&self, cwd: &Path, rev: &str) -> Result<(), ExtError> {
        self.write(cwd, &["reset", "--quiet", "--keep", rev], self.timeout)
            .map(drop)
    }

    fn update_ref(&self, name: &str, new: &Sha, old: &Sha) -> Result<(), ExtError> {
        self.write(
            &self.repo_root,
            &["update-ref", name, new.as_str(), old.as_str()],
            self.timeout,
        )
        .map(drop)
    }

    fn worktree_add_detached(&self, path: &Path, rev: &str) -> Result<(), ExtError> {
        let path = path.to_string_lossy();
        self.write(
            &self.repo_root,
            &["worktree", "add", "--quiet", "--detach", &path, rev],
            self.timeout,
        )
        .map(drop)
    }

    fn worktree_remove(&self, path: &Path) -> Result<(), ExtError> {
        let path = path.to_string_lossy();
        self.write(
            &self.repo_root,
            &["worktree", "remove", &path],
            self.timeout,
        )
        .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fullops_testkit::Sandbox;

    // 묶음의 합이 BATCH_BYTES를 넘으면 나눠 읽고, 결과는 요청한 순서와 자리 그대로다
    #[test]
    fn blob_prefixes_reads_in_bounded_groups() {
        let sb = Sandbox::empty();
        let repo = sb.init_repo("repo");
        let mut oids = Vec::new();
        for (name, fill) in [("a.bin", b'a'), ("b.bin", b'b'), ("c.bin", b'c')] {
            std::fs::write(repo.join(name), vec![fill; 6 * 1024 * 1024]).unwrap();
            oids.push(sb.git_ok(&repo, &["hash-object", "-w", name]));
        }
        oids.insert(1, "0".repeat(40));
        let found = GitCli::cli(repo).blob_prefixes(&oids, 3).yes().unwrap();
        let size = 6 * 1024 * 1024;
        assert_eq!(
            found,
            [
                Some((size, b"aaa".to_vec())),
                None,
                Some((size, b"bbb".to_vec())),
                Some((size, b"ccc".to_vec()))
            ]
        );
    }
}
