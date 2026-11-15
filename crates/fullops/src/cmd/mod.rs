//! 명령 표(설계 8.2절). 활성 레포와 정책 오류 검사는 `cli::run`이 이 표를 보고 한 곳에서 한다.

use crate::cli::{CliError, Ctx};

mod context;
mod decision;
mod dispatch;
mod doc;
mod doctor;
mod done;
mod env;
mod find;
mod gc;
mod hold;
mod init;
mod install;
mod lint;
mod merge;
mod migrate;
mod mode;
pub mod packet;
mod plan;
mod review;
mod route;
mod rules;
mod status;
mod sync;
mod task;
mod upgrade;
mod wait;
mod why;

pub struct Command {
    pub name: &'static str,
    pub about: &'static str,
    pub run: fn(&Ctx, lexopt::Parser) -> Result<u8, CliError>,
    /// 활성 표식이 없으면 종료 코드 2로 끝낸다(설계 7.8절).
    pub needs_repo: bool,
    /// 정책 오류에서도 받는 복구 명령이다(설계 7.5절 T-00의 통과 목록과 같다).
    pub recovery: bool,
}

const fn command(
    name: &'static str,
    about: &'static str,
    run: fn(&Ctx, lexopt::Parser) -> Result<u8, CliError>,
) -> Command {
    Command {
        name,
        about,
        run,
        needs_repo: true,
        recovery: false,
    }
}

const fn recovery(mut c: Command) -> Command {
    c.recovery = true;
    c
}

/// 활성 레포가 아니어도 도는 명령. 설정을 읽지 않으므로 정책 오류와도 무관하다.
const fn anywhere(mut c: Command) -> Command {
    c.needs_repo = false;
    c.recovery = true;
    c
}

pub static COMMANDS: &[Command] = &[
    command("route", "분류와 tuple 기록", route::run),
    command("find", "지시서에 맞는 코드·문서 후보 찾기(추천)", find::run),
    command("context", "후보 문서 분류(추천)", context::run),
    command(
        "packet",
        "탐색 패킷 만들기(build)와 검사(check)",
        packet::run,
    ),
    command(
        "task",
        "인박스 생성(new), 재오픈(reopen), 끊긴 worker 복구(recover)",
        task::run,
    ),
    command(
        "plan",
        "대기 목록 읽기와 편집(list, ready, check, add, set)",
        plan::run,
    ),
    command(
        "dispatch",
        "선행 조건 검사, 준비 커밋, 동기화, Orca 배정. worker가 부르면 하위 배정(--brief)",
        dispatch::run,
    ),
    command(
        "wait",
        "메시지 받기, 완료 보고 기록, 처리 표시(--ack), Orca가 깨울 수 없는 세션의 기다림",
        wait::run,
    ),
    command(
        "done",
        "완료 기록 보존, 검사, push, 보낼 본문 출력",
        done::run,
    ),
    command(
        "merge",
        "리뷰 판정, 병합, lint, push, 기록, 동기화",
        merge::run,
    ),
    command("hold", "통합 보류", hold::run),
    command("resume", "보류 재개", hold::run),
    command("settle", "통합 일괄 확인", hold::run),
    command("completion", "완료 보고의 SHA 지정(set-sha)", hold::run),
    command("sync", "예약된 워크트리 동기화", sync::run),
    command(
        "review",
        "리뷰(need, snapshot, check, waive, cleanup, supersede, prune)",
        review::run,
    ),
    command(
        "decision",
        "사용자 결정 기록(add, answer, drop, list)",
        decision::run,
    ),
    command("lint", "lint 실행과 통과 기록", lint::run),
    command(
        "doc",
        "산출물 문서 검사(check)와 front matter 갱신(stamp)",
        doc::run,
    ),
    command("env", "환경 파일 연결(link)", env::run),
    command("gc", "닫힌 기록 정리", gc::run),
    recovery(command("status", "요약", status::run)),
    recovery(command("why", "hook 추적과 이벤트별 실행 표식", why::run)),
    recovery(command("mode", "dev 모드 전환", mode::run)),
    recovery(command(
        "upgrade",
        "릴리스 노트와 필수 적용 검사",
        upgrade::run,
    )),
    anywhere(command("rules", "규칙 표", rules::run)),
    anywhere(command("init", "레포 초기화", init::run)),
    anywhere(command(
        "migrate",
        "Python판에서 전환과 되돌리기",
        migrate::run,
    )),
    anywhere(command(
        "install",
        "실행본 복사, 네 호스트의 플러그인 등록, 의존 플러그인·스킬 설치와 확인",
        install::run,
    )),
    anywhere(command("doctor", "설치와 레포 상태 점검", doctor::run)),
];

/// 실행본 단계(설계 10.3절). hook의 `SelfInstall` 효과(S-04의 자동 설치)가 `install --only runtime`과 같은 함수를 부른다.
pub use install::runtime_stage;
/// 유효한 `accepted.json`(설계 8.9절). X-02의 리뷰 증거가 `review check`·`merge`와 같은 판정을 쓴다.
pub use review::accepted;
