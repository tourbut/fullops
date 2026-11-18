//! 내장 자산의 상수(설계 10.1절). 원본은 `crates/fullops/assets/` 아래에 있다.
//! 아래 넷은 뼈대(묶음 A)가 둔 것이고 이름·파일 자리·표 머리·안내 문장을 동결 뒤 바꾸지 않는다(파서가 기댄다).
//! 그 밖의 자산 파일과 상수는 묶음 F가 이 모듈에 덧붙인다.

/// `.fullops/PLANS.md`의 내장 템플릿. `parse_plan`·`read_plan`의 `template` 인자다.
pub const PLANS_MD: &str = include_str!("../assets/repo/PLANS.md");

/// 인박스 본문의 내장 템플릿. `task new`가 쓰고 `done`의 R-04가 템플릿 안내 문장을 가리는 데 쓴다.
pub const HANDOVER_TEMPLATE: &str = include_str!("../assets/repo/templates/handover.md");

/// `.fullops/config/lint.json`의 내장 템플릿. `lint_excludes`의 `template_lint_json` 인자다.
pub const LINT_JSON: &str = include_str!("../assets/repo/config/lint.json");

/// 산출물 인덱스 `docs/deliverables/README.md`의 내장 템플릿. 인덱스 표의 기준이다.
pub const DELIVERABLES_README: &str = include_str!("../assets/repo/docs/deliverables/README.md");

/// 의존 목록의 내장 사본(설계 10.7절). `plugin/deps.toml`을 빌드할 때 읽은 글이고 `load_deps`의 `builtin` 인자다.
pub const DEPS_TOML: &str = include_str!("../../../plugin/deps.toml");

/// `guide/rules/delegation.md`의 내장 템플릿(병렬 방침의 정본. 설계 8.15절).
pub const DELEGATION_MD: &str = include_str!("../assets/repo/guide/rules/delegation.md");

/// 1.24.0 템플릿의 `rules/delegation.md`(LF로 맞춘 글)의 sha256. `migrate`가 사람이 고친 흔적이 있는지 가린다.
pub const DELEGATION_1_24_SHA256: &str =
    "b34ad71bceb0c81a90e53a2e900bc323095e5abf599dc4fe4059ee46367d2d4b";

/// 새 템플릿과 글이 다른 1.24.0 템플릿 문서 `(옛 자리 `.fullops-squad/` 기준 상대 경로, LF로 맞춘 글의 sha256)`. `migrate`가 기본 글 그대로인 파일을 새 템플릿으로 바꾼다.
/// 새 템플릿은 `template_for`가 찾는다.
pub const STOCK_1_24: &[(&str, &str)] = &[
    (
        "FULLOPS.md",
        "137e7ee33581ffef944fc11c3804ab858f89e89f49b0f2c20709387e6a74a304",
    ),
    (
        "docs/agents/document-writing.md",
        "253ae1ba3f18973f1a8d51d84ad8dd5e262b138a0fe939f0fd9088a20cb40c73",
    ),
    (
        "docs/agents/triage-labels.md",
        "4bf6ff243adbc70889aa23fadcc5bca2aaff08b790a99b28a11df5f3f84cebf2",
    ),
    (
        "docs/deliverables/README.md",
        "98ff905f13c17220c150b8602ee7d8722d3123630fddd557adb5f9d4338c7224",
    ),
    (
        "docs/planning/intents/_TEMPLATE.md",
        "f40cf313a6cdef6f61a1b8c7068ae7a74b47e5a49ef30564880cc0040756af09",
    ),
    (
        "handovers/_TEMPLATE.md",
        "3bf33becfe55e9449df909ebbeeab5a8931797362d4df9617dec8c6a8ab5428e",
    ),
    (
        "lint/README.md",
        "70bce7373f735831d1444b952e8165879d502c317fe34bb4f2b1484dc122dc04",
    ),
    (
        "orca-agents.md",
        "ff4f8166211058ce111396671571e43b330a9ae13f6de1621760f608a9236da8",
    ),
    (
        "review/_REPORT.md",
        "47145317fa03c8132db75b730467edd5d092a70f55e546f6b48c0c0302d6e8d3",
    ),
    (
        "rules/common/README.md",
        "1a6cef530153e0f811ca3ca1a116e3d545b181a03b279f8d9dd3dddbf3aea40b",
    ),
    (
        "rules/common/testing.md",
        "939ebea35c4bda2cb44536875dbb4c6527502fd31662902119334065f057ef7c",
    ),
    ("rules/delegation.md", DELEGATION_1_24_SHA256),
];

