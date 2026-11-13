//! 리뷰·통합 명령(`merge`의 M, `review`, `hold`·`resume`·`settle`, `sync`, `decision`, `gc`)의 문구. 묶음 E가 채운다. 다른 묶음은 이 파일을 고치지 않는다(설계 18.2절).

/// 규칙 ID의 사유 문구. 이 파일이 맡지 않는 ID면 `None`이다. `args`는 그 검사가 넘긴 값이다.
pub fn reason(_id: &str, _args: &[String]) -> Option<String> {
    None
}
