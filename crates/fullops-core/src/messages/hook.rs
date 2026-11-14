//! hook 규칙(S·P·T·X·F)의 문구. 묶음 B가 맡는다. 다른 묶음은 이 파일을 고치지 않는다(설계 18.2절).

use crate::model::{RoleKind, SkillText, TestLevel};

/// SessionStart의 `Context` 전체 한도(글자 수, 설계 7.7절).
pub const CONTEXT_LIMIT: usize = 9000;
/// S-06이 붙이는 caveman 본문의 한도.
const SKILL_BODY: usize = 3000;

/// `context_at_start`가 거짓인 호스트(grok)에서 S-01이 사용자에게 내는 한 줄.
pub const NO_CONTEXT_AT_START: &str = "이 호스트는 세션 시작 안내를 모델에게 전하지 못한다. 에이전트가 `fullops status`를 먼저 실행하게 한다(`AGENTS.md`의 FullOps 블록이 그렇게 지시한다)";

const PSTACK: &str = "`/pstack:*` 스킬의 '사람을 기다리지 않는다'와 '도구 오류 뒤 다른 모델로 재시도' 규칙은 이 레포에 적용하지 않는다. 사용자 결정, 리뷰 면제, 보류, 한도 재배정은 FullOps 규칙이 우선한다.";
const SKILL_HEAD: &str = "설치된 caveman 스킬을 이 새 세션에 full 강도로 적용한다. 사용자가 강도를 바꾸거나 끄라고 하면 따른다. 안전과 소통에 관한 상위 지시가 우선한다.";

fn arg(args: &[String], i: usize) -> &str {
    args.get(i).map_or("", String::as_str)
}

/// 규칙 ID의 사유 문구. 이 파일이 맡지 않는 ID면 `None`이다. `args`는 그 검사가 넘긴 값이다.
pub fn reason(id: &str, args: &[String]) -> Option<String> {
    let a = |i| arg(args, i);
    Some(match id {
        "S-00" | "P-03" => format!(
            "FullOps 정책 오류: {} 이 상태에서는 도구 호출이 거부되고 종료가 한 번 막힌다. `.fullops-squad/fullops.toml`을 고치거나 새 세션을 연다. 확인은 `fullops doctor`.",
            a(0)
        ),
        "S-05" | "P-01" => args.join("\n"),
        "T-00" => format!("정책 오류라 이 호출을 받지 않는다: {}", a(0)),
        // 인자를 준 T-01은 맡지 않는다: 뼈대 시험(messages/mod.rs)이 그 꼴로 기본 조립을 본다
        "T-01" if args.is_empty() => "날것 `orca orchestration worker-start`는 선행 조건 검사와 상태 기록을 거치지 않는다.".into(),
        "T-02" => "터미널 주입(`dispatch --inject`, 지시서 경로를 실은 `terminal send`)으로 배정하지 않는다.".into(),
        "T-03" => format!("`terminal send`의 글이 {}자다. 20자를 넘는 지시는 터미널에 직접 넣지 않는다.", a(0)),
        "T-04" => format!("날것 `task-create`를 받지 않는다: {}.", a(0)),
        "T-05" => {
            let why = match a(1) {
                "a" => "과제명 지시서를 만들지 않는다. 역할당 인박스는 `to_<역할>.md` 하나다",
                "b" => "인박스는 직접 만들지 않는다",
                "c" => "인박스를 직접 지우거나 비우지 않는다",
                _ => "인박스의 front matter를 없애거나 `key`를 바꾸지 않는다",
            };
            format!("`{}`: {why}.", a(0))
        }
        "T-06" => format!(
            "`{}`: route가 {}인 과제의 지시서(작성자 `{}`)는 lead가 직접 고치지 않는다.",
            a(0),
            a(1),
            a(2)
        ),
        "T-07" => format!("`{}`: 설계 역할은 코드 파일을 고치지 않는다.", a(0)),
        "T-08" => match a(0) {
            "e" => "병렬 방침이 off라 하위 에이전트를 쓰지 않는다.".into(),
            "f" => "병렬 방침이 read인데 하위 에이전트 지시에 `Purpose:`가 없다.".into(),
            _ => "병렬 방침이 read라 읽기 전용 조사·리뷰만 하위 에이전트에 맡길 수 있다.".into(),
        },
        "T-09" => format!("보고서가 링크하지 않은 QA 미디어가 커밋 대상에 있다: {}.", a(0)),
        "T-10" => format!(
            "`qa-reports/{}/`: 차수 폴더를 새로 만들지 않는다. 같은 과제의 `qa-reports/{}/`가 이미 있다.",
            a(0),
            a(1)
        ),
        "X-00" => format!("정책 오류: {}", a(0)),
        "X-01" => format!(
            "lint 통과 기록이 덮지 못하는 변경이 있다(시작점 {}): {}.",
            a(0),
            a(1)
        ),
        "X-02" => format!(
            "변경이 리뷰 기준을 넘는데({}) HEAD {}의 독립 리뷰나 면제 기록이 없다.",
            a(1),
            a(0)
        ),
        "X-03" => format!("dispatch {}의 완료 보고가 없다.", a(0)),
        "X-04" => format!("분류만 하고 배정하지 않은 과제가 있다: {}.", a(0)),
        "X-05" => args.join(" "),
        "X-06" => format!("통합되지 않은 완료 보고가 있다: {}.", a(0)),
        "X-07" => format!("이어 갈 과제가 있다: {}.", a(0)),
        _ => return None,
    })
}

