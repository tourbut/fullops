//! `cargo xtask dist`: 배포본 만들기(설계 10.4·10.5절). 묶음 F의 파일이다.
//! 통합 패키지(`dist/native/fullops`)와 디렉터리판(`dist/directory/fullops`), 파일별 `SHA256SUMS`, 루트의 마켓플레이스 카탈로그 둘을 만든다.
//! 매니페스트·hook 등록 파일·MCP 파일은 `plugin/`의 공통 값, `deps.toml`, 설계 7.1·7.2절의 표(`fullops_core::deps`)에서 만든다.

use fullops_core::deps::{HOOK_TIMEOUTS, package_items, pre_tool_matcher};
use fullops_core::model::{Deps, HostId, sha256_hex};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// `(타깃, 빌드 산출물 이름, 패키지 안의 이름)`.
const TARGETS: [(&str, &str, &str); 4] = [
    ("x86_64-pc-windows-msvc", "fullops.exe", "fullops.exe"),
    ("aarch64-apple-darwin", "fullops", "fullops-darwin-arm64"),
    ("x86_64-apple-darwin", "fullops", "fullops-darwin-x64"),
    ("x86_64-unknown-linux-gnu", "fullops", "fullops-linux-x64"),
];

const VERSION: &str = env!("CARGO_PKG_VERSION");
const USAGE: &str = "사용법: cargo xtask dist [--root <폴더>] [--target <타깃>]... [--prebuilt <폴더>] [--no-validate]";

type Res<T = ()> = Result<T, String>;

struct Args {
    /// 배포본을 둘 루트. 기본은 저장소 루트다(`<루트>/dist/…`와 `<루트>`의 카탈로그 둘).
    root: PathBuf,
    /// 싣는 타깃. 비어 있으면 넷 모두다.
    targets: Vec<String>,
    /// 타깃별로 이미 빌드한 바이너리를 모아 둔 폴더(CI). 패키지 안의 이름으로 들어 있다.
    prebuilt: Option<PathBuf>,
    validate: bool,
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

fn parse(args: &[String]) -> Res<Args> {
    let mut out = Args {
        root: repo(),
        targets: Vec::new(),
        prebuilt: None,
        validate: true,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .cloned()
                .ok_or(format!("{arg}에 값이 없습니다.\n{USAGE}"))
        };
        match arg.as_str() {
            "--root" => out.root = PathBuf::from(value()?),
            "--target" => out.targets.push(value()?),
            "--prebuilt" => out.prebuilt = Some(PathBuf::from(value()?)),
            "--no-validate" => out.validate = false,
            _ => return Err(format!("모르는 인자입니다: {arg}\n{USAGE}")),
        }
    }
    if let Some(unknown) = out
        .targets
        .iter()
        .find(|t| !TARGETS.iter().any(|(name, ..)| name == t))
    {
        return Err(format!("모르는 타깃입니다: {unknown}"));
    }
    Ok(out)
}

fn io<T>(result: std::io::Result<T>, what: &Path) -> Res<T> {
    result.map_err(|e| format!("{}: {e}", what.display()))
}

fn write(path: &Path, bytes: &[u8]) -> Res {
    if let Some(dir) = path.parent() {
        io(std::fs::create_dir_all(dir), dir)?;
    }
    io(std::fs::write(path, bytes), path)
}

fn write_json(path: &Path, value: &Value) -> Res {
    let mut text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    text.push('\n');
    write(path, text.as_bytes())
}

fn read_json(path: &Path) -> Res<Map<String, Value>> {
    let text = io(std::fs::read_to_string(path), path)?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn copy_tree(from: &Path, to: &Path) -> Res {
    if from.is_dir() {
        io(std::fs::create_dir_all(to), to)?;
        for entry in io(std::fs::read_dir(from), from)? {
            let entry = io(entry, from)?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    write(to, &io(std::fs::read(from), from)?)
}

/// 실행 비트. Windows에서 만든 배포본은 실행 비트가 없으므로 배포 커밋은 POSIX 러너에서 만든다(docs/build.md).
fn make_executable(path: &Path) -> Res {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        io(
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)),
            path,
        )?;
    }
    let _ = path;
    Ok(())
}

