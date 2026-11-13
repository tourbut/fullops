//! 사용자에게 보이는 문구. 한국어이고 이 파일 한 곳에 둔다(설계 7.2절).
//! 묶음마다 자기 규칙 ID 구역만 고친다(설계 18.3절).

/// 규칙의 사유 문구를 가리키는 키. 규칙 ID와 같다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsgId(pub &'static str);

/// 규칙에 걸린 사유. `args`는 `Finding::Hit`의 `args`다.
pub fn reason(id: MsgId, args: &[String]) -> String {
    match id.0 {
        // ── SessionStart (묶음 B) ─────────────────────────────
        // S-00 정책 오류 안내
        // S-01 세션 안내(설계 7.7절)
        // S-05 compact 뒤 한 줄

        // ── UserPromptSubmit (묶음 B) ─────────────────────────
        // P-01 dev 모드의 역할 안내
        // P-03 정책 오류 경고

        // ── PreToolUse (묶음 B) ───────────────────────────────
        // T-00 정책 오류
        // T-01 날것 worker-start
        // T-02 터미널 주입
        // T-03 긴 terminal send
        // T-04 분류 없는 task-create
        // T-05 인박스 규칙 (a)~(d)
        // T-06 지시서 작성 권한
        // T-07 설계 역할의 코드 수정
        // T-08 하위 위임 레벨 (e)~(g)
        // T-09 링크 없는 QA 미디어 커밋
        // T-10 차수 폴더

        // ── Stop (묶음 B) ─────────────────────────────────────
        // X-00 정책 오류
        // X-01 lint 통과 기록 없음
        // X-02 dev 직접 작업의 리뷰
        // X-03 미정산 dispatch
        // X-04 분류 후 미배정
        // X-05 Run 대기 (a)·(b)
        // X-06 통합 대기
        // X-07 준비된 행

        // ── StopFailure (묶음 B) ──────────────────────────────
        // F-01 한도 escalation의 제목과 본문

        // ── route (묶음 C): R-01~R-05, 설계 8.3절 ─────────────
        // ── task (묶음 C): H-01~H-04, 설계 8.4절 ──────────────
        // ── dispatch (묶음 C): D-01~D-11, 설계 8.5절 ──────────
        // ── done (묶음 C): R-01~R-09, 설계 8.7절 ──────────────
        // ── merge·통합 (묶음 E): M-01~M-13, 설계 8.8절 ────────
        // ── review·decision (묶음 E), 설계 8.9·8.10절 ─────────
        // ── lint·doc (묶음 D), 설계 8.11·8.12절 ───────────────
        // ── 설치·이행 (묶음 F), 설계 10·13절 ──────────────────
        _ => args.join(" "),
    }
}

/// 거부·거절 문구(설계 7.2·8.1절). `FullOps[<규칙>]: <사유> 다음: <탈출구>`
pub fn refusal(rule: &str, reason: &str, escape: &str) -> String {
    if escape.is_empty() {
        format!("FullOps[{rule}]: {reason}")
    } else {
        format!("FullOps[{rule}]: {reason} 다음: {escape}")
    }
}

/// Stop 차단 문구(설계 7.2절). 함께 걸린 규칙을 뒤에 붙인다.
pub fn block(rule: &str, reason: &str, escape: &str, also: &[&str]) -> String {
    let head = refusal(rule, reason, escape);
    if also.is_empty() {
        head
    } else {
        format!("{head} 함께 남은 것: {}", also.join(" / "))
    }
}

/// 경고 문구(설계 7.2절).
pub fn warn(text: &str) -> String {
    format!("FullOps: {text}")
}

pub const INACTIVE_REPO: &str = "활성 레포가 아닙니다";
