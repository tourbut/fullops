//! `fullops lint`: lint 실행과 통과 기록(설계 8.11절).

use crate::assets;
use crate::cli::{CliError, Ctx, EXIT_OK, EXIT_REFUSED};
use fullops_core::config::is_code;
use fullops_core::lint_rules::{self as rules, FileChange, LintCommand, LintConfig};
use fullops_core::messages::lint as msg;
use fullops_core::model::{
    Known, LintCommandResult, LintPass, LintResult, LintStatus, Refusal, Seat, Severity, Sha,
    TestLevel, path_text, sha256_hex,
};
use fullops_ext::{Git, GitCli, ProcStatus};
use fullops_store::StoreError;
use lexopt::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const LINT_JSON: &str = ".fullops-squad/lint/lint.json";
const CONFIG_TOML: &str = ".fullops-squad/fullops.toml";
const INDEX: &str = ".fullops-squad/docs/deliverables/README.md";

/// git을 읽지 못하면 실행 실패(종료 코드 1)다.
fn known<T>(value: Known<T>) -> Result<T, CliError> {
    match value {
        Known::Yes(value) => Ok(value),
        Known::Unknown(why) => Err(CliError::Internal(format!(
            "git 호출이 실패했습니다({why})."
        ))),
    }
}

fn tenths(elapsed: Duration) -> f64 {
    (elapsed.as_secs_f64() * 10.0).round() / 10.0
}

/// `--from`이 없을 때의 시작점: `changed()`의 시작점과 같은 규칙이다(설계 6.6절 1~3번).
fn default_start(ctx: &Ctx, git: &GitCli, head: &Sha) -> Result<Sha, CliError> {
    let (repo, config) = (ctx.repo()?, ctx.config()?);
    let caller = ctx.caller()?;
    let inbox = match &caller.actor.seat {
        Seat::Role { id, .. } => fullops_store::read_inbox(repo, id)
            .yes()
            .flatten()
            .map(|inbox| inbox.base),
        _ => None,
    };
    let start = inbox.or_else(|| caller.session.and_then(|s| s.anchor));
    let base_ref = match &config.remote {
        Some(remote) => format!("refs/remotes/{remote}/{}", config.base),
        None => format!("refs/heads/{}", config.base),
    };
    let floor = git.merge_base(head.as_str(), &base_ref).yes();
    let under = |older: &Sha, newer: &Sha| {
        git.is_ancestor(older.as_str(), newer.as_str()).yes() == Some(true)
    };
    start
        .filter(|start| under(start, head) && !floor.as_ref().is_some_and(|f| under(start, f)))
        .or(floor)
        .ok_or_else(|| {
            CliError::Usage("lint의 기준을 정할 수 없습니다. `--from <ref>`를 줍니다.".into())
        })
}

fn entry(command: &LintCommand, status: LintStatus, reason: String) -> LintCommandResult {
    LintCommandResult {
        name: command.name.clone(),
        kind: command.kind.clone(),
        run: command.run.clone(),
        cwd: command.cwd.clone(),
        status,
        exit_code: None,
        seconds: 0.0,
        reason,
        output_tail: String::new(),
        log: String::new(),
    }
}

