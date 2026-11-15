//! `fullops install --retire-python`: 옛 판(fullops-squad 1.24.0)이 이 PC에 남긴 것을 정리한다(설계 10.3절, `docs/install-and-retire.md` 3절 2단계·4절 P4).
//! 기본은 계획만 낸다. 지우지 않고 `~/.fullops/py-retired/<시각>/`로 옮기고 `--restore <시각>`으로 되돌린다.
//! 호스트 CLI에는 목록 조회와 계약 문서(`docs/contracts/grok.md`)에서 확인한 grok의 제거 명령만 부른다. `--help`는 붙이지 않는다.
//! Claude Code·Codex의 제거 명령과 npm 전역 패키지는 안내만 한다.

use crate::cli::{self, CliError, Ctx, EXIT_HUMAN, EXIT_OK, EXIT_REFUSED};
use fullops_core::deps::normalize_source;
use fullops_core::model::{Deps, HostId, Known, Refusal, path_text};
use fullops_ext::{Git, GitCli, hosts};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Default)]
pub struct Args {
    pub on: bool,
    /// 옛 표식이 남았는지 볼 레포. 각 레포의 모든 워크트리를 본다.
    pub repos: Vec<PathBuf>,
    /// 되돌릴 정리의 시각(`py-retired/<시각>`).
    pub restore: Option<String>,
    /// `fullops-meter`와 마켓플레이스 `fullops-squad`까지 지운다고 안내한다(기본은 남긴다).
    pub remove_meter: bool,
}

/// 옛 판의 플러그인·마켓플레이스 이름과 출처.
const OLD: &str = "fullops-squad";
const OLD_SOURCE: &str = "tourbut/fullops-squad-plugin";
const METER: &str = "fullops-meter@fullops-squad";

/// 1.24.0 코드가 각 호스트 홈 아래에 쓰는 것: 설치 기록(`deps.py:103-107`)과 스킬 백업(`deps.py:201`). 이 표 밖은 건드리지 않는다.
const IN_HOST_HOME: [&str; 2] = ["fullops-deps.json", "fullops-skill-backups"];
/// Jev 캐시(`jev_observe.py:266`).
const JEV_CACHE: &str = ".cache/fullops-squad/jev";
/// hook 런타임(`hook_runtime.py:15`). 옛 hook이 python3로 직접 부르므로 옛 플러그인이 없다고 확인한 뒤 마지막에 옮긴다.
const RUNTIME: &str = ".fullops-squad/runtime/fullops-squad";
/// 계획에 안내만 하는 npm 전역 패키지(`deps.py:53`).
const NPM_GLOBALS: &str = "@upstash/context7-mcp @alibaba-group/open-code-review";

const HOST_TIMEOUT: Duration = Duration::from_secs(600);

fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

/// 1.24.0이 설치 기록과 스킬 백업을 둔 호스트 홈들(환경 변수가 가리키는 홈, 기본 홈, Orca 세션의 홈).
fn host_homes(home: &Path) -> Vec<PathBuf> {
    let env = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let mut out: Vec<PathBuf> = ["CLAUDE_CONFIG_DIR", "CODEX_HOME", "GROK_HOME"]
        .iter()
        .filter_map(|name| env(name))
        .collect();
    out.extend([".claude", ".codex", ".grok", ".agy"].map(|dir| home.join(dir)));
    // shortcut: Orca 홈은 Windows의 기본 자리(%APPDATA%)만 본다, macOS·Linux의 Orca 홈 자리가 확인되면 더한다
    if cfg!(windows) {
        let orca = home.join("AppData").join("Roaming").join("orca");
        out.push(orca.join("codex-runtime-home").join("home"));
        out.extend(
            std::fs::read_dir(orca.join("claude-profiles"))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path().join("home")),
        );
    }
    out.sort();
    out.dedup();
    out
}

/// 옛 판으로 셀 플러그인 id인가. meter는 runtime을 쓰지 않으므로 `--remove-meter`일 때만 센다.
fn is_old(id: &str, remove_meter: bool) -> bool {
    id.split('@').next() == Some(OLD) || (remove_meter && id == METER)
}

