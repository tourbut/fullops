//! `fullops init`: 레포 초기화(설계 10.6절). 없는 파일만 만들고 커밋하지 않는다. 다시 실행하면 이어서 한다.
//! `fullops.toml`을 만드는 글과 내장 템플릿·관리 블록을 넣는 일은 `migrate`·`mode`·`upgrade`가 함께 쓴다.

use crate::assets;
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::layout;
use fullops_core::messages::install as msg;
use fullops_core::model::{
    Mode, Parallel, Refusal, RoleId, RoleKind, Routing, TestLevel, path_text,
};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `cwd`에서 위로 올라가며 `.git`이 있는 폴더를 찾는다. 활성 표식이 없는 레포에서도 돈다.
pub(super) fn git_root(start: &Path) -> Result<PathBuf, CliError> {
    let start = std::path::absolute(start)?;
    start
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
        .ok_or_else(|| CliError::Usage(format!("Git 레포가 아닙니다: {}", start.display())))
}

/// `(이 워크트리의 Git 디렉터리, Git 공용 디렉터리)`. git 프로세스를 띄우지 않는다.
pub(super) fn git_dirs(root: &Path) -> Option<(PathBuf, PathBuf)> {
    let dot = root.join(".git");
    let git_dir = if dot.is_dir() {
        dot
    } else {
        let text = std::fs::read_to_string(&dot).ok()?;
        root.join(text.trim().strip_prefix("gitdir:")?.trim())
    };
    let common = match std::fs::read_to_string(git_dir.join("commondir")) {
        Ok(text) => git_dir.join(text.trim()),
        Err(_) => git_dir.clone(),
    };
    Some((git_dir, common))
}

/// 체크아웃한 브랜치. detached면 `None`.
pub(super) fn current_branch(root: &Path) -> Option<String> {
    let (git_dir, _) = git_dirs(root)?;
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .map(str::to_string)
}

/// git을 한 번 실행한다. 성공하면 stdout이다.
pub(super) fn git(root: &Path, args: &[&str]) -> Result<String, CliError> {
    let argv: Vec<String> = std::iter::once("git")
        .chain(args.iter().copied())
        .map(String::from)
        .collect();
    let out = fullops_ext::run(
        &argv,
        root,
        Duration::from_secs(120),
        &[("GIT_TERMINAL_PROMPT", "0")],
        None,
    );
    if out.ok() {
        Ok(out.stdout_text())
    } else {
        Err(CliError::External(format!(
            "git {} 실패: {}",
            args.join(" "),
            out.stderr_text().trim()
        )))
    }
}

/// TOML 문자열 리터럴.
fn quoted(text: &str) -> String {
    toml::Value::String(text.to_string()).to_string()
}

pub(super) struct NewRole {
    pub id: RoleId,
    pub branch: String,
    pub kind: RoleKind,
    pub about: Option<String>,
}

/// 새 `fullops.toml`에 적을 값.
pub(super) struct NewConfig {
    pub mode: Mode,
    pub primary_role: Option<String>,
    pub primary_branch: Option<String>,
    pub routing: Routing,
    pub tech_plan_role: Option<String>,
    pub test_level: TestLevel,
    pub parallel: Parallel,
    pub base: String,
    pub remote: Option<String>,
    pub roles: Vec<NewRole>,
    /// `[review]`. 1.24.0의 `review_policy`를 옮길 때만 적는다.
    pub review: Option<String>,
}

fn role_table(role: &NewRole) -> String {
    let mut out = format!(
        "\n[roles.{}]\nbranch = {}\nkind = {}\n",
        role.id,
        quoted(&role.branch),
        quoted(role.kind.as_str())
    );
    if let Some(about) = &role.about {
        out.push_str(&format!("about = {}\n", quoted(about)));
    }
    out
}

