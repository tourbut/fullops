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

/// 후보 절에서 남긴 줄 위에 다는 제목. `upgrade`의 `models` 검사에 걸리지 않는 이름이다.
const KEPT_HEADING: &str = "## 옮기지 못한 모델 후보 글";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Outside,
    Heading,
    /// 후보 절 안. 참이면 코드 블록 안(울타리 줄 포함)이다.
    Inside(bool),
}

/// 줄마다 후보 절 안인지 본다. 절은 코드 블록 밖의 `## 모델 후보` 줄부터 코드 블록 밖의 다음 `# `·`## ` 제목 앞까지다.
/// `parse`·`strip`·`has_section`이 같은 판단을 쓴다.
fn scan(text: &str) -> Vec<(&str, Place)> {
    let (mut inside, mut fenced) = (false, false);
    let mut out = Vec::new();
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end();
        let heading = !fenced && (bare.starts_with("# ") || bare.starts_with("## "));
        if heading {
            inside = bare == HEADING;
        }
        let fence = bare.starts_with("```");
        let place = if heading && inside {
            Place::Heading
        } else if inside {
            Place::Inside(fenced || fence)
        } else {
            Place::Outside
        };
        if fence {
            fenced = !fenced;
        }
        out.push((line, place));
    }
    out
}

/// 코드 블록 밖에 `## 모델 후보` 절이 있는가.
pub(super) fn has_section(text: &str) -> bool {
    scan(text).iter().any(|(_, p)| *p == Place::Heading)
}

/// `## 모델 후보` 절에서 읽은 것.
#[derive(Default, Debug)]
pub(super) struct Parsed {
    pub models: BTreeMap<String, Vec<ModelCandidate>>,
    /// 읽지 못했거나 받지 않은 `- ` 줄과 그 사유. 이 줄들은 파일에 남는다.
    pub unread: Vec<String>,
    /// 옮긴 줄의 번호(`scan` 기준).
    moved: BTreeSet<usize>,
}

/// 후보 줄 `` - `<역할>` `<agent>` `<model>` `<effort>`: <설명> ``을 읽는다. 코드 블록 안의 줄(형식 예시)은 읽지 않는다.
/// `known`은 받을 자리(등록 역할과 `reviewer`)다.
pub(super) fn parse(text: &str, known: &BTreeSet<String>) -> Parsed {
    let mut parsed = Parsed::default();
    for (index, (line, place)) in scan(text).into_iter().enumerate() {
        let line = line.trim_end();
        let Some(item) = line
            .strip_prefix("- ")
            .filter(|_| place == Place::Inside(false))
        else {
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
            parsed.moved.insert(index);
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

/// 후보 절의 빈 줄 아닌 줄(제목과 `skip` 빼고) `(번호, 글)`.
fn section_lines<'a>(lines: &[(&'a str, Place)], skip: &BTreeSet<usize>) -> Vec<(usize, &'a str)> {
    lines
        .iter()
        .enumerate()
        .filter(|(i, (line, place))| {
            matches!(place, Place::Inside(_)) && !skip.contains(i) && !line.trim().is_empty()
        })
        .map(|(i, (line, _))| (i, line.trim_end()))
        .collect()
}

/// 후보 절을 뺀 글. 옮긴 줄을 빼고 남은 줄이 옛 템플릿의 설명·형식 예시 그대로면 절을 통째로 지운다.
/// 그 밖의 줄(사람이 적은 글, 옮기지 못한 후보 줄)이 하나라도 남으면 지우지 않고 `KEPT_HEADING` 아래에 그대로 둔다.
pub(super) fn strip(text: &str, parsed: &Parsed) -> String {
    let lines = scan(text);
    let left = section_lines(&lines, &parsed.moved);
    let left_text: Vec<&str> = left.iter().map(|(_, l)| *l).collect();
    let stock = assets::ORCA_AGENTS_OLD.iter().any(|t| {
        let lines = scan(t);
        let stock: Vec<&str> = section_lines(&lines, &BTreeSet::new())
            .into_iter()
            .map(|(_, l)| l)
            .collect();
        stock == left_text
    });
    let kept = !stock && !left.is_empty();
    let mut out = String::new();
    for (i, (line, place)) in lines.iter().enumerate() {
        match place {
            Place::Outside => out.push_str(line),
            Place::Heading if kept => {
                let end = if line.ends_with("\r\n") { "\r\n" } else { "\n" };
                out.push_str(&format!("{KEPT_HEADING}{end}"));
            }
            Place::Inside(_) if kept && !parsed.moved.contains(&i) => out.push_str(line),
            _ => {}
        }
    }
    out
}

/// 견줄 꼴: front matter와 운영 모드 블록을 빼고 빈 줄을 접는다. 배정표와 라우팅 줄은 사람이 적은 값이라 견준다.
fn normalized(text: &str) -> String {
    let lf = text.replace("\r\n", "\n");
    let mut body = assets::split_front_matter(&lf).1.trim_start_matches('\n');
    if let Some((_, rest)) = body
        .strip_prefix("<!-- fullops-mode:start -->")
        .and_then(|rest| rest.split_once("<!-- fullops-mode:end -->"))
    {
        body = rest;
    }
    let mut lines: Vec<&str> = Vec::new();
    for line in body.lines().map(str::trim_end) {
        if !(line.is_empty() && lines.last().is_none_or(|l| l.is_empty())) {
            lines.push(line);
        }
    }
    lines.join("\n").trim().to_string()
}

