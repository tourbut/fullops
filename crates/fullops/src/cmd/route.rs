//! `fullops route`: 분류와 tuple 기록(설계 8.3절). 분류는 추천이고 실패하면 폴백이다.

use super::dispatch::{usage, value};
use super::find::{path_refusal, repo_path};
use crate::cli::{CliError, Ctx, EXIT_OK};
use fullops_core::flow::refusal;
use fullops_core::model::{
    Config, DecidedBy, Known, Origin, RoleId, RoleKind, Route, RouteClass, RouteTuple, Routing,
    TaskKey, path_text, sha256_hex,
};
use fullops_ext::{Answer, JevCurl, Judge, Question, QuestionKind};
use lexopt::prelude::*;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// 임계값은 1.24.0과 같게 둔다(결과 비교 때문).
const SCOPE: f64 = 0.8;
const ROLE: f64 = 0.6;
const DELIVERABLE: f64 = 0.5;
const MAX_DELIVERABLES: usize = 3;
const MAX_REQUEST_CHARS: usize = 4000;

/// T-10의 차수 접미사(`-FIX`·`-RECHECK`·`-RETEST`·`-LOWS`·`-R<숫자>`, 뒤에 `-<숫자>` 허용)를 뗀 키.
fn rework_base(key: &str) -> Option<&str> {
    let upper = key.to_ascii_uppercase();
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    upper.match_indices('-').find_map(|(at, _)| {
        let rest = upper.get(at + 1..)?;
        let (word, tail) = rest.split_once('-').unwrap_or((rest, ""));
        let suffix = matches!(word, "FIX" | "RECHECK" | "RETEST" | "LOWS")
            || word.strip_prefix('R').is_some_and(digits);
        (suffix && at > 0 && (tail.is_empty() || digits(tail))).then(|| key.get(..at))?
    })
}

struct Classified {
    class: RouteClass,
    role: Option<RoleId>,
    deliverables: Vec<String>,
    decided_by: DecidedBy,
    probabilities: BTreeMap<String, f64>,
}

