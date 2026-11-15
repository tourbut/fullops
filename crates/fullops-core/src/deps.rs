//! 의존 설치의 계획(설계 10.3절, 9.5절). 순수 함수다. 묶음 F의 파일이다.
//! `Deps` 타입은 `model`에 있고 파일 읽기·TOML 해석·필드 검증은 `fullops-store::load_deps`가 한다.
//! 호스트 CLI의 명령 문법 표(`HOST_COMMANDS`, 9.5절)와 hook 등록의 타임아웃·매처 표(7.1·7.2절)도 여기 있다.

use crate::model::{Deps, HostId, Known, Refusal};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// 설치 계획. `--dry-run`은 이 계획의 명령 줄만 낸다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// 호스트별 `plugin` 단계와 `deps` 단계의 플러그인 명령. 마켓플레이스 출처 충돌이면 그 호스트만 `Err`다.
    pub per_host: BTreeMap<HostId, Result<Vec<Step>, Refusal>>,
    /// 스킬 묶음마다 하나. 호스트를 가로지른다(`Step.hosts`가 대상 호스트).
    pub skills: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub stage: InstallStage,
    pub kind: StepKind,
    pub argv: Vec<String>,
    /// 사람에게 보여 주는 한 줄. 실패하면 설치 기록의 `failed`에 들어간다.
    pub what: String,
    /// `per_host`의 Step은 그 호스트 하나, `skills`의 Step은 `--agent`에 실은 호스트 전부.
    pub hosts: Vec<HostId>,
}

/// `--only plugin|deps`가 이 값으로 거른다. `SkillInstall`은 항상 `Deps`다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStage {
    Plugin,
    Deps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    MarketAdd,
    MarketUpdate,
    PluginInstall,
    SelfInstall,
    SkillInstall,
    /// grok·agy 패키지 조립. `argv = [통합 패키지 폴더, 대상 폴더]`. 프로세스를 띄우지 않는다.
    Assemble,
    /// grok의 hook 등록 파일. `argv = [대상 경로, 실행본 경로]`. 프로세스를 띄우지 않는다.
    HookFile,
}

/// 한 호스트 CLI의 명령 문법(설계 9.5절의 표). 인자 배열은 실행 파일 이름 뒤에 붙는 부분이다.
pub struct HostCommands {
    pub host: HostId,
    pub program: &'static str,
    /// 마켓플레이스 목록. agy는 없다.
    pub market_list: Option<&'static [&'static str]>,
    /// 뒤에 `<출처>`를 붙인다.
    pub market_add: Option<&'static [&'static str]>,
    /// 뒤에 `<이름>`을 붙인다.
    pub market_update: Option<&'static [&'static str]>,
    /// 플러그인 설치 명령들. `{id}`가 `<이름>@<마켓플레이스>` 또는 조립한 패키지의 절대 경로로 바뀐다.
    pub plugin_install: &'static [&'static [&'static str]],
    pub plugin_list: &'static [&'static str],
    /// 스킬 설치 도구의 `--agent`에 주는 이름.
    pub skill_agent: &'static str,
    /// fullops 자신을 마켓플레이스가 아니라 조립한 패키지의 폴더 경로로 설치한다.
    pub local_package: bool,
}

