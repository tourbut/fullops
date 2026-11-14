//! 흐름 명령(`route`의 RT, `task`의 H, `dispatch`의 D, `done`의 R, `wait`, `status`)의 문구. 묶음 C가 채운다. 다른 묶음은 이 파일을 고치지 않는다(설계 18.2절).

/// 규칙 ID의 사유 문구. 이 파일이 맡지 않는 ID면 `None`이다. `args`는 그 검사가 넘긴 값이다.
/// 경우가 여럿인 규칙은 `args[0]`이 경우의 이름이다.
pub fn reason(id: &str, args: &[String]) -> Option<String> {
    let a = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    Some(match (id, a(0)) {
        ("RT-01", _) => "분류는 lead만 한다.".into(),
        ("RT-02", "syntax") => format!("과제 키 `{}`가 문법에 맞지 않는다.", a(1)),
        ("RT-02", _) => format!(
            "`{}`는 이미 분류된 `{}`의 재작업 키로 보인다. 같은 과제의 수정은 새 키를 만들지 않는다.",
            a(1),
            a(2)
        ),
        ("RT-03", _) => format!("요청 글이 {}자다. 4,000자를 넘지 않게 요약한다.", a(0)),
        ("RT-04", "role") => "`--class`가 unresolved가 아니면 `--role`이 필요하다.".into(),
        ("RT-04", _) => format!(
            "기존 기록의 역할 `{}`를 `{}`로 바꾸려면 근거가 필요하다.",
            a(1),
            a(2)
        ),
        ("RT-05", _) => "`--agent`·`--model`·`--effort`는 셋 다 주거나 다 주지 않는다.".into(),

        ("H-01", _) => format!("과제 `{}`의 분류 기록이 없거나 아직 정해지지 않았다.", a(0)),
        ("H-02", _) => format!(
            "이 세션은 `{}` 과제(분류 {})의 `{}` 지시서를 쓸 수 없다.",
            a(0),
            a(1),
            a(2)
        ),
        ("H-03", "brief") => format!(
            "`{}` 역할의 이전 차수를 기록으로 가져오기 전에 설계 역할의 지시서 보고 `{}`가 병합되지 않았다. 지금 가져오면 그 보고의 병합이 완료 기록과 지시서를 섞는다.",
            a(1),
            a(2)
        ),
        ("H-03", _) => format!(
            "`{}` 역할의 인박스가 이미 있다. 역할당 인박스는 하나다.",
            a(0)
        ),
        ("H-04", "missing") => format!(
            "과제 `{}`의 `{}` 완료 기록이 없어 다시 열 수 없다.",
            a(1),
            a(2)
        ),
        ("H-04", _) => format!("과제 `{}`의 `{}` 완료 기록이 이미 있다.", a(0), a(1)),

        ("D-01", _) => format!("과제 `{}`의 분류 기록이 없거나 unresolved다.", a(0)),
        ("D-02", "syntax") => format!("`{}` 역할의 인박스 front matter를 읽을 수 없다.", a(1)),
        ("D-02", "request") => format!(
            "과제 `{}`의 분류 기록에 요청 글이 없어 인박스 없이 받는 `{}` 역할이 할 일을 알 수 없다.",
            a(2),
            a(1)
        ),
        ("D-02", "done") => format!(
            "lead 체크아웃의 `{}` 인박스({}차)는 이미 완료 기록으로 옮겨진 차수다. 그대로 배정하면 worker에게 지시서가 가지 않는다.",
            a(1),
            a(2)
        ),
        ("D-02", "target") => format!(
            "동기화 뒤에도 대상 워크트리에 `{}` 역할의 인박스가 없다.",
            a(1)
        ),
        ("D-02", "key") => format!(
            "`{}` 역할의 인박스는 과제 `{}`의 것이다(배정하려는 과제는 `{}`).",
            a(1),
            a(2),
            a(3)
        ),
        ("D-02", _) => format!("lead 체크아웃에 `{}` 역할의 인박스가 없다.", a(1)),
        ("D-03", _) => format!("통합되지 않은 완료 보고가 있다: {}.", a(0)),
        ("D-04", "run") => format!("Run `{}`은 이 세션이 소유한 Run이 아니다.", a(1)),
        ("D-04", _) => "배정은 lead나 dispatch를 받은 세션만 한다.".into(),
        ("D-05", "role") => format!("`{}`는 등록된 역할이 아니다.", a(1)),
        ("D-05", "none") => "배정할 역할을 정할 수 없다.".into(),
        ("D-05", "route") => format!(
            "분류가 direct인 과제는 분류된 역할 `{}`에만 배정한다(준 역할 `{}`).",
            a(1),
            a(2)
        ),
        ("D-05", _) => format!("`{}` 역할의 브랜치를 체크아웃한 워크트리가 없다.", a(1)),
        ("D-06", "sha") => "리뷰 배정에는 `--sha <40자>`가 필요하다.".into(),
        ("D-06", "snapshot") => format!("SHA `{}`의 리뷰 snapshot이 없다.", a(1)),
        ("D-06", "terminal") => "리뷰는 구현한 세션과 다른 세션에 배정한다.".into(),
        ("D-06", _) => format!("리뷰 snapshot이 {}.", a(1)),
        ("D-07", "count") => format!("과제 `{}`의 한도 재배정이 이미 2회다.", a(1)),
        ("D-07", _) => "한도 재배정은 직전과 같은 agent·model·effort로만 한다.".into(),
        ("D-08", _) => format!("병렬 방침 `{}`가 이 위임을 허용하지 않는다.", a(0)),
        ("D-09", _) => "agent·model·effort를 정할 수 없다.".into(),
        ("D-10", "same") => format!(
            "`{}` 역할에 과제 `{}`의 같은 배정(`{}`)이 이미 실행 중이다.",
            a(1),
            a(2),
            a(3)
        ),
        ("D-10", _) => format!(
            "`{}` 역할에 끝나지 않은 다른 과제 `{}`의 배정이 있다.",
            a(0),
            a(1)
        ),
        ("D-11", _) => format!(
            "`{}` 역할의 브랜치가 동기화 뒤에도 기본 브랜치를 담지 않는다.",
            a(0)
        ),
        ("D-12", _) => format!("탐색 패킷이 지금의 지시서·워크트리와 맞지 않는다: {}", a(0)),
        ("R-10", _) => format!(
            "탐색 패킷의 처리 기록이 완료 검사를 통과하지 못했다: {}",
            a(0)
        ),
        ("D-13", "a") => "하위 worker는 다시 하위 배정을 하지 못한다.".into(),
        ("D-13", "b") => "하위 배정은 자기 과제와 자기 역할에만 한다.".into(),
        ("D-13", "c") => "하위 배정에는 `--brief`가 필요하다.".into(),
        ("D-13", _) => "하위 배정은 `--terminal`·`--sha`·`--reason`을 받지 않는다.".into(),

        ("R-01", "sub") => {
            "하위 worker는 `fullops done`을 하지 않는다. 커밋·lint·완료 기록은 부모가 한다.".into()
        }
        ("R-01", _) => "`fullops done`은 역할 워크트리의 worker나 dev 주 담당이 실행한다.".into(),
        ("R-02", _) => format!("작업 트리에 커밋하지 않은 변경이 {}건 있다.", a(0)),
        ("R-03", _) => format!("코드 변경 {}건에 대한 lint 통과 기록이 없다.", a(0)),
        ("R-04", "syntax") => format!(
            "`{}` 역할의 인박스 front matter를 읽을 수 없어 완료 보고와 완료 기록을 확인하지 못한다.",
            a(1)
        ),
        ("R-04", _) => "인박스의 `## 완료 보고` 절이 비어 있다.".into(),
        ("R-11", _) => "하위 worker의 보고를 아직 받아 처리하지 않았다.".into(),
        _ => return None,
    })
}