/// 폴더 안의 모든 파일(슬래시로 적은 상대 경로, 경로 순).
fn files(dir: &Path) -> Res<Vec<String>> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) -> Res {
        for entry in io(std::fs::read_dir(dir), dir)? {
            let path = io(entry, dir)?.path();
            if path.is_dir() {
                walk(&path, base, out)?;
            } else if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    out.sort();
    Ok(out)
}

/// 공통 값에 호스트별 덧붙임을 얹고 버전을 넣는다. `keep`이 있으면 그 필드만 남긴다(grok·agy의 스키마).
fn manifest(plugin: &Path, host: &str, keep: Option<&[&str]>) -> Res<Map<String, Value>> {
    let mut out = read_json(&plugin.join("plugin.json"))?;
    out.extend(read_json(
        &plugin.join("hosts").join(host).join("plugin.json"),
    )?);
    out.insert("version".into(), VERSION.into());
    if let Some(keep) = keep {
        out.retain(|key, _| keep.contains(&key.as_str()));
    }
    Ok(out)
}

fn timeout(event: &str) -> u32 {
    HOOK_TIMEOUTS
        .iter()
        .find(|(name, _)| *name == event)
        .map_or(10, |(_, seconds)| *seconds)
}

/// Claude Code·Codex 형식의 등록 파일: `{"hooks":{"<이벤트>":[{"matcher"?,"hooks":[handler]}]}}`.
fn hook_file(host: HostId, events: &[&str], handler: impl Fn(&str) -> Value) -> Value {
    let hooks: Map<String, Value> = events
        .iter()
        .map(|event| {
            let mut entry = json!({ "hooks": [handler(event)] });
            if *event == "PreToolUse" {
                entry["matcher"] = pre_tool_matcher(host).into();
            }
            (event.to_string(), json!([entry]))
        })
        .collect();
    json!({ "hooks": hooks })
}

/// Claude Code: exec form. 셸 없이 플러그인 폴더의 실행본을 직접 실행한다(설계 10.4절).
fn claude_hooks(in_place: bool) -> Value {
    let args = if in_place {
        json!(["hook", "claude", "--in-place"])
    } else {
        json!(["hook", "claude", "--plugin-root", "${CLAUDE_PLUGIN_ROOT}"])
    };
    let events: Vec<&str> = HOOK_TIMEOUTS.iter().map(|(event, _)| *event).collect();
    hook_file(HostId::Claude, &events, |event| {
        json!({
            "type": "command",
            "command": "${CLAUDE_PLUGIN_ROOT}/bin/fullops",
            "args": args,
            "onFailure": "continue",
            "timeout": timeout(event),
        })
    })
}

/// Codex: 고정 위치 실행본이 먼저, 없으면 플러그인 폴더의 실행본. StopFailure는 등록하지 않는다.
fn codex_hooks() -> Value {
    const POSIX: &str = r#"exec sh -c 'B="$HOME/.fullops/bin/fullops"; [ -x "$B" ] || B="$1/bin/fullops"; exec "$B" hook codex --plugin-root "$1"' sh "${PLUGIN_ROOT}""#;
    const WINDOWS: &str = r#"if exist "%USERPROFILE%\.fullops\bin\fullops.exe" ("%USERPROFILE%\.fullops\bin\fullops.exe" hook codex --plugin-root "${PLUGIN_ROOT}") else ("${PLUGIN_ROOT}\bin\fullops.exe" hook codex --plugin-root "${PLUGIN_ROOT}")"#;
    let events = ["SessionStart", "UserPromptSubmit", "PreToolUse", "Stop"];
    hook_file(HostId::Codex, &events, |event| {
        let mut handler = json!({
            "type": "command",
            "timeout": timeout(event),
            "command": POSIX,
            "commandWindows": WINDOWS,
        });
        if event == "SessionStart" {
            handler["additionalContextLimit"] = 20000.into();
        }
        handler
    })
}

/// agy: 패키지 루트의 `hooks.json`. 이벤트 이름을 인자로 주고 플러그인 폴더의 실행본을 그대로 쓴다.
fn agy_hooks() -> Value {
    let handler = |event: &str| {
        json!({
            "type": "command",
            "timeout": timeout(event),
            "command": format!("\"bin/fullops\" hook agy --event {event} --in-place"),
        })
    };
    json!({ "fullops": {
        "SessionStart": [handler("SessionStart")],
        "PreToolUse": [{ "matcher": pre_tool_matcher(HostId::Agy), "hooks": [handler("PreToolUse")] }],
        "Stop": [handler("Stop")],
    }})
}

/// MCP 등록. `typed`면 `"type":"http"`를 적고(Claude Code·Codex·grok), 아니면 `url`만 적는다(agy).
fn mcp(deps: &Deps, typed: bool, user_key: bool) -> Value {
    let servers: Map<String, Value> = deps
        .mcp
        .iter()
        .map(|server| {
            let mut entry = json!({ "url": server.url });
            if typed {
                entry["type"] = "http".into();
            }
            if user_key && server.name == "context7" {
                entry["headers"] =
                    json!({ "Authorization": "Bearer ${user_config.context7_api_key}" });
            }
            (server.name.clone(), entry)
        })
        .collect();
    json!({ "mcpServers": servers })
}

fn binaries(args: &Args, bin_dir: &Path) -> Res {
    for (target, built, packaged) in TARGETS {
        if !args.targets.is_empty() && !args.targets.iter().any(|t| t == target) {
            continue;
        }
        let to = bin_dir.join(packaged);
        let prebuilt = args.prebuilt.as_ref().map(|dir| dir.join(packaged));
        let from = match prebuilt.filter(|p| p.is_file()) {
            Some(path) => path,
            None => {
                let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
                let status = Command::new(cargo)
                    .current_dir(repo())
                    .args(["build", "--release", "-p", "fullops", "--target", target])
                    .status()
                    .map_err(|e| format!("cargo를 띄우지 못했습니다: {e}"))?;
                if !status.success() {
                    return Err(format!(
                        "{target} 빌드가 실패했습니다. 그 타깃의 러너에서 빌드한 바이너리를 `--prebuilt <폴더>`에 `{packaged}` 이름으로 두거나 `--target`으로 타깃을 고릅니다."
                    ));
                }
                repo()
                    .join("target")
                    .join(target)
                    .join("release")
                    .join(built)
            }
        };
        copy_tree(&from, &to)?;
        make_executable(&to)?;
    }
    Ok(())
}

/// 배포 폴더 하나를 만든다. `directory`면 디렉터리판이다.
fn package(args: &Args, deps: &Deps, out: &Path, directory: bool) -> Res {
    let src = repo();
    let plugin = src.join("plugin");
    if out.exists() {
        io(std::fs::remove_dir_all(out), out)?;
    }
    // 공통: 스킬(하위 폴더째), 런처와 바이너리, 의존 목록, 문서
    copy_tree(&plugin.join("skills"), &out.join("skills"))?;
    let launcher = out.join("bin/fullops");
    let script = io(std::fs::read_to_string(plugin.join("bin/fullops")), &plugin)?;
    write(&launcher, script.replace("\r\n", "\n").as_bytes())?;
    make_executable(&launcher)?;
    binaries(args, &out.join("bin"))?;
    copy_tree(&plugin.join("deps.toml"), &out.join("deps.toml"))?;
    copy_tree(&plugin.join("INSTALL.md"), &out.join("INSTALL.md"))?;
    copy_tree(
        &src.join("crates/fullops/assets/releases"),
        &out.join("releases"),
    )?;
    if src.join("LICENSE").is_file() {
        copy_tree(&src.join("LICENSE"), &out.join("LICENSE"))?;
    }
    write(&out.join("VERSION"), format!("{VERSION}\n").as_bytes())?;

    write_json(&out.join("hooks/hooks.json"), &claude_hooks(directory))?;
    write_json(&out.join(".mcp.json"), &mcp(deps, true, directory))?;
    if directory {
        // 디렉터리판: 자기 폴더만 쓴다. Codex 파일, hosts/, 의존 플러그인 선언이 없다
        let claude = manifest(&plugin, "claude-directory", None)?;
        return write_json(&out.join(".claude-plugin/plugin.json"), &claude.into());
    }
    let mut claude = manifest(&plugin, "claude", None)?;
    let dependencies: Vec<Value> = deps
        .plugins
        .iter()
        .filter(|p| p.hosts.contains(&HostId::Claude))
        .filter_map(|p| p.id.split_once('@'))
        .map(|(name, marketplace)| json!({ "name": name, "marketplace": marketplace }))
        .collect();
    claude.insert("dependencies".into(), dependencies.into());
    write_json(&out.join(".claude-plugin/plugin.json"), &claude.into())?;
    write_json(
        &out.join(".codex-plugin/plugin.json"),
        &manifest(&plugin, "codex", None)?.into(),
    )?;
    write_json(&out.join("hooks/codex.json"), &codex_hooks())?;
    let grok = manifest(&plugin, "grok", Some(&["name", "version", "description"]))?;
    write_json(&out.join("hosts/grok/plugin.json"), &grok.into())?;
    write_json(&out.join("hosts/grok/.mcp.json"), &mcp(deps, true, false))?;
    let agy = manifest(
        &plugin,
        "agy",
        Some(&["name", "displayName", "description", "version"]),
    )?;
    write_json(&out.join("hosts/agy/plugin.json"), &agy.into())?;
    write_json(&out.join("hosts/agy/hooks.json"), &agy_hooks())?;
    write_json(
        &out.join("hosts/agy/mcp_config.json"),
        &mcp(deps, false, false),
    )
}

/// G4: 배포본의 `.py`·`.js`·`.cjs`·`.mjs`는 `skills/fullops-motion/render.mjs` 하나뿐이다.
fn check_scripts(out: &Path) -> Res {
    let extra: Vec<String> = files(out)?
        .into_iter()
        .filter(|f| {
            [".py", ".js", ".cjs", ".mjs"]
                .iter()
                .any(|ext| f.ends_with(ext))
        })
        .filter(|f| f != "skills/fullops-motion/render.mjs")
        .collect();
    if extra.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{}에 실행 중 Python·Node에 기대는 파일이 있습니다(G4): {}",
            out.display(),
            extra.join(", ")
        ))
    }
}

