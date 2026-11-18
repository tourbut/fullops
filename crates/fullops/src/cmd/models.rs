//! 모델 후보를 `orca-agents.md`의 `## 모델 후보` 줄에서 `fullops.toml`의 `[models]`로 옮긴다(설계 6.1.1절, 13.7절).
//! `migrate --models`(2.x 레포)와 `migrate --from-python`(1.24.0)이 함께 쓴다.

use super::init;
use crate::assets;
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::layout;
use fullops_core::model::{
    ModelCandidate, REVIEWER_SLOT, Refusal, RoleId, host_of_agent, path_text,
};
use fullops_ext::{Git, GitCli};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

const HEADING: &str = "## 모델 후보";
const COMMIT: &str = "fullops: 모델 후보를 fullops.toml로 옮김";

/// `## 모델 후보` 절에서 읽은 것.
#[derive(Default, Debug)]
pub(super) struct Parsed {
    pub models: BTreeMap<String, Vec<ModelCandidate>>,
    /// 읽지 못했거나 받지 않은 `- ` 줄과 그 사유.
    pub unread: Vec<String>,
}

/// 후보 줄 `` - `<역할>` `<agent>` `<model>` `<effort>`: <설명> ``을 읽는다. 코드 블록 안의 줄(형식 예시)은 읽지 않는다.
/// `known`은 받을 자리(등록 역할과 `reviewer`)다.
pub(super) fn parse(text: &str, known: &BTreeSet<String>) -> Parsed {
    let mut parsed = Parsed::default();
    let (mut inside, mut fenced) = (false, false);
    for line in text.lines() {
        let line = line.trim_end();
        if line.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if line.starts_with("## ") {
            inside = line == HEADING;
            continue;
        }
        let Some(item) = line.strip_prefix("- ").filter(|_| inside && !fenced) else {
            continue;
        };
        // 백틱으로 나누면 ["", 자리, " ", agent, " ", model, " ", effort, 나머지]
        let parts: Vec<&str> = item.splitn(9, '`').collect();
        let well_formed = parts.len() == 9
            && parts[0].is_empty()
            && [2, 4, 6].iter().all(|&i| parts[i].trim().is_empty());
        if !well_formed {
            parsed.unread.push(format!("{line} (형식)"));
            continue;
        }
        let (slot, agent, model, effort) = (parts[1], parts[3], parts[5], parts[7]);
        // 옛 문서는 agy로 적었다. 기록은 Orca 에이전트 ID다
        let agent = if agent == "agy" { "antigravity" } else { agent };
        let about = parts[8].trim().strip_prefix(':').map(str::trim);
        if !known.contains(slot) {
            parsed.unread.push(format!("{line} (등록된 역할이 아니다)"));
        } else if host_of_agent(agent).is_none() {
            parsed.unread.push(format!("{line} (모르는 agent)"));
        } else if model.trim().is_empty() || effort.trim().is_empty() {
            parsed
                .unread
                .push(format!("{line} (model·effort가 비었다)"));
        } else {
            parsed
                .models
                .entry(slot.to_string())
                .or_default()
                .push(ModelCandidate {
                    agent: agent.to_string(),
                    model: model.to_string(),
                    effort: effort.to_string(),
                    about: about.filter(|a| !a.is_empty()).map(str::to_string),
                });
        }
    }
    parsed
}

/// `[models]` 표의 글. 후보가 없으면 빈 글이다.
pub(super) fn toml_text(models: &BTreeMap<String, Vec<ModelCandidate>>) -> String {
    if models.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n[models]\n");
    for (slot, list) in models {
        // 역할 ID와 `reviewer`는 TOML의 맨 키로 쓸 수 있다. 그 밖은 따옴표로 감싼다
        let bare = !slot.is_empty()
            && slot
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        let key = if bare {
            slot.clone()
        } else {
            init::quoted(slot)
        };
        out.push_str(&format!("{key} = [\n"));
        for c in list {
            out.push_str(&format!(
                "  {{ agent = {}, model = {}, effort = {}",
                init::quoted(&c.agent),
                init::quoted(&c.model),
                init::quoted(&c.effort)
            ));
            if let Some(about) = &c.about {
                out.push_str(&format!(", about = {}", init::quoted(about)));
            }
            out.push_str(" },\n");
        }
        out.push_str("]\n");
    }
    out
}

/// `## 모델 후보` 절(제목부터 다음 `## ` 제목 앞까지)을 뺀 글.
pub(super) fn strip(text: &str) -> String {
    let mut out = String::new();
    let (mut skipping, mut fenced) = (false, false);
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end();
        if !fenced && bare.starts_with("## ") {
            skipping = bare == HEADING;
        }
        if bare.starts_with("```") {
            fenced = !fenced;
        }
        if !skipping {
            out.push_str(line);
        }
    }
    out
}

