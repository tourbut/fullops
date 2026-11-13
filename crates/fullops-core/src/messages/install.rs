//! 설치·이행 명령(`install`, `doctor`, `init`, `mode`, `upgrade`, `migrate`)의 문구. 묶음 F가 채운다. 다른 묶음은 이 파일을 고치지 않는다(설계 18.2절).

/// 규칙 ID의 사유 문구. 이 파일이 맡지 않는 ID면 `None`이다. `args`는 그 검사가 넘긴 값이다.
pub fn reason(_id: &str, _args: &[String]) -> Option<String> {
    None
}