/// 함께 걸린 Stop 규칙을 `also`에 붙일 때의 한 줄(ID와 요약).
pub fn also_line(id: &str) -> &'static str {
    match id {
        "X-00" => "X-00 정책 오류",
        "X-01" => "X-01 lint 통과 없는 변경",
        "X-02" => "X-02 리뷰 없는 직접 작업",
        "X-03" => "X-03 완료 보고 없음",
        "X-04" => "X-04 분류 뒤 미배정",
        "X-05" => "X-05 Run 대기",
        "X-06" => "X-06 미통합 완료 보고",
        "X-07" => "X-07 준비된 과제",
        _ => "",
    }
}

/// 한 번 막은 뒤 같은 턴에 같은 상태로 다시 끝낼 때의 경고.
pub fn passed_once(rule: &str, reason: &str) -> String {
    format!("[{rule}] 이미 한 번 막았으므로 이번에는 통과시킨다: {reason}")
}

/// 세션당 한 번 내는 알림(설계 7.8절).
pub fn notice(key: &str) -> &'static str {
    match key {
        "state-newer" => {
            "`state.json`이 이 실행본보다 새 형식이다. 설치본이 낡았다. `fullops install`로 갱신한다."
        }
        "state-broken" => {
            "`state.json`을 읽지 못했다. 상태에 기대는 검사를 건너뛴다. `fullops doctor`로 복구한다."
        }
        "plan-parse" => {
            "`PLANS.md`의 과제 표에 읽지 못한 행이 있다. `fullops plan check`로 줄 번호를 본다."
        }
        _ => "",
    }
}

/// 역할 안내(설계 7.7절의 5종과 좌석 `Other`·`Reviewer`의 한 줄).
pub enum RoleGuide<'a> {
    DevPrimary,
    Coordinator,
    Role { id: &'a str, kind: RoleKind },
    Reviewer,
    Other,
}

/// 묻는 기준(1.24.0 BRIEF). 설계 역할·worker가 함께 쓴다.
const OWN_CHOICE: &str = "되돌릴 수 있는 디자인·연출·문구·배치 선택과 버그 수정 여부는";
/// 완료 보고를 보내는 법. 하위 worker에게는 내지 않는다(`fullops done`은 부모가 한다).
const SEND_DONE: &str = " 끝나면 커밋하고 `fullops done`이 출력한 본문으로 preamble의 `worker_done`을 한 번 보낸다. 보내지 않고 끝내면 종료가 한 번 막힌다.";