/// 1.24.0 setup이 문서 머리에 찍은 front matter와 그 뒤의 글 `(front matter, 본문)`. LF로 맞춘 글을 받는다.
pub fn split_front_matter(text: &str) -> (&str, &str) {
    let end = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.find("\n---\n"))
        .map_or(0, |at| at + "---\n".len() + "\n---\n".len());
    text.split_at(end)
}

/// 레포의 템플릿 문서가 1.24.0의 기본 글 그대로인가. setup이 찍는 front matter와 운영 모드 블록은 견주지 않는다.
pub fn is_stock_1_24(rel: &str, text: &str) -> bool {
    let Some((_, stock)) = STOCK_1_24.iter().find(|(path, _)| *path == rel) else {
        return false;
    };
    let text = text.replace("\r\n", "\n");
    let body = split_front_matter(&text).1.trim_start_matches('\n');
    // 운영 모드 블록은 setup이 본문 앞에 넣는다(testing.md는 템플릿에 이미 들어 있다)
    let without_mode = body
        .strip_prefix("<!-- fullops-mode:start -->")
        .and_then(|rest| rest.split_once("<!-- fullops-mode:end -->"))
        .map(|(_, rest)| rest.trim_start_matches('\n'));
    [Some(body), without_mode]
        .into_iter()
        .flatten()
        .any(|body| fullops_core::model::sha256_hex(body.as_bytes()) == *stock)
}

/// 옛 자리(`.fullops-squad/` 기준) 템플릿 문서의 새 내장 템플릿. `migrate --from-python`이 옛 자리에서 글을 바꿀 때 쓴다.
pub fn template_for(old_rel: &str) -> Option<&'static str> {
    let new = fullops_core::layout::moved(old_rel, &|_| false)?;
    REPO_FILES
        .iter()
        .find(|(rel, _)| *rel == new)
        .map(|(_, text)| *text)
}

/// 2.0.x까지의 템플릿 파일 자리(`.fullops-squad/` 기준). `migrate --from-python`이 옛 자리에 없는 것을 더한다.
pub const LEGACY_FILES: &[&str] = &[
    "FULLOPS.md",
    "project.md",
    "orca-agents.md",
    "PLANS.md",
    ".gitignore",
    ".env.example",
    "handovers/_TEMPLATE.md",
    "lint/lint.json",
    "lint/README.md",
    "review/rule.json",
    "review/_REPORT.md",
    "rules/delegation.md",
    "rules/common/README.md",
    "rules/common/coding-style.md",
    "rules/common/security.md",
    "rules/common/testing.md",
    "rules/common/ECC-LICENSE.txt",
    "docs/deliverables/README.md",
    "docs/agents/document-writing.md",
    "docs/agents/domain.md",
    "docs/agents/issue-tracker.md",
    "docs/agents/triage-labels.md",
    "docs/planning/intents/_TEMPLATE.md",
];

/// `init`이 `.fullops/` 아래에 없을 때만 만드는 파일 `(상대 경로, 내용)`. 트리는 설계 6.1절이다.
pub const REPO_FILES: &[(&str, &str)] = &[
    ("PLANS.md", PLANS_MD),
    (".gitignore", include_str!("../assets/repo/.gitignore")),
    (".env.example", include_str!("../assets/repo/.env.example")),
    ("config/lint.json", LINT_JSON),
    (
        "config/review.json",
        include_str!("../assets/repo/config/review.json"),
    ),
    (
        "guide/FULLOPS.md",
        include_str!("../assets/repo/guide/FULLOPS.md"),
    ),
    (
        "guide/project.md",
        include_str!("../assets/repo/guide/project.md"),
    ),
    (
        "guide/orca-agents.md",
        include_str!("../assets/repo/guide/orca-agents.md"),
    ),
    (
        "guide/lint.md",
        include_str!("../assets/repo/guide/lint.md"),
    ),
    ("guide/rules/delegation.md", DELEGATION_MD),
    (
        "guide/rules/common/README.md",
        include_str!("../assets/repo/guide/rules/common/README.md"),
    ),
    (
        "guide/rules/common/coding-style.md",
        include_str!("../assets/repo/guide/rules/common/coding-style.md"),
    ),
    (
        "guide/rules/common/security.md",
        include_str!("../assets/repo/guide/rules/common/security.md"),
    ),
    (
        "guide/rules/common/testing.md",
        include_str!("../assets/repo/guide/rules/common/testing.md"),
    ),
    (
        "guide/rules/common/ECC-LICENSE.txt",
        include_str!("../assets/repo/guide/rules/common/ECC-LICENSE.txt"),
    ),
    (
        "guide/agents/document-writing.md",
        include_str!("../assets/repo/guide/agents/document-writing.md"),
    ),
    (
        "guide/agents/domain.md",
        include_str!("../assets/repo/guide/agents/domain.md"),
    ),
    (
        "guide/agents/issue-tracker.md",
        include_str!("../assets/repo/guide/agents/issue-tracker.md"),
    ),
    (
        "guide/agents/triage-labels.md",
        include_str!("../assets/repo/guide/agents/triage-labels.md"),
    ),
    ("templates/handover.md", HANDOVER_TEMPLATE),
    (
        "templates/review-report.md",
        include_str!("../assets/repo/templates/review-report.md"),
    ),
    (
        "templates/intent.md",
        include_str!("../assets/repo/templates/intent.md"),
    ),
    ("docs/deliverables/README.md", DELIVERABLES_README),
];

