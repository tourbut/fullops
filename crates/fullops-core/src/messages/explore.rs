//! 탐색 명령(`find`·`context`의 J)의 문구. 묶음 J가 채운다. 다른 묶음은 이 파일을 고치지 않는다(설계 18.2절).

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
        "J-04" => format!(
            "{}이 이미 있고 입력이 달라졌습니다(달라진 것: {}). 있던 결과를 다시 내지도 덮어쓰지도 않았습니다.",
            a(0),
            a(1)
        ),
        _ => return None,
    })
}

/// J 규칙의 탈출구(설계 8.14절의 표).
pub fn escape(id: &str, key: &str, role: &str) -> String {
    match id {
        "J-01" => format!(
            "`fullops task new --role {role} --key {key} --goal <목표>`. 다른 과제가 인박스에 있으면 `fullops plan add`로 대기시킵니다."
        ),
        "J-02" => "`fullops find`의 `paths:`에서 좁힙니다.".into(),
        "J-04" => {
            "같은 명령에 `--force`를 붙이면 이전 결과를 history/에 남기고 다시 만듭니다.".into()
        }
        _ => String::new(),
    }
}