/// `split`은 `routing = "split"`(제품 기획과 기술 계획을 나눈 구조)이다.
pub fn role_guide(guide: &RoleGuide, sub: bool, split: bool) -> String {
    let mut out = match guide {
        RoleGuide::DevPrimary => "FullOps 역할: dev 모드의 주 담당이다. 사용자와 직접 기술 계획·구현·검증을 한다. 다른 역할에 맡길 일은 `fullops route` → `fullops task new` → `fullops dispatch`로 배정하고 완료 보고를 통합한다. 제품 범위를 바꾸는 일은 사용자에게 확인한다. 직접 작업은 dispatch나 `worker_done` 없이 커밋하고 lint를 통과시킨 뒤 `fullops done`으로 완료를 기록한다. 변경이 리뷰 기준을 넘으면(`fullops review need`) 다른 세션의 독립 리뷰를 받는다.".to_string(),
        RoleGuide::Coordinator => format!(
            "FullOps 역할: coordinator(lead)다. 직접 구현하지 않는다. 새 개발 요청은 과제 키를 정해 `fullops route <KEY> --request \"<요청>\"`으로 먼저 분류한다. {} 운영 작업(설치·설정 갱신, 워크트리 동기화, 병합)은 분류하지 않고 직접 처리한다. worker의 질문 가운데 경로·순서·재시도 같은 운영 문제는 `orca orchestration reply --id <메시지 ID> --body \"<답>\"`으로 직접 답하고, {} 과제나 역할이 둘 이상인 묶음은 `docs/planning/intents/<묶음 키>.md`를 쓰고 사용자 승인을 받은 뒤 배정한다.",
            if split {
                "`direct`면 `fullops task new`로 그 역할의 인박스를 만들어 목표·규칙·범위·완료 조건·정본 링크를 짧게 적고 `fullops dispatch <KEY>`로 배정한다. 기술 계획·분석·구현·테스트는 담당 worker의 같은 과제다. `product`(제품 규칙·범위·공유 제품 기준)만 제품 기획자에게 배정하고(`fullops dispatch <KEY> --role <기획 역할>`), `unresolved`는 근거를 확인해 `--force --class`로 정한다."
            } else {
                "`direct`면 `fullops task new`로 그 역할의 인박스를 만들어 `fullops dispatch <KEY>`로 배정하고, `design`이면 설계 역할에 배정한다(`fullops dispatch <KEY> --role <설계 역할>`)."
            },
            if split {
                "제품 규칙·범위 질문은 직접 답하지 않고 제품 기획자에게 넘긴다(기술 질문은 구현 담당에게 돌려준다)."
            } else {
                "설계·범위·공유 계약 질문은 직접 답하지 않고 설계 역할에 넘긴다."
            }
        ),
        RoleGuide::Role { id, kind: RoleKind::Design } => format!(
            "FullOps 역할: {} 코드 파일은 고치지 않는다. 구현은 `fullops task new --role <구현 역할> --key <KEY> --goal \"<목표>\"`로 만든 지시서에 적어 넘긴다. {OWN_CHOICE} 사용자 결정으로 올리지 않고 직접 정해 산출물에 선택과 되돌리는 방법을 적는다. 완료 보고의 본문은 `[설계] <KEY> | … | 지시서: <만든 인박스 경로>` 꼴이다.",
            if split {
                format!("제품 기획자 `{id}`다. 제품 목표, 규칙·수치, 화면·아트 방향, 우선순위, 사용자 완료 조건과 모호한 공유 제품 기준을 정한다. 기술 계획, 구조·API, 버그 수정 방법은 구현 역할의 책임이다.")
            } else {
                format!("설계 역할 `{id}`다. 설계 문서와 역할별 지시서만 쓴다.")
            }
        ),
        RoleGuide::Role { id, kind: RoleKind::Tester } => format!("FullOps 역할: tester `{id}`다. 인박스의 검증 범위를 테스트 레벨에 맞춰 확인한다. 제품 코드는 고치지 않고 테스트 코드·시나리오·검증 증거만 만든다. 도구와 순서는 `fullops-test` 스킬을 따르고 결과는 `qa-reports/<KEY>-test/report.md`에 남긴다. 실패는 재현 절차와 증거로 보고하고 직접 고치지 않는다."),
        RoleGuide::Role { id, .. } => format!(
            "FullOps 역할: worker `{id}`다. 인박스의 과제 하나를 완료 기준까지 구현한다. {} {OWN_CHOICE} 묻지 않고 추천안으로 진행하고 완료 보고에 선택과 되돌리는 방법을 적는다.",
            if split {
                "코드 파악, 짧은 기술 계획, 구현, 테스트와 기술 문서 갱신을 같은 과제에서 한다. 기술 판단은 직접 해결하고, 제품 규칙 변경·범위 확대·공유 제품 기준이 불명확한 것만 추측하지 않고 preamble의 `ask`로 묻는다."
            } else {
                "설계·범위·공유 계약·되돌릴 수 없는 일은 추측하지 않고 preamble의 `ask`로 묻는다."
            }
        ),
        RoleGuide::Reviewer => "FullOps: 읽기 전용 리뷰 사본이다. 파일을 고치지 않고 리뷰 결과만 남긴다.".to_string(),
        RoleGuide::Other => "FullOps: 이 체크아웃에는 역할 규칙이 적용되지 않는다.".to_string(),
    };
    if sub {
        out.push_str(" 하위 worker다. 커밋·lint·`fullops done`은 부모가 한다. 맡은 범위를 끝내면 preamble의 `worker_done`을 보낸다.");
    } else if matches!(guide, RoleGuide::Role { .. }) {
        out.push_str(SEND_DONE);
    }
    out
}

