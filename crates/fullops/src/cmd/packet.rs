//! `fullops packet build|check`: 탐색 패킷을 만들고 검사한다(설계 8.14절). 선택이고 Jev를 부르지 않는다(묶음 J).

use super::find::{
    Task, ids, inbox_round, index, load_task, path_refusal, prior, repo_path, same_head, shown,
    tree_of,
};
use crate::assets;
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::explore::{self, ContextResult, FindResult, Scope, Source};
use fullops_core::model::{
    EXPLORE_V, Packet, PacketCheck, PacketDecision, Refusal, RoleId, SourceBasis, TaskKey,
    parse_packet, sha256_hex,
};
use fullops_ext::{Git, GitCli};
use fullops_store::Repo;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// 기록 경로(설계 6.6절). 미커밋 변경이어도 명시 경로로 삼지 않는다.
const RECORDS: [&str; 5] = [
    ".fullops-squad/handovers/",
    ".fullops-squad/docs/evaluations/",
    ".fullops-squad/routes/",
    ".fullops-squad/reviews/",
    ".fullops-squad/PLANS.md",
];

struct BuildArgs {
    seeds: Vec<String>,
    required: Vec<String>,
    update_docs: Vec<String>,
    decisions: Vec<PacketDecision>,
    write_inbox: bool,
    force: bool,
}

/// 만든 결과(producer) 파일. 없거나 읽히지 않거나 `v`가 1이 아니면 없는 것이다. 있으면 그 바이트의 sha256을 적는다.
fn producer<T: DeserializeOwned>(
    repo: &Repo,
    file_name: &str,
    name: &str,
    hashes: &mut BTreeMap<String, String>,
) -> Option<T> {
    let bytes = std::fs::read(repo.explore_dir().join(file_name)).ok()?;
    let head: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let result = serde_json::from_slice(&bytes)
        .ok()
        .filter(|_| head["v"] == json!(EXPLORE_V))?;
    hashes.insert(name.to_string(), sha256_hex(&bytes));
    Some(result)
}

/// 작업 트리에서 원문 하나를 읽는다(`packet build` 4번). 지시서 파일의 글은 이미 읽은 인박스 글이다.
fn read_source(repo: &Repo, path: &str, task: &Task, basis: SourceBasis) -> Source {
    if path == task.inbox_path {
        let text = task.inbox.clone();
        return Source::Text { text, basis };
    }
    match path_refusal(repo, path) {
        Some("not a file") => return Source::Unreadable("missing"),
        Some(_) => return Source::Unreadable("sensitive"),
        None => {}
    }
    let full = repo.root.join(path);
    match std::fs::metadata(&full).map(|m| m.len()) {
        Err(_) => return Source::Unreadable("missing"),
        Ok(0) => return Source::Unreadable("empty"),
        Ok(size) if size > explore::MAX_BLOB as u64 => return Source::Unreadable("oversized"),
        Ok(_) => {}
    }
    let Ok(bytes) = std::fs::read(&full) else {
        return Source::Unreadable("missing");
    };
    match String::from_utf8(bytes) {
        Err(_) => Source::Unreadable("not utf-8"),
        Ok(text) if !explore::sendable_text(&text) => Source::Unreadable("sensitive"),
        Ok(text) => Source::Text { text, basis },
    }
}

