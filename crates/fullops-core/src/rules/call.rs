//! `decide` 한 번의 호출 문맥. `check(&HookInput, &mut dyn Facts)`의 시그니처로는 넘길 수 없는 것을 여기에 둔다:
//! 레포 루트와 Orca 실행 파일 이름(`Facts`에 없다), 분류 결과, 규칙이 내는 효과, 같은 호출 안에서 먼저 일어난 일(dispatch 묶기, 턴 증가).
//! hook은 이벤트당 프로세스 하나이고 `decide`는 한 스레드에서 한 번 돈다.

use super::Effect;
use super::decide::{HANDOVERS, QA_REPORTS};
use super::tool::CONFIG;
use crate::facts::Facts;
use crate::messages::hook::Brief;
use crate::model::{Actor, DispatchId, HookInput, Intent, Known, SkillText, ToolKind};
use crate::patch::{edit_targets, repo_relative};
use crate::shell::classify_shell;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// 바이너리가 넘기는 호출 문맥. `Facts`가 주지 않는 값이다.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    /// 작업 트리 루트. 편집 대상의 경로를 이 기준 상대 경로로 맞춘다.
    pub root: PathBuf,
    /// Orca 실행 파일로 볼 이름(`ORCA_CLI_COMMAND`의 파일 이름).
    pub orca_names: Vec<String>,
    /// S-04의 조건이 참이다(실행본의 위치와 버전은 바이너리가 본다).
    pub self_install: bool,
    /// PATH에 없는 필수 도구 이름(`git`, `orca`). S-01이 한 문장 낸다.
    pub missing_tools: Vec<String>,
}

/// 편집 대상 하나. 경로는 레포 루트 기준 상대 경로(`/` 구분)다.
#[derive(Debug, Clone)]
pub(super) struct Target {
    pub rel: String,
    pub before: Option<String>,
    pub after: Known<Option<String>>,
}

#[derive(Default)]
pub(super) struct Call {
    pub scene: Scene,
    pub intents: Vec<Intent>,
    targets: Option<Vec<Target>>,
    paths: Option<Vec<String>>,
    /// 실제 경로로 푼 레포 루트.
    real_root: Option<PathBuf>,
    pub effects: Vec<Effect>,
    /// 이 호출에서 P-00이 턴을 올렸다.
    pub turn_bump: bool,
    /// 이 호출에서 P-01이 묶은 dispatch.
    pub bound: Option<DispatchId>,
    pub created: bool,
    pub brief: Option<Brief>,
    pub skill: Option<SkillText>,
    pub x01_hit: bool,
}

thread_local! {
    static CALL: RefCell<Call> = RefCell::new(Call::default());
}

/// 문맥을 빌려 쓴다. 겹쳐 빌리는 곳은 없지만, 만일의 경우에도 panic 대신 빈 문맥을 쓴다.
pub(super) fn with<T>(f: impl FnOnce(&mut Call) -> T) -> T {
    CALL.with(|cell| match cell.try_borrow_mut() {
        Ok(mut call) => f(&mut call),
        Err(_) => f(&mut Call::default()),
    })
}

pub(super) fn begin(scene: &Scene, input: &HookInput) {
    let intents = match &input.tool {
        Some(tool) => match (tool.kind, &tool.command, tool.truncated) {
            (ToolKind::Shell(_), _, true) => vec![Intent::Unparsed],
            (ToolKind::Shell(dialect), Some(command), false) => {
                classify_shell(command, dialect, &scene.orca_names)
            }
            _ => Vec::new(),
        },
        None => Vec::new(),
    };
    with(|call| {
        *call = Call {
            scene: scene.clone(),
            intents,
            ..Call::default()
        }
    });
}

pub(super) fn effect(effect: Effect) {
    with(|call| call.effects.push(effect));
}

pub(super) fn intents() -> Vec<Intent> {
    with(|call| call.intents.clone())
}

pub(super) fn root() -> PathBuf {
    with(|call| call.scene.root.clone())
}

/// 편집 대상의 레포 루트 기준 상대 경로. 경로를 보는 규칙(T-00, T-05, T-06, T-07, T-10과 후보 가리기)이 모두 이 함수를 거친다.
/// 판정 전에 실제 경로로 푼다: 링크나 junction을 거친 경로와 Windows에서 대소문자만 다른 경로도 같은 파일이다.
pub(super) fn rel_path(path: &Path, input: &HookInput, facts: &mut dyn Facts) -> Option<String> {
    let real_root = match with(|call| call.real_root.clone()) {
        Some(root) => root,
        None => {
            let real = facts.real_path(&root());
            with(|call| call.real_root = Some(real.clone()));
            real
        }
    };
    let mut real = |path: &Path| facts.real_path(path);
    let rel = repo_relative(&path.to_string_lossy(), &input.cwd, &real_root, &mut real)?;
    if cfg!(windows) {
        // 아직 없는 폴더는 받은 표기 그대로다: 규칙이 견주는 이름을 정본 표기로 맞춘다
        for known in [HANDOVERS, QA_REPORTS, CONFIG, ".fullops-squad/"] {
            if let (Some(head), Some(tail)) = (rel.get(..known.len()), rel.get(known.len()..))
                && head.eq_ignore_ascii_case(known)
            {
                return Some(format!("{known}{tail}"));
            }
        }
    }
    Some(rel)
}