/// 모든 레벨에 붙는 문장(1.24.0 `test_brief`).
const TEST_COMMON: &str = "보안·데이터 손실 방지와 프로젝트 필수 검사는 모든 레벨에서 유지한다. 검사가 통과하면 인계하고, 새 변경·실패·근거 결함이 있을 때만 재검증한다. 같은 과제의 수정·재검증은 기존 `qa-reports/<KEY>-test/report.md`에 절을 덧붙이고, 커밋하는 캡처는 `report.md`가 링크하는 것만이다.";

pub fn test_level(level: TestLevel) -> String {
    let scope = match level {
        TestLevel::Minimal => "minimal: 기동과 변경 경로의 스모크 검사만 한다. 통합 후보의 주요 흐름은 `rules/common/testing.md`의 `담당과 반복 범위`대로 한 번 돌린다. 별도 tester·designer 검수 차수를 만들지 않는다. 개발 과제는 lint와 변경 동작 테스트로 검증하고 보고용 캡처를 만들지 않는다.",
        TestLevel::Lite => "lite: 변경한 핵심 동작과 필요한 실패 경계만 짧게 검증한다. 사소한 문구 변경에는 새 테스트를 만들지 않는다. 개발 과제는 lint와 변경 동작 테스트로 검증하고 보고용 캡처를 만들지 않는다. tester 독립 QA와 designer 시각 검수는 새 화면·새 흐름·통합 후보에서만 배정한다.",
        TestLevel::Standard => "standard: 변경 동작·실패 경계와 영향을 받는 연동의 회귀를 검증한다.",
        TestLevel::Full => "full: 안정된 통합 후보에서 전체 자동 회귀와 필요한 통합·수락 검사를 담당자별로 한 번 수행한다.",
        TestLevel::Exhaustive => "exhaustive: full 범위에 프로젝트에 필요한 E2E·성능·장시간·환경 검수를 더해 안정된 후보에서 한 번 수행한다.",
    };
    format!("테스트 레벨 {scope} {TEST_COMMON}")
}

pub const INBOX_RULE: &str = "인박스 규칙: 현재 과제는 역할 인박스(`.fullops-squad/handovers/to_<역할>.md`) 하나다. 다음 과제는 `PLANS.md`에 대기로 둔다. 끝나면 `fullops done`을 실행한다.";
pub const LINT_GATE: &str = "lint 게이트: 코드를 바꾸면 커밋하고 `fullops lint --from <기준>`을 통과시킨다. 통과 기록이 없으면 종료가 한 번 막힌다(dev 직접 작업은 계속 막힌다).";
pub const LEAD_MERGE: &str =
    "완료 보고마다 `fullops merge <KEY>`로 리뷰 판정·병합·push·동기화를 바로 처리한다.";