/// Windows의 CreateProcess는 `.exe`만 붙여 본다. `npx`처럼 확장자 없이 적은 `.cmd`·`.bat` 도구를 PATH에서 찾아 준다.
#[cfg(windows)]
fn resolve_program(name: &str) -> String {
    if name.contains(['/', '\\', '.']) {
        return name.to_string();
    }
    let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        for ext in exts.split(';') {
            let candidate = dir.join(format!("{name}{ext}"));
            if candidate.is_file() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    name.to_string()
}

#[cfg(not(windows))]
fn resolve_program(name: &str) -> String {
    name.to_string()
}

/// 명령 하나를 셸 없이 실행한다. 출력 전체는 `logs/<name>.log`에, 끝 40줄은 결과에 둔다(설계 8.11절 8번).
fn execute(root: &Path, logs: &Path, command: &LintCommand, timeout: u64) -> LintCommandResult {
    let seconds = command.timeout_seconds.unwrap_or(timeout);
    let mut argv = command.run.clone();
    if let Some(program) = argv.first_mut() {
        *program = resolve_program(program);
    }
    let clock = Instant::now();
    let done = fullops_ext::run(
        &argv,
        &root.join(&command.cwd),
        Duration::from_secs(seconds),
        &[],
        None,
    );
    let (status, exit_code, reason) = match done.status {
        ProcStatus::Exited(0) => (LintStatus::Passed, Some(0), String::new()),
        ProcStatus::Exited(code) => (LintStatus::Failed, Some(code), String::new()),
        ProcStatus::TimedOut => (
            LintStatus::Timeout,
            None,
            format!("제한 시간 {seconds}초를 넘겨 프로세스 트리를 끝냄"),
        ),
        ProcStatus::NotFound => (
            LintStatus::Unrunnable,
            None,
            "실행 파일이나 cwd를 찾지 못함".into(),
        ),
    };
    let mut entry = entry(command, status, reason);
    entry.exit_code = exit_code;
    entry.seconds = tenths(clock.elapsed());
    let output = format!("{}{}", done.stdout_text(), done.stderr_text());
    let lines: Vec<&str> = output.lines().collect();
    entry.output_tail = lines
        .get(lines.len().saturating_sub(40)..)
        .unwrap_or_default()
        .join("\n");
    if !output.is_empty() {
        let name: String = command
            .name
            .chars()
            .map(|c| match c {
                c if c.is_alphanumeric() || "_.-".contains(c) => c,
                _ => '_',
            })
            .collect();
        let log = logs.join(format!("{name}.log"));
        if std::fs::create_dir_all(logs)
            .and_then(|()| std::fs::write(&log, &output))
            .is_ok()
        {
            entry.log = path_text(&log);
        }
    }
    entry
}

/// 건너뛰지 않은 명령을 실행해 빈자리를 채운다(설계 8.11절 7번).
/// 시험이 아닌 명령을 차례로 모두 돌리고, 하나라도 실패하면 시험 명령은 건너뛴다. 통과하면 시험 명령을 동시에 돌린다.
fn run_stages(
    root: &Path,
    logs: &Path,
    config: &LintConfig,
    results: &mut [Option<LintCommandResult>],
) {
    let pending = |results: &[Option<LintCommandResult>], test: bool| -> Vec<usize> {
        (0..results.len().min(config.commands.len()))
            .filter(|&i| results[i].is_none() && config.commands[i].is_test() == test)
            .collect()
    };
    let exec = |i: usize| execute(root, logs, &config.commands[i], config.timeout_seconds);
    let failed =
        |r: &LintCommandResult| matches!(r.status, LintStatus::Failed | LintStatus::Timeout);
    let mut broke = false;
    for i in pending(results, false) {
        let done = exec(i);
        broke |= failed(&done);
        results[i] = Some(done);
    }
    let tests = pending(results, true);
    if broke || tests.len() < 2 || !config.parallel_tests {
        // 차례로 돌 때는 시험 하나가 실패하면 남은 시험을 건너뛴다
        for i in tests {
            let done = if broke {
                entry(
                    &config.commands[i],
                    LintStatus::Skipped,
                    msg::SKIP_EARLIER.into(),
                )
            } else {
                exec(i)
            };
            broke |= failed(&done);
            results[i] = Some(done);
        }
        return;
    }
    let done: Vec<LintCommandResult> = std::thread::scope(|scope| {
        let handles: Vec<_> = tests
            .iter()
            .map(|&i| (i, scope.spawn(move || exec(i))))
            .collect();
        handles
            .into_iter()
            .map(|(i, handle)| {
                handle.join().unwrap_or_else(|_| {
                    entry(
                        &config.commands[i],
                        LintStatus::Unrunnable,
                        "실행 중 내부 오류".into(),
                    )
                })
            })
            .collect()
    });
    for (i, done) in tests.into_iter().zip(done) {
        results[i] = Some(done);
    }
}

fn human(result: &LintResult) -> String {
    let mut lines = Vec::new();
    for c in &result.commands {
        let note = match c.status {
            LintStatus::Skipped => format!(" (건너뜀: {})", c.reason),
            _ if c.kind == "rule" => String::new(),
            _ => format!(" ({}초)", c.seconds),
        };
        lines.push(format!(
            "[{}] {}: {}{note}",
            c.status,
            c.name,
            c.run.join(" ")
        ));
        if matches!(
            c.status,
            LintStatus::Failed | LintStatus::Timeout | LintStatus::Unrunnable
        ) {
            lines.extend(
                [&c.reason, &c.output_tail]
                    .into_iter()
                    .filter(|t| !t.is_empty())
                    .cloned(),
            );
            if !c.log.is_empty() {
                lines.push(format!("전체 출력: {}", c.log));
            }
        }
    }
    for f in &result.findings {
        let line = f.line.map(|n| format!(":{n}")).unwrap_or_default();
        let place = format!("{}{line}", f.path);
        let parts = [
            f.severity.as_str(),
            &f.code,
            &place,
            &f.message,
            &f.suggestion,
        ];
        lines.push(
            parts
                .into_iter()
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    lines.push(format!(
        "{}: head {} / ERROR {} / WARNING {} / 실행 불가 {}",
        if result.passed { "통과" } else { "미통과" },
        result.head.short(12),
        result.errors,
        result.warnings,
        result.unrunnable
    ));
    lines.join("\n")
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let (mut from, mut raise, mut out) = (None, None, None);
    while let Some(arg) = args.next()? {
        match arg {
            Long("from") => from = Some(args.value()?.string()?),
            Long("level") => {
                let text = args.value()?.string()?;
                raise = Some(TestLevel::parse(&text).ok_or_else(|| {
                    CliError::Usage(format!(
                        "--level은 minimal·lite·standard·full·exhaustive 가운데 하나입니다: {text}"
                    ))
                })?);
            }
            Long("out") => out = Some(PathBuf::from(args.value()?)),
            other => return Err(other.unexpected().into()),
        }
    }
    let repo = ctx.repo()?;
    let git = GitCli::cli(repo.root.clone());

    // 1. 깨끗한 작업 트리와 기준
    let dirty = known(git.status(None))?;
    if !dirty.is_empty() {
        let files: Vec<&str> = dirty.iter().take(3).map(|e| e.path.as_str()).collect();
        return Err(CliError::Refused(vec![Refusal {
            rule: "LINT",
            reason: msg::reason("LINT", &[files.join(", ")]).unwrap_or_default(),
            escape: "커밋한 뒤 다시 실행합니다. 통과 뒤 완료 기록만 바꾼 커밋은 다시 돌리지 않아도 됩니다.".into(),
            exit: EXIT_REFUSED,
        }]));
    }
    let head = known(git.rev_parse("HEAD^{commit}"))?;
    let base = match &from {
        Some(from) => git
            .rev_parse(&format!("{from}^{{commit}}"))
            .yes()
            .ok_or_else(|| CliError::Usage(format!("--from {from}을 커밋으로 풀 수 없습니다.")))?,
        None => default_start(ctx, &git, &head)?,
    };

    // 2. 잠금과 이전 결과 지우기
    let dir = repo.lint_dir();
    let _lock =
        fullops_store::lock::acquire(&dir.join("lock"), Duration::ZERO).map_err(|e| match e {
            StoreError::LockBusy => CliError::Internal(
                "이 체크아웃의 lint가 실행 중입니다. 끝난 뒤 다시 실행합니다.".into(),
            ),
            other => other.into(),
        })?;
    let logs = dir.join("logs");
    for stale in ["result.json", "pass.json"] {
        let _ = std::fs::remove_file(dir.join(stale));
    }
    let _ = std::fs::remove_dir_all(&logs);
    let (started_at, clock) = (fullops_store::now(), Instant::now());

    // 3. 기준 시점의 설정. 브랜치가 스스로 규칙을 느슨하게 하지 못한다
    let mb = known(git.merge_base(base.as_str(), head.as_str()))?;
    let at = |rev: &Sha, path: &str| (rev.as_str().to_string(), path.to_string());
    let settings = known(git.show_blobs(&[
        at(&mb, LINT_JSON),
        at(&mb, CONFIG_TOML),
        at(&head, LINT_JSON),
        at(&head, INDEX),
    ]))?;
    let bytes = |i: usize| settings.get(i).and_then(|blob| blob.as_deref());
    let text = |i: usize| bytes(i).map(|b| String::from_utf8_lossy(b).into_owned());
    let lint_json = text(0);
    let config =
        rules::parse_lint_config(lint_json.as_deref(), assets::LINT_JSON).map_err(|e| {
            CliError::Internal(format!(
                "기준 시점({})의 lint.json을 쓸 수 없습니다: {e}",
                mb.short(12)
            ))
        })?;
    let baseline = fullops_store::baseline(text(1).as_deref());
    if baseline.fell_back {
        ctx.note("기준 시점 설정을 읽지 못해 기본값을 썼다.");
    }
    let exclude = rules::lint_excludes(lint_json.as_deref(), assets::LINT_JSON);
    let config_sha256 = sha256_hex(bytes(0).unwrap_or(assets::LINT_JSON.as_bytes()));
    let level = raise.map_or(baseline.test_level, |l| l.max(baseline.test_level));
    let index = rules::parse_deliverable_index(&text(3).unwrap_or_default());
    let mut findings = Vec::new();
    if bytes(0) != bytes(2) {
        findings.push(rules::config_finding("LINT-001", LINT_JSON));
    }

    // 4. 바뀐 파일과 추가된 줄
    let items = known(git.name_status(mb.as_str(), head.as_str()))?;
    let added = rules::added_lines(&known(git.diff_u0(mb.as_str(), head.as_str()))?);
    let old_path = |item: &fullops_ext::NameStatus| match item.status {
        'A' => None,
        _ => Some(item.old_path.clone().unwrap_or_else(|| item.path.clone())),
    };
    let mut wanted = Vec::new();
    for item in items.iter().filter(|i| i.status != 'D') {
        wanted.push(at(&head, &item.path));
        if let (Some(old), false) = (old_path(item), exclude.is_match(&item.path)) {
            wanted.push(at(&mb, &old));
        }
    }
    // NUL이 든 바이너리와 blob이 아닌 것(서브모듈)은 내용 검사를 건너뛴다
    let contents: HashMap<(String, String), String> = wanted
        .iter()
        .cloned()
        .zip(known(git.show_blobs(&wanted))?)
        .filter_map(|(key, blob)| {
            let blob = blob.filter(|b| !b.contains(&0))?;
            Some((key, String::from_utf8_lossy(&blob).into_owned()))
        })
        .collect();

    // 9. 내장 검사와 정규식 규칙
    let (regex_rules, broken) = rules::compile_rules(&config.rules);
    let (mut added_total, mut changed, mut code_changed) = (0u32, Vec::new(), false);
    for item in &items {
        let deleted = item.status == 'D';
        let old_path = old_path(item);
        let paths: Vec<&str> = old_path
            .as_deref()
            .into_iter()
            .chain((!deleted).then_some(item.path.as_str()))
            .collect();
        changed.extend(paths.iter().map(|p| p.to_string()));
        code_changed |= paths.iter().any(|p| is_code(p, &baseline.code, &exclude));
        let new = contents
            .get(&at(&head, &item.path))
            .filter(|_| !deleted)
            .map(String::as_str);
        if let Some(new) = new {
            findings.extend(rules::doc_findings(&item.path, new, &index));
        }
        if exclude.is_match(&item.path) {
            continue;
        }
        findings.extend(rules::dependency_finding(&item.path, &paths));
        if deleted {
            findings.extend(rules::deleted_test_finding(&item.path));
            continue;
        }
        let old = match &old_path {
            Some(path) => match contents.get(&at(&mb, path)) {
                Some(old) => Some(old.as_str()),
                None => continue,
            },
            None => None,
        };
        let Some(new) = new else { continue };
        let lines = added.get(&item.path).cloned().unwrap_or_default();
        added_total += lines.len() as u32;
        let change = FileChange {
            path: &item.path,
            old,
            new,
            added: &lines,
        };
        findings.extend(rules::check_file(
            &change,
            &config,
            &regex_rules,
            &baseline.code,
        ));
    }
    findings.extend(rules::diff_size_finding(
        added_total,
        config.size.max_diff_added,
    ));

    // 6~8. 명령 선택과 실행
    let mut results: Vec<Option<LintCommandResult>> = config
        .commands
        .iter()
        .map(|command| {
            rules::skip_reason(command, level, &changed, code_changed)
                .map(|reason| entry(command, LintStatus::Skipped, reason))
        })
        .collect();
    run_stages(&repo.root, &logs, &config, &mut results);
    let mut commands: Vec<LintCommandResult> = results.into_iter().flatten().collect();
    for (command, done) in config.commands.iter().zip(&commands) {
        if done.status != LintStatus::Skipped {
            findings.extend(rules::slow_finding(command, done.seconds, LINT_JSON));
        }
    }
    if config.commands.is_empty() {
        findings.push(rules::config_finding("LINT-000", LINT_JSON));
    }
    // 컴파일하지 못한 정규식 규칙은 실행 불가 한 건으로 센다
    commands.extend(broken.into_iter().map(|(code, error)| LintCommandResult {
        reason: format!("정규식 규칙 {code}를 컴파일하지 못함: {error}"),
        name: code,
        kind: "rule".into(),
        run: Vec::new(),
        cwd: String::new(),
        status: LintStatus::Unrunnable,
        exit_code: None,
        seconds: 0.0,
        output_tail: String::new(),
        log: String::new(),
    }));

    // 10~12. 결과와 통과 기록
    let count = |statuses: &[LintStatus]| {
        commands
            .iter()
            .filter(|c| statuses.contains(&c.status))
            .count() as u32
    };
    let severity = |s: Severity| findings.iter().filter(|f| f.severity == s).count() as u32;
    let errors = severity(Severity::Error) + count(&[LintStatus::Failed, LintStatus::Timeout]);
    let (warnings, unrunnable) = (
        severity(Severity::Warning),
        count(&[LintStatus::Unrunnable]),
    );
    let result = LintResult {
        v: 1,
        base,
        merge_base: mb,
        head,
        config_sha256,
        test_level: level,
        started_at,
        seconds: tenths(clock.elapsed()),
        commands,
        findings,
        errors,
        warnings,
        unrunnable,
        passed: errors == 0 && unrunnable == 0,
    };
    let result_path = dir.join("result.json");
    fullops_store::write_json(&result_path, &result)?;
    if let Some(out) = &out {
        fullops_store::write_json(out, &result)?;
    }
    if result.passed {
        let tree = known(git.rev_parse(&format!("{}^{{tree}}", result.head)))?;
        let pass = LintPass {
            v: 1,
            base: result.merge_base.clone(),
            head: result.head.clone(),
            tree,
            config_sha256: result.config_sha256.clone(),
            test_level: level,
            at: fullops_store::now(),
        };
        fullops_store::write_json(&dir.join("pass.json"), &pass)?;
    }
    if ctx.json {
        let mut fields = serde_json::to_value(&result)
            .map_err(|e| CliError::Internal(format!("결과를 JSON으로 쓰지 못했습니다: {e}")))?;
        fields["ok"] = result.passed.into();
        fields["result"] = path_text(&result_path).into();
        println!("{fields}");
    } else {
        println!("{}", human(&result));
    }
    Ok(if result.passed { EXIT_OK } else { EXIT_REFUSED })
}