/// `packet build` 한 번. 패킷 파일의 경로, 패킷, 있던 패킷을 다시 냈는가를 돌려준다.
fn build(
    repo: &Repo,
    git: &dyn Git,
    key: &TaskKey,
    role: &RoleId,
    args: &BuildArgs,
) -> Result<(PathBuf, Packet, bool), CliError> {
    let task = load_task(repo, git, key, role)?;
    let (tree, lint) = tree_of(git, &task.head)?;
    let mut map = index(repo, git, Scope::Code, &tree, lint.as_deref());
    let code: BTreeSet<String> = map.iter().map(|e| e.path.clone()).collect();
    let documents = index(repo, git, Scope::Documents, &tree, lint.as_deref());
    map.extend(documents.into_iter().filter(|e| !code.contains(&e.path)));

    let mut producer_sha256 = BTreeMap::new();
    let hashes = &mut producer_sha256;
    let find: Option<FindResult> = producer(repo, &format!("{key}-find.json"), "find", hashes);
    let name = format!("{key}-documents-find.json");
    let documents_find: Option<FindResult> = producer(repo, &name, "documents-find", hashes);
    let name = format!("{key}-context.json");
    let context: Option<ContextResult> = producer(repo, &name, "context", hashes);

    // 3번: 명시는 --seed와 추적 중인 미커밋 변경(기록 경로 제외)
    let status = git
        .status(None)
        .yes()
        .ok_or_else(|| CliError::External("git status를 읽지 못했습니다.".into()))?;
    let changed: BTreeSet<&str> = status
        .iter()
        .filter(|s| !matches!(s.index, '?' | '!'))
        .map(|s| s.path.as_str())
        .collect();
    let inside = |raw: &String| {
        repo_path(repo, raw)
            .map_err(|text| CliError::Usage(format!("레포 밖의 경로입니다: {text}")))
    };
    let mut seeds: Vec<String> = args.seeds.iter().map(inside).collect::<Result<_, _>>()?;
    let record = |path: &str| RECORDS.iter().any(|r| path.starts_with(r));
    seeds.extend(changed.iter().filter(|p| !record(p)).map(|p| p.to_string()));
    let required: Vec<String> = args.required.iter().map(inside).collect::<Result<_, _>>()?;
    let route = fullops_store::read_route(repo, key).yes().flatten();
    let deliverables = repo.squad().join("docs/deliverables/README.md");
    let deliverables = fullops_store::read_text(&deliverables)
        .yes()
        .flatten()
        .map(|text| fullops_core::parse_deliverable_index(&text))
        .unwrap_or_default();

    // 4번: 원문 읽기
    let tracked: BTreeSet<&str> = tree.iter().map(|e| e.path.as_str()).collect();
    let basis = |path: &str| match (changed.contains(path), tracked.contains(path)) {
        (true, _) => SourceBasis::Worktree,
        (false, true) => SourceBasis::Head,
        (false, false) => SourceBasis::Untracked,
    };
    let sources: BTreeMap<String, Source> = explore::packet_source_paths(
        &map,
        find.as_ref(),
        documents_find.as_ref(),
        context.as_ref(),
        &seeds,
        &required,
        &task.inbox_path,
    )
    .into_iter()
    .map(|path| {
        let source = read_source(repo, &path, &task, basis(&path));
        (path, source)
    })
    .collect();

    let packet = explore::build_packet_from(
        &explore::PacketInput {
            key,
            role,
            round: task.round,
            head: &task.head,
            created_at: fullops_store::now(),
            instruction_path: &task.inbox_path,
            instruction: &task.inbox,
            worktree_dirty: !status.is_empty(),
            map: &map,
            find: find.as_ref(),
            documents_find: documents_find.as_ref(),
            context: context.as_ref(),
            producer_sha256: &producer_sha256,
            seeds: &seeds,
            required: &required,
            update_docs: &args.update_docs,
            route_deliverables: route.as_ref().map_or(&[], |r| &r.deliverables),
            decisions: &args.decisions,
            sources: &sources,
            deliverables: &deliverables,
        },
        assets::HANDOVER_TEMPLATE,
    )
    .map_err(CliError::Refused)?;

    // 1~9번을 마친 뒤 입력을 견준다
    let file_name = format!("{key}-packet.json");
    let header = task.header(&packet.options, &packet.input_sha256);
    let old =
        prior(repo, &file_name, &header, args.force, &task)?.and_then(|bytes| parse_packet(&bytes));
    let (packet, reused) = match old {
        Some(old) => (old, true),
        None => {
            same_head(git, &task)?;
            fullops_store::write_explore_result(repo, &file_name, &packet, true)?;
            (packet, false)
        }
    };
    let path = repo.packet_path(key);
    // 블록은 지시서 지문에서 빠지므로 instruction_sha256도 지시서 항목의 source_sha256도 바뀌지 않는다
    if args.write_inbox {
        let block = explore::packet_block(&packet, &shown(repo, &path));
        let text = explore::with_block(&task.inbox, &block);
        fullops_store::write_atomic(&repo.inbox_path(role), text.as_bytes())?;
    }
    Ok((path, packet, reused))
}