/// `fullops.toml`의 글(설계 6.1.1절).
pub(super) fn config_text(new: &NewConfig) -> String {
    let mut out = format!(
        "schema = 2\napplied_version = {}\nmode = {}\n",
        quoted(env!("CARGO_PKG_VERSION")),
        quoted(new.mode.as_str())
    );
    let mut key = |name: &str, value: &Option<String>| {
        if let Some(value) = value {
            out.push_str(&format!("{name} = {}\n", quoted(value)));
        }
    };
    key("primary_role", &new.primary_role);
    key("primary_branch", &new.primary_branch);
    key("routing", &Some(new.routing.as_str().to_string()));
    key("tech_plan_role", &new.tech_plan_role);
    key("test_level", &Some(new.test_level.as_str().to_string()));
    key("parallel", &Some(new.parallel.as_str().to_string()));
    out.push_str(&format!("\n[git]\nbase = {}\n", quoted(&new.base)));
    if let Some(remote) = &new.remote {
        out.push_str(&format!("remote = {}\n", quoted(remote)));
    }
    for role in &new.roles {
        out.push_str(&role_table(role));
    }
    if let Some(review) = &new.review {
        out.push_str(&format!("\n[review]\n{review}"));
    }
    out
}

/// 첫 표(`[…]`) 앞의 최상위 키 하나를 바꾸거나 더한다. 주석과 나머지 글은 그대로 둔다.
pub(super) fn set_top_key(text: &str, key: &str, value: &str) -> String {
    let line = format!("{key} = {}", quoted(value));
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let first_table = lines
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .unwrap_or(lines.len());
    let found = lines.iter().take(first_table).position(|l| {
        l.trim_start()
            .strip_prefix(key)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    });
    match found {
        Some(at) => lines[at] = line,
        None => {
            // 빈 줄 뒤가 아니라 마지막 최상위 키 바로 뒤에 넣는다
            let at = lines
                .iter()
                .take(first_table)
                .rposition(|l| !l.trim().is_empty())
                .map_or(0, |i| i + 1);
            lines.insert(at, line);
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// 관리 블록의 표식이 한 쌍이 아니다. 어디가 블록인지 알 수 없어 그 문서는 고치지 않는다.
#[derive(Debug)]
pub(super) struct BrokenBlock;

/// 글에서 `start`·`end` 표식으로 감싼 블록의 자리. 표식이 없으면 `None`이다.
/// 하나만 있거나 여럿이거나 순서가 뒤집혔으면 `BrokenBlock`이다(`init`·`migrate`·`upgrade`가 함께 쓴다).
pub(super) fn block_span(
    text: &str,
    start: &str,
    end: &str,
) -> Result<Option<std::ops::Range<usize>>, BrokenBlock> {
    let once = text.matches(start).count() == 1 && text.matches(end).count() == 1;
    match (text.find(start), text.find(end)) {
        (None, None) => Ok(None),
        (Some(from), Some(to)) if once && from < to => Ok(Some(from..to + end.len())),
        _ => Err(BrokenBlock),
    }
}

/// 엔트리 문서의 글에 관리 블록을 넣는다. 이미 있으면 그 블록만 바꾼다.
pub(super) fn with_block(text: &str) -> Result<String, BrokenBlock> {
    let block = format!(
        "{}\n{}{}",
        assets::BLOCK_START,
        assets::MANAGED_BLOCK,
        assets::BLOCK_END
    );
    Ok(
        match block_span(text, assets::BLOCK_START, assets::BLOCK_END)? {
            Some(span) => format!("{}{block}{}", &text[..span.start], &text[span.end..]),
            None if text.trim().is_empty() => format!("{block}\n"),
            None => format!("{}\n\n{block}\n", text.trim_end()),
        },
    )
}

/// 레포 루트가 사용자 홈인가. 그러면 레포 폴더 `.fullops/`가 `~/.fullops/`(실행본·설치 기록)와 겹친다.
pub(super) fn is_home(root: &Path) -> bool {
    fullops_store::home()
        .is_some_and(|home| fullops_store::same_path(&path_text(&home), &path_text(root)))
}

pub(super) fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// 경로 자신이나 레포 안의 조상 폴더가 심볼릭 링크인가. 링크 대상에는 쓰지 않는다.
pub(super) fn linked(root: &Path, path: &Path) -> bool {
    path.ancestors().take_while(|a| *a != root).any(is_symlink)
}

/// 한 일의 기록. `--dry-run`이면 쓰지 않고 기록만 한다.
#[derive(Default)]
pub(super) struct Applied {
    pub created: Vec<String>,
    pub updated: Vec<String>,
    /// 심볼릭 링크라 건드리지 않은 대상.
    pub refused: Vec<String>,
    /// 관리 블록의 표식이 한 쌍이 아니라 고치지 않은 엔트리 문서.
    pub broken: Vec<String>,
}

fn write_new(path: &Path, text: &str, dry_run: bool) -> Result<(), CliError> {
    if dry_run {
        return Ok(());
    }
    Ok(fullops_store::write_atomic(path, text.as_bytes())?)
}

/// 내장 템플릿 가운데 없는 파일과 빈 폴더의 `.gitkeep`을 더한다.
pub(super) fn add_templates(
    root: &Path,
    dry_run: bool,
    done: &mut Applied,
) -> Result<(), CliError> {
    let squad = root.join(layout::DIR);
    if is_symlink(&squad) {
        done.refused.push(layout::DIR.into());
        return Ok(());
    }
    let keeps = assets::REPO_DIRS
        .iter()
        .map(|dir| (format!("{dir}/.gitkeep"), ""));
    let files = assets::REPO_FILES
        .iter()
        .map(|(rel, text)| (rel.to_string(), *text));
    for (rel, text) in files.chain(keeps) {
        let path = squad.join(&rel);
        let shown = layout::at(&rel);
        // 폴더에 이미 파일이 있으면 .gitkeep은 필요 없다
        let dir_used = rel.ends_with(".gitkeep")
            && path
                .parent()
                .is_some_and(|d| std::fs::read_dir(d).is_ok_and(|mut e| e.next().is_some()));
        if path.exists() || dir_used {
            continue;
        }
        if linked(root, &path) {
            done.refused.push(shown);
            continue;
        }
        write_new(&path, text, dry_run)?;
        done.created.push(shown);
    }
    Ok(())
}

/// `AGENTS.md`·`CLAUDE.md`·`GEMINI.md`에 관리 블록을 넣는다(없는 파일은 만든다).
pub(super) fn add_blocks(root: &Path, dry_run: bool, done: &mut Applied) -> Result<(), CliError> {
    for name in assets::ENTRY_DOCS {
        let path = root.join(name);
        if is_symlink(&path) {
            done.refused.push(name.to_string());
            continue;
        }
        let before = std::fs::read_to_string(&path).ok();
        let Ok(after) = with_block(before.as_deref().unwrap_or("")) else {
            done.broken.push(name.to_string());
            continue;
        };
        if before.as_deref() == Some(after.as_str()) {
            continue;
        }
        write_new(&path, &after, dry_run)?;
        match before {
            Some(_) => done.updated.push(name.to_string()),
            None => done.created.push(name.to_string()),
        }
    }
    Ok(())
}

struct Args {
    roles: Vec<String>,
    kinds: Vec<(String, String)>,
    mode: Option<String>,
    primary_role: Option<String>,
    remote: Option<String>,
    base: Option<String>,
    local_only: bool,
    test_level: Option<String>,
    parallel: Option<String>,
    dry_run: bool,
}

fn parse_args(mut parser: lexopt::Parser) -> Result<Args, CliError> {
    use lexopt::prelude::*;
    let mut args = Args {
        roles: Vec::new(),
        kinds: Vec::new(),
        mode: None,
        primary_role: None,
        remote: None,
        base: None,
        local_only: false,
        test_level: None,
        parallel: None,
        dry_run: false,
    };
    while let Some(arg) = parser.next()? {
        match arg {
            Long("roles") => {
                let value = parser.value()?.string()?;
                args.roles.extend(
                    value
                        .split([',', ' '])
                        .filter(|r| !r.is_empty())
                        .map(String::from),
                );
            }
            Long("kind") => {
                let value = parser.value()?.string()?;
                let (role, kind) = value.split_once('=').ok_or_else(|| {
                    CliError::Usage(
                        "--kind는 `<역할>=<worker|design|tester|coordinator>` 꼴입니다.".into(),
                    )
                })?;
                args.kinds.push((role.to_string(), kind.to_string()));
            }
            Long("mode") => args.mode = Some(parser.value()?.string()?),
            Long("primary-role") => args.primary_role = Some(parser.value()?.string()?),
            Long("remote") => args.remote = Some(parser.value()?.string()?),
            Long("base") => args.base = Some(parser.value()?.string()?),
            Long("local-only") => args.local_only = true,
            Long("test-level") => args.test_level = Some(parser.value()?.string()?),
            Long("parallel") => args.parallel = Some(parser.value()?.string()?),
            Long("dry-run") => args.dry_run = true,
            other => return Err(other.unexpected().into()),
        }
    }
    if args.local_only && (args.remote.is_some() || args.base.is_some()) {
        return Err(CliError::Usage(
            "--local-only와 --remote·--base는 함께 쓰지 않습니다.".into(),
        ));
    }
    Ok(args)
}

fn word<T>(
    parse: fn(&str) -> Option<T>,
    value: &str,
    name: &str,
    allowed: &str,
) -> Result<T, CliError> {
    parse(value).ok_or_else(|| {
        CliError::Usage(format!(
            "{name}의 값은 {allowed} 가운데 하나입니다: {value}"
        ))
    })
}

fn role_id(text: &str) -> Result<RoleId, CliError> {
    RoleId::new(text).map_err(|_| {
        CliError::Usage(format!(
            "역할 ID는 영문 소문자로 시작하고 소문자·숫자·밑줄·하이픈을 씁니다: {text}"
        ))
    })
}

fn kind_of(args: &Args, role: &str) -> Result<RoleKind, CliError> {
    match args.kinds.iter().find(|(r, _)| r == role) {
        Some((_, kind)) => word(
            RoleKind::parse,
            kind,
            "--kind",
            "worker, design, tester, coordinator",
        ),
        None => Ok(RoleKind::Worker),
    }
}

/// 새 레포의 `fullops.toml` 글.
fn fresh_config(args: &Args, root: &Path) -> Result<(String, Option<String>), CliError> {
    if args.roles.is_empty() {
        return Err(CliError::Usage(
            "--roles <역할,…>이 필요합니다. 예: fullops init --roles coor,designer,dev,ops,tester --kind designer=design".into(),
        ));
    }
    let mode = word(
        Mode::parse,
        args.mode.as_deref().unwrap_or("coor"),
        "--mode",
        "coor, dev",
    )?;
    let remote = (!args.local_only).then(|| args.remote.clone().unwrap_or_else(|| "origin".into()));
    if let Some(remote) = &remote {
        git(root, &["remote", "get-url", remote]).map_err(|_| {
            CliError::Usage(format!(
                "원격 `{remote}`이 없습니다. 원격을 연결하거나 --local-only를 줍니다."
            ))
        })?;
    }
    let mut roles = Vec::new();
    for role in &args.roles {
        let id = role_id(role)?;
        if roles.iter().any(|r: &NewRole| r.id == id) {
            continue;
        }
        roles.push(NewRole {
            branch: format!("fullops/{id}"),
            kind: kind_of(args, role)?,
            id,
            about: None,
        });
    }
    let dev = mode == Mode::Dev;
    if dev && args.primary_role.is_none() {
        return Err(CliError::Usage(
            "dev 모드에는 --primary-role <역할>이 필요합니다.".into(),
        ));
    }
    let new = NewConfig {
        mode,
        primary_role: args.primary_role.clone().filter(|_| dev),
        primary_branch: dev
            .then(|| current_branch(root))
            .flatten()
            .or(dev.then(|| "main".to_string())),
        routing: Routing::Legacy,
        tech_plan_role: None,
        test_level: word(
            TestLevel::parse,
            args.test_level.as_deref().unwrap_or("lite"),
            "--test-level",
            "minimal, lite, standard, full, exhaustive",
        )?,
        parallel: word(
            Parallel::parse,
            args.parallel.as_deref().unwrap_or("on"),
            "--parallel",
            "off, read, on, ultra",
        )?,
        base: args.base.clone().unwrap_or_else(|| "main".into()),
        remote: remote.clone(),
        roles,
        review: None,
    };
    Ok((config_text(&new), remote))
}

/// 있는 `fullops.toml`에 준 옵션만 반영한다. 역할은 더하기만 한다.
fn merged_config(args: &Args, before: &str, root: &Path) -> Result<String, CliError> {
    let table: toml::Table = toml::from_str(before).map_err(|e| {
        CliError::Usage(format!("fullops.toml을 읽을 수 없습니다: {}", e.message()))
    })?;
    let has_role = |id: &str| table.get("roles").and_then(|r| r.get(id)).is_some();
    let mut text = before.to_string();
    if let Some(level) = &args.test_level {
        word(
            TestLevel::parse,
            level,
            "--test-level",
            "minimal, lite, standard, full, exhaustive",
        )?;
        text = set_top_key(&text, "test_level", level);
    }
    if let Some(parallel) = &args.parallel {
        word(
            Parallel::parse,
            parallel,
            "--parallel",
            "off, read, on, ultra",
        )?;
        text = set_top_key(&text, "parallel", parallel);
    }
    if let Some(mode) = &args.mode {
        word(Mode::parse, mode, "--mode", "coor, dev")?;
        text = set_top_key(&text, "mode", mode);
        if mode == "dev" {
            let role = args.primary_role.as_deref().ok_or_else(|| {
                CliError::Usage("dev 모드에는 --primary-role <역할>이 필요합니다.".into())
            })?;
            text = set_top_key(&text, "primary_role", role);
            let branch = current_branch(root).unwrap_or_else(|| "main".into());
            text = set_top_key(&text, "primary_branch", &branch);
        }
    }
    for role in &args.roles {
        let id = role_id(role)?;
        if has_role(id.as_str()) {
            continue;
        }
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&role_table(&NewRole {
            branch: format!("fullops/{id}"),
            kind: kind_of(args, role)?,
            id,
            about: None,
        }));
    }
    Ok(text)
}

/// 원격에 없는 역할 브랜치 `fullops/<역할>`을 기준 브랜치의 tip에서 만든다. 있는 브랜치는 옮기지 않는다.
fn push_role_branches(
    root: &Path,
    remote: &str,
    base: &str,
    branches: &[String],
    dry_run: bool,
) -> Result<Vec<String>, CliError> {
    let tip = [
        format!("refs/remotes/{remote}/{base}"),
        format!("refs/heads/{base}"),
        "HEAD".into(),
    ]
    .into_iter()
    .find(|rev| {
        git(
            root,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{rev}^{{commit}}"),
            ],
        )
        .is_ok()
    })
    .ok_or_else(|| CliError::Usage(format!("기준 브랜치 `{base}`의 커밋을 찾지 못했습니다.")))?;
    let mut made = Vec::new();
    for branch in branches {
        let listed = git(root, &["ls-remote", "--heads", remote, branch])?;
        if !listed.trim().is_empty() {
            continue;
        }
        if !dry_run {
            git(
                root,
                &["push", remote, &format!("{tip}:refs/heads/{branch}")],
            )?;
        }
        made.push(format!("{remote}/{branch}"));
    }
    Ok(made)
}

pub fn run(ctx: &Ctx, parser: lexopt::Parser) -> Result<u8, CliError> {
    let args = parse_args(parser)?;
    let root = git_root(&ctx.start)?;
    let config_path = root.join(layout::config_toml());
    if is_symlink(&root.join(layout::DIR)) || is_symlink(&config_path) {
        return Err(CliError::Usage(
            "`.fullops`나 `fullops.toml`이 심볼릭 링크입니다. 링크 대상에는 쓰지 않습니다.".into(),
        ));
    }
    // 사용자 홈의 `.fullops/`는 실행본과 설치 기록의 자리다(설계 6.4절)
    if is_home(&root) {
        return Err(CliError::Refused(vec![Refusal {
            rule: "init",
            reason: "레포 루트가 사용자 홈이다. 레포 폴더 `.fullops/`가 `~/.fullops/`와 겹친다"
                .into(),
            escape: "홈 폴더가 아닌 레포에서 실행한다".into(),
            exit: 2,
        }]));
    }
    // Python판 레포에 새 표식을 만들면 두 판의 hook이 함께 돈다. 전환은 migrate가 한다
    if root.join(layout::LEGACY_DIR).join("fullops.json").exists() {
        return Err(CliError::Refused(vec![Refusal {
            rule: "init",
            reason: "Python판(`.fullops-squad/fullops.json`)을 쓰는 레포다".into(),
            escape: "`fullops migrate --from-python`으로 전환한다".into(),
            exit: 2,
        }]));
    }
    let before = std::fs::read_to_string(&config_path).ok();
    let text = match &before {
        Some(before) => merged_config(&args, before, &root)?,
        None => fresh_config(&args, &root)?.0,
    };
    // 쓰기 전에 검증한다. 어긋나면 아무것도 쓰지 않는다
    let config = fullops_store::parse_config(&text).map_err(|e| {
        CliError::Usage(format!(
            "이 구성으로는 fullops.toml을 만들 수 없습니다: {e}"
        ))
    })?;

    let mut done = Applied::default();
    if before.as_deref() != Some(text.as_str()) {
        write_new(&config_path, &text, args.dry_run)?;
        match before {
            Some(_) => done.updated.push(layout::config_toml().into()),
            None => done.created.push(layout::config_toml().into()),
        }
    }
    add_templates(&root, args.dry_run, &mut done)?;
    add_blocks(&root, args.dry_run, &mut done)?;
    let branches: Vec<String> = config.roles.values().map(|r| r.branch.clone()).collect();
    let pushed = match &config.remote {
        Some(remote) => push_role_branches(&root, remote, &config.base, &branches, args.dry_run)?,
        None => Vec::new(),
    };

    let mut lines = Vec::new();
    let head = if args.dry_run {
        "계획(쓰지 않는다)"
    } else {
        "초기화했다"
    };
    lines.push(format!("{head}: {}", path_text(&root)));
    for (label, items) in [
        ("만든 파일", &done.created),
        ("고친 파일", &done.updated),
        ("만든 원격 브랜치", &pushed),
        ("심볼릭 링크라 건드리지 않은 것", &done.refused),
        (msg::BLOCK_BROKEN, &done.broken),
    ] {
        if !items.is_empty() {
            lines.push(format!("{label}:\n  {}", items.join("\n  ")));
        }
    }
    if done.created.is_empty() && done.updated.is_empty() && pushed.is_empty() {
        lines.push("바꿀 것이 없다.".into());
    }
    lines.push("커밋하지 않았다. 변경을 확인하고 커밋한다. 역할 워크트리는 Orca로 만든다.".into());
    ctx.print(
        &lines.join("\n"),
        json!({
            "dry_run": args.dry_run, "root": path_text(&root), "created": done.created,
            "updated": done.updated, "branches": pushed, "refused": done.refused,
            "broken": done.broken,
        }),
    );
    Ok(EXIT_OK)
}