/// Jev에게 범위·담당 역할·산출물을 묻고 임계값으로 정한다. 어떤 실패든 폴백이다(설계 8.3절 6번).
fn classify(
    ctx: &Ctx,
    config: &Config,
    request: &str,
    env_file: Option<PathBuf>,
) -> Result<Classified, CliError> {
    let repo = ctx.repo()?;
    let split = config.routing == Routing::Split;
    let design = config.design_role().cloned();
    let fallback = |why: String| {
        ctx.note(&format!("경고: Jev 분류 없이 폴백으로 분류했다({why})"));
        let (class, role) = match (&design, split) {
            (Some(role), false) => (RouteClass::Design, Some(role.clone())),
            _ => (RouteClass::Unresolved, None),
        };
        Classified {
            class,
            role,
            deliverables: Vec::new(),
            decided_by: DecidedBy::Fallback,
            probabilities: BTreeMap::new(),
        }
    };
    let workers: Vec<&RoleId> = config
        .roles
        .iter()
        .filter(|(_, r)| matches!(r.kind, RoleKind::Worker | RoleKind::Tester))
        .map(|(id, _)| id)
        .collect();
    if workers.is_empty() {
        return Ok(fallback("worker 역할이 없다".into()));
    }
    let index = repo
        .squad()
        .join("docs")
        .join("deliverables")
        .join("README.md");
    let docs = fullops_core::parse_deliverable_index(
        &fullops_store::read_text(&index)
            .yes()
            .flatten()
            .unwrap_or_default(),
    );
    let (first, second, ask) = if split {
        (
            "implementation",
            "product",
            "Does `request` need a product-rule or scope decision (`product`), or can its owner plan and implement it under agreed requirements (`implementation`)? Technical complexity alone is implementation.",
        )
    } else {
        (
            "simple",
            "design",
            "Can a worker do `request` directly without a design step? `simple`: files, acceptance and checks are clear and it stays inside one role. `design`: it needs design decisions, changes a shared contract, spans several roles, or is ambiguous.",
        )
    };
    let mut questions = vec![
        Question {
            id: "scope".into(),
            kind: QuestionKind::Choice {
                options: vec![first.into(), second.into()],
            },
            instructions: ask.into(),
        },
        Question {
            id: "role".into(),
            kind: QuestionKind::Choice {
                options: workers.iter().map(|r| r.to_string()).collect(),
            },
            instructions: "Which role described in `roles` owns most of the work in `request`?"
                .into(),
        },
    ];
    questions.extend(docs.iter().map(|doc| Question {
        id: format!("doc_{}", doc.id),
        kind: QuestionKind::Noul,
        instructions: format!(
            "Must the project deliverable `deliverables.{}` be written or updated because of `request`?",
            doc.id
        ),
    }));
    let roles: BTreeMap<String, String> = workers
        .iter()
        .map(|id| {
            let about = config.roles.get(*id).and_then(|r| r.about.clone());
            (id.to_string(), about.unwrap_or_else(|| id.to_string()))
        })
        .collect();
    let deliverables: BTreeMap<&str, String> = docs
        .iter()
        .map(|d| (d.id.as_str(), format!("{} — {}", d.title, d.summary)))
        .collect();
    let state = json!({"request": request, "roles": roles, "deliverables": deliverables});
    let judged = match JevCurl::at(&repo.root, env_file).ask(&state.to_string(), &questions) {
        Ok(judged) => judged,
        Err(e) => return Ok(fallback(format!("{e:?}"))),
    };

    let (mut scope, mut role, mut picked) = (BTreeMap::new(), BTreeMap::new(), Vec::new());
    for answer in judged.answers {
        match answer {
            Answer::Choice {
                id, probabilities, ..
            } if id == "scope" => scope = probabilities,
            Answer::Choice {
                id, probabilities, ..
            } if id == "role" => role = probabilities,
            Answer::Noul { id, value } if value >= DELIVERABLE => {
                picked.push((value, id.trim_start_matches("doc_").to_string()));
            }
            _ => {}
        }
    }
    let p = |name: &str| scope.get(name).copied().unwrap_or(0.0);
    let owner = role
        .iter()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .and_then(|(id, p)| Some((RoleId::new(id.as_str()).ok()?, *p)));
    let direct = p(first) >= SCOPE && owner.as_ref().is_some_and(|(_, p)| *p >= ROLE);
    let (class, to) = match (direct, split, design) {
        (true, _, _) => (RouteClass::Direct, owner.as_ref().map(|(id, _)| id.clone())),
        (false, false, Some(design)) => (RouteClass::Design, Some(design)),
        (false, true, Some(design)) if p(second) >= SCOPE => (RouteClass::Product, Some(design)),
        _ => (RouteClass::Unresolved, None),
    };
    picked.sort_by(|a, b| b.0.total_cmp(&a.0));
    Ok(Classified {
        class,
        role: to,
        deliverables: picked
            .into_iter()
            .take(MAX_DELIVERABLES)
            .map(|(_, id)| id)
            .collect(),
        decided_by: DecidedBy::Jev,
        probabilities: BTreeMap::from([
            ("scope".to_string(), p(first)),
            ("role".to_string(), owner.map_or(0.0, |(_, p)| p)),
        ]),
    })
}

fn show(ctx: &Ctx, route: &Route, note: &str) {
    let tuples: Vec<String> = route
        .tuples
        .iter()
        .map(|(slot, t)| format!("{slot}={}/{}/{}", t.agent, t.model, t.effort))
        .collect();
    let human = format!(
        "{note}{}: {} → {} ({}){}{}",
        route.key,
        route.class,
        route.role.as_ref().map_or("미정", RoleId::as_str),
        route.decided_by,
        if route.deliverables.is_empty() {
            String::new()
        } else {
            format!(" 산출물 {}", route.deliverables.join(","))
        },
        if tuples.is_empty() {
            String::new()
        } else {
            format!(" tuple {}", tuples.join(" "))
        },
    );
    ctx.print(&human, serde_json::to_value(route).unwrap_or_default());
}