/// 편집 대상의 경로만(파일 글을 읽지 않는다). 규칙 후보를 가릴 때 쓴다. 한 호출에서 한 번만 계산한다.
pub(super) fn target_paths(input: &HookInput, facts: &mut dyn Facts) -> Vec<String> {
    if let Some(cached) = with(|call| call.paths.clone()) {
        return cached;
    }
    let list: Vec<String> = match &input.tool {
        Some(tool) => edit_targets(tool, &mut |_| Known::Unknown("skip"))
            .iter()
            .filter_map(|t| rel_path(&t.path, input, facts))
            .collect(),
        None => Vec::new(),
    };
    with(|call| call.paths = Some(list.clone()));
    list
}

/// 편집 대상과 바뀐 뒤의 글. 레포 루트 밖의 대상은 뺀다. 한 호출에서 한 번만 계산한다.
pub(super) fn targets(input: &HookInput, facts: &mut dyn Facts) -> Vec<Target> {
    if let Some(cached) = with(|call| call.targets.clone()) {
        return cached;
    }
    let root = root();
    let list: Vec<Target> = match &input.tool {
        Some(tool) => {
            // 대상마다 경로는 한 번만 푼다
            let mut rels: Vec<(PathBuf, Option<String>)> = Vec::new();
            let mut read = |path: &Path| {
                let rel = rel_path(path, input, facts);
                rels.push((path.to_path_buf(), rel.clone()));
                match rel {
                    Some(rel) => facts.file_text(&root.join(rel)),
                    None => Known::Unknown("outside"),
                }
            };
            let found = edit_targets(tool, &mut read);
            found
                .into_iter()
                .filter_map(|t| {
                    let (_, rel) = rels.iter().find(|(path, _)| *path == t.path)?;
                    Some(Target {
                        rel: rel.clone()?,
                        before: t.before,
                        after: t.after,
                    })
                })
                .collect()
        }
        None => Vec::new(),
    };
    with(|call| call.targets = Some(list.clone()));
    list
}

/// 호출자. 이 호출에서 방금 묶인 dispatch를 반영한다(효과는 판정 뒤에 실행되므로 `Facts`는 아직 모른다).
pub(super) fn actor(facts: &mut dyn Facts) -> Known<Actor> {
    let mut actor = match facts.actor() {
        Known::Yes(actor) => actor,
        Known::Unknown(why) => return Known::Unknown(why),
    };
    if let Some(id) = with(|call| call.bound.clone()) {
        actor.sub = facts
            .state()
            .yes()
            .and_then(|state| state.dispatch(&id))
            .is_some_and(|d| d.is_sub());
        actor.dispatch = Some(id);
        actor.direct_dev = false;
    }
    Known::Yes(actor)
}

/// 현재 턴. 세션 파일이 없으면 0에서 센다.
pub(super) fn turn(facts: &mut dyn Facts) -> u32 {
    let turn = facts.session().yes().map_or(0, |s| s.turn);
    turn + u32::from(with(|call| call.turn_bump))
}

/// 한 번 막기: 이 규칙을 같은 지문으로 이번 턴에 이미 막았는가(설계 7.6절).
pub(super) fn blocked_before(facts: &mut dyn Facts, rule: &str, fp: &str) -> bool {
    // 새 dispatch가 묶이면 blocks를 비운다
    if with(|call| call.bound.is_some()) {
        return false;
    }
    let now = turn(facts);
    facts
        .session()
        .yes()
        .and_then(|s| s.blocks.get(rule))
        .is_some_and(|mark| mark.fp == fp && mark.turn == now)
}

pub(super) fn noticed(facts: &mut dyn Facts, key: &str) -> bool {
    facts
        .session()
        .yes()
        .is_some_and(|s| s.noticed.iter().any(|k| k == key))
}

/// 세션 파일이 없으면 만들게 한다(S-02와 같은 내용). 한 호출에 한 번만 낸다.
pub(super) fn ensure_session(facts: &mut dyn Facts) {
    let missing = matches!(facts.session(), Known::Unknown(NO_SESSION));
    if missing && !with(|call| std::mem::replace(&mut call.created, true)) {
        effect(Effect::CreateSession);
    }
}

/// `Facts::session()`이 "세션 파일이 없다"를 알리는 사유. 읽지 못한 것과 구분한다.
pub const NO_SESSION: &str = "no_session";
