//! 세션이 만든 변경과 lint 통과 인정(설계 6.6절). Stop의 X-01·X-02(`facts_real`)와 `done`·`lint`가 같은 함수를 쓴다.

use crate::assets;
use fullops_core::derive::{is_change_target, is_record_path};
use fullops_core::model::{
    Actor, ChangeSet, CodeDef, Config, Known, Landing, LintPass, Mode, ReviewPolicy, RoleId, Seat,
    Sha, State, WorkOutcome,
};
use fullops_ext::{Git, GitCli};
use fullops_store::Repo;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const CONFIG_TOML: &str = ".fullops-squad/fullops.toml";
const LINT_JSON: &str = ".fullops-squad/lint/lint.json";

/// `Unknown`이면 그 사유로 돌려준다.
macro_rules! know {
    ($e:expr) => {
        match $e {
            Known::Yes(value) => value,
            Known::Unknown(why) => return Known::Unknown(why),
        }
    };
}

/// git 호출 창구. hook은 이벤트별 상한(설계 7.1절)을 주고 CLI는 상한 없이 쓴다. 상한을 넘는 요청은 `Unknown`이다.
pub struct GitGate {
    pub cli: GitCli,
    pub used: u32,
    pub cap: u32,
    /// 커밋 → "그 커밋부터 HEAD까지 기록 경로만 바뀌었는가". lint 통과와 리뷰 증거가 같은 답을 쓴다.
    records_since: BTreeMap<String, Known<bool>>,
}

impl GitGate {
    fn new(cli: GitCli, cap: u32) -> Self {
        Self {
            cli,
            used: 0,
            cap,
            records_since: BTreeMap::new(),
        }
    }

    /// hook용. 상한은 부르는 쪽이 이벤트에 맞춰 넣는다.
    pub fn hook(root: PathBuf) -> Self {
        Self::new(GitCli::hook(root), 0)
    }

    pub fn cli(root: PathBuf) -> Self {
        Self::new(GitCli::cli(root), u32::MAX)
    }

    pub fn ask<T>(&mut self, call: impl FnOnce(&GitCli) -> Known<T>) -> Known<T> {
        if self.used >= self.cap {
            return Known::Unknown("git_cap");
        }
        self.used += 1;
        call(&self.cli)
    }

    /// `from`이 `head`의 조상이고 그 사이에 기록 경로만 바뀌었는가.
    pub fn only_records_since(&mut self, from: &Sha, head: &Sha) -> Known<bool> {
        if let Some(known) = self.records_since.get(from.as_str()) {
            return *known;
        }
        let known = (|| {
            if !know!(self.ask(|g| g.is_ancestor(from.as_str(), head.as_str()))) {
                return Known::Yes(false);
            }
            let names = know!(self.ask(|g| g.diff_names(from.as_str(), head.as_str())));
            Known::Yes(names.iter().all(|path| is_record_path(path)))
        })();
        self.records_since.insert(from.to_string(), known);
        known
    }
}

/// 이 체크아웃의 역할: 역할 좌석이면 그 역할, dev 모드의 주 담당 좌석이면 `primary_role`.
pub fn own_role(config: &Config, actor: &Actor) -> Option<RoleId> {
    match &actor.seat {
        Seat::Role { id, .. } => Some(id.clone()),
        Seat::Lead if config.mode == Mode::Dev => config.primary_role.clone(),
        _ => None,
    }
}

/// 시작점 후보(6.6절 1번): 이 워크트리의 역할 인박스의 `base`, 없으면 세션의 `anchor`. dev 직접 작업은 항상 `anchor`다.
pub fn start_candidate(
    repo: &Repo,
    config: &Config,
    actor: &Actor,
    anchor: Option<Sha>,
) -> Option<Sha> {
    let inbox_base = own_role(config, actor)
        .filter(|_| !actor.direct_dev)
        .and_then(|role| fullops_store::read_inbox(repo, &role).yes().flatten())
        .map(|inbox| inbox.base);
    inbox_base.or(anchor)
}

/// 시작점(6.6절 2·3번): 후보가 바닥(`merge-base(HEAD, 통합 기준 ref)`)의 조상이거나 `HEAD`의 조상이 아니면 바닥을 쓴다.
/// 후보가 없으면 바닥이다. `lint`의 `--from` 기본값도 이 값이다.
pub fn start_point(
    git: &mut GitGate,
    head: &Sha,
    base_ref: &str,
    candidate: Option<Sha>,
) -> Known<Sha> {
    // 세션을 시작한 뒤 커밋이 없으면 바닥을 볼 필요가 없다
    if candidate.as_ref() == Some(head) {
        return Known::Yes(head.clone());
    }
    let floor = git.ask(|g| g.merge_base(head.as_str(), base_ref)).yes();
    let Some(start) = candidate.filter(|start| floor.as_ref() != Some(start)) else {
        return floor.map_or(Known::Unknown("no_start"), Known::Yes);
    };
    if let Some(floor) = &floor
        && git.ask(|g| g.is_ancestor(start.as_str(), floor.as_str())) == Known::Yes(true)
    {
        return Known::Yes(floor.clone());
    }
    // amend·rebase로 시작점이 HEAD의 조상이 아니게 됐으면 바닥을 쓴다
    if know!(git.ask(|g| g.is_ancestor(start.as_str(), head.as_str()))) {
        return Known::Yes(start);
    }
    floor.map_or(Known::Unknown("no_floor"), Known::Yes)
}