/// 이 홈의 설정 파일에 남은 옛 판 플러그인(Claude `settings.json`의 `enabledPlugins`, Codex `config.toml`의 `[plugins]`).
/// CLI 목록은 지금 환경의 홈만 보므로 다른 프로필·홈은 파일로 본다. 읽지 못하면 모른다.
fn stuck_in(home: &Path, remove_meter: bool) -> Known<Vec<String>> {
    let mut ids = Vec::new();
    let settings = home.join("settings.json");
    if settings.is_file() {
        let Some(json) = fullops_store::read_json::<Value>(&settings).yes().flatten() else {
            return Known::Unknown("settings.json");
        };
        ids.extend(
            json["enabledPlugins"]
                .as_object()
                .into_iter()
                .flat_map(|m| m.keys().cloned()),
        );
    }
    let config = home.join("config.toml");
    if config.is_file() {
        let Some(table) = std::fs::read_to_string(&config)
            .ok()
            .and_then(|t| t.parse::<toml::Table>().ok())
        else {
            return Known::Unknown("config.toml");
        };
        ids.extend(
            table
                .get("plugins")
                .and_then(toml::Value::as_table)
                .into_iter()
                .flat_map(|t| t.keys().cloned()),
        );
    }
    ids.retain(|id| is_old(id, remove_meter));
    Known::Yes(ids)
}

fn none_left(found: &Known<Vec<String>>) -> bool {
    matches!(found, Known::Yes(list) if list.is_empty())
}

/// 옮길 것(runtime 제외). 있는 것만. 옛 판이 남은 홈의 설치 기록은 그 홈의 플러그인을 지운 뒤에 옮긴다(2.1절).
fn leftovers(home: &Path, homes: &[(PathBuf, Known<Vec<String>>)]) -> Vec<PathBuf> {
    homes
        .iter()
        .flat_map(|(h, found)| {
            IN_HOST_HOME
                .iter()
                .filter(|name| none_left(found) || **name != "fullops-deps.json")
                .map(|name| h.join(name))
        })
        .chain([home.join(JEV_CACHE)])
        .filter(|p| exists(p))
        .collect()
}

/// 레포마다 모든 워크트리에 옛 표식(`.fullops-squad/fullops.json`)이 남았는지. 하나라도 걸리면 실행하지 않는다.
fn repo_blockers(repos: &[PathBuf]) -> (Vec<String>, usize) {
    if repos.is_empty() {
        return (
            vec!["레포 목록이 없어 옛 표식이 남았는지 볼 수 없다(`--repos <경로,…>`).".into()],
            0,
        );
    }
    let (mut blockers, mut seen) = (Vec::new(), 0);
    for repo in repos {
        let git = GitCli {
            repo_root: repo.clone(),
            timeout: Duration::from_secs(30),
        };
        match git.worktrees() {
            Known::Yes(list) => {
                seen += list.len();
                for wt in list {
                    let marker = wt.path.join(".fullops-squad").join("fullops.json");
                    if marker.is_file() {
                        blockers.push(format!("옛 표식이 남았다: {}", marker.display()));
                    }
                }
            }
            Known::Unknown(_) => blockers.push(format!(
                "워크트리 목록을 읽지 못했다(`git worktree list`): {}",
                repo.display()
            )),
        }
    }
    (blockers, seen)
}

/// 이 호스트에 남은 옛 판 플러그인 id. CLI가 PATH에 없으면 그 호스트의 기본 홈이 없을 때만 빈 목록이고, 있으면 모른다.
fn old_plugins(host: HostId, home: &Path, remove_meter: bool) -> Known<Vec<String>> {
    if super::install::offline() {
        return Known::Unknown("offline");
    }
    if !hosts::host_available(host) {
        return if exists(&home.join(format!(".{host}"))) {
            Known::Unknown("CLI 없음")
        } else {
            Known::Yes(Vec::new())
        };
    }
    hosts::host_plugins(host).map(|list| {
        list.into_iter()
            .filter(|id| is_old(id, remove_meter))
            .collect()
    })
}

/// `deps.toml`이 쓰는 플러그인·마켓플레이스 이름인가(공용 의존은 계획에서 뺀다).
fn shared(deps: &Deps, name: &str) -> bool {
    deps.own.plugin == name
        || deps.own.marketplace == name
        || deps.marketplaces.contains_key(name)
        || deps
            .plugins
            .iter()
            .any(|p| p.id.split('@').any(|part| part == name))
}