pub const LEAD_WAIT: &str = "대기: worker가 실행 중이면 턴을 끝낸다. Orca가 메일이 올 때 깨운다. 깨어나면 `fullops wait --run <R>`(또는 Orca 알림의 `orca orchestration check --run <R>`)로 받는다. Orca가 깨우는 세션에서는 이 명령이 기다리지 않고 받은 것만 돌려주고, Orca가 깨울 수 없는 세션에서는 기다린다.";
pub const LEAD_HUMAN: &str = "항상 사람 승인: 운영 배포, 운영 부하 테스트, backfill, 운영 데이터 정리. 되돌릴 수 있는 선택과 버그 수정 여부는 사용자에게 올리지 않고 스스로 정한다.";

/// S-01이 담는 내용(설계 7.7절). 줄이는 순서가 있는 항목만 따로 둔다.
#[derive(Debug, Clone, Default)]
pub struct Brief {
    /// 역할 안내부터 lead의 고정 항목까지. 줄이지 않는다.
    pub head: Vec<String>,
    /// 준비된 행 `(키, 목표)`. lead만.
    pub ready: Vec<(String, String)>,
    /// 병렬 방침이 `on`·`ultra`다.
    pub parallel_on: bool,
    /// `## 위임 범위`의 사용자 원문.
    pub delegation: Vec<String>,
    /// 열린 결정 한 줄씩.
    pub decisions: Vec<String>,
    /// 건수·예약·버전·도구 누락 같은 끝 항목. 줄이지 않는다.
    pub tail: Vec<String>,
    /// 호스트가 `claude`다(pstack 문장).
    pub pstack: bool,
}