/// lead 체크아웃에 병합됐지만 통합 기준 ref에는 아직 없는 완료 보고의 SHA. 그 변경은 이 세션이 만든 것이 아니다.
pub fn merged_in(state: &State, landing: &mut dyn FnMut(&Sha) -> Known<Landing>) -> Vec<Sha> {
    let open = state
        .completions
        .iter()
        .filter(|c| !c.direct && c.outcome == WorkOutcome::Succeeded && c.landed_at.is_none());
    open.filter_map(|c| c.sha.clone())
        .filter(|sha| landing(sha) == Known::Yes(Landing::PushLeft))
        .collect()
}

/// 기준 시점(시작점 커밋)의 검사 기준(설계 6.1.1절). X-02의 `review_need`가 `changed()`가 읽은 값을 그대로 쓴다.
pub struct Criteria {
    pub review: ReviewPolicy,
    pub code: CodeDef,
    pub exclude: globset::GlobSet,
}

pub struct Changes {
    pub set: ChangeSet,
    /// 대상 후보가 하나도 없으면 읽지 않는다.
    pub criteria: Option<Criteria>,
}

/// 세션이 만든 변경(6.6절 `changed()`). `merged`는 `merged_in`의 결과이고 lead가 아니면 비운다:
/// 병합으로 들어온 커밋이 바꾼 파일은 대상에서 뺀다(그 통합은 X-06이 맡는다).
pub fn changed(
    git: &mut GitGate,
    repo: &Repo,
    config: &Config,
    actor: &Actor,
    head: &Sha,
    candidate: Option<Sha>,
    merged: &[Sha],
) -> Known<Changes> {
    let start = know!(start_point(git, head, &config.base_ref(), candidate));
    let mut names = Vec::new();
    if start != *head {
        names = know!(git.ask(|g| g.diff_names(start.as_str(), head.as_str())));
        names.retain(|path| !is_record_path(path));
        for sha in merged.iter().filter(|sha| **sha != start) {
            if names.is_empty() {
                break;
            }
            let theirs = know!(git.ask(|g| g.diff_names(start.as_str(), sha.as_str())));
            names.retain(|path| !theirs.contains(path));
        }
    }
    let mut status = know!(git.ask(|g| g.status(None)));
    status.retain(|entry| !is_record_path(&entry.path));
    let empty = |start: Sha| ChangeSet {
        start,
        code: Vec::new(),
        dirty: Vec::new(),
    };
    if names.is_empty() && status.is_empty() {
        return Known::Yes(Changes {
            set: empty(start),
            criteria: None,
        });
    }
    // 검사 기준은 시작점 커밋의 설정이다: 브랜치가 스스로 규칙을 느슨하게 하지 못한다.
    // 읽지 못하면(hook의 호출 상한) 작업 트리의 설정으로 판정한다
    let at = |path: &str| (start.to_string(), path.to_string());
    let criteria = match git.ask(|g| g.show_blobs(&[at(CONFIG_TOML), at(LINT_JSON)])) {
        Known::Yes(blobs) => {
            let text = |i: usize| {
                let blob = blobs.get(i).cloned().flatten();
                blob.and_then(|bytes| String::from_utf8(bytes).ok())
            };
            let baseline = fullops_store::baseline(text(0).as_deref());
            Criteria {
                review: baseline.review,
                code: baseline.code,
                exclude: fullops_core::lint_excludes(text(1).as_deref(), assets::LINT_JSON),
            }
        }
        Known::Unknown(_) => {
            let text = fullops_store::read_text(&repo.root.join(LINT_JSON));
            Criteria {
                review: config.review.clone(),
                code: config.code.clone(),
                exclude: fullops_core::lint_excludes(
                    text.yes().flatten().as_deref(),
                    assets::LINT_JSON,
                ),
            }
        }
    };
    let target =
        |path: &str| is_change_target(path, &criteria.code, &criteria.exclude, actor.direct_dev);
    let mut files: BTreeSet<String> = names.into_iter().filter(|p| target(p)).collect();
    let mut dirty = Vec::new();
    for entry in status.iter().filter(|e| target(&e.path)) {
        let modified = std::fs::metadata(repo.root.join(&entry.path))
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64);
        dirty.push((entry.path.clone(), modified));
        files.insert(entry.path.clone());
    }
    Known::Yes(Changes {
        set: ChangeSet {
            start,
            code: files.into_iter().collect(),
            dirty,
        },
        criteria: Some(criteria),
    })
}

/// lint 통과 인정(6.6절 `pass_covers`). 인정되면 그 통과 기록이다. `start`가 없으면 head 쪽 조건만 본다.
pub fn pass_covers(
    git: &mut GitGate,
    repo: &Repo,
    head: &Sha,
    start: Option<&Sha>,
) -> Known<Option<LintPass>> {
    let Some(pass) = know!(fullops_store::read_lint_pass(repo)) else {
        return Known::Yes(None);
    };
    // 통과 뒤에 쌓인 커밋이 기록 경로만 바꿨으면 그 통과를 인정한다
    if pass.head != *head && !know!(git.only_records_since(&pass.head, head)) {
        return Known::Yes(None);
    }
    let covers = match start {
        Some(start) if pass.base != *start => {
            know!(git.ask(|g| g.is_ancestor(pass.base.as_str(), start.as_str())))
        }
        _ => true,
    };
    Known::Yes(covers.then_some(pass))
}
