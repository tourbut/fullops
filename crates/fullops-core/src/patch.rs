//! 편집 도구의 대상과 결과 계산(설계 7.4절). 호스트를 모른다: 어댑터가 옮긴 `ToolCall`만 본다.

use crate::model::{Known, ToolCall, ToolKind, path_text};
use std::path::{Path, PathBuf};

/// 편집 한 건. `after`가 `Yes(None)`이면 삭제다. `path`는 도구가 준 경로 그대로다(맞추기는 `repo_relative`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditTarget {
    pub path: PathBuf,
    pub before: Option<String>,
    pub after: Known<Option<String>>,
}

/// 경로 맞추기: `\`를 `/`로 바꾸고, 상대 경로면 `cwd`에 붙이고, 레포 루트 아래면 루트 기준 상대 경로를 돌려준다.
/// 루트 밖이면 `None`이다(그 대상에는 어떤 규칙도 걸리지 않는다). Windows에서는 대소문자를 가리지 않는다.
/// `real`은 존재하는 앞부분을 실제 경로로 푼다(`Facts::real_path`): 링크나 junction을 거친 경로도 실제 자리로 견준다.
/// `root`는 같은 방법으로 이미 푼 루트다.
pub fn repo_relative(
    path: &str,
    cwd: &Path,
    root: &Path,
    real: &mut dyn FnMut(&Path) -> PathBuf,
) -> Option<String> {
    let path = path.replace('\\', "/");
    let drive = path.as_bytes().get(1) == Some(&b':');
    let full = if path.starts_with('/') || drive {
        path
    } else {
        format!("{}/{path}", path_text(cwd))
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in full.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    // 앞의 `/`(UNC면 둘)는 남긴다
    let lead = full.len() - full.trim_start_matches('/').len();
    let lexical = format!("{}{}", "/".repeat(lead), parts.join("/"));
    let full = path_text(&real(Path::new(&lexical)));
    let parts: Vec<&str> = full.split('/').filter(|p| !p.is_empty()).collect();
    let root = path_text(root);
    let root: Vec<&str> = root.split('/').filter(|p| !p.is_empty()).collect();
    let same = |a: &str, b: &str| {
        if cfg!(windows) {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    };
    let under = parts.len() > root.len() && root.iter().zip(&parts).all(|(a, b)| same(a, b));
    under.then(|| parts.get(root.len()..).unwrap_or_default().join("/"))
}

/// `apply_patch` 본문의 파일 하나.
enum Section {
    Add(String, String),
    Delete(String),
    Update {
        path: String,
        move_to: Option<String>,
        hunks: Vec<Vec<(char, String)>>,
    },
}

/// 본문을 파일별로 나눈다. `*** Begin Patch`로 시작하지 않으면 빈 목록이다(문법은 docs/contracts/codex.md 3.1절).
fn sections(body: &str) -> Vec<Section> {
    let mut lines = body.lines().skip_while(|l| l.trim().is_empty());
    if lines.next().map(str::trim) != Some("*** Begin Patch") {
        return Vec::new();
    }
    let mut out: Vec<Section> = Vec::new();
    for line in lines {
        let mark = line.trim();
        if mark == "*** End Patch" {
            break;
        }
        if mark == "*** End of File" || mark.starts_with("*** Environment ID:") {
            continue;
        }
        if let Some(path) = mark.strip_prefix("*** Add File:") {
            out.push(Section::Add(path.trim().to_string(), String::new()));
        } else if let Some(path) = mark.strip_prefix("*** Delete File:") {
            out.push(Section::Delete(path.trim().to_string()));
        } else if let Some(path) = mark.strip_prefix("*** Update File:") {
            out.push(Section::Update {
                path: path.trim().to_string(),
                move_to: None,
                hunks: Vec::new(),
            });
        } else {
            match out.last_mut() {
                Some(Section::Add(_, text)) => {
                    if let Some(added) = line.strip_prefix('+') {
                        text.push_str(added);
                        text.push('\n');
                    }
                }
                Some(Section::Update { move_to, hunks, .. }) => {
                    if let Some(to) = mark.strip_prefix("*** Move to:") {
                        *move_to = Some(to.trim().to_string());
                    } else if mark == "@@" || mark.starts_with("@@ ") {
                        hunks.push(Vec::new());
                    } else {
                        // 빈 줄은 빈 문맥 줄이다. 첫 묶음은 머리줄 없이 시작할 수 있다
                        let (kind, text) = match line.chars().next() {
                            Some(k @ ('+' | '-' | ' ')) => (k, &line[1..]),
                            _ => (' ', line),
                        };
                        if hunks.is_empty() {
                            hunks.push(Vec::new());
                        }
                        if let Some(hunk) = hunks.last_mut() {
                            hunk.push((kind, text.to_string()));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// 묶음의 `-`·공백 줄을 현재 글에서 찾아 바꾼다. 못 찾으면 `None`이다.
fn apply_hunks(text: &str, hunks: &[Vec<(char, String)>]) -> Option<String> {
    let trailing = text.ends_with('\n');
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut cursor = 0;
    for hunk in hunks {
        let pick = |kinds: [char; 2]| -> Vec<&str> {
            hunk.iter()
                .filter(|(k, _)| kinds.contains(k))
                .map(|(_, t)| t.as_str())
                .collect()
        };
        let (old, new) = (pick([' ', '-']), pick([' ', '+']));
        let at = if old.is_empty() {
            lines.len()
        } else {
            let matches = |start: usize| {
                lines.get(start..start + old.len()).is_some_and(|window| {
                    window
                        .iter()
                        .zip(&old)
                        .all(|(a, b)| a.trim_end() == b.trim_end())
                })
            };
            (cursor..lines.len())
                .chain(0..cursor)
                .find(|start| matches(*start))?
        };
        let end = (at + old.len()).min(lines.len());
        lines.splice(at..end, new.iter().map(|l| l.to_string()));
        cursor = at + new.len();
    }
    let mut out = lines.join("\n");
    if trailing || text.is_empty() {
        out.push('\n');
    }
    Some(out)
}

/// 대상 하나를 만든다. 읽지 못한 파일과 잘린 입력은 판정하지 않는다(`after`가 `Unknown`).
fn target(
    tool: &ToolCall,
    read: &mut dyn FnMut(&Path) -> Known<Option<String>>,
    path: &str,
    after: &dyn Fn(Option<&str>) -> Known<Option<String>>,
) -> EditTarget {
    let path = PathBuf::from(path);
    let (before, after) = match read(&path) {
        Known::Yes(before) if tool.truncated => (before, Known::Unknown("truncated")),
        Known::Yes(before) => {
            let after = after(before.as_deref());
            (before, after)
        }
        Known::Unknown(why) => (None, Known::Unknown(why)),
    };
    EditTarget {
        path,
        before,
        after,
    }
}

fn edited(tool: &ToolCall, before: Option<&str>) -> Known<Option<String>> {
    let (Some(before), false) = (before, tool.edits.is_empty()) else {
        return Known::Unknown("no_edit");
    };
    let mut text = before.to_string();
    for edit in &tool.edits {
        if edit.old.is_empty() || !text.contains(&edit.old) {
            return Known::Unknown("no_match");
        }
        text = if edit.replace_all {
            text.replace(&edit.old, &edit.new)
        } else {
            text.replacen(&edit.old, &edit.new, 1)
        };
    }
    Known::Yes(Some(text))
}

/// 도구 호출이 바꾸는 파일과 편집 뒤의 글. `read`는 현재 파일 글을 읽는다(없으면 `Yes(None)`).
pub fn edit_targets(
    tool: &ToolCall,
    read: &mut dyn FnMut(&Path) -> Known<Option<String>>,
) -> Vec<EditTarget> {
    let mut out = Vec::new();
    match (tool.kind, &tool.file_path) {
        (ToolKind::Write, Some(path)) => {
            out.push(target(tool, read, path, &|_| match &tool.content {
                Some(content) => Known::Yes(Some(content.clone())),
                None => Known::Unknown("no_content"),
            }))
        }
        (ToolKind::Edit, Some(path)) => {
            out.push(target(tool, read, path, &|before| edited(tool, before)))
        }
        (ToolKind::Patch, _) => {
            for section in sections(tool.command.as_deref().unwrap_or_default()) {
                match section {
                    Section::Add(path, text) => out.push(target(tool, read, &path, &|_| {
                        Known::Yes(Some(text.clone()))
                    })),
                    Section::Delete(path) => {
                        out.push(target(tool, read, &path, &|_| Known::Yes(None)))
                    }
                    Section::Update {
                        path,
                        move_to,
                        hunks,
                    } => {
                        let patched = |before: Option<&str>| {
                            before
                                .and_then(|text| apply_hunks(text, &hunks))
                                .map_or(Known::Unknown("no_match"), |t| Known::Yes(Some(t)))
                        };
                        let Some(to) = move_to else {
                            out.push(target(tool, read, &path, &patched));
                            continue;
                        };
                        // 옛 경로는 삭제, 새 경로는 바뀐 글이다
                        let old = target(tool, read, &path, &|_| Known::Yes(None));
                        let moved = match &old.after {
                            Known::Yes(_) => patched(old.before.as_deref()),
                            Known::Unknown(why) => Known::Unknown(why),
                        };
                        out.push(old);
                        out.push(target(tool, read, &to, &|_| moved.clone()));
                    }
                }
            }
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EditOp;

    fn tool(kind: ToolKind) -> ToolCall {
        ToolCall {
            name: "t".into(),
            kind,
            command: None,
            file_path: None,
            content: None,
            edits: Vec::new(),
            text: None,
            truncated: false,
        }
    }

    fn files(path: &Path) -> Known<Option<String>> {
        Known::Yes(match path.to_string_lossy().as_ref() {
            "src/pay/retry.ts" => Some("const A = 1\nconst MAX = 3\nconst B = 2\n".into()),
            "a.md" => Some("one two one\n".into()),
            _ => None,
        })
    }

    #[test]
    fn applies_edits_and_patches() {
        let yes = |s: &str| Known::Yes(Some(s.to_string()));
        // Write와 Edit
        let write = ToolCall {
            file_path: Some("new.md".into()),
            content: Some("x".into()),
            ..tool(ToolKind::Write)
        };
        let got = edit_targets(&write, &mut files);
        assert_eq!(
            (got[0].before.clone(), got[0].after.clone()),
            (None, yes("x"))
        );
        let edit = |old: &str, all| ToolCall {
            file_path: Some("a.md".into()),
            edits: vec![EditOp {
                old: old.into(),
                new: "1".into(),
                replace_all: all,
            }],
            ..tool(ToolKind::Edit)
        };
        assert_eq!(
            edit_targets(&edit("one", false), &mut files)[0].after,
            yes("1 two one\n")
        );
        assert_eq!(
            edit_targets(&edit("one", true), &mut files)[0].after,
            yes("1 two 1\n")
        );
        assert_eq!(
            edit_targets(&edit("none", false), &mut files)[0].after,
            Known::Unknown("no_match")
        );
        // 잘린 입력은 판정하지 않는다
        let cut = ToolCall {
            truncated: true,
            ..write.clone()
        };
        assert_eq!(
            edit_targets(&cut, &mut files)[0].after,
            Known::Unknown("truncated")
        );

        // apply_patch: 계약 표본의 Update·Move·문맥 머리줄·End of File·Delete
        let body = "*** Begin Patch\n*** Environment ID: x\n*** Update File: src/pay/retry.ts\n*** Move to: src/pay/retry_policy.ts\n@@ export function retry()\n const A = 1\n-const MAX = 3\n+const MAX = 5\n const B = 2\n*** End of File\n*** Delete File: src/pay/old_retry.ts\n*** Add File: .fullops/inbox/to_dev.md\n+---\n+key: PAY-12\n+---\n*** End Patch";
        let patch = ToolCall {
            command: Some(body.into()),
            ..tool(ToolKind::Patch)
        };
        let shown: Vec<(String, Known<Option<String>>)> = edit_targets(&patch, &mut files)
            .iter()
            .map(|t| (t.path.to_string_lossy().into_owned(), t.after.clone()))
            .collect();
        assert_eq!(
            shown,
            vec![
                ("src/pay/retry.ts".to_string(), Known::Yes(None)),
                (
                    "src/pay/retry_policy.ts".to_string(),
                    yes("const A = 1\nconst MAX = 5\nconst B = 2\n")
                ),
                ("src/pay/old_retry.ts".to_string(), Known::Yes(None)),
                (
                    ".fullops/inbox/to_dev.md".to_string(),
                    yes("---\nkey: PAY-12\n---\n")
                ),
            ]
        );
        // 묶음을 못 찾으면 모름이고, 봉투가 없으면 대상이 없다
        let miss = ToolCall {
            command: Some(
                "*** Begin Patch\n*** Update File: a.md\n@@\n-없는 줄\n+x\n*** End Patch\n".into(),
            ),
            ..tool(ToolKind::Patch)
        };
        assert_eq!(
            edit_targets(&miss, &mut files)[0].after,
            Known::Unknown("no_match")
        );
        let bare = ToolCall {
            command: Some("*** Update File: a.md\n".into()),
            ..tool(ToolKind::Patch)
        };
        assert!(edit_targets(&bare, &mut files).is_empty());
    }

    #[test]
    fn aligns_paths_to_the_repo_root() {
        let (cwd, root) = (Path::new("/home/dev/repo/sub"), Path::new("/home/dev/repo"));
        let mut same = |p: &Path| p.to_path_buf();
        let mut rel = |p: &str| repo_relative(p, cwd, root, &mut same);
        assert_eq!(rel("a.md").as_deref(), Some("sub/a.md"));
        assert_eq!(rel("../.fullops/x.md").as_deref(), Some(".fullops/x.md"));
        assert_eq!(rel("/home/dev/repo/src\\a.rs").as_deref(), Some("src/a.rs"));
        assert_eq!(rel("/home/dev/other/a.rs"), None);
        assert_eq!(rel("/home/dev/repo"), None);
        let win = Path::new("C:\\project");
        assert_eq!(
            repo_relative(
                "C:\\project\\.fullops\\inbox\\to_dev.md",
                win,
                win,
                &mut |p| p.to_path_buf()
            )
            .as_deref(),
            Some(".fullops/inbox/to_dev.md")
        );
        if cfg!(windows) {
            assert_eq!(
                repo_relative("c:\\PROJECT\\a.rs", win, win, &mut |p| p.to_path_buf()).as_deref(),
                Some("a.rs")
            );
        }
        // 링크를 거친 경로는 실제 자리로 견준다
        let mut link =
            |p: &Path| PathBuf::from(path_text(p).replace("/repo/hv", "/repo/.fullops/inbox"));
        assert_eq!(
            repo_relative("../hv/to_dev.md", cwd, root, &mut link).as_deref(),
            Some(".fullops/inbox/to_dev.md")
        );
    }
}