/// 패킷 검사(설계 8.14절 `check_packet`)를 이 체크아웃에서 한다. 패킷 조건이 참이었는가와 걸린 것을 돌려준다.
/// 지시서 글은 인박스가 그 과제의 것이면 인박스, 아니면(완료 뒤) 완료 기록에서 읽는다.
fn check(
    repo: &Repo,
    git: &dyn Git,
    key: &TaskKey,
    role: &RoleId,
    mode: PacketCheck,
    head: &str,
) -> (bool, Vec<Refusal>) {
    let route = fullops_store::read_route(repo, key).yes().flatten();
    let packet = fullops_store::read_packet(repo, key).yes().flatten();
    let outcomes = fullops_store::read_packet_outcomes(repo, key)
        .yes()
        .flatten();
    let read = |path: PathBuf| fullops_store::read_text(&path).yes().flatten();
    let inbox = read(repo.inbox_path(role))
        .and_then(|text| inbox_round(&text, key).map(|round| (Some(text), round)));
    let (instruction, round) = inbox.unwrap_or_else(|| {
        let round = packet.as_ref().map_or(0, |p| p.round);
        (read(repo.handover_log_path(key, role, round)), round)
    });
    let refusals = explore::check_packet(
        mode,
        route.as_ref(),
        packet.as_ref(),
        outcomes.as_ref(),
        instruction.as_deref(),
        key,
        role,
        round,
        &mut |older| git.is_ancestor(older, head),
        &mut |path| fullops_store::read_text(&repo.root.join(path)),
    );
    let on = packet.is_some() || route.is_some_and(|r| r.requires_packet);
    (on, refusals)
}

fn run_build(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    use lexopt::prelude::*;
    let (mut key, mut role, mut decisions) = (None, None, None::<PathBuf>);
    let mut build_args = BuildArgs {
        seeds: Vec::new(),
        required: Vec::new(),
        update_docs: Vec::new(),
        decisions: Vec::new(),
        write_inbox: false,
        force: false,
    };
    while let Some(arg) = args.next()? {
        match arg {
            Value(value) if key.is_none() => key = Some(value.string()?),
            Long("role") => role = Some(args.value()?.string()?),
            Long("seed") => build_args.seeds.push(args.value()?.string()?),
            Long("required") => build_args.required.push(args.value()?.string()?),
            Long("update-doc") => build_args.update_docs.push(args.value()?.string()?),
            Long("decisions") => decisions = Some(args.value()?.into()),
            Long("write-inbox") => build_args.write_inbox = true,
            Long("force") => build_args.force = true,
            other => return Err(other.unexpected().into()),
        }
    }
    let (key, role) = ids(key, role)?;
    if let Some(file) = decisions {
        let bytes = std::fs::read(&file)?;
        build_args.decisions = serde_json::from_slice(&bytes).map_err(|_| {
            let what = format!(
                "`{}`을 [{{path, action, reason}}] 목록으로 읽을 수 없다",
                file.display()
            );
            CliError::Refused(vec![explore::refuse("J-03", &[what], &key, &role)])
        })?;
    }
    let repo = ctx.repo()?;
    let git = GitCli::cli(repo.root.clone());
    let (path, packet, reused) = build(repo, &git, &key, &role, &build_args)?;
    let path = shown(repo, &path);
    let pairs: BTreeSet<&str> = packet.delivered.iter().map(|d| d.path.as_str()).collect();
    let human = format!(
        "{path}\n항목 {} · 전달 {} · partial={} · 미확인 {}",
        packet.items.len(),
        pairs.len(),
        packet.partial,
        packet.unknown.len()
    );
    ctx.print(
        &human,
        json!({ "path": path, "reused": reused, "packet": packet }),
    );
    Ok(EXIT_OK)
}