/// 같은 과제의 수정 배정 명령(리뷰 reject, M-07, RT-02가 함께 가리킨다). 병합 전에도 통한다.
/// 수정은 lead가 연 구현 역할의 다음 차수다(설계 과제도 같다). 배정에도 역할을 적는다: 설계 과제의 route 역할은 설계 역할이다.
/// `design`(그 과제의 route class)이 있으면 반려된 보고가 설계 역할 자신의 것이다: 설계 역할은 인박스 없이 받으므로
/// 반려 사유를 요청 글로 남기고 다시 배정한다(reopen·`--purpose fix`는 설계 역할에 통하지 않는다).
pub fn fix_round(key: &str, role: Option<&str>, design: Option<&str>) -> String {
    let role = role.unwrap_or("<구현 역할>");
    if let Some(class) = design {
        return format!(
            "`fullops route {key} --force --request \"<반려 사유를 담은 재작업 글>\" --class {class} --role {role}` → `fullops dispatch {key} --role {role}`"
        );
    }
    format!(
        "`fullops task reopen --role {role} --key {key} --goal \"<수정 목표>\"` → `fullops dispatch {key} --role {role} --purpose fix`"
    )
}

/// 배정되지 않은 지시서 한 건의 안내(`leftover`).
pub fn unassigned_inbox(key: &crate::model::TaskKey, role: &crate::model::RoleId) -> String {
    format!(
        "{key}(`{role}` 지시서가 배정되지 않았다: `fullops dispatch {key} --role {role} --agent … --model … --effort …`)"
    )
}

