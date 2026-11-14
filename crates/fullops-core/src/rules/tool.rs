//! PreToolUse 규칙의 `check`(설계 7.5절). 위에서 아래로 평가하고 먼저 걸린 거부 하나를 낸다.

use super::decide::{HANDOVERS, QA_REPORTS};
use super::{Effect, Finding, call, hit, hit_case};
use crate::config::is_code;
use crate::facts::Facts;
use crate::frontmatter::{inbox_of, parse_front_matter};
use crate::model::{
    HookInput, Intent, Known, Parallel, RoleId, RouteClass, RunId, TaskKey, ToolKind, TurnKind,
};
use std::collections::BTreeMap;

/// 정책 오류에서도 받는 하위 명령(CLI의 복구 명령 목록과 같다).
const RECOVERY: [&str; 9] = [
    "doctor", "mode", "init", "migrate", "upgrade", "status", "why", "install", "rules",
];
const MEDIA: [&str; 8] = [
    ".png", ".jpg", ".jpeg", ".webp", ".gif", ".mp4", ".webm", ".mov",
];
pub(super) const CONFIG: &str = ".fullops-squad/fullops.toml";

/// `handovers/` 바로 아래의 `.md` 파일 이름. `_TEMPLATE.md`는 뺀다.
fn handover_name(rel: &str) -> Option<&str> {
    let name = rel.strip_prefix(HANDOVERS)?;
    (!name.contains('/') && name.ends_with(".md") && name != "_TEMPLATE.md").then_some(name)
}

/// `to_<역할 문법>.md`의 역할.
fn inbox_role(name: &str) -> Option<RoleId> {
    let role = name.strip_prefix("to_")?.strip_suffix(".md")?;
    RoleId::new(role).ok()
}

/// 소문자로 바꾼 글에서 첫 `purpose:`(또는 `purpose=`. 기호 앞뒤의 공백은 봐준다) 뒤의 글.
fn purpose_of(lower: &str) -> Option<&str> {
    lower.match_indices("purpose").find_map(|(at, word)| {
        let rest = lower.get(at + word.len()..)?.trim_start();
        Some(rest.strip_prefix([':', '='])?.trim_start())
    })
}

fn front_key(text: &str) -> Option<String> {
    let (fm, _) = parse_front_matter(text).ok()?;
    fm.get("key").map(str::to_string)
}

/// T-00
pub(super) fn t00(input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let (Some(error), Some(tool)) = (facts.policy_error(), &input.tool) else {
        return Finding::None;
    };
    // 모르는 도구는 통과다
    if tool.kind == ToolKind::Other {
        return Finding::None;
    }
    if let [Intent::Fullops { sub }] = call::intents().as_slice()
        && RECOVERY.contains(&sub.as_str())
    {
        return Finding::None;
    }
    let paths = call::target_paths(input, facts);
    if !paths.is_empty() && paths.iter().all(|p| p == CONFIG) {
        return Finding::None;
    }
    hit(error.kind.as_str(), vec![error.detail])
}

/// T-01
pub(super) fn t01(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    if call::intents().contains(&Intent::OrcaWorkerStart) {
        hit("", Vec::new())
    } else {
        Finding::None
    }
}

/// T-02
pub(super) fn t02(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    if call::intents().contains(&Intent::OrcaInject) {
        hit("", Vec::new())
    } else {
        Finding::None
    }
}

/// T-03
pub(super) fn t03(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    let long = call::intents().into_iter().find_map(|i| match i {
        Intent::OrcaTerminalSend { chars } if chars > 20 => Some(chars),
        _ => None,
    });
    match long {
        Some(chars) => hit("", vec![chars.to_string()]),
        None => Finding::None,
    }
}