/// 신뢰 인자(`--trust` 등)와 `--help`는 어느 줄에도 없다(설계 2.3절, 9.5절).
pub static HOST_COMMANDS: [HostCommands; 4] = [
    HostCommands {
        host: HostId::Claude,
        program: "claude",
        market_list: Some(&["plugin", "marketplace", "list", "--json"]),
        market_add: Some(&["plugin", "marketplace", "add"]),
        market_update: Some(&["plugin", "marketplace", "update"]),
        plugin_install: &[
            &["plugin", "install", "{id}"],
            &["plugin", "update", "{id}", "--scope", "user"],
        ],
        plugin_list: &["plugin", "list", "--json"],
        skill_agent: "claude-code",
        local_package: false,
    },
    HostCommands {
        host: HostId::Codex,
        program: "codex",
        market_list: Some(&["plugin", "marketplace", "list", "--json"]),
        market_add: Some(&["plugin", "marketplace", "add"]),
        market_update: Some(&["plugin", "marketplace", "upgrade"]),
        plugin_install: &[&["plugin", "add", "{id}"]],
        plugin_list: &["plugin", "list", "--json"],
        skill_agent: "codex",
        local_package: false,
    },
    HostCommands {
        host: HostId::Grok,
        program: "grok",
        market_list: Some(&["plugin", "marketplace", "list", "--json"]),
        market_add: Some(&["plugin", "marketplace", "add"]),
        market_update: Some(&["plugin", "marketplace", "update"]),
        plugin_install: &[&["plugin", "install", "{id}"]],
        plugin_list: &["plugin", "list", "--json"],
        skill_agent: "grok",
        local_package: true,
    },
    HostCommands {
        host: HostId::Agy,
        program: "agy",
        market_list: None,
        market_add: None,
        market_update: None,
        plugin_install: &[&["plugin", "install", "{id}"]],
        plugin_list: &["plugin", "list", "--json"],
        skill_agent: "antigravity-cli",
        local_package: true,
    },
];

pub const ALL_HOSTS: [HostId; 4] = [HostId::Claude, HostId::Codex, HostId::Grok, HostId::Agy];

pub fn host_commands(host: HostId) -> &'static HostCommands {
    match host {
        HostId::Claude => &HOST_COMMANDS[0],
        HostId::Codex => &HOST_COMMANDS[1],
        HostId::Grok => &HOST_COMMANDS[2],
        HostId::Agy => &HOST_COMMANDS[3],
    }
}

/// hook 등록 타임아웃(초). 네 호스트의 등록 파일에 같은 값으로 적는다(설계 7.1절의 표).
pub const HOOK_TIMEOUTS: [(&str, u32); 5] = [
    ("SessionStart", 10),
    ("UserPromptSubmit", 5),
    ("PreToolUse", 8),
    ("Stop", 20),
    ("StopFailure", 15),
];

/// PreToolUse 매처(설계 7.2절의 표).
pub fn pre_tool_matcher(host: HostId) -> &'static str {
    match host {
        HostId::Claude => "Bash|PowerShell|Write|Edit|MultiEdit|Agent|Task|Workflow",
        HostId::Codex => "Bash|apply_patch|spawn_agent",
        HostId::Grok => "run_terminal_command|search_replace|hashline_edit|write|spawn_subagent",
        HostId::Agy => {
            "run_command|write_to_file|replace_file_content|multi_replace_file_content|invoke_subagent"
        }
    }
}

/// 조립한 패키지에 들어가는 것(설계 10.3절의 표). `(통합 패키지 기준 재료, 조립한 폴더 안의 자리)`.
/// `install`의 조립과 `xtask dist`의 검증이 같은 표를 쓴다.
pub fn package_items(host: HostId) -> &'static [(&'static str, &'static str)] {
    match host {
        HostId::Grok => &[
            ("hosts/grok/plugin.json", "plugin.json"),
            ("hosts/grok/.mcp.json", ".mcp.json"),
            ("skills", "skills"),
            ("VERSION", "VERSION"),
        ],
        HostId::Agy => &[
            ("hosts/agy/plugin.json", "plugin.json"),
            ("hosts/agy/hooks.json", "hooks.json"),
            ("hosts/agy/mcp_config.json", "mcp_config.json"),
            ("skills", "skills"),
            ("bin", "bin"),
            ("VERSION", "VERSION"),
        ],
        _ => &[],
    }
}