/// 실패한 채 남은 과제 한 건의 안내(`leftover`).
/// `fix`면 실패한 차수가 수정 배정이었다: reopen 출력과 같은 명령을 낸다.
pub fn failed_task(
    key: &crate::model::TaskKey,
    role: Option<&crate::model::RoleId>,
    fix: bool,
) -> String {
    let role = role.map(|r| format!(" --role {r}")).unwrap_or_default();
    let purpose = if fix { " --purpose fix" } else { "" };
    format!(
        "{key}(실패 보고 뒤 멈췄다: 원인을 고쳐 `fullops dispatch {key}{role}{purpose}`로 다시 배정하거나 `fullops plan set {key} \"보류(<사유>)\"`로 미룬다. 행이 없으면 `fullops plan add {key} … --status \"보류(<사유>)\"`)"
    )
}

/// `task reopen`이 예산을 넘었을 때의 알림(거절이 아니다. 설계 8.4절).
pub fn over_budget(budget: u32) -> String {
    format!("같은 과제의 수정·재검증이 예산({budget})을 넘었다. 사용자에게 올려라")
}

/// 지표 옆에 붙이는 한 줄(설계 6.7절).
pub const METRICS_CAVEAT: &str = "기계 입력 일부가 사람 턴으로 세어질 수 있다";

/// dispatch를 찾지 못한 `done`의 알림(설계 8.7절 R-08).
pub const DONE_NO_DISPATCH: &str = "dispatch를 찾지 못해 완료 기록을 state에 남기지 않았다. SHA는 본문으로 전달되고 후속 요청은 본문의 `후속:`으로만 전달된다";

/// 울트라코드로 띄운 배정의 알림 두 줄(설계 8.15절).
pub const ULTRA_STARTED: &str = "울트라코드로 띄웠다(effort는 xhigh가 된다)\nworker의 권한 모드가 auto나 bypass가 아니면 Workflow 실행마다 승인 창에서 멈춘다";

/// grok worker의 폴더 신뢰 알림(설계 8.5절 7단계).
pub const GROK_TRUST: &str = "grok worker는 원본 레포의 폴더 신뢰가 필요하다. 사용자가 grok 화면이나 `/hooks-trust`로 승인한다";
