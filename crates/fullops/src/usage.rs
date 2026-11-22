//! 사용량 기록 `<git-common-dir>/fullops/usage/<UTC 날짜>.jsonl`(설계 6.2절). hook·CLI·lint가 호출마다 한 줄을 더한다.
//! 원문(명령 문자열, 경로, 프롬프트, 인자 값, lint 메시지)은 쓰지 않는다. 규칙 ID, 판정, 명령 이름, 종료 코드, 개수만 쓴다.
//! 실패는 버린다(hook 판정과 명령 결과에 영향을 주지 않는다). gc·snapshot 정리는 이 폴더를 지우지 않는다.

use fullops_core::model::{HostId, RuleTrace, Verdict};
use fullops_store::Repo;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::Path;

/// 하루 파일의 한도. 넘으면 그날 남은 줄은 버린다.
const DAY_LIMIT: u64 = 1024 * 1024;
/// 남기는 날 수. 새 날 파일을 만들 때 오래된 것부터 지운다.
const KEEP_DAYS: usize = 60;

/// 하위 명령으로 남기는 낱말. 이 밖의 첫 인자(과제 키, 경로 등)는 남기지 않는다.
const SUBS: &[&str] = &[
    "new",
    "reopen",
    "recover",
    "list",
    "ready",
    "check",
    "add",
    "set",
    "need",
    "snapshot",
    "waive",
    "cleanup",
    "supersede",
    "prune",
    "answer",
    "drop",
    "stamp",
    "link",
    "set-sha",
];

fn append(repo: &Repo, mut line: Map<String, Value>) {
    let now = fullops_store::now();
    line.insert("t".into(), now.into());
    let dir = repo.shared().join("usage");
    let path = dir.join(format!("{}.jsonl", crate::cmd::date_of(now)));
    match std::fs::metadata(&path) {
        Ok(meta) if meta.len() > DAY_LIMIT => return,
        Ok(_) => {}
        Err(_) => prune(&dir),
    }
    let _ = fullops_store::append_line(&path, &line);
}

/// 날 파일을 `KEEP_DAYS`개 미만으로 줄인다(이제 하나를 새로 만든다).
fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut days: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    if days.len() < KEEP_DAYS {
        return;
    }
    days.sort();
    for old in &days[..=days.len() - KEEP_DAYS] {
        let _ = std::fs::remove_file(old);
    }
}

fn verdict_name(v: &Verdict) -> &'static str {
    match v {
        Verdict::Pass => "pass",
        Verdict::Context(_) => "context",
        Verdict::Warn(_) => "warn",
        Verdict::Deny { .. } => "deny",
        Verdict::Block { .. } => "block",
    }
}

/// hook 호출 한 번. `rules`는 평가한 규칙마다 `<ID>:<판정>`이다(판정은 추적의 것에 `pass`를 더한 것).
pub fn hook(
    repo: &Repo,
    host: HostId,
    event: &str,
    verdict: Option<&Verdict>,
    rules: &[RuleTrace],
    err: Option<&str>,
) {
    let mut line = Map::new();
    line.insert("hook".into(), event.into());
    line.insert("host".into(), json!(host));
    if let Some(v) = verdict {
        line.insert("v".into(), verdict_name(v).into());
    }
    if !rules.is_empty() {
        let rules: Vec<String> = rules
            .iter()
            .map(|r| format!("{}:{}", r.id, r.verdict))
            .collect();
        line.insert("rules".into(), rules.into());
    }
    if let Some(err) = err {
        line.insert("err".into(), err.into());
    }
    append(repo, line);
}

/// 하위 명령 낱말. `SUBS`에 없으면 `None`이다.
pub fn sub_of(first: Option<&str>) -> Option<&'static str> {
    let first = first?;
    SUBS.iter().copied().find(|s| *s == first)
}

/// CLI 명령 한 번.
pub fn cli(repo: &Repo, command: &str, sub: Option<&str>, exit: u8) {
    let mut line = Map::new();
    line.insert("cli".into(), command.into());
    if let Some(sub) = sub {
        line.insert("sub".into(), sub.into());
    }
    line.insert("exit".into(), exit.into());
    append(repo, line);
}

