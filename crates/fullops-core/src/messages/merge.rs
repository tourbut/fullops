//! 리뷰·통합 명령(`merge`의 M, `review`, `hold`·`resume`·`settle`, `sync`, `decision`, `gc`)의 문구. 묶음 E가 채운다. 다른 묶음은 이 파일을 고치지 않는다(설계 18.2절).
//! 설계가 ID를 주지 않은 거절에는 이 파일이 ID를 붙였다: `RV-01`~`RV-09`는 `review check`의 실패 항목 번호(8.9절),
//! `RV-00`은 그 밖의 `review` 거절, `HD`는 `resume`·`completion set-sha`, `DC`는 `decision answer`(8.10절)다.

use crate::model::Refusal;

/// 규칙 ID의 사유 문구. 이 파일이 맡지 않는 ID면 `None`이다. `args`의 첫 값은 같은 규칙 안의 경우이고 나머지는 그 검사가 넘긴 값이다.
pub fn reason(id: &str, args: &[String]) -> Option<String> {
    let (case, rest) = args.split_first()?;
    let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
    text(id, case, &rest).map(|(reason, _)| reason)
}

/// 거절 한 건. 종료 코드는 2다.
pub fn refusal(rule: &'static str, case: &str, args: &[&str]) -> Refusal {
    let (reason, escape) =
        text(rule, case, args).unwrap_or_else(|| (args.join(" "), String::new()));
    Refusal {
        rule,
        reason,
        escape,
        exit: 2,
    }
}

/// 사람 손이 필요한 종료(코드 3)의 글. 거절과 같은 꼴이라 규칙 ID가 보인다.
pub fn human(rule: &str, case: &str, args: &[&str]) -> String {
    let (reason, escape) =
        text(rule, case, args).unwrap_or_else(|| (args.join(" "), String::new()));
    super::refusal(rule, &reason, &escape)
}

