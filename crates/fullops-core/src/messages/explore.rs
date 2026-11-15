//! 탐색 명령(`find`·`context`·`packet`의 J)의 문구. 묶음 J가 채운다. 다른 묶음은 이 파일을 고치지 않는다(설계 18.2절).

/// 규칙 ID의 사유 문구. 이 파일이 맡지 않는 ID면 `None`이다. `args`는 그 검사가 넘긴 값이다.
pub fn reason(id: &str, args: &[String]) -> Option<String> {
    let a = |n: usize| args.get(n).map_or("", String::as_str);
    Some(match id {
        "J-01" => format!(
            "역할 {}의 인박스에 과제 {}의 지시서가 없습니다.",
            a(0),
            a(1)
        ),
        "J-02" => format!("--paths가 {}개입니다. 한 번에 20개까지 분류합니다.", a(0)),
        "J-03" => format!("--decisions의 항목이 맞지 않습니다: {}.", a(0)),
        "J-04" => format!(
            "{}이 이미 있고 입력이 달라졌습니다(달라진 것: {}). 있던 결과를 다시 내지도 덮어쓰지도 않았습니다.",
            a(0),
            a(1)
        ),
        "J-10" => format!("패킷이 지금 지시서와 맞지 않습니다(다른 것: {}).", a(0)),
        "J-11" => format!(
            "패킷을 만든 커밋 {}이 대상 워크트리 HEAD의 조상이 아닙니다.",
            a(0)
        ),
        "J-12" => format!(
            "패킷을 만든 뒤 전달 항목의 원문이 바뀌었거나 없습니다: {}.",
            a(0)
        ),
        "J-13" => format!(
            "패킷을 만든 커밋 {}이 검사하는 커밋의 조상이 아닙니다.",
            a(0)
        ),
        "J-14" => format!("패킷 처리 기록을 쓸 수 없습니다: {}.", a(0)),
        "J-15" => format!("패킷 처리 기록이 전달 목록과 맞지 않습니다: {}.", a(0)),
        "J-16" => format!(
            "패킷에 미확인 항목이 {}건인데 처리 기록의 uncertainty_review가 비었습니다.",
            a(0)
        ),
        "J-17" => format!(
            "과제 {}는 탐색 패킷이 필요한데(route의 requires_packet) 패킷 파일이 없습니다.",
            a(0)
        ),
        _ => return None,
    })
}

/// J 규칙의 탈출구(설계 8.14절의 표).
pub fn escape(id: &str, key: &str, role: &str) -> String {
    let build = format!("fullops packet build {key} --role {role}");
    let outcomes = format!(".fullops-squad/docs/evaluations/jev/{key}-packet-outcomes.json");
    match id {
        "J-01" => format!(
            "`fullops task new --role {role} --key {key} --goal <목표>`. 다른 과제가 인박스에 있으면 `fullops plan add`로 대기시킵니다."
        ),
        "J-02" => "`fullops find`의 `paths:`에서 좁힙니다.".into(),
        "J-03" => "경로를 더하려면 `--seed <경로>`를 씁니다.".into(),
        "J-04" => {
            "같은 명령에 `--force`를 붙이면 이전 결과를 history/에 남기고 다시 만듭니다.".into()
        }
        "J-10" => format!(
            "지시가 바뀌었으면 `{build} --force --write-inbox`, 실수로 본문을 고쳤으면 되돌립니다."
        ),
        "J-11" | "J-12" => format!("`{build} --force --write-inbox` 뒤 다시 배정합니다."),
        "J-13" => "패킷을 만든 커밋 위에서 작업했는지 확인합니다.".into(),
        "J-14" => {
            format!("`{outcomes}`에 처리 기록(v, key, round, packet_input_sha256, items)을 씁니다.")
        }
        "J-15" => format!(
            "`{outcomes}`의 items를 고칩니다. 전달 항목마다 status(completed·no_change)와 reason을 적습니다."
        ),
        "J-16" => {
            format!("`{outcomes}`의 uncertainty_review에 미확인 항목을 확인한 근거를 적습니다.")
        }
        "J-17" => format!("`{build} --write-inbox`"),
        _ => String::new(),
    }
}
