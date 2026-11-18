//! lint와 문서 명령(`lint`, `doc`)의 문구. 묶음 D가 채운다. 다른 묶음은 이 파일을 고치지 않는다(설계 18.2절).

/// 규칙 ID의 사유 문구. 이 파일이 맡지 않는 ID면 `None`이다. `args`는 그 검사가 넘긴 값이다.
/// lint 검사 코드(설계 8.11절 9번)는 결과의 `message`가 된다.
pub fn reason(id: &str, args: &[String]) -> Option<String> {
    let arg = |n: usize| args.get(n).map_or("", String::as_str);
    Some(match id {
        // args: 줄 수, 이전 줄 수, 상한
        "SIZE-001" => format!("{}줄(이전 {}, 상한 {}).", arg(0), arg(1), arg(2)),
        // args: 추가 줄 합, 상한
        "SIZE-002" => format!("변경 추가 {}줄 > 상한 {}줄.", arg(0), arg(1)),
        "DEP-001" => "의존성 선언 파일 변경.".into(),
        "ANTI-002" => "eval()/exec() 금지.".into(),
        "ANTI-003" => "범위 없는 억제 금지.".into(),
        "ANTI-004" => "테스트 skip·only 표식 추가 금지.".into(),
        // args: 줄어든 케이스 수. 없으면 파일 삭제다
        "ANTI-005" if arg(0).is_empty() => "테스트 파일 삭제.".into(),
        "ANTI-005" => format!("테스트 케이스 {}개 감소.", arg(0)),
        "SEC-001" => "하드코딩 비밀값 의심.".into(),
        "DOC-001" => {
            "새 코드 파일에 무엇을 하는지 적은 헤더 설명(docstring·주석 1~3줄)이 없다.".into()
        }
        // args: 문제, 문서 경로(.fullops 기준), 산출물 ID
        "DOC-002" => format!("{} front matter: {}.", arg(2), arg(0)),
        "DOC-003" => format!("front matter: {}.", arg(0)),
        "QA-001" => "커밋하는 QA 증거에 압축·영상·trace 파일이 있다.".into(),
        // args: 경우(count|size), 값(장 수 또는 KB), 상한
        "QA-002" if arg(0) == "count" => {
            format!("QA 폴더의 이미지가 {}장이다(상한 {}장).", arg(1), arg(2))
        }
        "QA-002" => format!("QA 이미지가 {}KB다(상한 {}KB).", arg(1), arg(2)),
        "LINT-000" => "프로젝트 lint 명령이 등록되지 않았다.".into(),
        "LINT-001" => "이 브랜치의 lint 설정 변경은 적용하지 않았다.".into(),
        // args: 명령 이름, 걸린 초
        "LINT-002" => format!(
            "명령 {}이 {}초 걸렸다. 범위 없이 코드 변경마다 돈다.",
            arg(0),
            arg(1)
        ),
        // `fullops lint`의 거절(규칙 ID는 둘 다 `LINT`). 리뷰 사본에서 불렀다
        "LINT-SNAPSHOT" => {
            "읽기 전용 리뷰 사본에서는 lint를 돌리지 않습니다(설치된 의존성이 없고 실행 산출물이 사본을 더럽힙니다).".into()
        }
        // args: 더러운 경로들
        "LINT" => format!(
            "작업 트리가 깨끗하지 않습니다({}). lint 결과는 커밋에 고정됩니다.",
            arg(0)
        ),
        _ => return None,
    })
}

/// lint 검사 코드의 고치는 방법. 결과의 `suggestion`이 된다. `args`는 `reason`과 같다.
pub fn suggestion(id: &str, args: &[String]) -> String {
    let arg = |n: usize| args.get(n).map_or("", String::as_str);
    match id {
        "SIZE-001" => {
            "① 삭제 → ② 압축 → ③ 분할 순으로 처리하고 docstring·헤더를 깎지 않는다".into()
        }
        "SIZE-002" => "지시서의 예상 변경 규모와 차이·분할하지 않은 이유를 검토한다".into(),
        "DEP-001" => "완료 보고에 의존성 변경 여부와 필요성·표준 라이브러리 대안을 기록한다".into(),
        "ANTI-002" => "입력을 해석하는 명시적 코드로 바꾼다".into(),
        // args: 좁힌 표기
        "ANTI-003" => format!("{}로 좁힌다", arg(0)),
        "ANTI-004" => "실패 원인을 고치거나 테스트를 정당하게 바꾼다".into(),
        "ANTI-005" => "대체 테스트나 삭제 이유를 확인한다".into(),
        "SEC-001" => "환경변수·설정으로 옮긴다".into(),
        "DOC-001" => "파일 맨 위에 설명을 적는다".into(),
        "DOC-002" | "DOC-003" => format!("`fullops doc stamp --path {}`로 다시 쓴다", arg(1)),
        "QA-001" => {
            "판정에 쓴 장면만 이미지로 남기고 원본은 그 폴더의 `raw/`로 옮긴다(커밋되지 않는다)"
                .into()
        }
        "QA-002" if arg(0) == "count" => {
            "보고서의 판정 항목이 링크하는 이미지만 남기고 나머지는 `raw/`로 옮긴다".into()
        }
        "QA-002" => "필요한 영역만 잘라 내거나 해상도를 낮춘다".into(),
        "LINT-000" => "`.fullops/config/lint.json`의 `commands`에 기존 도구를 연결한다".into(),
        "LINT-001" => "병합 뒤 적용되니 변경 이유를 검토한다".into(),
        "LINT-002" => "개발 중 필요 없으면 `level`이나 `paths`로 범위를 좁힌다".into(),
        _ => String::new(),
    }
}

/// 명령을 건너뛴 사유(설계 8.11절 6·7번).
pub const SKIP_PATHS: &str = "바뀐 파일이 명령의 paths와 맞지 않음";
pub const SKIP_NO_CODE: &str = "코드 파일 변경 없음";
pub const SKIP_EARLIER: &str = "앞 단계 명령이 실패해 실행하지 않음";