/// (사유, 탈출구).
fn text(id: &str, case: &str, a: &[&str]) -> Option<(String, String)> {
    let v = |i: usize| a.get(i).copied().unwrap_or_default();
    // 수정 배정은 반려된 보고를 쓴 역할에 간다. 그 다음 값은 그 역할이 설계 역할일 때만 있는 route class다
    let role = |i: usize| a.get(i).copied().filter(|r| !r.is_empty());
    let (reason, escape) = match (id, case) {
        ("M-01", _) => (
            format!("`fullops merge`는 lead만 실행합니다(호출자: {}).", v(0)),
            "lead 세션에서 다시 실행합니다.".to_string(),
        ),
        ("M-02", "none") => (
            format!("{}의 통합 대기 완료 보고가 없습니다.", v(0)),
            "`fullops wait --run <R>`로 보고를 받거나 `fullops status`로 확인합니다.".into(),
        ),
        ("M-02", "held") => (
            format!("{}의 완료 보고가 보류 중입니다.", v(0)),
            format!("`fullops resume {}`", v(0)),
        ),
        ("M-02", "many") => (
            format!("{}의 통합 대기 완료 보고가 여럿입니다: {}.", v(0), v(1)),
            "`--completion <메시지 ID>`로 하나를 고릅니다.".into(),
        ),
        ("M-02", "nosha") => (
            format!("완료 보고 {}에 SHA가 없습니다.", v(0)),
            format!("`fullops completion set-sha {} <SHA>`", v(0)),
        ),
        ("M-03", "nowt") => (
            format!("기본 브랜치 `{}`를 체크아웃한 워크트리가 없습니다.", v(0)),
            "기본 브랜치를 체크아웃한 워크트리를 둔 뒤 다시 실행합니다.".into(),
        ),
        ("M-03", "dirty") => (
            format!(
                "기본 워크트리({})에 커밋하지 않은 변경이 있습니다: {}.",
                v(0),
                v(1)
            ),
            "커밋하거나 치운 뒤 다시 실행합니다.".into(),
        ),
        ("M-03", "merging") => (
            "기본 워크트리에 병합이 진행 중입니다.".into(),
            format!(
                "충돌을 해결해 스테이징했으면 `fullops merge {0} --continue`, 포기하면 `fullops merge {0} --abort`",
                v(0)
            ),
        ),
        ("M-03", "diverged") => (
            format!(
                "원격 기본 브랜치가 먼저 나아가 {}를 병합하다 충돌했습니다({}).",
                v(1),
                v(2)
            ),
            format!(
                "해결해 스테이징한 뒤 `fullops merge {0} --continue`, 포기하면 `fullops merge {0} --abort`",
                v(0)
            ),
        ),
        ("M-04", "rejected") => (
            format!(
                "{} {}의 리뷰 결론이 reject이거나 미해결 critical·high가 있습니다.",
                v(0),
                v(1)
            ),
            format!(
                "수정 배정({}) 뒤 새 SHA로 다시 리뷰합니다.",
                super::flow::fix_round(v(0), role(2), role(3))
            ),
        ),
        ("M-04", "unfinished") => (
            format!("{} {}의 리뷰 기록이 아직 수락되지 않았습니다.", v(0), v(1)),
            format!("리뷰를 마친 뒤 `fullops review check {}`", v(0)),
        ),
        ("M-04", "auto") => (
            format!("{} {}은 독립 리뷰가 필요합니다({}).", v(0), v(1), v(2)),
            format!("`fullops review snapshot {}`", v(0)),
        ),
        ("M-04", "skip") => (
            format!(
                "결정 {}의 답이 생략인데 {} {}의 면제 기록이 없습니다.",
                v(2),
                v(0),
                v(1)
            ),
            format!(
                "`fullops review waive {} --decision {} --reason \"<사용자 지시 요지>\"`",
                v(0),
                v(2)
            ),
        ),
        ("M-04", "proceed") => (
            format!(
                "결정 {}의 답이 진행인데 {} {}의 리뷰 기록이 없습니다.",
                v(2),
                v(0),
                v(1)
            ),
            format!("`fullops review snapshot {}`", v(0)),
        ),
        ("M-04", "ask") => (
            format!(
                "{} {}은 독립 리뷰가 필요합니다({}). 결정 {}가 열려 있습니다.",
                v(0),
                v(1),
                v(2),
                v(3)
            ),
            format!(
                "사용자에게 진행·생략을 묻고 `fullops decision answer {} --answer <진행|생략> --sha {}`",
                v(3),
                v(4)
            ),
        ),
        ("M-05", _) => (
            format!("{} {}을 덮는 lint 통과 기록이 없습니다.", v(0), v(1)),
            format!(
                "worker 워크트리에서 `fullops lint`, 또는 `fullops merge {} --accept-missing-lint \"<사유>\"`",
                v(0)
            ),
        ),
        ("M-06", _) => (
            format!("{} 병합이 충돌했습니다({}).", v(0), v(1)),
            format!(
                "해결해 스테이징한 뒤 `fullops merge {0} --continue`, 포기하면 `fullops merge {0} --abort`",
                v(0)
            ),
        ),
        ("M-07", _) => (
            format!(
                "병합 뒤 lint가 통과하지 못해 기본 브랜치를 병합 전({})으로 되돌렸습니다. {}",
                v(1),
                v(2)
            ),
            format!(
                "수정 배정({}) 또는 `fullops hold {} --reason … --owner … --resume …`",
                super::flow::fix_round(v(0), role(3), role(4)),
                v(0)
            ),
        ),
        ("M-11", "pending") => (
            format!("동기화하지 못한 역할이 있습니다: {}.", v(0)),
            "그 worker의 완료 뒤 `fullops sync --role <R>`".into(),
        ),
        ("M-11", "conflict") => (
            format!(
                "기본 브랜치를 반영하다 충돌해 되돌렸습니다: {}.",
                v(0)
            ),
            "그 워크트리에서 기본 브랜치를 손으로 병합한 뒤 `fullops sync`".into(),
        ),
        ("HD-01", _) => (
            format!("{}의 보류 중인 완료 보고가 없습니다.", v(0)),
            "`fullops status`로 확인합니다.".into(),
        ),
        ("HD-02", "unknown") => (
            format!("완료 보고 {}를 찾지 못했습니다.", v(0)),
            "`fullops status --json`의 `pending[].id`를 씁니다.".into(),
        ),
        ("HD-02", "landed") => (
            format!("완료 보고 {}는 이미 통합됐습니다.", v(0)),
            String::new(),
        ),
        ("HD-02", "sha") => (
            format!("`{}`를 커밋으로 풀지 못했습니다.", v(0)),
            "이 레포에 있는 커밋의 SHA를 줍니다.".into(),
        ),
        ("DC-00", _) => (
            format!("결정 {}를 찾지 못했습니다.", v(0)),
            "`fullops decision list`".into(),
        ),
        ("DC-01", _) => (
            format!(
                "결정 {}는 SHA {}에 고정돼 있는데 `--sha`가 없거나 다릅니다.",
                v(0),
                v(1)
            ),
            format!(
                "`fullops decision answer {} --answer … --sha {}`",
                v(0),
                v(1)
            ),
        ),
        ("DC-02", _) => (
            format!("결정 {}는 만료됐습니다(연 지 24시간이 지났다).", v(0)),
            "다시 묻습니다: `fullops merge <KEY>` 또는 `fullops decision add`".into(),
        ),
        ("DC-03", _) => (
            format!("{}는 결정 {}에 답할 수 있는 사람이 아닙니다.", v(1), v(0)),
            format!("허용된 사람({})이 `--by`로 답합니다.", v(2)),
        ),
        ("DC-04", _) => (
            format!(
                "결정 {}를 연 뒤 lead 세션에 사람 턴이 없습니다. 사람이 정할 것을 에이전트가 스스로 답할 수 없습니다.",
                v(0)
            ),
            "사용자에게 물어 답을 받은 뒤 다시 실행합니다.".into(),
        ),
        ("RV-00", "implementer") => (
            format!("{}의 구현자 세션을 알 수 없습니다.", v(0)),
            "`fullops review snapshot <KEY> --implementer-session <구현한 세션 ID>`".into(),
        ),
        ("RV-00", "nohead") => (
            format!("{}의 리뷰할 SHA를 정하지 못했습니다.", v(0)),
            "`--sha <SHA>`로 줍니다.".into(),
        ),
        ("RV-00", "exists") => (
            format!("리뷰 기록 {}가 이미 있습니다. 덮어쓰지 않습니다.", v(0)),
            "같은 SHA의 리뷰는 그 폴더에서 이어 가고, 새 리뷰는 새 SHA로 엽니다.".into(),
        ),
        ("RV-00", "norecord") => (
            format!("리뷰 기록 {}가 없거나 읽을 수 없습니다.", v(0)),
            format!("`fullops review snapshot {}`", v(1)),
        ),
        ("RV-00", "nosnapshot") => (
            format!("{}의 열린 snapshot이 없습니다.", v(0)),
            "`fullops review prune`으로 목록을 봅니다.".into(),
        ),
        ("RV-00", "held") => (
            format!("snapshot {}를 정리하지 못했습니다: {}", v(0), v(1)),
            "사유를 없앤 뒤 `fullops review cleanup`을 다시 실행합니다.".into(),
        ),
        ("RV-00", "decision") => (
            format!(
                "결정 {}가 answered가 아니거나 SHA가 리뷰할 head({})와 다릅니다.",
                v(0),
                v(1)
            ),
            "그 SHA에 고정된 결정에 사용자의 답을 `fullops decision answer`로 기록합니다.".into(),
        ),
        ("RV-00", "supersede") => (
            format!("{}를 {}로 대체할 수 없습니다: {}", v(0), v(1), v(2)),
            "새 리뷰를 `fullops review check`로 수락한 뒤 다시 실행합니다.".into(),
        ),
        (id, _) if id.starts_with("RV-") => (
            v(0).to_string(),
            "리뷰 기록을 고친 뒤 `fullops review check`를 다시 실행합니다. base·head가 달라졌으면 새 SHA로 `fullops review snapshot`".into(),
        ),
        _ => return None,
    };
    Some((reason, escape))
}