/// 빈 채로 두는 폴더. `init`이 `.gitkeep`을 둔다.
pub const REPO_DIRS: &[&str] = &[
    "inbox",
    "tasks",
    "guide/roles",
    "docs/design-docs/adr",
    "docs/design-docs/mockups",
    "docs/evaluations/scenarios",
    "docs/exec-plans/phases",
    "docs/generated",
    "docs/operations",
    "docs/planning/intents",
    "docs/planning/product-specs",
];

/// `.fullops/.gitignore`의 템플릿 줄(주석 빼고). `migrate --layout`이 옮긴 `.gitignore`에 없는 줄을 더한다.
pub fn ignore_lines() -> impl Iterator<Item = &'static str> {
    include_str!("../assets/repo/.gitignore")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
}

/// `.fullops-squad/.gitignore`에 있어야 하는 줄(옛 자리. `migrate --from-python`이 쓴다). `init`의 템플릿에 있고 `migrate`가 없으면 더한다.
/// 2.1.0부터 비었다(탐색 결과가 Git 공용 디렉터리로 가서 `docs/evaluations/jev/history/` 줄이 필요 없다).
pub const GITIGNORE_LINES: &[&str] = &[];

/// 엔트리 문서(`AGENTS.md`·`CLAUDE.md`·`GEMINI.md`)의 관리 블록. 셋 다 같은 글이다(설계 10.6절).
pub const MANAGED_BLOCK: &str = include_str!("../assets/managed-block.md");
pub const BLOCK_START: &str = "<!-- fullops:start -->";
pub const BLOCK_END: &str = "<!-- fullops:end -->";
pub const ENTRY_DOCS: [&str; 3] = ["AGENTS.md", "CLAUDE.md", "GEMINI.md"];