/// `Assemble`의 `argv[0]`과 `HookFile`의 `argv`에 `install_plan`이 넣어 두는 자리표시자.
/// 순수 함수는 플러그인 루트와 사용자 홈을 모르므로 부르는 쪽이 `Plan::bind`로 실제 경로를 채운다.
pub const PACKAGE_ROOT: &str = "<통합 패키지>";
pub const GROK_HOOK_FILE: &str = "<GROK_HOME>/hooks/fullops.json";
pub const FIXED_BIN: &str = "<고정 위치 실행본>";

/// 마켓플레이스 출처를 견주는 꼴(설계 9.5절): 앞의 `https://github.com/`, 끝의 `.git`·`/`를 떼고 소문자로.
pub fn normalize_source(source: &str) -> String {
    let lower = source.trim().to_lowercase();
    let rest = lower.strip_prefix("https://github.com/").unwrap_or(&lower);
    let rest = rest.trim_end_matches('/');
    rest.strip_suffix(".git")
        .unwrap_or(rest)
        .trim_end_matches('/')
        .to_string()
}

fn command(cmds: &HostCommands, args: &[&str], tail: &[&str]) -> Vec<String> {
    std::iter::once(cmds.program)
        .chain(args.iter().copied())
        .chain(tail.iter().copied())
        .map(str::to_string)
        .collect()
}

fn step(stage: InstallStage, kind: StepKind, argv: Vec<String>, host: HostId) -> Step {
    Step {
        stage,
        kind,
        what: argv.join(" "),
        argv,
        hosts: vec![host],
    }
}

/// 마켓플레이스 하나의 등록·갱신 명령. 출처가 다르게 등록돼 있으면 `Err`다.
fn market_step(
    cmds: &HostCommands,
    stage: InstallStage,
    name: &str,
    source: &str,
    registered: Option<&BTreeMap<String, String>>,
) -> Result<Option<Step>, Refusal> {
    let (Some(add), Some(update)) = (cmds.market_add, cmds.market_update) else {
        return Ok(None);
    };
    Ok(Some(match registered.and_then(|map| map.get(name)) {
        Some(found) if normalize_source(found) != normalize_source(source) => {
            return Err(Refusal {
                rule: "install",
                reason: format!(
                    "{}에 마켓플레이스 `{name}`이 다른 출처(`{found}`)로 등록돼 있다. deps.toml의 출처는 `{source}`다. 덮어쓰지 않는다.",
                    cmds.program
                ),
                escape: format!(
                    "등록된 마켓플레이스를 사람이 정리한 뒤 `fullops install --hosts {}`",
                    cmds.host
                ),
                exit: 4,
            });
        }
        Some(_) => step(
            stage,
            StepKind::MarketUpdate,
            command(cmds, update, &[name]),
            cmds.host,
        ),
        // 등록 여부를 모르면 등록 명령을 그대로 부른다(다시 해도 해가 없다)
        None => step(
            stage,
            StepKind::MarketAdd,
            command(cmds, add, &[source]),
            cmds.host,
        ),
    }))
}

fn install_steps(cmds: &HostCommands, stage: InstallStage, kind: StepKind, id: &str) -> Vec<Step> {
    cmds.plugin_install
        .iter()
        .map(|args| {
            let argv = std::iter::once(cmds.program.to_string())
                .chain(args.iter().map(|a| a.replace("{id}", id)))
                .collect();
            step(stage, kind, argv, cmds.host)
        })
        .collect()
}

