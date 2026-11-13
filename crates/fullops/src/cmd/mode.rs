//! `fullops mode dev --primary-role R`: dev 모드 전환(설계 10.6절).
//! `fullops.toml`의 `mode`·`primary_role`·`primary_branch`만 원자 교체로 바꾼다. dev → coor는 지원하지 않는다.

use super::init;
use crate::cli::{CliError, Ctx, EXIT_OK};
use serde_json::json;

pub fn run(ctx: &Ctx, mut parser: lexopt::Parser) -> Result<u8, CliError> {
    use lexopt::prelude::*;
    let (mut mode, mut primary) = (None, None);
    while let Some(arg) = parser.next()? {
        match arg {
            Value(value) if mode.is_none() => mode = Some(value.string()?),
            Long("primary-role") => primary = Some(parser.value()?.string()?),
            other => return Err(other.unexpected().into()),
        }
    }
    match mode.as_deref() {
        Some("dev") => {}
        Some("coor") => {
            return Err(CliError::Usage(
                "dev에서 coor로 돌아가는 전환은 지원하지 않습니다.".into(),
            ));
        }
        _ => {
            return Err(CliError::Usage(
                "사용법: fullops mode dev --primary-role <역할>".into(),
            ));
        }
    }
    let primary =
        primary.ok_or_else(|| CliError::Usage("--primary-role <역할>이 필요합니다.".into()))?;
    let repo = ctx.repo()?;
    let path = repo.config_path();
    let before = std::fs::read_to_string(&path)?;
    let branch = init::current_branch(&repo.root).ok_or_else(|| {
        CliError::Usage(
            "브랜치를 체크아웃한 상태에서 실행합니다(detached HEAD가 아니어야 합니다).".into(),
        )
    })?;
    let mut text = init::set_top_key(&before, "mode", "dev");
    text = init::set_top_key(&text, "primary_role", &primary);
    text = init::set_top_key(&text, "primary_branch", &branch);
    // 주 담당이 등록 역할이 아니면 여기서 걸린다. 어긋난 설정은 쓰지 않는다
    fullops_store::parse_config(&text)
        .map_err(|e| CliError::Usage(format!("이 값으로는 전환할 수 없습니다: {e}")))?;
    fullops_store::write_atomic(&path, text.as_bytes())?;
    ctx.note("떠 있는 세션은 정책 오류가 된다. 새 세션을 연다. 변경을 커밋한다.");
    ctx.print(
        &format!("dev 모드로 바꿨다: 주 담당 {primary}, 브랜치 {branch}"),
        json!({ "mode": "dev", "primary_role": primary, "primary_branch": branch }),
    );
    Ok(EXIT_OK)
}