fn run_check(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    use lexopt::prelude::*;
    let (mut key, mut role, mut head) = (None, None, "HEAD".to_string());
    let mut mode = PacketCheck::Delivery;
    while let Some(arg) = args.next()? {
        match arg {
            Value(value) if key.is_none() => key = Some(value.string()?),
            Long("role") => role = Some(args.value()?.string()?),
            Long("completion") => mode = PacketCheck::Completion,
            Long("head") => head = args.value()?.string()?,
            other => return Err(other.unexpected().into()),
        }
    }
    let (key, role) = ids(key, role)?;
    let repo = ctx.repo()?;
    let (on, refusals) = check(
        repo,
        &GitCli::cli(repo.root.clone()),
        &key,
        &role,
        mode,
        &head,
    );
    if !refusals.is_empty() {
        return Err(CliError::Refused(refusals));
    }
    let human = match on {
        true => "패킷 검사를 통과했습니다.",
        false => "이 과제에는 패킷이 없어 검사하지 않았습니다.",
    };
    ctx.print(human, json!({ "checked": on }));
    Ok(EXIT_OK)
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let sub = match args.next()? {
        Some(lexopt::Arg::Value(value)) => value.to_string_lossy().into_owned(),
        _ => String::new(),
    };
    match sub.as_str() {
        "build" => run_build(ctx, args),
        "check" => run_check(ctx, args),
        _ => Err(CliError::Usage(
            "사용법: fullops packet build <KEY> --role <R> [--seed <경로>]... [--required <경로>]... [--update-doc <Dnn>]... [--decisions <JSON 파일>] [--write-inbox] [--force] | fullops packet check <KEY> --role <R> [--completion] [--head <SHA>]".into(),
        )),
    }
}