/// T-04
pub(super) fn t04(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let Some(spec) = call::intents().into_iter().find_map(|i| match i {
        Intent::OrcaTaskCreate { spec } => Some(spec),
        _ => None,
    }) else {
        return Finding::None;
    };
    // `--spec-file`이나 풀지 못한 변수로 준 spec은 모른다
    let Known::Yes(spec) = spec else {
        return Finding::Unknown;
    };
    let key = spec.split_once("Task key:").and_then(|(_, rest)| {
        let rest = rest.trim_start();
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || "._-".contains(c)))
            .unwrap_or(rest.len());
        TaskKey::new(rest.get(..end)?).ok()
    });
    let Some(key) = key else {
        return hit("", vec!["spec에 `Task key: <KEY>`가 없다".into()]);
    };
    let route = match facts.route(&key) {
        Known::Yes(route) => route,
        Known::Unknown(_) => return Finding::Unknown,
    };
    let Some(route) = route else {
        return hit(
            key.as_str(),
            vec![format!(
                "과제 {key}의 분류 기록(`routes/{key}.json`)이 없다"
            )],
        );
    };
    let review = purpose_of(&spec.to_lowercase()).is_some_and(|p| p.starts_with("review"));
    if review && route.tuples.contains_key("reviewer") {
        return Finding::None;
    }
    if route.class == RouteClass::Unresolved {
        return hit(
            key.as_str(),
            vec![format!("과제 {key}가 아직 분류되지 않았다(unresolved)")],
        );
    }
    Finding::None
}

/// T-05
pub(super) fn t05(input: &HookInput, facts: &mut dyn Facts) -> Finding {
    for target in call::targets(input, facts) {
        let Some(name) = handover_name(&target.rel) else {
            continue;
        };
        // 바뀐 뒤 글을 모르면 통과다
        let Known::Yes(after) = &target.after else {
            continue;
        };
        let blank = after.as_deref().is_none_or(|text| text.trim().is_empty());
        let case = match (inbox_role(name), &target.before, after) {
            // 과제명 파일을 지우거나 비우는 정리는 통과다
            (None, _, _) if blank => continue,
            (None, _, _) => "a",
            (Some(_), None, None) => continue,
            (Some(_), None, Some(_)) => "b",
            (Some(_), Some(_), _) if blank => "c",
            (Some(_), Some(before), Some(after)) => {
                let key = front_key(after);
                if key.is_some() && key == front_key(before) {
                    continue;
                }
                "d"
            }
            (Some(_), Some(_), None) => "c",
        };
        return hit_case(
            format!("{}:{case}", target.rel),
            vec![target.rel.clone(), case.to_string()],
            case,
        );
    }
    Finding::None
}

/// T-06
pub(super) fn t06(input: &HookInput, facts: &mut dyn Facts) -> Finding {
    for target in call::targets(input, facts) {
        let inbox = handover_name(&target.rel)
            .and_then(inbox_role)
            .and(target.before.as_deref())
            .and_then(|text| parse_front_matter(text).ok())
            .and_then(|(fm, _)| inbox_of(&fm));
        if let Some(inbox) = inbox
            && matches!(inbox.route, RouteClass::Design | RouteClass::Product)
            && inbox.from != "lead"
        {
            return hit(
                target.rel.clone(),
                vec![target.rel, inbox.route.to_string(), inbox.from],
            );
        }
    }
    Finding::None
}

/// T-07
pub(super) fn t07(input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let Ok(code) = facts.config().map(|c| c.code.clone()) else {
        return Finding::Unknown;
    };
    let none = globset::GlobSet::empty();
    for path in call::target_paths(input, facts) {
        if !path.starts_with(".fullops-squad/") && is_code(&path, &code, &none) {
            return hit(path.clone(), vec![path]);
        }
    }
    Finding::None
}

/// T-08
pub(super) fn t08(input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let Some(tool) = input.tool.as_ref().filter(|t| t.kind == ToolKind::SubAgent) else {
        return Finding::None;
    };
    let Ok(parallel) = facts.config().map(|c| c.parallel) else {
        return Finding::Unknown;
    };
    let case = match parallel {
        Parallel::On | Parallel::Ultra => return Finding::None,
        Parallel::Off => "e",
        Parallel::Read => {
            // 지시 글을 모르면(인자 이름이 확인되지 않은 호스트) 형식 검사는 통과다
            let Some(text) = tool.text.as_deref().map(str::to_lowercase) else {
                return Finding::Unknown;
            };
            let purpose = purpose_of(&text).and_then(|rest| {
                ["research", "review", "implementation"]
                    .into_iter()
                    .find(|p| rest.starts_with(p))
            });
            let read_only = ["read only", "read-only", "읽기 전용"]
                .iter()
                .any(|mark| text.contains(mark));
            match purpose {
                None => "f",
                Some("implementation") => "g",
                Some(_) if !read_only => "g",
                Some(_) => return Finding::None,
            }
        }
    };
    hit_case(case, vec![case.to_string()], case)
}