fn clip(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

fn skill_text(skill: &SkillText, with_body: bool) -> String {
    let path = skill.path.display();
    if !with_body {
        return format!("{SKILL_HEAD} (본문은 `{path}`를 읽는다)");
    }
    let body = skill.body.trim();
    if body.chars().count() > SKILL_BODY {
        format!(
            "{SKILL_HEAD}\n{}\n(나머지는 `{path}`를 읽는다)",
            clip(body, SKILL_BODY)
        )
    } else {
        format!("{SKILL_HEAD}\n{body}")
    }
}

/// 줄이기 단계 `cut`(0~5)을 적용한 글.
fn render(brief: &Brief, skill: Option<&SkillText>, cut: u8) -> String {
    let mut parts = brief.head.clone();
    if brief.pstack {
        parts.push(PSTACK.into());
    }
    if !brief.ready.is_empty() {
        let shown = match cut {
            0 | 1 => 10,
            2..=4 => 5,
            _ => 0,
        };
        let mut text = format!("준비된 과제 {}건", brief.ready.len());
        if shown == 0 {
            text.push_str("(`fullops plan ready`)");
        } else {
            text.push(':');
            for (key, goal) in brief.ready.iter().take(shown) {
                text.push_str(&format!("\n- {key}: {}", clip(goal, 60)));
            }
            if brief.ready.len() > shown {
                text.push_str("\n- 나머지는 `fullops plan ready`");
            }
        }
        text.push_str("\n계속할지 묻지 않고 배정한다.");
        if brief.parallel_on {
            text.push_str(" 담당 역할이 서로 다른 행은 한 턴에 함께 배정한다.");
        }
        parts.push(text);
    }
    if !brief.delegation.is_empty() {
        parts.push(if cut >= 3 {
            "위임 범위: `PLANS.md`의 `## 위임 범위`를 읽는다.".into()
        } else {
            format!("위임 범위(사용자 원문):\n{}", brief.delegation.join("\n"))
        });
    }
    if !brief.decisions.is_empty() {
        parts.push(if cut >= 4 {
            format!(
                "열린 결정 {}건(`fullops decision list --open`)",
                brief.decisions.len()
            )
        } else {
            format!("열린 결정:\n{}", brief.decisions.join("\n"))
        });
    }
    parts.extend(brief.tail.iter().cloned());
    if let Some(skill) = skill {
        parts.push(skill_text(skill, cut == 0));
    }
    parts.join("\n\n")
}

/// S-01의 안내와 S-06의 caveman 글을 한 `Context`로 잇는다. 한도를 넘으면 설계 7.7절의 순서로 줄인다.
pub fn session_context(brief: &Brief, skill: Option<&SkillText>) -> String {
    for cut in 0..=5 {
        let text = render(brief, skill, cut);
        if text.chars().count() <= CONTEXT_LIMIT {
            return text;
        }
    }
    const TAIL: &str = "(안내가 잘렸다. `fullops status`)";
    let keep = CONTEXT_LIMIT - TAIL.chars().count();
    format!("{}{TAIL}", clip(&render(brief, skill, 5), keep))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_guides_fit_the_prompt_limit_and_sub_workers_do_not_run_done() {
        use RoleKind::*;
        let guides = [
            RoleGuide::DevPrimary,
            RoleGuide::Coordinator,
            RoleGuide::Role { id: "r", kind: Design },
            RoleGuide::Role { id: "r", kind: Tester },
            RoleGuide::Role { id: "r", kind: Worker },
        ];
        for guide in &guides {
            for split in [false, true] {
                // P-01과 S-05도 이 글을 쓴다. UserPromptSubmit의 Context는 2,000자 이하다(설계 7.7절)
                assert!(role_guide(guide, true, split).chars().count() < 1500);
            }
        }
        let worker = RoleGuide::Role { id: "r", kind: Worker };
        assert!(role_guide(&worker, false, false).contains("`fullops done`이 출력한 본문으로"));
        let sub = role_guide(&worker, true, false);
        assert!(sub.contains("`fullops done`은 부모가 한다") && !sub.contains("이 출력한 본문"));
        // lead는 완료 보고를 보내지 않는다
        assert!(!role_guide(&RoleGuide::Coordinator, false, false).contains("worker_done"));
        for level in [TestLevel::Minimal, TestLevel::Exhaustive] {
            assert!(test_level(level).contains("모든 레벨에서 유지한다"));
        }
    }

    #[test]
    fn session_context_shrinks_in_design_order() {
        let skill = SkillText {
            path: "/h/.agents/skills/caveman/SKILL.md".into(),
            body: "가".repeat(5000),
        };
        let mut brief = Brief {
            head: vec!["역할".into()],
            ready: (0..12)
                .map(|i| (format!("K-{i}"), "목표".repeat(40)))
                .collect(),
            delegation: vec!["위임".repeat(10)],
            decisions: vec!["D001 질문".into()],
            ..Brief::default()
        };
        // 한도 안: 본문 3,000자와 경로, 준비된 행 10개와 목표 60자
        let text = session_context(&brief, Some(&skill));
        assert!(text.contains(&"가".repeat(3000)) && !text.contains(&"가".repeat(3001)));
        assert!(text.contains("(나머지는 `/h/.agents/skills/caveman/SKILL.md`를 읽는다)"));
        assert!(text.contains("- K-9: ") && !text.contains("- K-10: "));
        assert!(text.contains("나머지는 `fullops plan ready`"));
        // 1단계: 본문을 뺀다
        brief.head.push("나".repeat(6500));
        let text = session_context(&brief, Some(&skill));
        assert!(text.contains("(본문은 `") && !text.contains("가가가") && text.contains("- K-9: "));
        // 2~4단계: 준비된 행 5개, 위임 범위 한 문장, 결정 건수
        brief.delegation = vec!["다".repeat(1500)];
        brief.head.push("라".repeat(1300));
        let text = session_context(&brief, Some(&skill));
        assert!(text.contains("- K-4: ") && !text.contains("- K-5: "));
        assert!(text.contains("`## 위임 범위`를 읽는다") && text.chars().count() <= CONTEXT_LIMIT);
        // 다 줄여도 넘으면 자른다
        brief.head.push("마".repeat(9000));
        let text = session_context(&brief, Some(&skill));
        assert_eq!(text.chars().count(), CONTEXT_LIMIT);
        assert!(text.ends_with("(안내가 잘렸다. `fullops status`)"));
    }
}
