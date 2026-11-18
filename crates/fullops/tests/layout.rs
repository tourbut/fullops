//! 레포 폴더 `.fullops/`의 통합 시험(설계 6.1절, 13.6절): 옛 자리 레포의 안내와 거절, `fullops migrate --layout`,
//! QA lint의 크기 규칙, doctor의 추적 경고. `--from-python`이 새 자리로 끝나는 것은 `bundle_f.rs`의 N3가 본다.

use fullops_testkit::{Sandbox, Setup};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const COMMIT: &str = "fullops: .fullops 자리로 옮김";
const OLD: &str = ".fullops-squad";

/// 역할도 원격도 없는 활성 레포의 lead 체크아웃.
fn repo() -> (Sandbox, PathBuf) {
    let sb = Sandbox::with(&Setup {
        remote: false,
        roles: BTreeMap::new(),
        ..Setup::default()
    });
    let lead = sb.path("lead");
    (sb, lead)
}

fn commit(sb: &Sandbox, repo: &Path, message: &str) {
    sb.git_ok(repo, &["add", "-A"]);
    sb.git_ok(repo, &["commit", "-q", "-m", message]);
}

/// 2.0.x의 옛 자리 레포: 새 자리의 폴더를 옛 이름으로 옮겨 커밋한다.
fn legacy_repo() -> (Sandbox, PathBuf) {
    let (sb, lead) = repo();
    sb.git_ok(&lead, &["mv", ".fullops", OLD]);
    sb.write(&lead.join(OLD).join(".gitignore"), ".env*\nlocal/\n");
    commit(&sb, &lead, "2.0.x의 옛 자리");
    (sb, lead)
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn layout(sb: &Sandbox, lead: &Path) -> fullops_testkit::Run {
    sb.cli(lead, &["migrate", "--layout"])
}

/// 거절은 종료 코드 2이고 아무것도 옮기지 않는다.
fn refused(sb: &Sandbox, lead: &Path, why: &str) {
    for args in [
        &["migrate", "--layout", "--dry-run"][..],
        &["migrate", "--layout"],
    ] {
        let run = sb.cli(lead, args);
        assert_eq!(run.code, Some(2), "{why}: {}{}", run.stdout, run.stderr);
        assert!(
            run.stderr.contains("FullOps[migrate]") && run.stderr.contains(why),
            "{why}: {}",
            run.stderr
        );
    }
    assert!(lead.join(OLD).join("fullops.toml").is_file(), "{why}");
}

/// 옛 자리 레포: hook은 막지 않고 세션 시작 때 옮기라고 알리며, 명령은 옮기는 것과 설치·점검만 돈다.
#[test]
fn legacy_repo_guides_instead_of_running() {
    let (sb, lead) = legacy_repo();
    let start = |host: &str, sid: &str| {
        let input = json!({"hook_event_name": "SessionStart", "session_id": sid, "cwd": lead, "source": "startup"});
        let out = sb.hook(&[host], &lead, &input);
        serde_json::from_str::<Value>(out.stdout.trim()).unwrap_or_else(|e| panic!("{e}: {out:?}"))
    };
    // 모델에게 문맥이 가는 호스트는 문맥으로, 아닌 호스트(grok)는 사용자 경고로 낸다
    let claude = start("claude", "s1");
    let context = claude["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap_or_else(|| panic!("{claude}"));
    assert!(
        context.contains("옛 자리(`.fullops-squad/`)")
            && context.contains("fullops migrate --layout"),
        "{context}"
    );
    let grok = start("grok", "s2");
    let warn = grok["systemMessage"]
        .as_str()
        .unwrap_or_else(|| panic!("{grok}"));
    assert!(warn.contains("fullops migrate --layout"), "{warn}");
    // 비활성이다: 날것 worker-start도 막지 않고 세션 파일도 만들지 않는다
    let tool = json!({"hook_event_name": "PreToolUse", "session_id": "s1", "cwd": lead, "tool_name": "Bash",
        "tool_input": {"command": "orca orchestration worker-start --spec x"}});
    assert_eq!(sb.hook(&["claude"], &lead, &tool).stdout.trim(), "{}");
    assert!(!lead.join(".git/fullops/sessions").exists());

    // 명령: 종료 코드 2와 같은 안내. 레포를 만들거나 고치지 않는다
    for args in [
        &["plan", "list"][..],
        &["lint"],
        &["dispatch", "A-1", "--role", "dev"],
        &["sync"],
        &["why"],
        &["init", "--roles", "dev", "--local-only"],
    ] {
        let run = sb.cli(&lead, args);
        assert_eq!(run.code, Some(2), "{args:?}: {}{}", run.stdout, run.stderr);
        assert!(
            run.stderr.contains("FullOps[layout]")
                && run.stderr.contains("fullops migrate --layout"),
            "{args:?}: {}",
            run.stderr
        );
    }
    let json_refusal = sb.cli(&lead, &["plan", "list", "--json"]);
    let out: Value = serde_json::from_str(&json_refusal.stdout).unwrap();
    assert_eq!(out["refused"][0]["rule"], "layout", "{out}");
    assert!(!lead.join(".fullops").exists());
    // 요약과 upgrade는 안내만 내고 0으로 끝난다(npx 실행기의 `--repo`가 upgrade를 부른다). doctor는 행 하나로 알린다
    for command in ["status", "upgrade"] {
        let run = sb.cli(&lead, &[command]);
        assert_eq!(run.code, Some(0), "{command}: {}", run.stderr);
        assert!(
            run.stdout.contains("fullops migrate --layout"),
            "{command}: {}",
            run.stdout
        );
    }
    let doctor = sb.cli(&lead, &["doctor"]);
    assert_eq!(doctor.code, Some(0), "{}", doctor.stderr);
    assert!(
        doctor.stdout.contains("옛 자리(`.fullops-squad/`)")
            && doctor.stdout.contains("fullops migrate --layout"),
        "{}",
        doctor.stdout
    );
    assert_eq!(sb.cli(&lead, &["rules"]).code, Some(0));
    assert_eq!(sb.git_ok(&lead, &["status", "--porcelain"]), "");
}

/// 옛 트리의 파일들. `(옛 자리 기준 경로, 글)`.
const OLD_TREE: &[(&str, &str)] = &[
    (
        "FULLOPS.md",
        "# 지도\n\n인박스는 `.fullops-squad/handovers/to_<역할>.md`, 규칙은 `.fullops-squad/rules/common/README.md`다.\n끝난 지시서는 `.fullops-squad/handovers/logs/<KEY>.<역할>.r<차수>.md`, QA는 `.fullops-squad/docs/evaluations/qa-reports/<과제 키>-test/report.md`.\n",
    ),
    (
        "project.md",
        "# 프로젝트\n\n문서는 `.fullops-squad/docs/`에 둔다.\n",
    ),
    ("orca-agents.md", "# 배정\n"),
    (
        "rules/common/README.md",
        "# 공통 규칙\n\n`.fullops-squad/local/`에 임시 파일을 둔다.\n",
    ),
    (
        "rules/delegation.md",
        "# 위임\n\n방침은 `.fullops-squad/fullops.toml`의 `parallel`이다.\n",
    ),
    ("contexts/dev.md", "# dev 맥락\n"),
    ("contexts/archive/2026.md", "# 옛 맥락\n"),
    ("docs/agents/domain.md", "# 도메인\n"),
    (
        "handovers/_TEMPLATE.md",
        "# 틀\n\n공통 기준은 `.fullops-squad/rules/common/README.md`다.\n",
    ),
    ("handovers/logs/.gitkeep", ""),
    (
        "handovers/logs/A-1.dev.r0.md",
        "# A-1\n\n그때의 인박스는 `.fullops-squad/handovers/to_dev.md`였다.\n",
    ),
    (
        "handovers/logs/2026-10-01_to_dev.md",
        "# 1.24.0의 날짜 로그 `.fullops-squad/handovers/`\n",
    ),
    ("routes/A-1.json", "{\"v\": 1}\n"),
    ("routes/A-1.request.md", "요청 원문\n"),
    ("routes/notes.txt", "1.24.0의 메모\n"),
    ("reviews/A-1/0123456789ab/result.json", "{}\n"),
    (
        "lint/lint.json",
        "{\n  \"schema_version\": 1,\n  \"commands\": [],\n  \"exclude\": [\n    \"**/dist/**\",\n    \".fullops-squad/handovers/**\",\n    \".fullops-squad/contexts/**\"\n  ]\n}\n",
    ),
    (
        "lint/README.md",
        "# lint\n\n설정은 `.fullops-squad/lint/lint.json`이다.\n",
    ),
    (
        "review/rule.json",
        "{\n  \"include\": [\"**/*.md\"],\n  \"exclude\": [\n    \".fullops-squad/handovers/logs/**\",\n    \"**/*.bak\"\n  ],\n  \"rules\": []\n}\n",
    ),
    ("review/_REPORT.md", "# 리뷰 보고\n"),
    ("docs/planning/intents/_TEMPLATE.md", "# 의도 틀\n"),
    (
        "docs/planning/spec.md",
        "# 요구사항\n\n옛 경로 `.fullops-squad/handovers/`를 적은 제품 문서(고치지 않는다).\n",
    ),
    (
        "docs/deliverables/README.md",
        "# 산출물 인덱스\n\n아래 경로는 `.fullops-squad/` 기준이다.\n",
    ),
    (
        "docs/evaluations/qa-reports/A-1-test/report.md",
        "# A-1 QA\n\n![화면](shot.png)\n",
    ),
    ("docs/evaluations/qa-reports/A-1-test/shot.png", "png"),
    (
        "docs/evaluations/qa-reports/A-1-designer-test/report.md",
        "# A-1 시각 검수\n",
    ),
    (
        "docs/evaluations/qa-reports/A-1-designer-test/hero.png",
        "png",
    ),
    (
        "docs/evaluations/qa-reports/OLD-9-test/report.md",
        "# 1.24.0의 QA\n",
    ),
    (
        "docs/evaluations/qa-reports/OLD-9-review/report.md",
        "# 1.24.0의 리뷰\n",
    ),
    (
        "docs/evaluations/qa-reports/OLD-9-review/preview.json",
        "{}\n",
    ),
    (
        "docs/evaluations/qa-reports/OLD-9-review/rules.json",
        "{}\n",
    ),
    ("docs/evaluations/jev/A-1-find.json", "{}\n"),
    ("docs/evaluations/scenarios/login.md", "# 시나리오\n"),
    ("board/index.html", "<html></html>\n"),
];

fn full_legacy_repo() -> (Sandbox, PathBuf) {
    let (sb, lead) = legacy_repo();
    let old = lead.join(OLD);
    for (rel, text) in OLD_TREE {
        sb.write(&old.join(rel), text);
    }
    sb.write(
        &old.join("PLANS.md"),
        &format!(
            "{}\n지시서는 `.fullops-squad/handovers/to_dev.md`에 있다.\n",
            read(&old.join("PLANS.md"))
        ),
    );
    sb.write(
        &lead.join("CLAUDE.md"),
        "# 우리 안내\n\n<!-- fullops:start -->\n## FullOps\n\n지도는 `.fullops-squad/FULLOPS.md`다.\n<!-- fullops:end -->\n\n## 우리 절\n",
    );
    commit(&sb, &lead, "옛 트리");
    // 무시 목록에 있으면서 추적되는 파일과 추적되지 않는 파일
    sb.write(
        &old.join("docs/evaluations/qa-reports/A-1-test/raw/trace.zip"),
        "zip",
    );
    sb.write(&old.join("local/kept.log"), "한 번 커밋된 임시 파일\n");
    sb.git_ok(
        &lead,
        &[
            "add",
            "-f",
            ".fullops-squad/docs/evaluations/qa-reports/A-1-test/raw/trace.zip",
            ".fullops-squad/local/kept.log",
        ],
    );
    sb.git_ok(
        &lead,
        &["commit", "-q", "-m", "무시 목록의 파일을 추적한다"],
    );
    sb.write(
        &old.join(".gitignore"),
        ".env*\nlocal/\n**/raw/\nscratch/\n",
    );
    commit(&sb, &lead, "무시 목록");
    sb.write(&old.join(".env"), "KEY=1\n");
    sb.write(&old.join("local/tool.log"), "log\n");
    sb.write(&old.join("scratch/note.txt"), "메모\n");
    assert_eq!(sb.git_ok(&lead, &["status", "--porcelain"]), "");
    (sb, lead)
}

/// `migrate --layout`: 표대로 옮기고, 지울 것은 추적에서만 빼고, 커밋되는 글의 경로를 고치고, 커밋 하나를 만든다.
#[test]
fn migrate_layout_moves_rewrites_and_untracks() {
    let (sb, lead) = full_legacy_repo();
    let (old, new) = (lead.join(OLD), lead.join(".fullops"));
    let before = sb.git_ok(&lead, &["rev-parse", "HEAD"]);

    // --dry-run: 수와 고칠 파일만 내고 아무것도 바꾸지 않는다
    let dry = sb.cli(&lead, &["migrate", "--layout", "--dry-run"]);
    assert_eq!(dry.code, Some(0), "{}", dry.stderr);
    for line in [
        "계획(--dry-run. 아무것도 바꾸지 않았다)",
        "추적에서 빼는 파일 5개",
        "추적되지 않는 파일 3개",
        ".fullops/guide/FULLOPS.md",
        ".fullops/config/review.json",
        ".fullops/fullops.toml",
        "CLAUDE.md",
    ] {
        assert!(dry.stdout.contains(line), "{line}\n{}", dry.stdout);
    }
    assert!(!dry.stdout.contains(".fullops/tasks/"), "{}", dry.stdout);
    assert!(!new.exists() && old.join("fullops.toml").is_file());
    assert_eq!(sb.git_ok(&lead, &["rev-parse", "HEAD"]), before);
    assert_eq!(sb.git_ok(&lead, &["status", "--porcelain"]), "");

    let run = layout(&sb, &lead);
    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert!(
        run.stdout.contains("`.fullops/` 자리로 옮겼다"),
        "{}",
        run.stdout
    );
    // 커밋 하나, 깨끗한 작업 트리, 옛 폴더 없음
    assert_eq!(sb.git_ok(&lead, &["log", "--format=%s", "-1"]), COMMIT);
    assert_eq!(sb.git_ok(&lead, &["rev-parse", "HEAD~1"]), before);
    assert_eq!(sb.git_ok(&lead, &["status", "--porcelain"]), "");
    assert!(!old.exists(), "옛 폴더가 남았다");

    // 표대로 옮겼다
    let tracked = sb.git_ok(&lead, &["ls-files", ".fullops"]);
    let tracked: Vec<&str> = tracked.lines().collect();
    for rel in [
        "fullops.toml",
        "PLANS.md",
        ".gitignore",
        "config/lint.json",
        "config/review.json",
        "guide/FULLOPS.md",
        "guide/project.md",
        "guide/orca-agents.md",
        "guide/lint.md",
        "guide/rules/common/README.md",
        "guide/rules/delegation.md",
        "guide/roles/dev.md",
        "guide/agents/domain.md",
        "templates/handover.md",
        "templates/review-report.md",
        "templates/intent.md",
        "tasks/A-1/route.json",
        "tasks/A-1/request.md",
        "tasks/A-1/dev.r0.md",
        "tasks/A-1/reviews/0123456789ab/result.json",
        "tasks/A-1/qa/report.md",
        "tasks/A-1/qa/shot.png",
        "tasks/A-1/qa/designer.md",
        "tasks/A-1/qa/hero.png",
        "docs/planning/spec.md",
        "docs/deliverables/README.md",
        "docs/evaluations/scenarios/login.md",
        "archive/contexts/2026.md",
        "archive/handovers/.gitkeep",
        "archive/handovers/2026-10-01_to_dev.md",
        "archive/routes/notes.txt",
        "archive/qa-reports/OLD-9-test/report.md",
        "archive/qa-reports/OLD-9-review/report.md",
        "archive/board/index.html",
    ] {
        assert!(
            tracked.contains(&format!(".fullops/{rel}").as_str()),
            "{rel}\n{tracked:?}"
        );
        assert!(new.join(rel).is_file(), "{rel}");
    }
    assert_eq!(tracked.len(), 34, "{tracked:?}");
    // 추적에서 뺀 것: 탐색 기록, OCR 중간 파일, 무시 목록에 있던 파일. 작업 트리의 파일은 local/legacy/에 남는다
    for rel in [
        "docs/evaluations/jev/A-1-find.json",
        "docs/evaluations/qa-reports/OLD-9-review/preview.json",
        "docs/evaluations/qa-reports/OLD-9-review/rules.json",
        "docs/evaluations/qa-reports/A-1-test/raw/trace.zip",
        "local/kept.log",
    ] {
        assert!(new.join("local/legacy").join(rel).is_file(), "{rel}");
    }
    // 추적되지 않던 파일: .env*와 local/은 같은 이름으로, 그 밖은 local/legacy/로
    assert_eq!(read(&new.join(".env")), "KEY=1\n");
    assert_eq!(read(&new.join("local/tool.log")), "log\n");
    assert_eq!(read(&new.join("local/legacy/scratch/note.txt")), "메모\n");

    // 커밋되는 글의 경로를 고쳤다(긴 접두어부터)
    assert_eq!(
        read(&new.join("guide/FULLOPS.md")),
        "# 지도\n\n인박스는 `.fullops/inbox/to_<역할>.md`, 규칙은 `.fullops/guide/rules/common/README.md`다.\n끝난 지시서는 `.fullops/tasks/<KEY>/<역할>.r<차수>.md`, QA는 `.fullops/tasks/<과제 키>/qa/report.md`.\n"
    );
    assert!(read(&new.join("guide/rules/delegation.md")).contains("`.fullops/fullops.toml`"));
    assert!(
        read(&new.join("templates/handover.md"))
            .contains("`.fullops/guide/rules/common/README.md`")
    );
    assert!(read(&new.join("guide/lint.md")).contains("`.fullops/config/lint.json`"));
    assert!(read(&new.join("PLANS.md")).contains("`.fullops/inbox/to_dev.md`"));
    assert!(read(&new.join("docs/deliverables/README.md")).contains("`.fullops/` 기준"));
    // 관리 블록은 새 블록이고 블록 밖의 글은 그대로다
    let claude = read(&lead.join("CLAUDE.md"));
    assert!(
        claude.starts_with("# 우리 안내\n") && claude.ends_with("\n## 우리 절\n"),
        "{claude}"
    );
    assert!(
        claude.contains("`.fullops/fullops.toml`") && !claude.contains(OLD),
        "{claude}"
    );
    // 과제 기록, 보관, 제품 문서는 고치지 않는다
    for rel in [
        "tasks/A-1/dev.r0.md",
        "archive/handovers/2026-10-01_to_dev.md",
        "docs/planning/spec.md",
    ] {
        assert!(
            read(&new.join(rel)).contains(".fullops-squad/handovers/"),
            "{rel}"
        );
    }
    // 설정: lint·리뷰 제외 목록, .gitignore, applied_version
    let excludes = |name: &str| -> Vec<String> {
        let value: Value = serde_json::from_str(&read(&new.join(name))).unwrap();
        let list = value["exclude"].as_array().unwrap();
        list.iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(
        excludes("config/lint.json"),
        [
            "**/dist/**",
            ".fullops/inbox/**",
            ".fullops/guide/roles/**",
            ".fullops/tasks/**",
            ".fullops/archive/**"
        ]
    );
    assert_eq!(
        excludes("config/review.json"),
        [".fullops/tasks/**", "**/*.bak", ".fullops/archive/**"]
    );
    assert_eq!(
        read(&new.join(".gitignore")),
        ".env*\nlocal/\n**/raw/\nscratch/\n!.env.example\n"
    );
    assert!(read(&new.join("fullops.toml")).contains(&format!(
        "applied_version = \"{}\"",
        env!("CARGO_PKG_VERSION")
    )));

    // 새 자리에서 돈다: 명령, hook, doctor(무시 목록에 있으면서 추적되는 파일이 없다)
    assert_eq!(sb.cli(&lead, &["plan", "list"]).code, Some(0));
    let tool = json!({"hook_event_name": "PreToolUse", "session_id": "s1", "cwd": lead, "tool_name": "Bash",
        "tool_input": {"command": "orca orchestration worker-start --spec x"}});
    assert!(sb.hook(&["claude"], &lead, &tool).stdout.contains("T-01"));
    let doctor = sb.cli(&lead, &["doctor"]);
    assert!(
        !doctor.stdout.contains("무시 목록에 있으면서 추적되는"),
        "{}",
        doctor.stdout
    );
    // 다시 실행하면 거절한다
    let again = layout(&sb, &lead);
    assert_eq!(again.code, Some(2));
    assert!(
        again.stderr.contains("이미 `.fullops/` 자리다"),
        "{}",
        again.stderr
    );

    // 되돌리기: 그 커밋의 revert가 추적 파일을 옛 자리에 되살린다
    sb.git_ok(&lead, &["revert", "--no-edit", "HEAD"]);
    assert!(
        old.join("fullops.toml").is_file() && old.join("handovers/logs/A-1.dev.r0.md").is_file()
    );
    assert!(read(&old.join("FULLOPS.md")).contains("`.fullops-squad/handovers/to_<역할>.md`"));
    assert!(old.join("docs/evaluations/jev/A-1-find.json").is_file());
}

/// 자리 옮김 뒤 인자 없는 `fullops sync`가 역할 워크트리를 따라오게 하고, 옛 폴더 잔재를 `local/legacy/`로 옮기고 `.env`를 다시 잇는다.
#[test]
fn sync_after_migrate_follows_tidies_and_relinks() {
    let sb = Sandbox::with(&Setup {
        remote: false,
        ..Setup::default()
    });
    let (lead, dev) = (sb.path("lead"), sb.path("dev"));
    sb.git_ok(&lead, &["mv", ".fullops", OLD]);
    sb.write(&lead.join(OLD).join(".gitignore"), ".env*\nlocal/\n");
    commit(&sb, &lead, "2.0.x의 옛 자리");
    sb.git_ok(&dev, &["merge", "--ff-only", "-q", "main"]);
    // 옛 자리의 무시되던 파일: lead의 키, dev의 끊긴 키 사본과 임시 파일
    sb.write(&lead.join(OLD).join(".env"), "KEY=1\n");
    sb.write(&dev.join(OLD).join(".env"), "OLD\n");
    sb.write(&dev.join(OLD).join("local/run.log"), "log\n");
    assert_eq!(sb.git_ok(&dev, &["status", "--porcelain"]), "");

    assert_eq!(layout(&sb, &lead).code, Some(0));
    let sync = sb.cli(&lead, &["sync"]);
    assert_eq!(sync.code, Some(0), "{}", sync.stderr);
    assert!(
        sync.stdout.contains("동기화한 역할: dev"),
        "{}",
        sync.stdout
    );

    assert_eq!(sb.git_ok(&dev, &["log", "--format=%s", "-1"]), COMMIT);
    assert_eq!(sb.git_ok(&dev, &["status", "--porcelain"]), "");
    assert!(!dev.join(OLD).exists(), "옛 폴더가 남았다");
    let legacy = dev.join(".fullops/local/legacy");
    assert_eq!(read(&legacy.join(".env")), "OLD\n");
    assert_eq!(read(&legacy.join("local/run.log")), "log\n");
    assert_eq!(read(&dev.join(".fullops/.env")), "KEY=1\n");
}

/// 거절 조건(13.6절 1번)마다 종료 코드 2로 멈추고 아무것도 옮기지 않는다.
#[test]
fn migrate_layout_refusals() {
    // 옮길 것이 없는 레포: 이미 새 자리, 표식 없음, Python판
    let (sb, lead) = repo();
    let run = layout(&sb, &lead);
    assert_eq!(run.code, Some(2), "{}", run.stdout);
    assert!(
        run.stderr.contains("이미 `.fullops/` 자리다"),
        "{}",
        run.stderr
    );
    let bare = sb.init_repo("bare");
    assert!(layout(&sb, &bare).stderr.contains("옛 자리 표식"));
    sb.write(&bare.join(OLD).join("fullops.json"), "{}");
    assert!(
        layout(&sb, &bare)
            .stderr
            .contains("fullops migrate --from-python")
    );

    // 작업 트리가 깨끗하지 않다
    let (sb, lead) = legacy_repo();
    sb.write(&lead.join("notes.txt"), "커밋하지 않은 파일\n");
    refused(&sb, &lead, "작업 트리가 깨끗하지 않다");
    std::fs::remove_file(lead.join("notes.txt")).unwrap();
    // `.fullops/`가 이미 있다(무시되는 파일만 있어도 같다)
    sb.write(&lead.join(".git/info/exclude"), ".fullops/\n");
    sb.write(&lead.join(".fullops/local/x.log"), "x\n");
    refused(&sb, &lead, "`.fullops/`가 이미 있다");
    std::fs::remove_dir_all(lead.join(".fullops")).unwrap();
    sb.write(&lead.join(".git/info/exclude"), "");
    // 레포 루트가 사용자 홈이다
    let home = lead.to_string_lossy().into_owned();
    let at_home = sb.fullops(
        &lead,
        &["migrate", "--layout"],
        &[("USERPROFILE", &home), ("HOME", &home)],
        b"",
    );
    assert_eq!(at_home.code, Some(2), "{}", at_home.stdout);
    assert!(
        at_home.stderr.contains("레포 루트가 사용자 홈이다"),
        "{}",
        at_home.stderr
    );
    // 기준 브랜치를 체크아웃한 체크아웃이 아니다
    sb.git_ok(&lead, &["checkout", "-q", "-b", "side"]);
    refused(
        &sb,
        &lead,
        "기준 브랜치(`main`)를 체크아웃한 lead 체크아웃이 아니다(지금 `side`)",
    );
    sb.git_ok(&lead, &["checkout", "-q", "main"]);
    // 인박스에 진행 중 과제가 있다
    sb.write(
        &lead.join(OLD).join("handovers/to_dev.md"),
        "---\nkey: A-1\n---\n# A-1\n",
    );
    commit(&sb, &lead, "진행 중 인박스");
    refused(
        &sb,
        &lead,
        "인박스에 진행 중 과제가 있다: handovers/to_dev.md",
    );
    sb.git_ok(&lead, &["rm", "-q", ".fullops-squad/handovers/to_dev.md"]);
    commit(&sb, &lead, "인박스 비움");
    // 보류가 있다 / 열린 dispatch가 있다
    let state = lead.join(".git/fullops/state.json");
    let held = json!({"v": 1, "completions": [{
        "id": "msg_1", "dispatch": null, "key": "A-1", "role": null, "branch": null, "sha": null, "sha_source": null,
        "direct": false, "outcome": "succeeded", "received_at": 1,
        "hold": {"reason": "검수 대기", "owner": "사용자", "resume": "검수 뒤", "at": 1},
        "released_hold": null, "landed_at": null, "drift": null,
    }]});
    sb.write(&state, &held.to_string());
    refused(&sb, &lead, "보류가 1건 있다");
    let open = json!({"v": 1, "dispatches": [{
        "id": "dsp_1", "request": "req_0000000000000001", "task_id": "task_1", "run": "run_1", "key": "A-1",
        "role": "dev", "purpose": "build", "tuple": {"agent": "claude", "model": "m", "effort": "high"},
        "terminal": null, "worktree": "/w/dev", "session_mode": "fresh", "reason": null, "started_at": 1,
        "bound_session": null, "sub_of": null, "brief": null, "reported": null, "settled": null,
    }]});
    sb.write(&state, &open.to_string());
    refused(&sb, &lead, "열린 dispatch가 1건 있다");
    std::fs::remove_file(&state).unwrap();
    // 새 자리에서 경로가 겹친다: 과제의 QA 폴더 둘에 같은 이름의 캡처가 있다
    for folder in ["A-1-test", "A-1-designer-test"] {
        let qa = lead
            .join(OLD)
            .join("docs/evaluations/qa-reports")
            .join(folder);
        sb.write(&qa.join("shot.png"), folder);
    }
    sb.write(&lead.join(OLD).join("routes/A-1.json"), "{}\n");
    commit(&sb, &lead, "겹치는 캡처");
    refused(&sb, &lead, "새 자리에서 경로가 겹친다");
    sb.git_ok(
        &lead,
        &[
            "mv",
            ".fullops-squad/docs/evaluations/qa-reports/A-1-designer-test/shot.png",
            ".fullops-squad/docs/evaluations/qa-reports/A-1-designer-test/hero.png",
        ],
    );
    commit(&sb, &lead, "이름을 바꿨다");

    // `.fullops-squad`가 심볼릭 링크다(링크를 만들 권한이 없는 환경은 건너뛴다)
    let real = lead.join("squad-real");
    std::fs::rename(lead.join(OLD), &real).unwrap();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(&real, lead.join(OLD));
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&real, lead.join(OLD));
    if made.is_ok() {
        let linked = layout(&sb, &lead);
        assert_eq!(linked.code, Some(2), "{}", linked.stdout);
        assert!(linked.stderr.contains("심볼릭 링크다"), "{}", linked.stderr);
        std::fs::remove_dir(lead.join(OLD))
            .or_else(|_| std::fs::remove_file(lead.join(OLD)))
            .unwrap();
    } else {
        eprintln!("심볼릭 링크를 만들 권한이 없는 환경이라 건너뛴다");
    }
    std::fs::rename(&real, lead.join(OLD)).unwrap();

    // 남은 것을 정리하면 옮겨진다. 무시 목록에 둔 엔트리 문서는 블록만 고치고 커밋에 넣지 않는다
    sb.write(&lead.join(".git/info/exclude"), "GEMINI.md\n");
    sb.write(
        &lead.join("GEMINI.md"),
        "<!-- fullops:start -->\n지도는 `.fullops-squad/FULLOPS.md`다.\n<!-- fullops:end -->\n",
    );
    let run = layout(&sb, &lead);
    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert!(lead.join(".fullops/tasks/A-1/qa/hero.png").is_file());
    assert!(read(&lead.join("GEMINI.md")).contains("`.fullops/fullops.toml`"));
    assert_eq!(sb.git_ok(&lead, &["ls-files", "GEMINI.md"]), "");
    assert_eq!(sb.git_ok(&lead, &["status", "--porcelain"]), "");
}

/// QA-002의 크기: 바뀐 QA 이미지 한 장이 1MB를 넘으면 경고한다. `raw/`의 것과 QA 폴더 밖의 것은 보지 않는다.
#[test]
fn lint_warns_on_a_large_qa_image() {
    let (sb, lead) = repo();
    sb.write(
        &lead.join(".fullops/config/lint.json"),
        r#"{"schema_version": 1, "commands": [{"name": "version", "run": ["git", "--version"]}]}"#,
    );
    commit(&sb, &lead, "기준");
    sb.git_ok(&lead, &["tag", "base"]);
    let big = vec![b'x'; 1024 * 1024 + 1];
    let qa = lead.join(".fullops/tasks/A-1/qa");
    for path in [
        qa.join("big.png"),
        qa.join("raw/big.png"),
        lead.join("assets/big.png"),
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &big).unwrap();
    }
    sb.write(&qa.join("small.png"), "png");
    commit(&sb, &lead, "QA 증거");
    let run = sb.cli(&lead, &["lint", "--from", "base"]);
    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    let found: Vec<&str> = run
        .stdout
        .lines()
        .filter(|l| l.contains("QA-002"))
        .collect();
    assert_eq!(
        found,
        [
            "WARNING QA-002 .fullops/tasks/A-1/qa/big.png QA 이미지가 1025KB다(상한 1024KB). 필요한 영역만 잘라 내거나 해상도를 낮춘다"
        ],
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("ERROR 0 / WARNING 1"), "{}", run.stdout);
}

/// doctor: `.fullops/` 아래에 무시 목록에 있으면서 추적되는 파일이 있으면 알린다.
#[test]
fn doctor_reports_tracked_files_that_are_ignored() {
    let (sb, lead) = repo();
    let doctor = || sb.cli(&lead, &["doctor"]).stdout;
    sb.write(
        &lead.join(".fullops/.gitignore"),
        ".env*\nlocal/\n**/raw/\n",
    );
    commit(&sb, &lead, "무시 목록");
    assert!(
        !doctor().contains("무시 목록에 있으면서 추적되는"),
        "{}",
        doctor()
    );
    sb.write(&lead.join(".fullops/tasks/A-1/qa/raw/trace.zip"), "zip");
    sb.write(&lead.join(".fullops/local/tool.log"), "log\n");
    sb.git_ok(
        &lead,
        &[
            "add",
            "-f",
            ".fullops/tasks/A-1/qa/raw/trace.zip",
            ".fullops/local/tool.log",
        ],
    );
    sb.git_ok(&lead, &["commit", "-q", "-m", "무시 목록의 파일"]);
    let out = doctor();
    let row = out
        .lines()
        .find(|l| l.contains("무시 목록에 있으면서 추적되는"));
    let row = row.unwrap_or_else(|| panic!("{out}"));
    for part in [
        "2개",
        ".fullops/local/tool.log",
        ".fullops/tasks/A-1/qa/raw/trace.zip",
        "git rm --cached",
    ] {
        assert!(row.contains(part), "{part}\n{row}");
    }
    assert!(out.contains("[알림] 레포"), "{out}");
}