pub fn run(ctx: &Ctx, mut args: lexopt::Parser) -> Result<u8, CliError> {
    let (mut key, mut request, mut request_file, mut class, mut role) =
        (None, None, None, None, None);
    let (mut reason, mut agent, mut model, mut effort, mut why) = (None, None, None, None, None);
    let (mut origin, mut deliverables) = (Origin::User, Vec::new());
    let (mut force, mut distinct, mut tuple_only) = (false, false, false);
    let mut env_file = None;
    while let Some(arg) = args.next()? {
        match arg {
            Value(v) if key.is_none() => key = Some(v.string()?),
            Long("request") => request = Some(value(&mut args)?),
            Long("request-file") => request_file = Some(value(&mut args)?),
            Long("class") => {
                let word = value(&mut args)?;
                class = Some(
                    RouteClass::parse(&word)
                        .filter(|c| *c != RouteClass::Untracked)
                        .ok_or_else(|| {
                            usage(format!(
                                "모르는 분류다: {word}. direct|design|product|unresolved"
                            ))
                        })?,
                );
            }
            Long("role") => role = Some(value(&mut args)?),
            Long("reason") => reason = Some(value(&mut args)?),
            Long("agent") => agent = Some(value(&mut args)?),
            Long("model") => model = Some(value(&mut args)?),
            Long("effort") => effort = Some(value(&mut args)?),
            Long("why") => why = Some(value(&mut args)?),
            Long("origin") => {
                let word = value(&mut args)?;
                origin = Origin::parse(&word)
                    .ok_or_else(|| usage(format!("모르는 origin이다: {word}. user|bot")))?;
            }
            Long("deliverable") => deliverables.push(value(&mut args)?),
            Long("force") => force = true,
            Long("distinct") => distinct = true,
            Long("tuple-only") => tuple_only = true,
            Long("env-file") => env_file = Some(PathBuf::from(value(&mut args)?)),
            other => return Err(other.unexpected().into()),
        }
    }
    let raw_key = key.ok_or_else(|| {
        usage("사용법: fullops route <KEY> (--request-file <경로> | --request \"<글>\") …")
    })?;
    let (repo, config) = (ctx.repo()?, ctx.config()?);
    let caller = ctx.caller()?;
    let refuse = |rule: &'static str, args: &[&str], escape: &str| {
        Err(CliError::Refused(vec![refusal(rule, args, escape, 2)]))
    };
    if !caller.actor.is_lead() {
        return refuse("RT-01", &[], "lead 세션에서 실행한다");
    }
    let Ok(key) = TaskKey::new(raw_key.as_str()) else {
        return refuse(
            "RT-02",
            &["syntax", &raw_key],
            "영문자·숫자로 시작하고 `.`·`_`·`-`만 쓰는 64자 이하의 키를 준다",
        );
    };
    let tuple = match (agent, model, effort) {
        (Some(agent), Some(model), Some(effort)) => Some(RouteTuple {
            agent,
            model,
            effort,
            why: why.unwrap_or_default(),
        }),
        (None, None, None) if !tuple_only => None,
        _ => {
            return refuse(
                "RT-05",
                &[],
                "`--agent <CLI> --model <모델> --effort <effort>`",
            );
        }
    };
    // 읽지 못한 기록을 '없음'으로 보면 tuple·requires_packet·이력을 잃고 덮어쓴다
    let Known::Yes(existing) = fullops_store::read_route(repo, &key) else {
        return Err(usage(format!(
            "`.fullops-squad/routes/{key}.json`을 읽을 수 없다. JSON을 고치거나 다른 이름으로 옮긴 뒤 다시 분류한다."
        )));
    };
    let blank = |class, role: Option<RoleId>, decided_by| Route {
        v: 1,
        key: key.clone(),
        class,
        role,
        deliverables: Vec::new(),
        origin,
        decided_by,
        reason: String::new(),
        request_file: None,
        request_sha256: None,
        probabilities: BTreeMap::new(),
        thresholds: BTreeMap::new(),
        tuples: BTreeMap::new(),
        requires_packet: false,
        created_at: fullops_store::now(),
        history: Vec::new(),
    };

    if tuple_only {
        // 호출 없이 tuple만 쓴다. 기록이 없으면 untracked로 만든다
        let slot =
            role.ok_or_else(|| usage("`--tuple-only`에는 `--role <역할|reviewer>`가 필요하다."))?;
        let mut route =
            existing.unwrap_or_else(|| blank(RouteClass::Untracked, None, DecidedBy::Manual));
        if let Some(tuple) = tuple {
            route.tuples.insert(slot, tuple);
        }
        fullops_store::write_route(repo, &route)?;
        show(ctx, &route, "tuple 기록 ");
        return Ok(EXIT_OK);
    }

    if let (false, Some(base)) = (distinct, rework_base(key.as_str()))
        && let Ok(base) = TaskKey::new(base)
        && repo.route_path(&base).is_file()
    {
        // 수정은 lead가 연 구현 역할의 다음 차수다: 그 키를 보고한 구현 역할(tester·설계 역할이 아니다), 없으면 구현 역할인 route 역할
        let state = fullops_store::read_state(repo).yes().unwrap_or_default();
        let base_route = fullops_store::read_route(repo, &base).yes().flatten();
        let implementer = |role: &&RoleId| {
            config
                .roles
                .get(*role)
                .is_some_and(|r| r.kind == RoleKind::Worker)
        };
        let role = state
            .completions
            .iter()
            .filter(|c| c.key.as_ref() == Some(&base))
            .filter_map(|c| {
                c.role
                    .as_ref()
                    .filter(implementer)
                    .map(|r| (c.received_at, r))
            })
            .max_by_key(|(at, _)| *at)
            .map(|(_, r)| r)
            // route 역할로 내려갈 때는 설계 역할만 뺀다(tester로 분류된 direct 과제는 그 역할이 고친다)
            .or(base_route
                .as_ref()
                .and_then(|r| r.role.as_ref())
                .filter(|role| {
                    config
                        .roles
                        .get(*role)
                        .is_some_and(|r| r.kind != RoleKind::Design)
                }));
        let mut fix =
            fullops_core::messages::flow::fix_round(base.as_str(), role.map(|r| r.as_str()), None);
        let brief = super::task::pending_brief(config, &state, &base);
        // 리뷰가 반려한 설계 역할의 보고는 병합할 수 없다: 그 설계 역할에 다시 맡긴다
        let rejected = brief.filter(|c| {
            c.sha
                .as_ref()
                .is_some_and(|sha| super::review::rejected(repo, &base, sha))
        });
        if let Some(brief) = rejected {
            let class = super::review::design_class(repo, config, &base, brief.role.as_ref());
            fix = fullops_core::messages::flow::fix_round(
                base.as_str(),
                brief.role.as_ref().map(|r| r.as_str()),
                class.as_deref(),
            );
        } else if let Some(brief) = brief {
            // 설계 역할의 지시서 보고가 병합 전이면 그것을 먼저 병합한다(reopen이 이전 차수 기록을 가져오기 전에)
            fix = format!(
                "먼저 설계 역할의 지시서 보고를 병합(`fullops merge {base} --completion {}`)한 뒤 수정 배정({fix})",
                brief.id
            );
        }
        // 요청이 바뀌어 설계 자체를 다시 써야 할 때만 설계 역할에 다시 맡긴다
        if let (None, Some(design), Some(r)) = (rejected, config.design_role(), &base_route)
            && matches!(r.class, RouteClass::Design | RouteClass::Product)
        {
            fix.push_str(&format!(
                ". 요청이 바뀌어 설계를 다시 써야 하면 `fullops route {base} --force --request \"<바뀐 요청>\" --class {} --role {design}` → `fullops dispatch {base} --role {design}`",
                r.class
            ));
        }
        return refuse(
            "RT-02",
            &["rework", key.as_str(), base.as_str()],
            &format!("{fix}. 정말 다른 과제면 `--distinct`"),
        );
    }
    // 요청 파일의 글은 Jev로 나간다. 탐색과 같은 경로 규칙을 거친 레포 안의 파일만 읽는다
    let request_file = match (&request, request_file) {
        (None, Some(file)) => {
            let refused = |why: &str| {
                usage(format!(
                    "요청 파일로 쓸 수 없다: {file} ({why}). 레포 안의 일반 파일을 주거나 `--request \"<글>\"`로 준다."
                ))
            };
            let rel = repo_path(repo, &file).map_err(|_| refused("outside repo"))?;
            match path_refusal(repo, &rel) {
                Some(why) => return Err(refused(why)),
                None => Some(rel),
            }
        }
        (_, file) => file,
    };
    let text = match (&request, &request_file) {
        (Some(text), _) => text.clone(),
        (None, Some(file)) => std::fs::read_to_string(repo.root.join(file))
            .map_err(|e| usage(format!("요청 파일을 읽지 못했다: {file}: {e}")))?,
        (None, None) => {
            return Err(usage(
                "`--request-file <경로>`나 `--request \"<글>\"`이 필요하다.",
            ));
        }
    };
    let chars = text.chars().count();
    if chars > MAX_REQUEST_CHARS {
        return refuse(
            "RT-03",
            &[&chars.to_string()],
            "요약한 요청으로 다시 실행한다",
        );
    }
    if let (Some(found), false) = (&existing, force) {
        show(ctx, found, "기존 기록 ");
        return Ok(EXIT_OK);
    }
    let role = role
        .map(|r| {
            RoleId::new(r.as_str())
                .ok()
                .filter(|id| config.roles.contains_key(id))
                .ok_or_else(|| usage(format!("등록된 역할이 아니다: {r}")))
        })
        .transpose()?;

    let decided = match class {
        Some(class) => {
            if class != RouteClass::Unresolved && role.is_none() {
                return refuse("RT-04", &["role"], "`--role <역할>`");
            }
            Classified {
                class,
                role: role.clone().filter(|_| class != RouteClass::Unresolved),
                deliverables: Vec::new(),
                decided_by: DecidedBy::Manual,
                probabilities: BTreeMap::new(),
            }
        }
        None => classify(ctx, config, &text, env_file)?,
    };
    if let (Some(old), Some(new), None) = (
        existing.as_ref().and_then(|r| r.role.as_ref()),
        &decided.role,
        &reason,
    ) && old != new
        && decided.decided_by == DecidedBy::Manual
    {
        return refuse(
            "RT-04",
            &["change", old.as_str(), new.as_str()],
            "`--reason \"<근거>\"`",
        );
    }

    let mut route = blank(decided.class, decided.role, decided.decided_by);
    route.deliverables = decided.deliverables;
    for id in deliverables {
        if !route.deliverables.contains(&id) {
            route.deliverables.push(id);
        }
    }
    route.reason = reason.unwrap_or_default();
    // `--request "<글>"`도 기록 옆에 남긴다: 인박스 없이 받는 설계 역할은 이 파일로 할 일을 안다(준비 커밋에 들어간다)
    route.request_file = match (&request, request_file) {
        (Some(text), _) if fullops_core::explore::sendable_text(text) => {
            let file = format!(".fullops-squad/routes/{key}.request.md");
            fullops_store::write_atomic(&repo.root.join(&file), text.as_bytes())?;
            Some(file)
        }
        // 비밀값 무늬가 있는 글은 커밋되는 자리에 쓰지 않는다. 워크트리들이 함께 보는 Git 공용 디렉터리에 둔다
        (Some(text), _) => {
            let path = repo.shared().join("requests").join(format!("{key}.md"));
            fullops_store::write_atomic(&path, text.as_bytes())?;
            ctx.note("요청 글에 비밀값 무늬가 있어 커밋되지 않는 자리에 남겼다. 설계 역할에는 그 경로가 간다.");
            Some(path_text(&path))
        }
        (None, file) => file,
    };
    route.request_sha256 = Some(sha256_hex(text.as_bytes()));
    route.probabilities = decided.probabilities;
    route.thresholds = BTreeMap::from([
        ("scope".to_string(), SCOPE),
        ("role".to_string(), ROLE),
        ("deliverable".to_string(), DELIVERABLE),
    ]);
    match existing {
        // 재분류: tuple 기록과 requires_packet은 잇고, 이전 객체는 history 끝에 넣는다
        Some(mut old) => {
            route.tuples = old.tuples.clone();
            route.requires_packet = old.requires_packet;
            route.history = std::mem::take(&mut old.history);
            route.history.push(old);
        }
        // 새 기록: 같은 키의 1.24.0 기록에서 켜져 있던 값만 옮긴다
        None => {
            let legacy = repo.explore_dir().join(format!("{key}-route.json"));
            let old: Option<serde_json::Value> = fullops_store::read_json(&legacy).yes().flatten();
            route.requires_packet = old.is_some_and(|v| v["requires_packet"] == true);
        }
    }
    match (tuple, &route.role) {
        (Some(tuple), Some(role)) => {
            route.tuples.insert(role.to_string(), tuple);
        }
        (Some(_), None) => ctx.note("역할이 정해지지 않아 tuple을 기록하지 않았다. `fullops route <KEY> --tuple-only --role <역할> …`"),
        _ => {}
    }
    fullops_store::write_route(repo, &route)?;
    if let Some(sid) = &caller.sid {
        // 세션 파일이 없으면(hook이 돌지 않은 세션) 건너뛴다
        let _ = fullops_store::update_session(repo, sid, fullops_store::CLI_WAIT, |s| {
            s.add_routed(&key)
        });
    }
    show(ctx, &route, "분류 ");
    Ok(EXIT_OK)
}

#[cfg(test)]
mod tests {
    use super::rework_base;

    #[test]
    fn rework_suffix_follows_t10() {
        assert_eq!(rework_base("PAY-12-FIX"), Some("PAY-12"));
        assert_eq!(rework_base("PAY-12-r2"), Some("PAY-12"));
        assert_eq!(rework_base("PAY-12-RETEST-3"), Some("PAY-12"));
        assert_eq!(rework_base("PAY-12"), None);
        assert_eq!(rework_base("FIX-12"), None);
        assert_eq!(rework_base("PAY-FIXTURE"), None);
        assert_eq!(rework_base("PAY-R"), None);
    }
}
