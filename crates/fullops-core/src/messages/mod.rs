//! 사용자에게 보이는 문구. 한국어이고 이 모듈에 둔다(설계 7.2절, 10.1절).
//! 이 파일은 공통 조립 함수와 `parallel_brief`만 가진다(묶음 A). 규칙별 사유 문구는 묶음마다 자기 파일에 둔다(설계 18.2절).

pub mod explore;
pub mod flow;
pub mod hook;
pub mod install;
pub mod lint;
pub mod merge;

use crate::model::{BriefAt, HostId, Parallel, caps_of};

/// 규칙의 사유 문구를 가리키는 키. 값은 규칙 ID다(`MessageId`는 Orca 메시지 ID다).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsgId(pub &'static str);

/// 규칙에 걸린 사유. `args`는 `Finding::Hit`의 `args`(CLI 거절은 그 검사가 넘긴 값)다.
/// 묶음별 파일을 차례로 묻고, 어느 파일에도 문구가 없으면 `args`를 이어 낸다.
pub fn reason(id: MsgId, args: &[String]) -> String {
    hook::reason(id.0, args)
        .or_else(|| flow::reason(id.0, args))
        .or_else(|| merge::reason(id.0, args))
        .or_else(|| lint::reason(id.0, args))
        .or_else(|| explore::reason(id.0, args))
        .or_else(|| install::reason(id.0, args))
        .unwrap_or_else(|| args.join(" "))
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

/// 미구현 명령과 함수가 내는 문구. 뼈대의 빈 골격이 쓴다.
pub const UNIMPLEMENTED: &str = "아직 구현되지 않았습니다";

/// 병렬 방침의 안내 문장(설계 8.15절의 고정 문구). S-01과 `fullops status`의 첫머리(`Session`), dispatch의 spec(`Spec`)이 함께 쓴다.
/// `host`는 문구에 넣을 서브에이전트 도구 이름을 정하고, 모르면(`None`) "하위 에이전트 도구"라고 적는다.
/// `Spec`이고 하위 배정이 아니면 `on`·`ultra`의 문장 끝에 하위 배정 문장이 붙는다(하위 worker는 하위 배정을 하지 못한다).
pub fn parallel_brief(parallel: Parallel, host: Option<HostId>, at: BriefAt) -> String {
    let tool = host.map_or("하위 에이전트 도구", |h| caps_of(h).subagent_tool);
    let mut out = match parallel {
        Parallel::Off => {
            return "병렬 방침 off: 하위 에이전트와 하위 worker를 쓰지 않고 직접 처리한다.".into();
        }
        Parallel::Read => {
            return format!(
                "병렬 방침 read: 읽기 전용 조사·리뷰에만 하위 에이전트(`{tool}`)를 쓴다. 지시에 `Purpose: research` 또는 `Purpose: review`와 `read only`를 적는다."
            );
        }
        Parallel::On | Parallel::Ultra => format!(
            "병렬 방침 {parallel}: 이 과제는 병렬 진행을 요청한다. 서로 겹치지 않는 파일 묶음의 구현과 독립된 조사는 하위 에이전트(`{tool}`)로 동시에 진행한다(한 번에 4개 이하로 시작한다). 묶음마다 맡는 파일과 완료 조건을 적어 주고, 워크트리 격리는 쓰지 않으며, 결과는 직접 확인해 이 워크트리에서 커밋한다. 나눌 수 없는 일은 직접 한다. 리뷰와 테스트를 하위 에이전트로 더 돌리지 않는다. 사용량이 빨리 준다. 끄려면 사람이 `fullops.toml`의 `parallel`을 `off`로 바꾼다."
        ),
    };
    if parallel == Parallel::Ultra && host == Some(HostId::Claude) {
        out.push_str(ULTRA_HINT);
    }
    if let BriefAt::Spec {
        command,
        key,
        sub: false,
    } = at
    {
        out.push_str(&format!(
            " 다른 모델이 맡아야 하는 부분만 `{command} dispatch {key} --brief \"<맡길 파일 묶음과 완료 조건>\"`로 하위 worker에 맡기고, 보고를 받아 확인한 뒤 `{command} done`을 한다."
        ));
    }
    out
}

/// spec에 적는 fullops 실행 명령(설계 8.5절). `fixed_bin`은 고정 위치 실행본의 절대 경로(`path_text` 표기)다.
/// Claude Code만 플러그인의 `bin/`을 도구 셸의 PATH에 넣으므로 이름 그대로 적고, 그 밖은 전체 경로를 적는다.
/// 미정-M1(Windows의 Claude Code에서 맨 이름이 풀리는가)이 바꾸는 곳은 이 함수 하나다.
pub fn spec_command(host: Option<HostId>, fixed_bin: &str) -> String {
    match host {
        Some(HostId::Claude) => "fullops".into(),
        _ if fixed_bin.contains(char::is_whitespace) => format!("\"{fixed_bin}\""),
        _ => fixed_bin.into(),
    }
}

/// `ultra`이고 호스트가 `claude`일 때 붙는 문장.
pub const ULTRA_HINT: &str = " 울트라코드가 켜져 있으면 조각이 많은 일은 Workflow로 조율한다. 켜져 있지 않으면 `Agent` 도구를 쓴다. 울트라코드는 사용자가 켠다(`/effort ultracode`).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_brief_follows_design_table() {
        use BriefAt::Session;
        let key = crate::model::TaskKey::new("PAY-12").unwrap();
        let spec = |sub| BriefAt::Spec {
            command: "fullops",
            key: &key,
            sub,
        };
        let brief = parallel_brief;
        assert_eq!(
            brief(Parallel::Off, Some(HostId::Claude), spec(false)),
            "병렬 방침 off: 하위 에이전트와 하위 worker를 쓰지 않고 직접 처리한다."
        );
        assert!(brief(Parallel::Read, Some(HostId::Codex), Session).contains("(`spawn_agent`)"));
        assert!(brief(Parallel::Read, None, spec(false)).contains("(`하위 에이전트 도구`)"));
        let on = brief(Parallel::On, Some(HostId::Agy), Session);
        assert!(on.starts_with("병렬 방침 on: ") && on.contains("(`invoke_subagent`)"));
        assert!(on.ends_with("`off`로 바꾼다."));
        // ultra는 머리말만 다르고 claude에만 울트라코드 문장이 붙는다
        let ultra = brief(Parallel::Ultra, Some(HostId::Grok), Session);
        assert_eq!(
            ultra,
            brief(Parallel::On, Some(HostId::Grok), Session).replace("방침 on", "방침 ultra")
        );
        assert!(brief(Parallel::Ultra, Some(HostId::Claude), Session).ends_with(ULTRA_HINT));
        // spec의 on·ultra에만 하위 배정 문장이 붙고, 하위 배정의 spec에는 붙지 않는다. 명령과 키가 채워진다
        let session = brief(Parallel::Ultra, Some(HostId::Claude), Session);
        let top = brief(Parallel::Ultra, Some(HostId::Claude), spec(false));
        assert_eq!(
            top.strip_prefix(session.as_str()),
            Some(
                " 다른 모델이 맡아야 하는 부분만 `fullops dispatch PAY-12 --brief \"<맡길 파일 묶음과 완료 조건>\"`로 하위 worker에 맡기고, 보고를 받아 확인한 뒤 `fullops done`을 한다."
            )
        );
        assert_eq!(
            brief(Parallel::Ultra, Some(HostId::Claude), spec(true)),
            session
        );
        assert!(!brief(Parallel::Read, Some(HostId::Claude), spec(false)).contains("--brief"));
        // 실행 명령: claude만 이름 그대로, 그 밖은 고정 위치의 전체 경로(공백이 있으면 따옴표)
        let bin = "C:/Users/a b/.fullops/bin/fullops.exe";
        assert_eq!(spec_command(Some(HostId::Claude), bin), "fullops");
        assert_eq!(spec_command(Some(HostId::Codex), bin), format!("\"{bin}\""));
        assert_eq!(
            spec_command(None, "/home/a/.fullops/bin/fullops"),
            "/home/a/.fullops/bin/fullops"
        );
        let codex = BriefAt::Spec {
            command: "/home/a/.fullops/bin/fullops",
            key: &key,
            sub: false,
        };
        assert!(
            brief(Parallel::On, Some(HostId::Codex), codex)
                .contains("`/home/a/.fullops/bin/fullops dispatch PAY-12 --brief")
        );
    }

    #[test]
    fn refusal_and_block_shapes() {
        assert_eq!(
            refusal("D-03", "사유", "`fullops merge`"),
            "FullOps[D-03]: 사유 다음: `fullops merge`"
        );
        assert_eq!(refusal("X", "사유", ""), "FullOps[X]: 사유");
        assert!(
            block("X-06", "사유", "탈출구", &["X-07 준비된 행"])
                .ends_with("함께 남은 것: X-07 준비된 행")
        );
        assert_eq!(reason(MsgId("T-01"), &["a".into(), "b".into()]), "a b");
    }
}