/// T-09
pub(super) fn t09(_input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let intents = call::intents();
    let Some(all) = intents.iter().find_map(|i| match i {
        Intent::GitCommit { all } => Some(*all),
        _ => None,
    }) else {
        return Finding::None;
    };
    let unstaged = all || intents.contains(&Intent::GitAdd);
    let Known::Yes(paths) = facts.qa_commit_candidates(unstaged) else {
        return Finding::Unknown;
    };
    let root = call::root();
    let mut reports: BTreeMap<String, Known<Option<String>>> = BTreeMap::new();
    let mut unlinked = Vec::new();
    for path in &paths {
        let lower = path.to_lowercase();
        let media = MEDIA.iter().any(|ext| lower.ends_with(ext)) && !path.contains("/raw/");
        let Some((folder, inner)) = path
            .strip_prefix(QA_REPORTS)
            .filter(|_| media)
            .and_then(|rest| rest.split_once('/'))
        else {
            continue;
        };
        let report = reports.entry(folder.to_string()).or_insert_with(|| {
            facts.file_text(&root.join(QA_REPORTS).join(folder).join("report.md"))
        });
        match report {
            Known::Yes(Some(text)) if text.contains(inner) => {}
            Known::Yes(_) => unlinked.push(path.clone()),
            // 보고서를 읽지 못하면 그 파일은 판정하지 않는다
            Known::Unknown(_) => {}
        }
    }
    if unlinked.is_empty() {
        return Finding::None;
    }
    unlinked.truncate(5);
    hit("", vec![unlinked.join(", ")])
}

/// 차수 접미사: `FIX`·`RECHECK`·`RETEST`·`LOWS`·`R<숫자>`(대소문자 무시).
fn is_round(segment: &str) -> bool {
    let upper = segment.to_ascii_uppercase();
    let numbered = upper
        .strip_prefix('R')
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    numbered || matches!(upper.as_str(), "FIX" | "RECHECK" | "RETEST" | "LOWS")
}

/// T-10
pub(super) fn t10(input: &HookInput, facts: &mut dyn Facts) -> Finding {
    let qa = call::root().join(QA_REPORTS);
    for path in call::target_paths(input, facts) {
        let Some((folder, _)) = path
            .strip_prefix(QA_REPORTS)
            .and_then(|rest| rest.split_once('/'))
        else {
            continue;
        };
        if facts.path_exists(&qa.join(folder)) != Known::Yes(false) {
            continue;
        }
        let key = folder
            .strip_suffix("-test")
            .or(folder.strip_suffix("-review"))
            .unwrap_or(folder);
        let parts: Vec<&str> = key.split('-').collect();
        let Some(at) = (1..parts.len()).find(|i| parts.get(*i).is_some_and(|p| is_round(p))) else {
            continue;
        };
        let base = parts.get(..at).unwrap_or_default().join("-");
        for existing in [
            base.clone(),
            format!("{base}-test"),
            format!("{base}-review"),
        ] {
            if facts.path_exists(&qa.join(&existing)) == Known::Yes(true) {
                return hit(folder, vec![folder.to_string(), existing]);
            }
        }
    }
    Finding::None
}

/// T-11
pub(super) fn t11(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    let Some(run) = call::intents().into_iter().find_map(|i| match i {
        Intent::OrcaCheck { peek: false, run } => Some(run),
        _ => None,
    }) else {
        return Finding::None;
    };
    call::effect(Effect::PeekCompletions {
        run: run.and_then(|r| RunId::new(r).ok()),
    });
    hit("", Vec::new())
}

/// T-12
pub(super) fn t12(_input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    let fullops = |i: &Intent| matches!(i, Intent::Fullops { .. });
    if !call::intents().iter().any(fullops) {
        return Finding::None;
    }
    call::effect(Effect::StampCurrent);
    hit("", Vec::new())
}

/// T-13
pub(super) fn t13(input: &HookInput, _facts: &mut dyn Facts) -> Finding {
    let kind = match input.tool.as_ref().map(|t| t.kind) {
        Some(ToolKind::SubAgent) => TurnKind::Subagent,
        Some(ToolKind::Workflow) => TurnKind::Workflow,
        _ => return Finding::None,
    };
    call::effect(Effect::RecordTurn(kind));
    hit("", Vec::new())
}
