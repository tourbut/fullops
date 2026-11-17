//! 탐색 색인과 탐색 결과 파일(설계 6.1.5·6.3·6.5절). 묶음 J가 맡는다.

use crate::files::{now, read_json, write_json};
use crate::repo::Repo;
use fullops_core::explore::{IndexEntry, PARSER, Scope, SearchIndex, index_entry};
use fullops_core::model::{EXPLORE_V, Known, TreeEntry, sha256_hex};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

/// 탐색 결과를 `<git-common-dir>/fullops/explore/<file_name>`에 쓰고 그 경로를 돌려준다.
/// `keep_history`면 있던 파일을 `history/<이름>.<있던 파일의 created_at, 없으면 지금>.json`으로 옮긴다
/// (같은 이름이 있으면 끝에 `-<n>`). 덮어쓰는 모든 경우(`--force`, 읽히지 않는 옛 파일)에 참으로 부른다.
pub fn write_explore_result<T: Serialize>(
    repo: &Repo,
    file_name: &str,
    value: &T,
    keep_history: bool,
) -> io::Result<PathBuf> {
    let path = repo.explore_dir().join(file_name);
    if keep_history && path.exists() {
        let created = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|old| old["created_at"].as_i64())
            .unwrap_or_else(now);
        let stem = file_name.strip_suffix(".json").unwrap_or(file_name);
        let dir = repo.explore_dir().join("history");
        std::fs::create_dir_all(&dir)?;
        let mut target = dir.join(format!("{stem}.{created}.json"));
        let mut n = 2;
        while target.exists() {
            target = dir.join(format!("{stem}.{created}-{n}.json"));
            n += 1;
        }
        std::fs::rename(&path, target)?;
    }
    write_json(&path, value)?;
    Ok(path)
}

/// `files_sha256`: `files`를 키 정렬한 JSON의 sha256.
fn files_sha256(files: &[IndexEntry]) -> String {
    let json = serde_json::to_value(files).map(|v| v.to_string());
    sha256_hex(json.unwrap_or_default().as_bytes())
}