/// 견줄 꼴: front matter·운영 모드 블록·후보 절·배정표 행·`## 라우팅 기준`의 `- ` 줄(기계 값은 `fullops.toml`에 있다)을 빼고 빈 줄을 접는다.
fn normalized(text: &str) -> String {
    let lf = text.replace("\r\n", "\n");
    let mut body = assets::split_front_matter(&lf).1.trim_start_matches('\n');
    if let Some((_, rest)) = body
        .strip_prefix("<!-- fullops-mode:start -->")
        .and_then(|rest| rest.split_once("<!-- fullops-mode:end -->"))
    {
        body = rest;
    }
    let stripped = strip(body);
    let (mut routing, mut fenced) = (false, false);
    let mut lines = Vec::new();
    for line in stripped.lines().map(str::trim_end) {
        if line.starts_with("```") {
            fenced = !fenced;
        } else if !fenced && line.starts_with("## ") {
            routing = line == "## 라우팅 기준";
        }
        let derived = !fenced && (line.starts_with('|') || (routing && line.starts_with("- ")));
        if !derived && !(line.is_empty() && lines.last().is_none_or(|l: &&str| l.is_empty())) {
            lines.push(line);
        }
    }
    lines.join("\n").trim().to_string()
}

/// 후보 절을 뺀 뒤 남길 글. 나머지가 옛 기본 템플릿과 같으면 `None`(파일을 지운다).
pub(super) fn leftover(text: &str) -> Option<String> {
    let mine = normalized(text);
    let stock = assets::ORCA_AGENTS_OLD
        .iter()
        .any(|t| normalized(t) == mine);
    (!stock).then(|| strip(text))
}

/// 한 레포의 후보 옮김 계획.
pub(super) struct Plan {
    pub parsed: Parsed,
    /// `orca-agents.md`의 새 글. `None`이면 지운다.
    pub guide_after: Option<String>,
}

pub(super) fn plan(guide_text: &str, known: &BTreeSet<String>) -> Plan {
    Plan {
        parsed: parse(guide_text, known),
        guide_after: leftover(guide_text),
    }
}

/// 받을 자리: 등록 역할과 `reviewer`.
pub(super) fn slots<'a>(roles: impl Iterator<Item = &'a RoleId>) -> BTreeSet<String> {
    roles
        .map(|r| r.to_string())
        .chain([REVIEWER_SLOT.to_string()])
        .collect()
}

/// 보고서 줄.
pub(super) fn report(plan: &Plan, guide: &str) -> Vec<String> {
    let count: usize = plan.parsed.models.values().map(Vec::len).sum();
    let mut lines = vec![format!(
        "모델 후보 {count}개를 `[models]`로 옮긴다({}).",
        plan.parsed
            .models
            .iter()
            .map(|(slot, list)| format!("{slot} {}", list.len()))
            .collect::<Vec<_>>()
            .join(", ")
    )];
    lines.extend(
        plan.parsed
            .unread
            .iter()
            .map(|l| format!("옮기지 않은 후보 줄: {l}")),
    );
    lines.push(match &plan.guide_after {
        None => format!("`{guide}`는 나머지가 기본 글이라 지운다(git 기록에 남는다)."),
        Some(_) => format!(
            "`{guide}`에는 사람이 적은 글이 있어 후보 절만 빼고 남긴다. 필요 없으면 직접 정리한다."
        ),
    });
    lines
}