/// 묶음 사이에 부르는 진입점(설계 10.2절). `task recover`(C)가 부른다.
/// 패킷 파일이 없거나 `v != 1`이면 `Ok(None)`. 있으면 그 `options`(`seeds`에 `extra_seeds`를 더한다)와 `decisions`로
/// `packet build --force --write-inbox`와 같은 일을 하고 패킷 경로를 돌려준다. 커밋은 부르는 쪽이 한다.
/// 거절이 아닌 실패(git·입출력)는 규칙 이름 `packet`의 `Refusal`로 돌려주고 `exit`가 그 종료 코드다.
pub fn rebuild_packet(
    repo: &Repo,
    key: &TaskKey,
    role: &RoleId,
    extra_seeds: &[String],
) -> Result<Option<PathBuf>, Refusal> {
    let Some(old) = fullops_store::read_packet(repo, key).yes().flatten() else {
        return Ok(None);
    };
    let args = BuildArgs {
        seeds: old
            .options
            .seeds
            .iter()
            .chain(extra_seeds)
            .cloned()
            .collect(),
        required: old.options.required,
        update_docs: old.options.update_docs,
        decisions: old.options.decisions,
        write_inbox: true,
        force: true,
    };
    match build(repo, &GitCli::cli(repo.root.clone()), key, role, &args) {
        Ok((path, ..)) => Ok(Some(path)),
        Err(CliError::Refused(mut refused)) if !refused.is_empty() => Err(refused.remove(0)),
        Err(e) => Err(Refusal {
            rule: "packet",
            exit: e.exit(),
            reason: match e {
                CliError::Usage(text) | CliError::Internal(text) | CliError::External(text) => text,
                _ => "패킷을 다시 만들지 못했습니다.".into(),
            },
            escape: format!("`fullops packet build {key} --role {role} --force --write-inbox`"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::super::find::tests::sandbox;
    use super::*;
    use fullops_core::model::{PacketCategory, PacketItemStatus};

    fn args() -> BuildArgs {
        BuildArgs {
            seeds: vec!["src/pay/retry.rs".into()],
            required: Vec::new(),
            update_docs: Vec::new(),
            decisions: Vec::new(),
            write_inbox: true,
            force: false,
        }
    }

    // P2·P3·P12의 뼈대: 만든 패킷이 배정 검사를 통과하고, rebuild_packet이 seed를 더해 다시 만들며, 패킷 없는 과제는 건드리지 않는다
    #[test]
    fn packet_survives_its_own_inbox_block_and_rebuilds_with_extra_seeds() {
        let (sb, repo, key, role) =
            sandbox("`retry`의 중복 클릭을 막는다. 호출하는 곳은 `charge_card`다.");
        let lead = sb.path("lead");
        assert_eq!(rebuild_packet(&repo, &key, &role, &[]), Ok(None));
        sb.write(
            &lead.join("src/pay/card.rs"),
            "pub fn charge_card() -> u32 {\n    retry(1)\n}\n",
        );
        sb.write(
            &lead.join("docs/pay.md"),
            "# 결제 설계\n\n[재시도](../src/pay/retry.rs) 정책.\n",
        );
        sb.git_ok(&lead, &["add", "src", "docs"]);
        sb.git_ok(&lead, &["commit", "-q", "-m", "호출하는 곳"]);
        let git = GitCli::cli(repo.root.clone());
        let (path, packet, reused) = build(&repo, &git, &key, &role, &args()).unwrap();
        assert!(
            !reused && path.is_file() && !packet.partial,
            "{:?}",
            packet.unknown
        );
        let item = |packet: &Packet, path: &str| {
            packet
                .items
                .iter()
                .find(|i| i.path == path)
                .cloned()
                .unwrap()
        };
        // 명시 경로는 direct_edit이고 정의 근거가 붙는다
        let seed = item(&packet, "src/pay/retry.rs");
        assert_eq!(seed.categories, [PacketCategory::DirectEdit]);
        assert_eq!(
            (
                seed.evidence[0].method.as_str(),
                seed.evidence[0].symbol.as_deref(),
                seed.evidence[0].line
            ),
            ("definition", Some("retry"), Some(1))
        );
        // 심볼이 나오는 파일은 impact_check, 명시 경로로 링크한 문서는 document_read다
        let caller = item(&packet, "src/pay/card.rs");
        assert_eq!(caller.categories, [PacketCategory::ImpactCheck]);
        assert!(caller.evidence.iter().any(|e| e.method == "string-search"
            && e.symbol.as_deref() == Some("retry")
            && e.line == Some(2)));
        assert!(
            item(&packet, "docs/pay.md")
                .evidence
                .iter()
                .any(|e| e.method == "markdown-link"
                    && e.seed.as_deref() == Some("src/pay/retry.rs"))
        );
        // 지시서 파일은 context 없이도 필수 항목이고 지문은 지시서 지문이다
        let inbox = item(&packet, ".fullops-squad/handovers/to_dev.md");
        assert!(inbox.required && inbox.status == PacketItemStatus::Inferred);
        assert_eq!(
            inbox.source_sha256.as_deref(),
            Some(packet.instruction_sha256.as_str())
        );
        assert_eq!(packet.required_paths.len(), 7);
        assert!(packet.delivered.iter().any(|d| d.path == "src/pay/card.rs"));

        // --write-inbox 직후의 배정 검사가 걸리지 않는다. 같은 입력이면 있던 패킷을 다시 낸다
        let text = std::fs::read_to_string(repo.inbox_path(&role)).unwrap();
        assert!(
            text.contains(explore::BLOCK_START)
                && text.contains("- `src/pay/retry.rs` (direct_edit) · 줄 1 · inferred")
        );
        assert_eq!(
            check(&repo, &git, &key, &role, PacketCheck::Delivery, "HEAD"),
            (true, Vec::new())
        );
        let (_, again, reused) = build(&repo, &git, &key, &role, &args()).unwrap();
        assert!(reused && again == packet);
        assert_eq!(
            std::fs::read_to_string(repo.inbox_path(&role)).unwrap(),
            text
        );

        // task recover: WIP 커밋이 바꾼 경로가 seed에 더해져 direct_edit 항목이 된다
        sb.write(
            &lead.join("src/pay/card.rs"),
            "pub fn charge_card() -> u32 {\n    retry(2)\n}\n",
        );
        sb.git_ok(&lead, &["commit", "-q", "-am", "WIP"]);
        let rebuilt = rebuild_packet(&repo, &key, &role, &["src/pay/card.rs".into()]).unwrap();
        assert_eq!(rebuilt.as_deref(), Some(path.as_path()));
        let packet = fullops_store::read_packet(&repo, &key)
            .yes()
            .flatten()
            .unwrap();
        assert_eq!(
            packet.options.seeds,
            ["src/pay/card.rs", "src/pay/retry.rs"]
        );
        assert!(
            item(&packet, "src/pay/card.rs")
                .categories
                .contains(&PacketCategory::DirectEdit)
        );
        assert_eq!(
            check(&repo, &git, &key, &role, PacketCheck::Delivery, "HEAD"),
            (true, Vec::new())
        );
        assert!(
            repo.explore_dir()
                .join("history")
                .read_dir()
                .unwrap()
                .count()
                == 1
        );
    }
}