fn sums(out: &Path) -> Res {
    let mut text = String::new();
    for file in files(out)?.iter().filter(|f| *f != "SHA256SUMS") {
        let path = out.join(file);
        text.push_str(&format!(
            "{}  {file}\n",
            sha256_hex(&io(std::fs::read(&path), &path)?)
        ));
    }
    write(&out.join("SHA256SUMS"), text.as_bytes())
}

/// 루트의 마켓플레이스 카탈로그 둘. Claude Code와 Codex가 같은 통합 패키지 폴더를 가리킨다.
fn catalogs(root: &Path, deps: &Deps) -> Res {
    let own = &deps.own;
    let source = "./dist/native/fullops";
    let description = read_json(&repo().join("plugin/plugin.json"))?
        .remove("description")
        .unwrap_or_default();
    let markets: Vec<&str> = deps
        .plugins
        .iter()
        .filter(|p| p.hosts.contains(&HostId::Claude))
        .filter_map(|p| p.id.split_once('@'))
        .map(|(_, market)| market)
        .collect();
    write_json(
        &root.join(".claude-plugin/marketplace.json"),
        &json!({
            "name": own.marketplace,
            "description": description,
            "owner": { "name": "tourbut" },
            "allowCrossMarketplaceDependenciesOn": markets,
            "plugins": [{ "name": own.plugin, "source": source }],
        }),
    )?;
    write_json(
        &root.join(".agents/plugins/marketplace.json"),
        &json!({
            "name": own.marketplace,
            "interface": { "displayName": "FullOps" },
            "plugins": [{
                "name": own.plugin,
                "source": { "source": "local", "path": source },
                "policy": { "installation": "AVAILABLE", "authentication": "ON_INSTALL" },
                "category": "Productivity",
            }],
        }),
    )
}