/// 내장 릴리스 노트 `(버전, 글)`. 오래된 것부터다. `upgrade`가 `applied_version` 이후의 것을 낸다.
pub const RELEASES: &[(&str, &str)] = &[
    ("2.0.0", include_str!("../assets/releases/2.0.0.md")),
    ("2.0.1", include_str!("../assets/releases/2.0.1.md")),
    ("2.0.2", include_str!("../assets/releases/2.0.2.md")),
    ("2.0.3", include_str!("../assets/releases/2.0.3.md")),
    ("2.0.4", include_str!("../assets/releases/2.0.4.md")),
    ("2.1.0", include_str!("../assets/releases/2.1.0.md")),
    ("2.1.1", include_str!("../assets/releases/2.1.1.md")),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_1_24_ignores_what_setup_stamps() {
        // 1.24.0의 docs/agents/triage-labels.md 그대로
        let stock = "# Triage labels\n\n로컬 역할 분류는 `fullops.json`의 `roles` 키를 그대로 쓴다.\n외부 트래커 라벨은 아직 설정하지 않았다. 연결 시 실제 라벨을 조회하고 역할별 대응표를 기록한다. 원본 레포의 라벨을 존재한다고 가정하지 않는다.\n";
        let rel = "docs/agents/triage-labels.md";
        assert!(is_stock_1_24(rel, stock));
        // setup이 찍는 front matter와 운영 모드 블록, Windows 체크아웃의 CRLF는 견주지 않는다
        let stamped = format!("---\ntitle: Triage labels\nupdated: 2026-11-01\n---\n\n{stock}");
        assert_eq!(
            split_front_matter(&stamped).0,
            "---\ntitle: Triage labels\nupdated: 2026-11-01\n---\n"
        );
        assert!(is_stock_1_24(rel, &stamped));
        assert!(is_stock_1_24(rel, &stamped.replace('\n', "\r\n")));
        let moded = format!(
            "---\ntitle: t\n---\n<!-- fullops-mode:start -->\n## 운영 책임\n<!-- fullops-mode:end -->\n\n{stock}"
        );
        assert!(is_stock_1_24(rel, &moded));
        // 사람이 고친 글과 표에 없는 경로는 기본 글이 아니다
        assert!(!is_stock_1_24(rel, &format!("{stock}- 사람이 더한 줄\n")));
        assert!(!is_stock_1_24("project.md", stock));
        // 옛 자리의 템플릿 파일은 새 템플릿 전부와 하나씩 맞는다
        assert_eq!(LEGACY_FILES.len(), REPO_FILES.len());
        for rel in LEGACY_FILES {
            assert!(template_for(rel).is_some(), "{rel}");
        }
        // 표의 경로에는 모두 새 템플릿이 있고 새 템플릿은 1.24.0의 글이 아니다
        for (rel, _) in STOCK_1_24 {
            assert!(
                template_for(rel).is_some_and(|text| !is_stock_1_24(rel, text)),
                "{rel}"
            );
        }
    }

    #[test]
    fn templates_keep_the_parts_parsers_depend_on() {
        // PLANS.md: 과제 표의 머리와 위임 범위 절. 템플릿 자신은 행도 위임 문장도 없는 것으로 읽힌다
        let header = "| 과제 키 | 목표 | 담당 | 상태 | 지시서·원천 링크 |\n|---|---|---|---|---|\n";
        assert!(PLANS_MD.contains(header));
        let plan = fullops_core::parse_plan(PLANS_MD, PLANS_MD);
        assert!(plan.rows.is_empty() && plan.delegation.is_empty() && plan.parse_errors.is_empty());
        let row = format!("{header}| PAY-12 | 결제 재시도 | DEV | 대기 | 사용자 지시 |\n");
        let filled =
            format!("{PLANS_MD}- 2026-11-14 \"결제 묶음은 끝까지\"\n").replace(header, &row);
        let plan = fullops_core::parse_plan(&filled, PLANS_MD);
        assert_eq!((plan.rows.len(), plan.delegation.len()), (1, 1));
        assert_eq!(plan.rows[0].role.as_ref().map(|r| r.as_str()), Some("dev"));

        // 인박스 템플릿: 설계 6.1.3절이 든 절 제목과 완료 보고 절
        for heading in [
            "## 현재 상황과 확인 근거",
            "## 적용 기준과 예외",
            "## 먼저 읽을 문서",
            "## 해야 할 일과 파일 소유권",
            "## 완료 기준과 검증",
            "## 갱신할 산출물",
            "## 제약·협업·후속",
            "## 완료 보고",
        ] {
            assert!(
                HANDOVER_TEMPLATE.lines().any(|line| line == heading),
                "{heading}"
            );
        }
        assert!(HANDOVER_TEMPLATE.contains("`fullops done`"));
        // 1.24.0의 스크립트 표기와 CR이 없다
        for template in [PLANS_MD, HANDOVER_TEMPLATE, LINT_JSON, DELIVERABLES_README] {
            assert!(!template.contains(".py ") && !template.contains("python3"));
            assert!(!template.contains('\r'));
        }

        // lint.json: 1.24.0 형식 그대로. exclude가 있다
        let lint: serde_json::Value = serde_json::from_str(LINT_JSON).expect("lint.json");
        assert!(
            lint["exclude"].as_array().is_some_and(|e| !e.is_empty()) && lint["rules"].is_array()
        );

        // 그 밖의 템플릿에도 1.24.0의 스크립트 표기와 CR이 없고 .gitignore에 필요한 줄이 있다
        for (path, text) in REPO_FILES.iter().chain(&[("관리 블록", MANAGED_BLOCK)]) {
            assert!(!text.contains(char::from(13)), "{path}: CR");
            for banned in ["python3", "fullops.json", "subagent_level", "jev_"] {
                assert!(!text.contains(banned), "{path}: {banned}");
            }
        }
        let ignore = REPO_FILES
            .iter()
            .find(|(p, _)| *p == ".gitignore")
            .unwrap()
            .1;
        assert!(
            GITIGNORE_LINES
                .iter()
                .all(|l| ignore.lines().any(|i| i == *l))
        );
        assert!(DELEGATION_MD.contains("fullops dispatch <자기 KEY> --brief"));
        assert!(!is_stock_1_24("rules/delegation.md", DELEGATION_MD));

        // 산출물 인덱스: 표 머리와 D01~D13 행
        assert!(DELIVERABLES_README.contains("| ID | 단계 | 산출물 | 원천 | 상태 |"));
        for n in 1..=13 {
            assert!(
                DELIVERABLES_README.contains(&format!("| D{n:02} |")),
                "D{n:02}"
            );
        }
    }
}