/// 부를 grok 명령(계약 문서 `grok.md` 1.1절 3번의 제거 꼴): 플러그인 제거, 이어서 마켓플레이스 제거.
fn grok_commands(plugins: &Known<Vec<String>>) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    if matches!(plugins, Known::Yes(list) if !list.is_empty()) {
        // 확인 질문에 막히지 않게 계약의 `--confirm`을 준다. --apply가 사람의 확인이다
        out.push(
            ["grok", "plugin", "uninstall", OLD, "--confirm"]
                .map(String::from)
                .to_vec(),
        );
    }
    if let Some(markets) = hosts::host_marketplaces(HostId::Grok).yes() {
        for (name, source) in markets {
            if name == OLD || normalize_source(&source) == OLD_SOURCE {
                out.push(
                    ["grok", "plugin", "marketplace", "remove", source.as_str()]
                        .map(String::from)
                        .to_vec(),
                );
            }
        }
    }
    out
}

fn host_line(host: HostId, found: &Known<Vec<String>>) -> String {
    match found {
        Known::Yes(list) if list.is_empty() => format!("[{host}] 옛 판 플러그인 없음"),
        Known::Yes(list) => format!("[{host}] 남은 옛 판 플러그인: {}", list.join(", ")),
        Known::Unknown(_) => format!("[{host}] 목록을 읽지 못해 모른다"),
    }
}

/// 사람이 할 일(실행하지 않는다).
fn guidance(remove_meter: bool) -> Vec<String> {
    let meter = if remove_meter {
        format!(
            " `{METER}`도 같은 꼴로 지운 뒤 마지막에 마켓플레이스 `{OLD}`를 지운다(후보 `claude plugin marketplace remove {OLD}`)."
        )
    } else {
        format!(" `{METER}`를 남기므로 마켓플레이스 `{OLD}`도 남긴다.")
    };
    vec![
        format!(
            "[안내] Claude Code: 제거 명령이 계약 문서에 없어 부르지 않는다. 문서로 확인한 뒤 `~/.claude`와 Orca 프로필마다 CLAUDE_CONFIG_DIR를 바꿔 `{OLD}@{OLD}`를 user·project 범위에서 지운다(후보 `claude plugin uninstall {OLD}@{OLD} --scope user`).{meter}"
        ),
        format!(
            "[안내] Codex: 제거 명령이 계약 문서에 없어 부르지 않는다. 문서로 확인한 뒤 `~/.codex`와 Orca Codex 홈에서 CODEX_HOME을 바꿔 `{OLD}@{OLD}`와 마켓플레이스 `{OLD}`를 지운다."
        ),
        format!(
            "[안내] npm 전역 패키지({NPM_GLOBALS})는 지우지 않는다. 다른 용도가 없다고 확인하면 사람이 npm의 전역 제거로 지운다."
        ),
        "[안내] runtime은 옛 판 세션(Claude Code, Codex, grok)을 모두 닫은 뒤에 옮긴다. 호스트 목록은 지금 환경의 홈만 보고, 다른 프로필·홈은 `settings.json`의 `enabledPlugins`와 `config.toml`의 `[plugins]`로 본다.".into(),
    ]
}

/// 옮길 자리 `py-retired/<시각>`. 같은 초에 다시 실행해도 앞의 기록을 덮지 않게 시각을 올려 찾는다.
fn fresh_dir(retired: &Path) -> PathBuf {
    let now = fullops_store::now();
    (now..now + 60)
        .map(|at| retired.join(at.to_string()))
        .find(|p| !p.exists())
        .unwrap_or_else(|| retired.join(now.to_string()))
}

/// 하나를 옮기고 기록(`moved.json`)을 그때마다 새로 쓴다. 중간에 멈춰도 옮긴 것은 되돌릴 수 있다.
fn move_one(path: &Path, dest: &Path, moved: &mut Vec<Value>) -> std::io::Result<()> {
    std::fs::create_dir_all(dest)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let to = dest.join(format!("{}-{name}", moved.len()));
    std::fs::rename(path, &to)?;
    moved.push(json!({"from": path_text(path), "to": path_text(&to)}));
    fullops_store::write_json(&dest.join("moved.json"), moved)
}