/// 후보 절을 뺀 뒤 남길 글. 나머지가 옛 기본 템플릿 그대로면 `None`(파일을 지운다).
pub(super) fn leftover(text: &str, parsed: &Parsed) -> Option<String> {
    let after = strip(text, parsed);
    let mine = normalized(&after);
    let stock = assets::ORCA_AGENTS_OLD
        .iter()
        .any(|t| normalized(&strip(t, &Parsed::default())) == mine);
    (!stock).then_some(after)
}

/// 한 레포의 후보 옮김 계획.
pub(super) struct Plan {
    pub parsed: Parsed,
    /// `orca-agents.md`의 새 글. `None`이면 지운다.
    pub guide_after: Option<String>,
}

pub(super) fn plan(guide_text: &str, known: &BTreeSet<String>) -> Plan {
    let parsed = parse(guide_text, known);
    Plan {
        guide_after: leftover(guide_text, &parsed),
        parsed,
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
        Some(text) if text.lines().any(|l| l.trim_end() == KEPT_HEADING) => format!(
            "`{guide}`에는 사람이 적은 글이 있어 남긴다. 후보 절에서 옮기지 못한 줄은 `{KEPT_HEADING}` 아래에 남긴다. 필요 없으면 직접 정리한다."
        ),
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
    // `[models]`는 lead 체크아웃에서 읽는다. 역할 브랜치에 커밋하면 기본 브랜치에 오지 않는다
    if !ctx.caller()?.actor.is_lead() {
        return Err(CliError::Refused(vec![Refusal {
            rule: "migrate",
            reason: "lead 체크아웃이 아니다.".into(),
            escape: "기준 브랜치를 체크아웃한 lead 체크아웃에서 `fullops migrate --models`".into(),
            exit: 2,
        }]));
    }
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
    if !has_section(&guide_text) {
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
mod tests {
    use super::*;

    fn plan_of(text: &str, roles: &[&str]) -> Plan {
        let ids: Vec<RoleId> = roles.iter().map(|r| RoleId::new(*r).unwrap()).collect();
        plan(text, &slots(ids.iter()))
    }

    const LINES: &str = "- `dev` `claude` `claude-sonnet-5-5` `medium`: 일반 구현\n- `dev` `agy` `gemini-3` `high`\n- `reviewer` `claude` `claude-opus-5-5` `high`: 리뷰\n";

    fn with_lines(stock: &str, lines: &str) -> String {
        stock.replace("## 모델 후보\n", &format!("## 모델 후보\n\n{lines}"))
    }

    #[test]
    fn candidates_move_and_only_stock_guides_go() {
        for stock in assets::ORCA_AGENTS_OLD {
            // 후보만 채운 기본 글은 지운다. front matter와 CRLF는 견주지 않고, 형식 예시 블록은 읽지 않는다
            let filled = format!("---\ntitle: t\n---\n{}", with_lines(stock, LINES));
            for text in [filled.clone(), filled.replace('\n', "\r\n")] {
                let plan = plan_of(&text, &["dev"]);
                assert!(plan.guide_after.is_none(), "stock");
                assert_eq!(plan.parsed.models["dev"].len(), 2);
                assert_eq!(plan.parsed.models["dev"][1].agent, "antigravity");
                assert_eq!(plan.parsed.models["dev"][1].about, None);
                assert_eq!(
                    plan.parsed.models["reviewer"][0].about.as_deref(),
                    Some("리뷰")
                );
            }

            // 배정표에 적은 고정 모델, 라우팅 기준에 더한 줄은 사람이 적은 글이다: 파일을 남긴다
            let row = with_lines(stock, LINES).replace(
                "| coordinator / 병합 책임자",
                "| dev | 구현 | dev 워크트리 | claude / claude-opus-5-5 / high | worker-start |\n| coordinator / 병합 책임자",
            );
            let kept = plan_of(&row, &["dev"]).guide_after.expect("row");
            assert!(kept.contains("claude / claude-opus-5-5 / high"));
            assert!(!has_section(&kept) && !kept.contains("`gemini-3`"));
            let routing = with_lines(stock, LINES).replace(
                "## 라우팅 기준\n",
                "## 라우팅 기준\n\n- 결제·인증 변경은 항상 design\n",
            );
            let kept = plan_of(&routing, &["dev"]).guide_after.expect("routing");
            assert!(kept.contains("- 결제·인증 변경은 항상 design"));

            // 옮기지 못한 후보 줄과 절 안의 사람 글은 지우지 않고 남긴다(옮긴 줄만 뺀다)
            let ghost = with_lines(
                stock,
                &format!("{LINES}- `qa` `claude` `m` `low`: 없는 역할\n메모: 비용 한도 주의\n"),
            );
            let plan = plan_of(&ghost, &["dev"]);
            assert_eq!(plan.parsed.unread.len(), 1);
            let kept = plan.guide_after.expect("ghost");
            assert!(kept.contains(KEPT_HEADING) && !has_section(&kept));
            assert!(
                kept.contains("- `qa` `claude` `m` `low`") && kept.contains("메모: 비용 한도 주의")
            );
            assert!(!kept.contains("`gemini-3`") && kept.contains("## 라우팅 기준"));
        }

        // 닫히지 않은 코드 블록이 있어도 절 뒤의 글을 잃지 않는다. 코드 블록 안의 제목은 절이 아니다
        let open = "# 배정\n\n## 모델 후보\n\n- `dev` `claude` `s` `medium`\n```text\n## 예시\n- `dev` `claude` `x` `low`\n\n## 완료 통합\n\n바로 통합한다.\n";
        let plan = plan_of(open, &["dev"]);
        assert_eq!(plan.parsed.models["dev"].len(), 1);
        let kept = plan.guide_after.expect("open fence");
        assert!(kept.contains("바로 통합한다.") && kept.contains("- `dev` `claude` `x` `low`"));
        assert!(!has_section("```text\n## 모델 후보\n```\n"));

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