/// 탐색 색인 `search/<scope>.json`을 `tree`(그 범위의 대상 blob)에 맞춰 고치고 항목 전체를 `tree`의 순서로 돌려준다(설계 6.3·6.5절).
/// `read`는 blob OID 묶음의 (크기, 앞부분)을 읽는다. 색인에 같은 `(path, blob)`이 있는 항목은 읽지 않고, 읽을 것이 없으면 부르지 않는다.
/// 잠금(`search/lock`)은 한 번만 시도한다. 못 얻으면 색인 파일을 읽지도 쓰지도 않고 전부 다시 읽어 돌려준다.
/// 색인이 깨졌거나 `parser`·`config_sha256`이 다르면 전부 다시 만든다. 쓰지 못해도 결과는 돌려준다.
pub fn update_index(
    repo: &Repo,
    scope: Scope,
    config_sha256: &str,
    tree: &[TreeEntry],
    read: &mut dyn FnMut(&[String]) -> Known<Vec<Option<(u64, Vec<u8>)>>>,
) -> Vec<IndexEntry> {
    let dir = repo.search_dir();
    let path = dir.join(format!("{scope}.json"));
    let guard = crate::lock::acquire(&dir.join("lock"), Duration::ZERO).ok();
    let mut old: BTreeMap<(String, String), IndexEntry> = guard
        .as_ref()
        .and_then(|_| read_json::<SearchIndex>(&path).yes().flatten())
        .filter(|index| {
            index.v == EXPLORE_V
                && index.parser == PARSER
                && index.config_sha256 == config_sha256
                && index.files_sha256 == files_sha256(&index.files)
        })
        .map(|index| index.files)
        .unwrap_or_default()
        .into_iter()
        // 읽지 못했던 항목은 다시 읽는다
        .filter(|entry| entry.read_error.is_none())
        .map(|entry| ((entry.path.clone(), entry.blob.clone()), entry))
        .collect();
    let key = |e: &TreeEntry| (e.path.clone(), e.oid.clone());
    let missing: Vec<String> = tree
        .iter()
        .filter(|e| !old.contains_key(&key(e)))
        .map(|e| e.oid.clone())
        .collect();
    let fetched = match missing.is_empty() {
        true => Known::Yes(Vec::new()),
        false => read(&missing),
    };
    let complete = matches!(fetched, Known::Yes(_));
    let mut fetched = fetched.yes().unwrap_or_default().into_iter();
    let files: Vec<IndexEntry> = tree
        .iter()
        .map(|e| match old.remove(&key(e)) {
            Some(entry) => entry,
            None => match fetched.next().flatten() {
                Some((size, prefix)) => index_entry(&e.path, &e.oid, &prefix, size),
                None => IndexEntry {
                    read_error: Some("unreadable".into()),
                    ..index_entry(&e.path, &e.oid, &[], 0)
                },
            },
        })
        .collect();
    // 바뀐 것이 있을 때만 쓴다: 새로 읽었거나, 색인에 남은 것(없어진 경로)이 있다
    if guard.is_some() && complete && (!missing.is_empty() || !old.is_empty()) {
        let index = SearchIndex {
            v: EXPLORE_V,
            parser: PARSER.to_string(),
            config_sha256: config_sha256.to_string(),
            files_sha256: files_sha256(&files),
            files,
        };
        let _ = write_json(&path, &index);
        return index.files;
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, oid: &str) -> TreeEntry {
        TreeEntry {
            mode: "100644".into(),
            kind: "blob".into(),
            oid: oid.into(),
            path: path.into(),
        }
    }

    // P11: 바뀐 파일만 다시 읽는다. 색인을 지우거나 설정이 달라지면 전부 다시 만든다. 잠금을 못 얻어도 결과는 같다
    #[test]
    fn index_rereads_only_changed_blobs() {
        let root = std::env::temp_dir().join(format!("fullops-search-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let repo = Repo {
            root: root.clone(),
            git_dir: root.join(".git"),
            common_dir: root.join(".git"),
        };
        let asked = std::cell::RefCell::new(Vec::<Vec<String>>::new());
        let mut read = |oids: &[String]| {
            asked.borrow_mut().push(oids.to_vec());
            let blob = |oid: &String| Some((10, format!("fn {oid}() {{\n").into_bytes()));
            Known::Yes(oids.iter().map(blob).collect())
        };
        let tree = vec![
            entry("a.rs", "a1"),
            entry("b.rs", "b1"),
            entry("c.rs", "c1"),
        ];
        let first = update_index(&repo, Scope::Code, "cfg", &tree, &mut read);
        assert_eq!(first.len(), 3);
        assert_eq!(first[0].definitions, ["fn a1()"]);
        assert_eq!(asked.borrow().last().unwrap().len(), 3);

        // 파일 하나가 바뀌고 하나가 없어졌다: 바뀐 blob 하나만 읽는다
        let tree = vec![entry("a.rs", "a1"), entry("b.rs", "b2")];
        let second = update_index(&repo, Scope::Code, "cfg", &tree, &mut read);
        assert_eq!(asked.borrow().last().unwrap(), &["b2".to_string()]);
        assert_eq!(
            (second.len(), second[1].definitions[0].as_str()),
            (2, "fn b2()")
        );
        // 바뀐 것이 없으면 읽지 않는다
        let calls = asked.borrow().len();
        assert_eq!(
            update_index(&repo, Scope::Code, "cfg", &tree, &mut read),
            second
        );
        assert_eq!(asked.borrow().len(), calls);

        // 잠금을 다른 쪽이 쥐고 있으면 색인을 보지 않고 전부 읽는다. 결과는 같고 색인은 그대로다
        let index_file = repo.search_dir().join("code.json");
        let before = std::fs::read(&index_file).unwrap();
        let held = crate::lock::acquire(&repo.search_dir().join("lock"), Duration::ZERO).unwrap();
        assert_eq!(
            update_index(&repo, Scope::Code, "cfg", &tree, &mut read),
            second
        );
        assert_eq!(asked.borrow().last().unwrap().len(), 2);
        assert_eq!(std::fs::read(&index_file).unwrap(), before);
        drop(held);

        // 설정이 달라졌거나, 파일이 깨졌거나, 지워졌으면 전부 다시 만든다
        update_index(&repo, Scope::Code, "cfg2", &tree, &mut read);
        assert_eq!(asked.borrow().last().unwrap().len(), 2);
        std::fs::write(&index_file, b"{ broken").unwrap();
        update_index(&repo, Scope::Code, "cfg2", &tree, &mut read);
        assert_eq!(asked.borrow().last().unwrap().len(), 2);
        std::fs::remove_file(&index_file).unwrap();
        assert_eq!(
            update_index(&repo, Scope::Code, "cfg2", &tree, &mut read),
            second
        );
        assert_eq!(asked.borrow().last().unwrap().len(), 2);
        let saved: SearchIndex = read_json(&index_file).yes().flatten().unwrap();
        assert_eq!(saved.files_sha256, files_sha256(&saved.files));

        // 읽기가 통째로 실패하면 결과는 돌려주되 색인을 쓰지 않는다
        let tree = vec![entry("d.rs", "d1")];
        let failed = update_index(&repo, Scope::Code, "cfg2", &tree, &mut |_| {
            Known::Unknown("git")
        });
        assert!(failed[0].read_error.is_some() && !failed[0].searchable);
        let saved: SearchIndex = read_json(&index_file).yes().flatten().unwrap();
        assert_eq!(saved.files.len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }
}