/// 호스트 CLI의 `plugin validate`. 그 CLI가 PATH에 있을 때만 돌린다. `--help`는 붙이지 않는다(설계 9.5절).
fn validate(native: &Path, directory: &Path) -> Res {
    let scratch = std::env::temp_dir().join(format!("fullops-dist-{}", std::process::id()));
    let assembled = |host: HostId| -> Res<PathBuf> {
        let dir = scratch.join(host.as_str());
        for (from, to) in package_items(host) {
            copy_tree(&native.join(from), &dir.join(to))?;
        }
        Ok(dir)
    };
    let text = |p: &Path| p.to_string_lossy().into_owned();
    let runs: [(&str, Vec<String>); 4] = [
        (
            "claude",
            vec!["plugin".into(), "validate".into(), text(native)],
        ),
        (
            "claude",
            vec![
                "plugin".into(),
                "validate".into(),
                "--strict".into(),
                text(directory),
            ],
        ),
        (
            "grok",
            vec![
                "plugin".into(),
                "validate".into(),
                text(&assembled(HostId::Grok)?),
            ],
        ),
        (
            "agy",
            vec![
                "plugin".into(),
                "validate".into(),
                "--strict".into(),
                text(&assembled(HostId::Agy)?),
            ],
        ),
    ];
    let mut failed = Vec::new();
    for (program, rest) in runs {
        let Some(found) = fullops_ext::hosts::find_program(program) else {
            println!(
                "{program} CLI가 PATH에 없어 `{program} {}` 검증을 건너뛴다.",
                rest.join(" ")
            );
            continue;
        };
        let argv: Vec<String> = std::iter::once(text(&found)).chain(rest.clone()).collect();
        let out = fullops_ext::run(&argv, &repo(), Duration::from_secs(120), &[], None);
        if !out.ok() {
            failed.push(format!(
                "{program} {}\n{}",
                rest.join(" "),
                out.stderr_text()
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("검증 실패:\n{}", failed.join("\n")))
    }
}

pub fn run(args: &[String]) -> Result<(), String> {
    let args = parse(args)?;
    let deps_path = repo().join("plugin/deps.toml");
    let deps = fullops_store::parse_deps(&io(std::fs::read_to_string(&deps_path), &deps_path)?)
        .map_err(|e| e.to_string())?;
    let native = args.root.join("dist/native/fullops");
    let directory = args.root.join("dist/directory/fullops");
    for (out, is_directory) in [(&native, false), (&directory, true)] {
        package(&args, &deps, out, is_directory)?;
        check_scripts(out)?;
        sums(out)?;
    }
    catalogs(&args.root, &deps)?;
    if args.validate {
        validate(&native, &directory)?;
    }
    println!("{}\n{}", native.display(), directory.display());
    Ok(())
}
