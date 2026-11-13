//! 내장 자산의 상수(설계 10.1절). 원본은 `crates/fullops/assets/` 아래에 있다.
//! 아래 넷은 뼈대(묶음 A)가 둔 것이고 이름·파일 자리·표 머리·안내 문장을 동결 뒤 바꾸지 않는다(파서가 기댄다).
//! 그 밖의 자산 파일과 상수는 묶음 F가 이 모듈에 덧붙인다.

/// `.fullops-squad/PLANS.md`의 내장 템플릿. `parse_plan`·`read_plan`의 `template` 인자다.
pub const PLANS_MD: &str = include_str!("../assets/repo/PLANS.md");

/// 인박스 본문의 내장 템플릿. `task new`가 쓰고 `done`의 R-04가 템플릿 안내 문장을 가리는 데 쓴다.
pub const HANDOVER_TEMPLATE: &str = include_str!("../assets/repo/handovers/_TEMPLATE.md");

/// `.fullops-squad/lint/lint.json`의 내장 템플릿. `lint_excludes`의 `template_lint_json` 인자다.
pub const LINT_JSON: &str = include_str!("../assets/repo/lint/lint.json");

/// 산출물 인덱스 `docs/deliverables/README.md`의 내장 템플릿. 인덱스 표의 기준이다.
pub const DELIVERABLES_README: &str = include_str!("../assets/repo/docs/deliverables/README.md");

/// 의존 목록의 내장 사본(설계 10.7절). `plugin/deps.toml`을 빌드할 때 읽은 글이고 `load_deps`의 `builtin` 인자다.
pub const DEPS_TOML: &str = include_str!("../../../plugin/deps.toml");

/// `rules/delegation.md`의 내장 템플릿(병렬 방침의 정본. 설계 8.15절).
pub const DELEGATION_MD: &str = include_str!("../assets/repo/rules/delegation.md");

/// 1.24.0 템플릿의 `rules/delegation.md`(LF로 맞춘 글)의 sha256. `migrate`가 사람이 고친 흔적이 있는지 가린다.
pub const DELEGATION_1_24_SHA256: &str =
    "b34ad71bceb0c81a90e53a2e900bc323095e5abf599dc4fe4059ee46367d2d4b";

/// `init`이 `.fullops-squad/` 아래에 없을 때만 만드는 파일 `(상대 경로, 내용)`.
pub const REPO_FILES: &[(&str, &str)] = &[
    ("FULLOPS.md", include_str!("../assets/repo/FULLOPS.md")),
    ("project.md", include_str!("../assets/repo/project.md")),
    (
        "orca-agents.md",
        include_str!("../assets/repo/orca-agents.md"),
    ),
    ("PLANS.md", PLANS_MD),
    (".gitignore", include_str!("../assets/repo/.gitignore")),
    (".env.example", include_str!("../assets/repo/.env.example")),
    ("handovers/_TEMPLATE.md", HANDOVER_TEMPLATE),
    ("lint/lint.json", LINT_JSON),
    (
        "lint/README.md",
        include_str!("../assets/repo/lint/README.md"),
    ),
    (
        "review/rule.json",
        include_str!("../assets/repo/review/rule.json"),
    ),
    (
        "review/_REPORT.md",
        include_str!("../assets/repo/review/_REPORT.md"),
    ),
    ("rules/delegation.md", DELEGATION_MD),
    (
        "rules/common/README.md",
        include_str!("../assets/repo/rules/common/README.md"),
    ),
    (
        "rules/common/coding-style.md",
        include_str!("../assets/repo/rules/common/coding-style.md"),
    ),
    (
        "rules/common/security.md",
        include_str!("../assets/repo/rules/common/security.md"),
    ),
    (
        "rules/common/testing.md",
        include_str!("../assets/repo/rules/common/testing.md"),
    ),
    (
        "rules/common/ECC-LICENSE.txt",
        include_str!("../assets/repo/rules/common/ECC-LICENSE.txt"),
    ),
    ("docs/deliverables/README.md", DELIVERABLES_README),
    (
        "docs/agents/document-writing.md",
        include_str!("../assets/repo/docs/agents/document-writing.md"),
    ),
    (
        "docs/agents/domain.md",
        include_str!("../assets/repo/docs/agents/domain.md"),
    ),
    (
        "docs/agents/issue-tracker.md",
        include_str!("../assets/repo/docs/agents/issue-tracker.md"),
    ),
    (
        "docs/agents/triage-labels.md",
        include_str!("../assets/repo/docs/agents/triage-labels.md"),
    ),
    (
        "docs/planning/intents/_TEMPLATE.md",
        include_str!("../assets/repo/docs/planning/intents/_TEMPLATE.md"),
    ),
];

/// 빈 채로 두는 폴더. `init`이 `.gitkeep`을 둔다.
pub const REPO_DIRS: &[&str] = &[
    "handovers/logs",
    "docs/design-docs/adr",
    "docs/design-docs/mockups",
    "docs/evaluations/qa-reports",
    "docs/exec-plans/phases",
    "docs/generated",
    "docs/operations",
    "docs/planning/product-specs",
];

/// `.fullops-squad/.gitignore`에 있어야 하는 줄. `init`의 템플릿에 있고 `migrate`가 없으면 더한다.
pub const GITIGNORE_LINES: &[&str] = &["docs/evaluations/jev/history/"];

/// 엔트리 문서(`AGENTS.md`·`CLAUDE.md`·`GEMINI.md`)의 관리 블록. 셋 다 같은 글이다(설계 10.6절).
pub const MANAGED_BLOCK: &str = include_str!("../assets/managed-block.md");
pub const BLOCK_START: &str = "<!-- fullops:start -->";
pub const BLOCK_END: &str = "<!-- fullops:end -->";
pub const ENTRY_DOCS: [&str; 3] = ["AGENTS.md", "CLAUDE.md", "GEMINI.md"];

/// 내장 릴리스 노트 `(버전, 글)`. 오래된 것부터다. `upgrade`가 `applied_version` 이후의 것을 낸다.
pub const RELEASES: &[(&str, &str)] = &[("2.0.0", include_str!("../assets/releases/2.0.0.md"))];

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_ne!(
            fullops_core::model::sha256_hex(DELEGATION_MD.as_bytes()),
            DELEGATION_1_24_SHA256
        );

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