pub fn run(ctx: &Ctx, args: &Args, apply: bool) -> Result<u8, CliError> {
    let home = fullops_store::home()
        .ok_or_else(|| CliError::Internal("사용자 홈을 찾지 못했습니다".into()))?;
    let retired = home.join(".fullops").join("py-retired");
    if let Some(at) = &args.restore {
        return restore(ctx, &retired, at, apply);
    }
    let (deps, _) = super::install::load_deps(cli::plugin_root().as_deref())?;
    let (blockers, worktrees) = repo_blockers(&args.repos);
    let old_now = |h: HostId| old_plugins(h, &home, args.remove_meter);
    let hosts_now = [HostId::Claude, HostId::Codex, HostId::Grok].map(|h| (h, old_now(h)));
    let homes: Vec<(PathBuf, Known<Vec<String>>)> = host_homes(&home)
        .into_iter()
        .map(|h| {
            let found = stuck_in(&h, args.remove_meter);
            (h, found)
        })
        .collect();
    let homes_clean = homes.iter().all(|(_, found)| none_left(found));
    // 공용 의존과 이름이 겹치면 호스트 쪽은 건드리지 않는다
    let grok = if shared(&deps, OLD) || super::install::offline() {
        Vec::new()
    } else {
        grok_commands(&hosts_now[2].1)
    };
    let files = leftovers(&home, &homes);
    let runtime = home.join(RUNTIME);

    let mut lines = Vec::new();
    if blockers.is_empty() {
        lines.push(format!(
            "[레포] 옛 표식 없음({}개 레포, 워크트리 {worktrees}개)",
            args.repos.len()
        ));
    }
    lines.extend(blockers.iter().map(|b| format!("[레포] {b}")));
    lines.extend(hosts_now.iter().map(|(h, found)| host_line(*h, found)));
    lines.extend(homes.iter().filter_map(|(h, found)| match found {
        Known::Yes(list) if list.is_empty() => None,
        Known::Yes(list) => Some(format!(
            "[홈] {}: 남은 옛 판 플러그인 {}. 그 홈의 fullops-deps.json은 남긴다",
            h.display(),
            list.join(", ")
        )),
        Known::Unknown(why) => Some(format!(
            "[홈] {}: {why}을(를) 읽지 못해 모른다. 그 홈의 fullops-deps.json은 남긴다",
            h.display()
        )),
    }));
    lines.extend(
        grok.iter()
            .map(|argv| format!("[grok] 실행: {}", argv.join(" "))),
    );
    lines.extend(guidance(args.remove_meter));
    let target = retired.join("<시각>");
    lines.extend(
        files
            .iter()
            .map(|p| format!("[옮김] {} → {}", p.display(), target.display())),
    );
    if exists(&runtime) {
        lines.push(format!(
            "[옮김·마지막] {} → {} (옛 판 플러그인이 어느 호스트·홈에도 없다고 확인될 때만)",
            runtime.display(),
            target.display()
        ));
    }
    if !apply {
        lines.push(
            "계획만 냈다. 실행하려면 `fullops install --retire-python --repos <경로,…> --apply`"
                .into(),
        );
        ctx.print(
            &lines.join("\n"),
            json!({"apply": false, "blockers": blockers, "grok": grok, "move": files.iter().map(|p| path_text(p)).collect::<Vec<_>>()}),
        );
        return Ok(EXIT_OK);
    }
    if !blockers.is_empty() {
        ctx.note(&lines.join("\n"));
        return Err(CliError::Refused(vec![Refusal {
            rule: "install",
            reason: format!("옛 판 정리를 실행하지 않는다. {}", blockers.join(" ")),
            escape: "모든 레포·워크트리를 `fullops migrate --from-python`으로 옮긴 뒤 `--repos <경로,…>`를 주고 다시 실행한다".into(),
            exit: EXIT_REFUSED,
        }]));
    }

    let mut done = Vec::new();
    let mut failed = Vec::new();
    for argv in &grok {
        let out = hosts::run_argv(argv, HOST_TIMEOUT);
        if out.ok() {
            done.push(format!("실행함: {}", argv.join(" ")));
        } else {
            failed.push(format!("{}\n{}", argv.join(" "), out.stderr_text().trim()));
        }
    }
    let dest = fresh_dir(&retired);
    let mut moved = Vec::new();
    let mut move_err = None;
    for path in &files {
        if let Err(e) = move_one(path, &dest, &mut moved) {
            move_err = Some(format!("{}: {e}", path.display()));
            break;
        }
    }
    // runtime은 옛 플러그인을 지운 뒤의 목록으로 다시 보고, 어디에도 없을 때만 옮긴다
    let clean = homes_clean
        && [HostId::Claude, HostId::Codex, HostId::Grok]
            .iter()
            .all(|&h| none_left(&old_now(h)));
    let mut runtime_pending = false;
    if move_err.is_none() && exists(&runtime) {
        if clean && failed.is_empty() {
            if let Err(e) = move_one(&runtime, &dest, &mut moved) {
                move_err = Some(format!("{}: {e}", runtime.display()));
            }
        } else {
            runtime_pending = true;
        }
    }
    done.extend(moved.iter().map(|m| {
        format!(
            "옮김: {} → {}",
            m["from"].as_str().unwrap_or(""),
            m["to"].as_str().unwrap_or("")
        )
    }));
    if runtime_pending {
        done.push(format!(
            "runtime을 옮기지 않았다: 옛 판 플러그인이 어느 호스트·홈에 남았거나 목록·설정을 읽지 못했다. 위 안내대로 지운 뒤 다시 실행한다({})",
            runtime.display()
        ));
    }
    let at = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !moved.is_empty() {
        done.push(format!(
            "되돌리기: `fullops install --retire-python --restore {at} --apply`"
        ));
    }
    done.extend(guidance(args.remove_meter));
    let fields = json!({"apply": true, "at": at, "moved": moved, "failed": failed, "runtime_pending": runtime_pending});
    if let Some(e) = move_err {
        ctx.note(&done.join("\n"));
        return Err(CliError::Internal(format!(
            "옮기지 못해 멈췄다: {e}. 옮긴 것은 `fullops install --retire-python --restore {at}`로 되돌린다."
        )));
    }
    if !failed.is_empty() {
        ctx.note(&done.join("\n"));
        return Err(CliError::External(format!(
            "grok 명령이 실패했다. 원인을 고친 뒤 다시 실행한다(다시 실행해도 안전하다).\n{}",
            failed.join("\n")
        )));
    }
    ctx.print(&done.join("\n"), fields);
    Ok(if runtime_pending { EXIT_HUMAN } else { EXIT_OK })
}

