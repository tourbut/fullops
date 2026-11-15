//! `cargo xtask orca-catalog`: Orca 패널(`panel/`)을 싣는 카탈로그 v2 `orca-catalog.json`을 만든다(install-and-retire.md 1.5절 (A)).
//! `orca plugin release-entry panel`이 내는 releases 항목을 같은 규칙으로 만든다. 릴리스 CI에는 orca CLI가 없기 때문이다.
//! 파일은 작업 트리가 아니라 커밋의 git 객체에서 읽는다. Orca도 그 커밋의 객체를 받아 해시를 다시 잰다.

use crate::dist::repo;
use fullops_core::model::sha256_hex;
use serde_json::{Map, Value, json};
use std::path::PathBuf;
use std::process::Command;

const USAGE: &str = "사용법: cargo xtask orca-catalog [--root <폴더>] [--commit <SHA>]";
/// 저장소 안의 패널 자리. 카탈로그의 `source.path`다.
const PANEL: &str = "panel";
/// Orca가 한 항목에 받는 releases의 최대 수(`PLUGIN_CATALOG_RELEASE_LIMIT`).
const RELEASE_LIMIT: usize = 32;

type Res<T = ()> = Result<T, String>;

fn git(args: &[&str]) -> Res<Vec<u8>> {
    let out = Command::new("git")
        .current_dir(repo())
        .args(args)
        .output()
        .map_err(|e| format!("git을 띄우지 못했습니다: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} 실패: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

/// 그 커밋의 `panel/` 아래 파일(패널 기준 상대 경로, 내용). 심볼릭 링크·서브모듈은 Orca가 거절하므로 여기서도 멈춘다.
fn panel_files(commit: &str) -> Res<Vec<(String, Vec<u8>)>> {
    let tree = format!("{commit}:{PANEL}");
    let listing = git(&["ls-tree", "-r", "-z", "--full-tree", &tree])?;
    let mut out = Vec::new();
    for line in listing.split(|b| *b == 0).filter(|l| !l.is_empty()) {
        let line = String::from_utf8_lossy(line);
        let (meta, path) = line
            .split_once('\t')
            .ok_or(format!("ls-tree 줄을 읽지 못했습니다: {line}"))?;
        let mut meta = meta.split(' ');
        let (mode, kind, oid) = (meta.next(), meta.next(), meta.next());
        if !matches!(mode, Some("100644" | "100755")) || kind != Some("blob") {
            return Err(format!(
                "{PANEL}/{path}: Orca 플러그인에는 일반 파일만 둡니다(심볼릭 링크·서브모듈 불가)"
            ));
        }
        out.push((
            path.to_string(),
            git(&["cat-file", "blob", oid.unwrap_or("")])?,
        ));
    }
    Ok(out)
}

/// Orca의 `hashPluginTree`: 폴더마다 이름순으로 내려가며 파일마다 `경로 길이·경로·크기·내용`을 잇는다(길이는 u64 빅엔디언).
fn tree_hash(files: &[(String, Vec<u8>)]) -> String {
    let mut sorted: Vec<&(String, Vec<u8>)> = files.iter().collect();
    // 경로 조각별 비교가 폴더마다 이름순 깊이 우선과 같다(`a.txt`보다 `a/` 안이 먼저)
    sorted.sort_by(|a, b| a.0.split('/').cmp(b.0.split('/')));
    let mut bytes = b"orca-plugin-tree-v1\0".to_vec();
    for (path, content) in sorted {
        bytes.extend((path.len() as u64).to_be_bytes());
        bytes.extend(path.as_bytes());
        bytes.extend((content.len() as u64).to_be_bytes());
        bytes.extend(content);
    }
    sha256_hex(&bytes)
}

/// Orca의 `pluginCatalogReleaseEntry`와 같은 항목. 패널이 쓰는 기여(`panels`)만 요약한다.
fn release_entry(manifest: &Map<String, Value>, commit: &str, content_hash: &str) -> Res<Value> {
    let text = |key: &str| {
        manifest
            .get(key)
            .and_then(Value::as_str)
            .ok_or(format!("{PANEL}/orca-plugin.json에 {key}가 없습니다"))
    };
    if manifest.get("manifestVersion") != Some(&json!(2)) {
        return Err("카탈로그에는 manifestVersion 2 매니페스트만 올릴 수 있습니다".into());
    }
    let engine = manifest
        .get("engines")
        .and_then(|e| e.get("orca"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let minimum = engine
        .strip_prefix(">=")
        .filter(|v| v.split('.').count() == 3 && v.split('.').all(|n| n.parse::<u32>().is_ok()))
        .ok_or(format!(
            "engines.orca는 `>=x.y.z` 꼴이어야 합니다: {engine:?}"
        ))?;
    let mut contributes = Vec::new();
    for (kind, items) in manifest
        .get("contributes")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        let items = items.as_array().map(Vec::as_slice).unwrap_or_default();
        if items.is_empty() {
            continue;
        }
        if kind != "panels" {
            // Orca는 요약이 매니페스트와 다르면 설치를 거절한다. 모르는 기여를 빼고 올리지 않는다
            return Err(format!(
                "기여 {kind}의 요약 규칙이 아직 없습니다. xtask/src/orca_catalog.rs에 Orca의 pluginContributionSummary 규칙을 더합니다"
            ));
        }
        for panel in items {
            let title = panel.get("title").and_then(Value::as_str).unwrap_or("");
            contributes.push(json!({ "kind": "panels", "name": title }));
        }
    }
    let mut capabilities: Vec<&str> = Vec::new();
    for kind in manifest
        .get("capabilities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.get("kind").and_then(Value::as_str))
    {
        if !capabilities.contains(&kind) {
            capabilities.push(kind);
        }
    }
    let mut entry = json!({
        "version": text("version")?,
        "commit": commit,
        "contentHash": content_hash,
        "execution": text("execution")?,
        "minimumOrca": minimum,
        "name": text("name")?,
        "contributes": contributes,
    });
    if !capabilities.is_empty() {
        entry["capabilities"] = json!(capabilities);
    }
    Ok(entry)
}

/// 새 항목을 맨 앞에 둔다. 내용이 같으면 그대로 두고, 내용이 바뀌었는데 버전이 같으면 멈춘다.
fn merge(mut releases: Vec<Value>, entry: Value) -> Res<Vec<Value>> {
    if releases.first().map(|r| &r["contentHash"]) == Some(&entry["contentHash"]) {
        return Ok(releases);
    }
    if releases.iter().any(|r| r["version"] == entry["version"]) {
        return Err(format!(
            "패널 내용이 바뀌었는데 버전 {}이 이미 카탈로그에 있습니다. {PANEL}/orca-plugin.json의 version을 올립니다",
            entry["version"]
        ));
    }
    releases.insert(0, entry);
    releases.truncate(RELEASE_LIMIT);
    Ok(releases)
}

/// (루트, 커밋). 루트의 기본이 `xtask dist`처럼 `target/dist`인 것은 저장소 루트의 카탈로그를 릴리스 커밋(`--root .`)만 바꾸게 하려는 것이다.
fn parse(args: &[String]) -> Res<(PathBuf, Option<String>)> {
    let mut root = repo().join("target").join("dist");
    let mut commit = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or(format!("{arg}에 값이 없습니다.\n{USAGE}"))
        };
        match arg.as_str() {
            "--root" => root = PathBuf::from(value()?),
            "--commit" => commit = Some(value()?),
            _ => return Err(format!("모르는 인자입니다: {arg}\n{USAGE}")),
        }
    }
    Ok((root, commit))
}

pub fn run(args: &[String]) -> Result<(), String> {
    let (root, commit) = parse(args)?;
    let commit = match commit {
        Some(c) => c,
        None => String::from_utf8_lossy(&git(&["rev-parse", "HEAD"])?)
            .trim()
            .to_string(),
    };
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(format!("커밋은 소문자 40자 SHA-1이어야 합니다: {commit}"));
    }
    let files = panel_files(&commit)?;
    let manifest_bytes = files
        .iter()
        .find(|(path, _)| path == "orca-plugin.json")
        .map(|(_, bytes)| bytes.as_slice())
        .ok_or(format!("{commit}의 {PANEL}/에 orca-plugin.json이 없습니다"))?;
    let manifest: Map<String, Value> =
        serde_json::from_slice(manifest_bytes).map_err(|e| format!("orca-plugin.json: {e}"))?;
    let entry = release_entry(&manifest, &commit, &tree_hash(&files))?;

    let deps_path = repo().join("plugin/deps.toml");
    let deps_text =
        std::fs::read_to_string(&deps_path).map_err(|e| format!("{}: {e}", deps_path.display()))?;
    let deps = fullops_store::parse_deps(&deps_text).map_err(|e| e.to_string())?;
    let source = &deps.own.source;
    let owner = source.split('/').next().unwrap_or("").to_lowercase();
    let id = format!(
        "{owner}.{}",
        manifest.get("id").and_then(Value::as_str).unwrap_or("")
    );

    let path = root.join("orca-catalog.json");
    let previous: Vec<Value> = match std::fs::read_to_string(&path) {
        Ok(text) => {
            let old: Value =
                serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            old["plugins"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|p| p["id"] == json!(id))
                .and_then(|p| p["releases"].as_array().cloned())
                .unwrap_or_default()
        }
        Err(_) => Vec::new(),
    };
    let catalog = json!({
        "catalogVersion": 2,
        "name": deps.own.marketplace,
        "owner": owner,
        "plugins": [{
            "id": id,
            "source": {
                "kind": "git",
                "url": format!("https://github.com/{source}.git"),
                "ref": "main",
                "path": PANEL,
            },
            "description": manifest.get("description").cloned().unwrap_or_default(),
            "releases": merge(previous, entry)?,
        }],
    });
    let mut text = serde_json::to_string_pretty(&catalog).map_err(|e| e.to_string())?;
    text.push('\n');
    std::fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_hash_matches_orca() {
        // 기대값은 Orca 1.4.256의 hashPluginTree를 같은 파일들에 돌려 얻었다
        let files = vec![
            ("src/a.js".to_string(), b"a\n".to_vec()),
            ("orca-plugin.json".to_string(), b"{}".to_vec()),
            ("src.txt".to_string(), b"x".to_vec()),
        ];
        assert_eq!(
            tree_hash(&files),
            "1bce7ae0b964cc2a43e345fa90f512f84e6a9bb2a847e65eaa5b38e72a27fb70"
        );
    }

    #[test]
    fn entry_and_merge() {
        let manifest: Map<String, Value> = serde_json::from_str(
            &std::fs::read_to_string(repo().join("panel/orca-plugin.json")).unwrap(),
        )
        .unwrap();
        let commit = "0".repeat(40);
        let entry = release_entry(&manifest, &commit, &"a".repeat(64)).unwrap();
        assert_eq!(entry["execution"], "trusted-worker");
        assert_eq!(entry["minimumOrca"], "1.4.256");
        assert_eq!(
            entry["contributes"],
            json!([{ "kind": "panels", "name": "fullops" }])
        );
        assert_eq!(entry["capabilities"], json!(["workspace:read"]));

        let kept = merge(vec![entry.clone()], entry.clone()).unwrap();
        assert_eq!(kept.len(), 1);
        let mut changed = entry.clone();
        changed["contentHash"] = json!("b".repeat(64));
        assert!(merge(vec![entry.clone()], changed.clone()).is_err());
        changed["version"] = json!("2.0.1");
        let both = merge(vec![entry], changed).unwrap();
        assert_eq!(both[0]["version"], "2.0.1");
    }

    #[test]
    fn default_root_is_target_dist() {
        // 기본으로 돌려도 저장소 루트에 카탈로그가 생기지 않아야 개발 커밋에 실리지 않는다
        let (root, commit) = parse(&[]).unwrap();
        assert_eq!(root, repo().join("target").join("dist"));
        assert!(commit.is_none());
        let args = ["--root", ".", "--commit", "abc"].map(String::from);
        assert_eq!(
            parse(&args).unwrap(),
            (PathBuf::from("."), Some("abc".into()))
        );
    }

    #[test]
    fn workflows_and_docs_match_code() {
        let read = |path: &str| std::fs::read_to_string(repo().join(path)).unwrap();
        let release = read(".github/workflows/release.yml");
        let ci = read(".github/workflows/ci.yml");
        // 기본 루트는 target/dist라 릴리스는 저장소 루트를 직접 가리켜야 한다
        let runs: Vec<&str> = release
            .lines()
            .filter(|l| l.contains("cargo xtask dist") || l.contains("cargo xtask orca-catalog"))
            .collect();
        assert_eq!(runs.len(), 3, "{runs:?}");
        assert!(runs.iter().all(|l| l.contains("--root .")), "{runs:?}");
        assert!(release.contains("require('./package.json').version"));
        assert!(ci.contains("node --test npx/fullops.test.cjs"));

        let design = read("docs/design.md");
        let main = include_str!("main.rs");
        let usage = main.split("cargo xtask <").nth(1).unwrap();
        let listing = design.lines().find(|l| l.starts_with("xtask/ ")).unwrap();
        for cmd in usage[..usage.find('>').unwrap()].split('|') {
            assert!(
                listing.contains(cmd),
                "설계 10.1절의 xtask 줄에 {cmd}가 없다"
            );
        }
        assert_eq!(
            design.contains("CI는 Windows 러너에서"),
            ci.contains("xtask bench"),
            "설계의 bench CI 문장과 ci.yml이 다르다"
        );
        assert!(!read("docs/install-and-retire.md").contains("ref: <커밋>"));
    }
}