fn host_steps(
    deps: &Deps,
    host: HostId,
    registered: Option<&BTreeMap<String, String>>,
    package_dir: Option<&Path>,
) -> Result<Vec<Step>, Refusal> {
    let cmds = host_commands(host);
    let mut steps = Vec::new();
    let mut seen_markets = BTreeSet::new();
    let own = &deps.own;
    if own.hosts.contains(&host) {
        if cmds.local_package {
            if let Some(dir) = package_dir {
                let target = dir.join(host.as_str()).to_string_lossy().into_owned();
                steps.push(Step {
                    stage: InstallStage::Plugin,
                    kind: StepKind::Assemble,
                    what: format!("조립: {PACKAGE_ROOT} → {target}"),
                    argv: vec![PACKAGE_ROOT.to_string(), target.clone()],
                    hosts: vec![host],
                });
                steps.extend(install_steps(
                    cmds,
                    InstallStage::Plugin,
                    StepKind::SelfInstall,
                    &target,
                ));
            }
            if host == HostId::Grok {
                steps.push(Step {
                    stage: InstallStage::Plugin,
                    kind: StepKind::HookFile,
                    what: format!("쓰기: {GROK_HOOK_FILE} (실행본 {FIXED_BIN})"),
                    argv: vec![GROK_HOOK_FILE.to_string(), FIXED_BIN.to_string()],
                    hosts: vec![host],
                });
            }
        } else {
            seen_markets.insert(own.marketplace.as_str());
            steps.extend(market_step(
                cmds,
                InstallStage::Plugin,
                &own.marketplace,
                &own.source,
                registered,
            )?);
            let id = format!("{}@{}", own.plugin, own.marketplace);
            steps.extend(install_steps(
                cmds,
                InstallStage::Plugin,
                StepKind::SelfInstall,
                &id,
            ));
        }
    }
    for plugin in deps.plugins.iter().filter(|p| p.hosts.contains(&host)) {
        let Some((_, market)) = plugin.id.split_once('@') else {
            continue;
        };
        if seen_markets.insert(market) {
            let source = if market == own.marketplace {
                Some(&own.source)
            } else {
                deps.marketplaces.get(market)
            };
            if let Some(source) = source {
                steps.extend(market_step(
                    cmds,
                    InstallStage::Deps,
                    market,
                    source,
                    registered,
                )?);
            }
        }
        steps.extend(install_steps(
            cmds,
            InstallStage::Deps,
            StepKind::PluginInstall,
            &plugin.id,
        ));
    }
    Ok(steps)
}

/// 대상 호스트 전체의 계획을 한 번에 만든다. `registered`는 호스트 → 등록된 마켓플레이스(이름 → 출처)이고
/// 키가 없거나 `Unknown`이면 "모른다"다. `package_dir`은 조립한 패키지의 자리 `~/.fullops/package/<버전>`이고
/// `None`이면 grok·agy의 `Assemble`과 `SelfInstall`을 넣지 않는다(grok의 `HookFile`은 넣는다).
/// `Assemble`의 재료 폴더와 `HookFile`의 두 경로는 자리표시자로 두고 부르는 쪽이 `Plan::bind`로 채운다.
pub fn install_plan(
    deps: &Deps,
    hosts: &[HostId],
    registered: &BTreeMap<HostId, Known<BTreeMap<String, String>>>,
    package_dir: Option<&Path>,
) -> Plan {
    let mut plan = Plan::default();
    for &host in hosts {
        let known = match registered.get(&host) {
            Some(Known::Yes(map)) => Some(map),
            _ => None,
        };
        plan.per_host
            .insert(host, host_steps(deps, host, known, package_dir));
    }
    for bundle in &deps.skills {
        // 출처 충돌로 멈춘 호스트는 스킬 명령의 대상에서도 뺀다
        let targets: Vec<HostId> = bundle
            .hosts
            .as_deref()
            .unwrap_or(&ALL_HOSTS)
            .iter()
            .copied()
            .filter(|h| matches!(plan.per_host.get(h), Some(Ok(_))))
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut argv = deps.skill_cli.clone();
        argv.extend(["add".to_string(), bundle.source.clone(), "--skill".into()]);
        argv.extend(bundle.names.iter().cloned());
        argv.extend(["--global".to_string(), "--agent".into()]);
        argv.extend(
            targets
                .iter()
                .map(|h| host_commands(*h).skill_agent.to_string()),
        );
        argv.push("--yes".into());
        plan.skills.push(Step {
            stage: InstallStage::Deps,
            kind: StepKind::SkillInstall,
            what: argv.join(" "),
            argv,
            hosts: targets,
        });
    }
    plan
}

