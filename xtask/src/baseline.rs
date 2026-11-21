//! `cargo xtask baseline [--out <파일>] [--against <파일>]`: 역할을 접는 작업 앞뒤에만 돌리는 행동 기준선 10개.
//! 상시 CI에 넣지 않는다. 모델을 부르지 않고 기존 시나리오(rs 구동기)와 스킬 핵심 본문의 문구로 채점한다.
//! 근거와 쓰는 법: `docs/decisions/single-agent-loop.md`.

use crate::compare::{failed, run_rs, workspace};
use fullops_testkit::scenario;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

/// 고정 케이스 하나. 시나리오는 하네스의 판정을, `terms`는 흔한 경로의 절차가 스킬 핵심 본문에 남았는지를 본다.
struct Case {
    id: &'static str,
    what: &'static str,
    scenario: &'static str,
    skill: &'static str,
    terms: &'static [&'static str],
}

const CASES: &[Case] = &[
    Case {
        id: "B01",
        what: "과제 3개를 배정·완료·병합까지(선행 1, 사람 대기 1)",
        scenario: "k4.toml",
        skill: "fullops-orca",
        terms: &[
            "fullops task new",
            "fullops dispatch",
            "fullops wait",
            "fullops merge",
            "fullops plan ready",
        ],
    },
    Case {
        id: "B02",
        what: "dev 모드 세션 시작 안내",
        scenario: "b3.toml",
        skill: "fullops-work",
        terms: &["fullops status"],
    },
    Case {
        id: "B03",
        what: "병렬 방침 read: 읽기 전용 선언이 있어야 하위 에이전트 통과",
        scenario: "d7-read.toml",
        skill: "fullops-work",
        terms: &["기본 `read`", "Purpose: research", "read only", "T-08"],
    },
    Case {
        id: "B04",
        what: "worker가 lint 없이 끝내면 한 번 막힘",
        scenario: "g1.toml",
        skill: "fullops-work",
        terms: &["fullops lint --from", "fullops done", "worker_done"],
    },
    Case {
        id: "B05",
        what: "dev 직접 작업의 큰 변경은 리뷰나 면제 기록이 있어야 끝남",
        scenario: "g5.toml",
        skill: "fullops-review",
        terms: &["fullops review need"],
    },
    Case {
        id: "B06",
        what: "fullops done의 lint 증거와 완료 보고",
        scenario: "h4.toml",
        skill: "fullops-work",
        terms: &["## 완료 보고", "fullops done", "--followup"],
    },
    Case {
        id: "B07",
        what: "review need: 399줄 / 400줄 / 15파일 / always / 문서만",
        scenario: "l1.toml",
        skill: "fullops-review",
        terms: &["fullops review need"],
    },
    Case {
        id: "B08",
        what: "대규모 변경 merge: 사용자 결정 없음 / 답 생략 뒤 waive",
        scenario: "l2.toml",
        skill: "fullops-orca",
        terms: &["fullops merge", "종료 코드 3", "fullops decision answer"],
    },
    Case {
        id: "B09",
        what: "merge fast-forward와 역할 워크트리 동기화",
        scenario: "l5.toml",
        skill: "fullops-orca",
        terms: &["fullops merge", "fullops sync"],
    },
    Case {
        id: "B10",
        what: "한도로 끝난 worker의 재배정은 같은 tuple로 두 번까지",
        scenario: "d6.toml",
        skill: "fullops-orca",
        terms: &["--reason limit"],
    },
];

pub fn run(args: &[String]) -> Result<(), String> {
    let mut out = None;
    let mut against = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--out" => out = rest.next().map(PathBuf::from),
            "--against" => against = rest.next().map(PathBuf::from),
            other => {
                return Err(format!(
                    "모르는 인자: {other}\n사용법: cargo xtask baseline [--out <파일>] [--against <파일>]"
                ));
            }
        }
    }
    let root = workspace();
    let mut table = String::from(
        "| 케이스 | 무엇 | 시나리오 | 스킬 | 결과 | 사유 |\n|---|---|---|---|---|---|\n",
    );
    let mut results = Vec::new();
    let mut sizes = BTreeMap::new();
    for case in CASES {
        eprintln!("{}: {}", case.id, case.what);
        let mut why = Vec::new();
        let scenario = scenario::load(&root.join("scenarios").join(case.scenario))?;
        let ran = std::panic::catch_unwind(|| run_rs(&scenario))
            .unwrap_or_else(|_| failed(&scenario, "rs 구동기가 죽었습니다".into()));
        for (n, step) in ran.steps.iter().enumerate() {
            if step.verdict == "pending" || !step.note.is_empty() {
                why.push(format!("{}단계 {}: {}", n + 1, step.verdict, step.note));
            }
        }
        let path = root.join("plugin/skills").join(case.skill).join("SKILL.md");
        let body =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        sizes.insert(case.skill, body.chars().count());
        for term in case.terms.iter().filter(|t| !body.contains(**t)) {
            why.push(format!("핵심 본문에 `{term}` 없음"));
        }
        let pass = why.is_empty();
        let shown = why.join("; ").replace('|', "\\|");
        let verdict = if pass { "pass" } else { "FAIL" };
        let _ = writeln!(
            table,
            "| {} | {} | {} | {} | {verdict} | {shown} |",
            case.id, case.what, case.scenario, case.skill
        );
        results.push((case.id, pass));
    }
    let passed = results.iter().filter(|(_, p)| *p).count();
    println!("{table}\n통과 {passed}/{}\n", results.len());
    println!("| 스킬 핵심 본문 | 글자 수 |\n|---|---|");
    for (skill, chars) in &sizes {
        println!("| {skill} | {chars} |");
    }
    let mut worse = 0;
    if let Some(file) = against {
        let before =
            std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let old: BTreeMap<&str, bool> = before
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(id, r)| (id, r == "pass"))
            .collect();
        println!("\n뒤집힌 케이스(기준: {})", file.display());
        let mut any = false;
        for (id, pass) in &results {
            match old.get(id) {
                Some(was) if was != pass => {
                    any = true;
                    worse += usize::from(*was);
                    println!(
                        "- {id}: {} → {}",
                        if *was { "pass" } else { "FAIL" },
                        if *pass { "pass" } else { "FAIL" }
                    );
                }
                None => println!("- {id}: 기준 파일에 없음"),
                _ => {}
            }
        }
        if !any {
            println!("- 없음");
        }
    }
    if let Some(file) = out {
        let text: String = results
            .iter()
            .map(|(id, p)| format!("{id}\t{}\n", if *p { "pass" } else { "fail" }))
            .collect();
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    if passed < results.len() || worse > 0 {
        return Err(format!(
            "기준선 실패: 통과 {passed}/{}, pass에서 FAIL로 뒤집힌 케이스 {worse}개",
            results.len()
        ));
    }
    Ok(())
}