/// `fullops migrate --models [--dry-run]`: 2.x 레포의 `guide/orca-agents.md` 후보를 `fullops.toml`로 옮기고 커밋한다.
pub fn run(ctx: &Ctx, dry_run: bool) -> Result<u8, CliError> {
    let (repo, config) = (ctx.repo()?, ctx.config()?);
    let root = &repo.root;
    let (toml_rel, guide_rel) = (
        layout::config_toml().to_string(),
        layout::guide("orca-agents.md"),
    );
    let refuse = |why: String| {
        CliError::Refused(vec![Refusal {
            rule: "migrate",
            reason: why,
            escape: "고친 뒤 lead 체크아웃에서 `fullops migrate --models`".into(),
            exit: 2,
        }])
    };
    let guide_text = std::fs::read_to_string(root.join(&guide_rel)).unwrap_or_default();
    if !guide_text.lines().any(|l| l.trim_end() == HEADING) {
        ctx.print(
            &format!("`{guide_rel}`에 `{HEADING}` 절이 없다. 옮길 후보가 없다."),
            json!({ "moved": 0 }),
        );
        return Ok(EXIT_OK);
    }
    if !config.models.is_empty() {
        return Err(refuse(format!(
            "`{toml_rel}`에 이미 `[models]`가 있다. 남은 후보 줄은 손으로 옮기고 `{guide_rel}`의 `## 모델 후보` 절을 지운다."
        )));
    }
    let git = GitCli::cli(root.clone());
    for rel in [&toml_rel, &guide_rel] {
        if git.status(Some(rel)).yes().is_none_or(|c| !c.is_empty()) {
            return Err(refuse(format!(
                "`{rel}`에 커밋하지 않은 변경이 있다. 커밋하거나 되돌린 뒤 다시 실행한다."
            )));
        }
    }
    let plan = plan(&guide_text, &slots(config.roles.keys()));
    let toml_before = std::fs::read_to_string(root.join(&toml_rel))
        .map_err(|e| CliError::Internal(format!("{toml_rel}: {e}")))?;
    let mut toml_after = toml_before.clone();
    if !toml_after.ends_with('\n') {
        toml_after.push('\n');
    }
    toml_after.push_str(&toml_text(&plan.parsed.models));
    fullops_store::parse_config(&toml_after)
        .map_err(|e| refuse(format!("새 `[models]`가 검증을 통과하지 못했다: {e}")))?;
    let mut lines = report(&plan, &guide_rel);
    if dry_run {
        lines.insert(0, "모델 후보 옮김 계획(실행하지 않는다)".into());
        ctx.print(
            &lines.join("\n"),
            json!({ "dry_run": true, "report": lines }),
        );
        return Ok(EXIT_OK);
    }
    fullops_store::write_atomic(&root.join(&toml_rel), toml_after.as_bytes())?;
    match &plan.guide_after {
        Some(text) => fullops_store::write_atomic(&root.join(&guide_rel), text.as_bytes())?,
        None => std::fs::remove_file(root.join(&guide_rel))
            .map_err(|e| CliError::Internal(format!("{guide_rel}: {e}")))?,
    }
    let commit = git.commit_only(&[toml_rel.clone(), guide_rel.clone()], COMMIT)?;
    lines.push(match commit {
        Some(sha) => format!("커밋 {} \"{COMMIT}\".", &sha.as_str()[..12]),
        None => "바뀐 것이 없어 커밋하지 않았다.".into(),
    });
    ctx.print(
        &lines.join("\n"),
        json!({ "dry_run": false, "report": lines, "root": path_text(root) }),
    );
    Ok(EXIT_OK)
}

/// 시험에서 쓰는 레포 경로 없이 계획만 본다.
#[cfg(test)]
fn plan_of(text: &str, roles: &[&str]) -> Plan {
    let ids: Vec<RoleId> = roles.iter().map(|r| RoleId::new(*r).unwrap()).collect();
    plan(text, &slots(ids.iter()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_move_and_stock_guides_go() {
        // 옛 기본 템플릿 그대로면(배정표 행·라우팅 줄을 채웠어도) 지운다. 형식 예시 블록은 읽지 않는다
        for stock in assets::ORCA_AGENTS_OLD {
            let filled = stock
                .replace(
                    "## 모델 후보\n",
                    "## 모델 후보\n\n- `dev` `claude` `claude-sonnet-5-5` `medium`: 일반 구현\n- `dev` `agy` `gemini-3` `high`\n- `reviewer` `claude` `claude-opus-5-5` `high`: 리뷰\n- `ghost` `claude` `m` `low`: 없는 역할\n",
                )
                .replace(
                    "| coordinator / 병합 책임자",
                    "| dev | 구현 | dev 워크트리 | 모델 후보 | worker-start |\n| coordinator / 병합 책임자",
                );
            let plan = plan_of(&format!("---\ntitle: t\n---\n{filled}"), &["dev"]);
            assert!(plan.guide_after.is_none(), "stock");
            assert_eq!(plan.parsed.models["dev"].len(), 2);
            assert_eq!(plan.parsed.models["dev"][1].agent, "antigravity");
            assert_eq!(plan.parsed.models["dev"][1].about, None);
            assert_eq!(
                plan.parsed.models["reviewer"][0].about.as_deref(),
                Some("리뷰")
            );
            assert_eq!(plan.parsed.unread.len(), 1);
        }
        // 사람이 적은 절이 있으면 후보 절만 빼고 남긴다
        let custom = format!(
            "{}\n## 완료 통합\n\n사용자 지시로 바로 main에 통합한다.\n",
            assets::ORCA_AGENTS_OLD[1]
        );
        let plan = plan_of(&custom, &["dev"]);
        let kept = plan.guide_after.expect("custom");
        assert!(
            kept.contains("## 완료 통합")
                && !kept.lines().any(|l| l == HEADING)
                && kept.contains("## 라우팅 기준")
        );
        // 만든 표는 검증을 통과하는 TOML이다
        let models = plan_of(
            "## 모델 후보\n- `dev` `codex` `gpt-6-sol` `low`: \"따옴표\" 글\n",
            &["dev"],
        )
        .parsed
        .models;
        let text = format!(
            "schema = 2\napplied_version = \"2.2.0\"\nmode = \"coor\"\nrouting = \"legacy\"\ntest_level = \"lite\"\nparallel = \"on\"\n\n[git]\nbase = \"main\"\n\n[roles.dev]\nbranch = \"fullops/dev\"\nkind = \"worker\"\n{}",
            toml_text(&models)
        );
        let config = fullops_store::parse_config(&text).expect("valid");
        assert_eq!(
            config.candidates("dev")[0].about.as_deref(),
            Some("\"따옴표\" 글")
        );
    }
}