/// `--restore <시각>`: 그 정리가 옮긴 것을 원래 자리로 되돌린다. 원래 자리에 이미 무엇이 있으면 건너뛴다.
fn restore(ctx: &Ctx, retired: &Path, at: &str, apply: bool) -> Result<u8, CliError> {
    if at.is_empty() || !at.chars().all(|c| c.is_ascii_digit()) {
        return Err(CliError::Usage(format!(
            "--restore의 값은 `py-retired/` 아래 폴더 이름(숫자)입니다: {at}"
        )));
    }
    let record = retired.join(at).join("moved.json");
    let moved: Vec<Value> = fullops_store::read_json(&record)
        .yes()
        .flatten()
        .ok_or_else(|| CliError::Internal(format!("되돌릴 기록이 없다: {}", record.display())))?;
    let mut lines = Vec::new();
    let mut restored = Vec::new();
    for item in moved.iter().rev() {
        let (Some(from), Some(to)) = (item["from"].as_str(), item["to"].as_str()) else {
            continue;
        };
        let (from, to) = (Path::new(from), Path::new(to));
        if exists(from) {
            lines.push(format!("이미 있어 건너뜀: {}", from.display()));
            continue;
        }
        if !exists(to) {
            lines.push(format!("옮긴 것이 없다: {}", to.display()));
            continue;
        }
        lines.push(format!("되돌림: {} → {}", to.display(), from.display()));
        if apply {
            if let Some(parent) = from.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(to, from)?;
            restored.push(path_text(from));
        }
    }
    lines.push("호스트 플러그인은 되돌리지 않는다. 옛 판을 다시 쓰려면 옛 설치기(`npx --yes github:tourbut/Fullops-Squad-plugin install`)를 실행한다.".into());
    if !apply {
        lines.push(format!(
            "계획만 냈다. 실행하려면 `fullops install --retire-python --restore {at} --apply`"
        ));
    }
    ctx.print(
        &lines.join("\n"),
        json!({"apply": apply, "restored": restored}),
    );
    Ok(EXIT_OK)
}