/// 레포 규칙 코드도 `[A-Z]+-숫자` 꼴만 그대로 남긴다. 그 밖은 `OTHER`로 센다.
fn code_name(code: &str) -> &str {
    let ok = code.split_once('-').is_some_and(|(a, n)| {
        !a.is_empty()
            && a.bytes().all(|b| b.is_ascii_uppercase())
            && !n.is_empty()
            && n.bytes().all(|b| b.is_ascii_digit())
    });
    if ok && code.len() <= 16 {
        code
    } else {
        "OTHER"
    }
}

fn tally<'a>(items: impl Iterator<Item = &'a str>) -> Value {
    let mut map = BTreeMap::<&str, u32>::new();
    for item in items {
        *map.entry(item).or_default() += 1;
    }
    json!(map)
}

/// lint 실행 한 번. `codes`는 finding의 코드, `statuses`는 레포 명령의 결과(`passed`·`failed` 등)다.
pub fn lint<'a>(
    repo: &Repo,
    codes: impl Iterator<Item = &'a str>,
    statuses: impl Iterator<Item = &'a str>,
    passed: bool,
) {
    let mut line = Map::new();
    line.insert("lint".into(), tally(codes.map(code_name)));
    line.insert("cmds".into(), tally(statuses));
    line.insert("passed".into(), passed.into());
    append(repo, line);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(name: &str) -> Repo {
        let dir = std::env::temp_dir().join(format!("fullops-usage-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Repo {
            root: dir.clone(),
            git_dir: dir.join(".git"),
            common_dir: dir.join(".git"),
        }
    }

    fn lines(repo: &Repo) -> Vec<Value> {
        let dir = repo.shared().join("usage");
        let path = std::fs::read_dir(dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn writes_counts_without_raw_text() {
        let repo = repo("counts");
        let rule = |id: &str, verdict: &str| RuleTrace {
            id: id.into(),
            verdict: verdict.into(),
            fp: Some("secret-fingerprint".into()),
        };
        let block = Verdict::Block {
            rule: "X-01",
            reason: "reason text".into(),
            also: vec![],
        };
        hook(
            &repo,
            HostId::Claude,
            "Stop",
            Some(&block),
            &[rule("X-00", "pass"), rule("X-01", "block")],
            None,
        );
        cli(&repo, "review", sub_of(Some("check")), 0);
        cli(&repo, "route", sub_of(Some("TASK-1")), 3);
        lint(
            &repo,
            ["SIZE-001", "SIZE-001", "my rule!"].into_iter(),
            ["passed", "failed"].into_iter(),
            false,
        );
        let got = lines(&repo);
        assert_eq!(got.len(), 4);
        assert_eq!(got[0]["hook"], "Stop");
        assert_eq!(got[0]["v"], "block");
        assert_eq!(got[0]["rules"], json!(["X-00:pass", "X-01:block"]));
        assert_eq!(got[1]["sub"], "check");
        assert!(got[2].get("sub").is_none());
        assert_eq!(got[2]["exit"], 3);
        assert_eq!(got[3]["lint"], json!({"SIZE-001": 2, "OTHER": 1}));
        assert_eq!(got[3]["cmds"], json!({"passed": 1, "failed": 1}));
        let text = serde_json::to_string(&got).unwrap();
        for raw in ["secret-fingerprint", "reason text", "TASK-1", "my rule"] {
            assert!(!text.contains(raw), "{raw}");
        }
    }

    #[test]
    fn stops_at_the_day_limit_and_prunes_old_days() {
        let repo = repo("limit");
        let dir = repo.shared().join("usage");
        std::fs::create_dir_all(&dir).unwrap();
        for day in 0..KEEP_DAYS {
            std::fs::write(dir.join(format!("2000-01-{day:02}.jsonl")), "").unwrap();
        }
        cli(&repo, "status", None, 0);
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names.len(), KEEP_DAYS);
        assert_eq!(names[0], "2000-01-01.jsonl");

        let today = dir.join(format!(
            "{}.jsonl",
            crate::cmd::date_of(fullops_store::now())
        ));
        std::fs::write(&today, vec![b'x'; DAY_LIMIT as usize + 1]).unwrap();
        cli(&repo, "status", None, 0);
        assert_eq!(std::fs::metadata(&today).unwrap().len(), DAY_LIMIT + 1);
    }
}