impl Plan {
    /// 자리표시자를 실제 경로로 바꾼다. `package_root`는 통합 패키지 폴더(플러그인 루트)다.
    pub fn bind(&mut self, package_root: Option<&Path>, grok_hook_file: &Path, fixed_bin: &Path) {
        let pairs = [
            (
                PACKAGE_ROOT,
                package_root.map(|p| p.to_string_lossy().into_owned()),
            ),
            (
                GROK_HOOK_FILE,
                Some(grok_hook_file.to_string_lossy().into_owned()),
            ),
            (FIXED_BIN, Some(fixed_bin.to_string_lossy().into_owned())),
        ];
        for step in self.per_host.values_mut().flatten().flatten() {
            for (mark, value) in &pairs {
                let Some(value) = value else { continue };
                step.what = step.what.replace(mark, value);
                for arg in &mut step.argv {
                    if arg == mark {
                        *arg = value.clone();
                    }
                }
            }
        }
    }
}

/// grok의 hook 등록 파일 `<GROK_HOME 또는 ~/.grok>/hooks/fullops.json`의 내용(설계 10.4절).
/// exec form이다: `command`에 실행본의 절대 경로를 글자 그대로 적고 `shell`·`async`·`statusMessage`는 쓰지 않는다.
/// SessionStart에는 matcher를 두지 않고 `--plugin-root`를 주지 않는다.
pub fn grok_hook_json(fullops_bin: &Path) -> String {
    let command = fullops_bin.to_string_lossy();
    let hooks: serde_json::Map<String, serde_json::Value> = HOOK_TIMEOUTS
        .iter()
        .map(|(event, timeout)| {
            let handler = serde_json::json!({
                "type": "command", "command": command, "args": ["hook", "grok"], "timeout": timeout,
            });
            let mut entry = serde_json::json!({ "hooks": [handler] });
            if *event == "PreToolUse" {
                entry["matcher"] = pre_tool_matcher(HostId::Grok).into();
            }
            (event.to_string(), serde_json::json!([entry]))
        })
        .collect();
    let mut text =
        serde_json::to_string_pretty(&serde_json::json!({ "hooks": hooks })).unwrap_or_default();
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_compare_without_prefix_suffix_and_case() {
        for same in [
            "tourbut/fullops",
            "https://github.com/Tourbut/Fullops.git",
            "https://github.com/tourbut/fullops/",
        ] {
            assert_eq!(normalize_source(same), "tourbut/fullops");
        }
        assert_ne!(normalize_source("other/fullops"), "tourbut/fullops");
    }

    #[test]
    fn no_command_carries_trust_or_help() {
        for cmds in &HOST_COMMANDS {
            let all = cmds
                .plugin_install
                .iter()
                .copied()
                .chain(cmds.market_list)
                .chain(cmds.market_add)
                .chain(cmds.market_update)
                .chain([cmds.plugin_list]);
            for args in all {
                assert!(
                    !args
                        .iter()
                        .any(|a| a.contains("trust") || a.contains("help"))
                );
            }
        }
    }

    #[test]
    fn grok_hook_file_is_exec_form() {
        let text = grok_hook_json(Path::new("/home/a/.fullops/bin/fullops"));
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        let hooks = json["hooks"].as_object().unwrap();
        assert_eq!(hooks.len(), 5);
        let pre = &hooks["PreToolUse"][0];
        assert!(pre["matcher"].as_str().unwrap().contains("spawn_subagent"));
        assert!(hooks["SessionStart"][0].get("matcher").is_none());
        let handler = &pre["hooks"][0];
        assert_eq!(handler["command"], "/home/a/.fullops/bin/fullops");
        assert_eq!(handler["args"], serde_json::json!(["hook", "grok"]));
        assert_eq!(handler["timeout"], 8);
        assert!(!text.contains("shell") && !text.contains("plugin-root"));
    }
}
