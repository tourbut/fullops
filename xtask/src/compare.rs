//! `cargo xtask compare [<시나리오 폴더 | 파일.toml>] [--out <보고서>]`: 같은 시나리오를 py·rs 두 구동기로 돌려
//! 의도 수준 결과(판정 종류, 흐름)를 견주고 `scenarios/report.md`를 낸다(설계 12.1·12.4절).
//!
//! py 구동기는 환경 변수 `FULLOPS_PY_BASELINE`(1.24.0 사본의 루트)이 있을 때만 돌린다.
//! 다른 단계의 분류는 `classify`에 있다. 회귀나 rs 기대와 다른 단계가 있으면 실패로 끝난다.

use fullops_testkit::driver::mismatches;
use fullops_testkit::scenario::{self, Scenario, Step};
use fullops_testkit::{Driver, bin};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn workspace() -> PathBuf {
    let xtask = Path::new(env!("CARGO_MANIFEST_DIR"));
    xtask.parent().unwrap_or(xtask).to_path_buf()
}

/// 구동기 하나가 낸 단계 하나.
#[derive(Clone)]
pub(crate) struct Out {
    /// `pass`·`context`·`warn`·`deny`·`block`·`human`·`external`·`error`, 건너뛴 단계는 `pending`, 1.24.0에 대응이 없으면 `n/a`.
    pub(crate) verdict: String,
    ms: u128,
    /// rs의 기대와 다른 점, 또는 구동 실패 사유.
    pub(crate) note: String,
}

impl Out {
    fn flow(&self) -> &'static str {
        match self.verdict.as_str() {
            "pass" | "context" | "warn" => "continues",
            "pending" | "n/a" => "-",
            _ => "stopped",
        }
    }
}

pub(crate) struct Ran {
    pub(crate) steps: Vec<Out>,
    /// 통합 기준 ref에 들어간 과제 수(`<KEY> 구현` 커밋).
    merged: Option<usize>,
}

pub(crate) fn failed(scenario: &Scenario, why: String) -> Ran {
    let step = Out {
        verdict: "error".into(),
        ms: 0,
        note: why,
    };
    Ran {
        steps: vec![step; scenario.steps.len()],
        merged: None,
    }
}

pub(crate) fn run_rs(scenario: &Scenario) -> Ran {
    let mut driver = Driver::new(&scenario.setup);
    let steps = scenario.steps.iter().map(|step| {
        let (verdict, ms, note) = match (&step.pending, &step.expect) {
            (Some(bundle), _) => ("pending".to_string(), 0, format!("묶음 {bundle}")),
            (None, expect) => match driver.step(step) {
                Ok(result) => {
                    let diff = expect.as_ref().map(|e| mismatches(e, &result));
                    let note = diff.unwrap_or_default().join(", ");
                    (result.verdict.to_string(), result.ms, note)
                }
                Err(why) => ("error".to_string(), 0, why),
            },
        };
        Out { verdict, ms, note }
    });
    let steps = steps.collect();
    let (place, name) = if scenario.setup.remote {
        ("origin.git", "main")
    } else {
        ("lead", "main")
    };
    let log = driver
        .sb
        .git(&driver.sb.path(place), &["log", "--format=%s", name]);
    let keys: std::collections::BTreeSet<&str> = log
        .stdout
        .lines()
        .filter_map(|subject| subject.strip_suffix(" 구현"))
        .collect();
    Ran {
        steps,
        merged: Some(keys.len()),
    }
}

/// py 구동기를 돌릴 Python. 이름만으로 띄우지 않고 PATH에서 찾은 전체 경로를 쓴다.
pub fn python() -> Option<PathBuf> {
    fullops_ext::hosts::find_program(if cfg!(windows) { "python" } else { "python3" })
}

fn run_py(file: &Path, scenario: &Scenario, baseline: &str) -> Ran {
    let Some(python) = python() else {
        return failed(scenario, "python을 PATH에서 찾지 못했습니다".into());
    };
    let fakes = bin("fake-orca");
    // fullops는 py 구동기가 `migrate` 단계를 실제로 돌릴 때 쓴다
    for name in ["fake-curl", "fake-host", "fullops"] {
        bin(name);
    }
    let out = Command::new(&python)
        .args(["-X", "utf8", "-B"])
        .arg(workspace().join("scenarios/drivers/py_driver.py"))
        .arg(file)
        .env("FULLOPS_PY_BASELINE", baseline)
        .env("FULLOPS_FAKES", fakes.parent().unwrap_or(Path::new(".")))
        .output();
    let out = match out {
        Ok(out) => out,
        Err(e) => return failed(scenario, format!("python을 실행하지 못했습니다: {e}")),
    };
    let parsed: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    let Some(steps) = parsed["steps"].as_array() else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let last = stderr.lines().last().unwrap_or("출력 없음");
        return failed(scenario, format!("py 구동기 실패: {last}"));
    };
    let steps = steps.iter().map(|step| Out {
        verdict: step["verdict"].as_str().unwrap_or("error").to_string(),
        ms: step["ms"].as_u64().unwrap_or(0).into(),
        note: String::new(),
    });
    Ran {
        steps: steps.collect(),
        merged: parsed["merged"].as_u64().map(|n| n as usize),
    }
}

/// 설계 12.2절의 행 하나.
struct Row {
    id: String,
    /// "1.24.0" 칸.
    baseline: String,
    text: String,
}

fn section<'a>(design: &'a str, from: &str, to: &str) -> &'a str {
    let start = design.find(from).unwrap_or(design.len());
    let end = design[start..].find(to).map_or(design.len(), |n| start + n);
    &design[start..end]
}

fn is_id(cell: &str) -> bool {
    let mut chars = cell.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && !chars.as_str().is_empty()
        && chars.all(|c| c.is_ascii_digit())
}

fn rows(design: &str) -> Vec<Row> {
    let table = section(design, "### 12.2", "### 12.3");
    let cells = |line: &'_ str| -> Vec<String> {
        line.split(" | ")
            .map(|c| c.trim_matches(['|', ' ']).to_string())
            .collect()
    };
    table
        .lines()
        .map(|line| (line, cells(line)))
        .filter(|(_, cells)| cells.len() >= 5 && is_id(&cells[0]))
        .map(|(line, cells)| Row {
            id: cells[0].clone(),
            baseline: cells[2].clone(),
            text: line.to_string(),
        })
        .collect()
}

/// 글 안의 `DF-<번호>`들.
fn dfs(text: &str) -> Vec<String> {
    let mut found: Vec<String> = text
        .split("DF-")
        .skip(1)
        .map(|rest| rest.chars().take_while(char::is_ascii_digit).collect())
        .filter(|digits: &String| !digits.is_empty())
        .map(|digits| format!("DF-{digits}"))
        .collect();
    found.sort();
    found.dedup();
    found
}

fn flow_verb(act: &str) -> bool {
    act.starts_with("lead_") || act.starts_with("worker_")
}

/// 단계 하나의 결과 칸. 설계 12.4절: 판정 종류와 흐름이 같거나, 다르면 12.3절에 근거가 있어야 한다.
/// 흐름 동사는 1.24.0에서 hook 거부이고 새 판에서 CLI 거절일 수 있어 흐름(이어짐·멈춤)만 견준다(12.1절).
/// 근거는 그 단계의 `df = "DF-n"`이다. `stepwise`(그 파일에 `df`를 적은 단계가 있다)가 거짓이면
/// 시나리오 ID의 12.2절 행에 있는 DF로 가린다(행 단위라 같은 행의 다른 차이도 함께 가려진다).
fn classify(
    step: &Step,
    py: Option<&Out>,
    rs: &Out,
    row: Option<&Row>,
    known: &[String],
    stepwise: bool,
) -> String {
    let wrong = if rs.note.is_empty() || rs.verdict == "pending" {
        String::new()
    } else {
        format!(" · rs 기대와 다름({})", rs.note)
    };
    if rs.verdict == "pending" {
        return format!("보류({})", rs.note);
    }
    let Some(py) = py.filter(|py| py.verdict != "n/a") else {
        return format!("기준선 없음{wrong}");
    };
    let same = if flow_verb(&step.act) {
        py.flow() == rs.flow()
    } else {
        py.verdict == rs.verdict
    };
    if same {
        return format!("같음{wrong}");
    }
    if let Some(df) = step.str("df") {
        return if known.iter().any(|k| k == df) {
            format!("의도한 차이({df}){wrong}")
        } else {
            format!("회귀({df}는 12.3절에 없다){wrong}")
        };
    }
    // 1.24.0은 caveman 스킬이 없으면 세션 시작마다 경고를 낸다. 새 판은 내지 않는다
    if step.act == "session_starts" && py.verdict == "warn" && rs.flow() == "continues" {
        return format!("의도한 차이(DF-12){wrong}");
    }
    let Some(row) = row else {
        return format!("회귀{wrong}");
    };
    let named: Vec<String> = dfs(&row.text)
        .into_iter()
        .filter(|df| !stepwise && known.contains(df))
        .collect();
    if !named.is_empty() {
        format!(
            "의도한 차이(12.2절 {} 행: {}){wrong}",
            row.id,
            named.join("·")
        )
    } else if row.text.contains("더 나음") {
        format!("의도한 차이(12.2절 {}: 더 나음){wrong}", row.id)
    } else if row.baseline == "-" {
        format!("기준선 없음(12.2절 {}){wrong}", row.id)
    } else {
        format!("회귀{wrong}")
    }
}

/// lead가 받은 차단 수: lead가 한 단계 가운데 판정이 차단·거부인 것.
fn lead_blocks(scenario: &Scenario, steps: &[Out]) -> usize {
    let by_lead = |step: &Step| {
        !step.act.starts_with("worker_") && step.str("who").unwrap_or("lead") == "lead"
    };
    scenario
        .steps
        .iter()
        .zip(steps)
        .filter(|(step, out)| by_lead(step) && matches!(out.verdict.as_str(), "block" | "deny"))
        .count()
}

pub fn run(args: &[String]) -> Result<(), String> {
    let root = workspace();
    let mut target = root.join("scenarios");
    let mut out = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--out" => out = rest.next().map(PathBuf::from),
            other => target = PathBuf::from(other),
        }
    }
    let files: Vec<PathBuf> = if target.is_dir() {
        let entries = std::fs::read_dir(&target).map_err(|e| e.to_string())?;
        let mut files: Vec<PathBuf> = entries
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| path.extension().is_some_and(|e| e == "toml"))
            .collect();
        files.sort();
        files
    } else {
        vec![target.clone()]
    };
    let folder = if target.is_dir() {
        target.clone()
    } else {
        root.join("scenarios")
    };
    let out = out.unwrap_or_else(|| folder.join("report.md"));
    let design = std::fs::read_to_string(root.join("docs/design.md")).map_err(|e| e.to_string())?;
    let rows = rows(&design);
    let known = dfs(section(&design, "### 12.3", "### 12.4"));
    let baseline = std::env::var("FULLOPS_PY_BASELINE")
        .ok()
        .filter(|b| !b.is_empty());

    let mut table = String::new();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut pending: BTreeMap<String, usize> = BTreeMap::new();
    let mut covered = Vec::new();
    let mut flows = String::new();
    let (mut wrong, mut coarse) = (0, 0);
    let mut broken = Vec::new();
    for file in &files {
        let stem = file
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let id = stem.split('-').next().unwrap_or_default().to_uppercase();
        let row = rows.iter().find(|row| row.id == id);
        covered.push(id);
        let scenario = scenario::load(file)?;
        eprintln!("{stem}: {}", scenario.name);
        let rs = std::panic::catch_unwind(|| run_rs(&scenario))
            .unwrap_or_else(|_| failed(&scenario, "rs 구동기가 죽었습니다".into()));
        let py = baseline.as_deref().map(|b| run_py(file, &scenario, b));
        // py 구동기가 끝까지 돌지 못한 시나리오는 견준 것이 아니다. 12.2절 행의 `-`에 가려지지 않게 따로 센다
        let crashed = py.as_ref().and_then(|py| py.steps.first());
        if let Some(why) = crashed.map(|step| &step.note).filter(|n| !n.is_empty()) {
            broken.push(format!("{stem}({why})"));
        }
        let stepwise = scenario.steps.iter().any(|s| s.str("df").is_some());
        for (n, step) in scenario.steps.iter().enumerate() {
            let rs_out = &rs.steps[n];
            let py_out = py.as_ref().and_then(|py| py.steps.get(n));
            let mut result = classify(step, py_out, rs_out, row, &known, stepwise);
            // 단계의 `py`는 py 구동기가 내야 하는 판정이다(전환·되돌린 레포에서 1.24.0이 내는 것. N3)
            if let (Some(want), Some(got)) = (step.str("py"), py_out)
                && got.verdict != want
            {
                let _ = write!(result, " · py 기대와 다름({want}가 아니라 {})", got.verdict);
            }
            let kind = ["같음", "의도한 차이", "기준선 없음", "보류", "회귀"]
                .into_iter()
                .find(|kind| result.starts_with(kind))
                .unwrap_or("회귀");
            *counts.entry(kind).or_default() += 1;
            wrong += usize::from(result.contains("기대와 다름"));
            coarse += usize::from(result.contains(" 행: "));
            if let Some(bundle) = &step.pending {
                *pending.entry(bundle.clone()).or_default() += 1;
            }
            let cell = |out: Option<&Out>, f: fn(&Out) -> String| out.map_or("-".to_string(), f);
            let _ = writeln!(
                table,
                "| {stem} | {}. {} | {} | {} | {} | {} | {result} | {} | {} |",
                n + 1,
                step.act,
                cell(py_out, |o| o.verdict.clone()),
                rs_out.verdict,
                cell(py_out, |o| o.flow().to_string()),
                rs_out.flow(),
                cell(py_out, |o| format!("{}ms", o.ms)),
                if rs_out.verdict == "pending" {
                    "-".to_string()
                } else {
                    format!("{}ms", rs_out.ms)
                },
            );
        }
        // 끝까지 간 정도(12.1절): 병합된 과제 수와 lead가 받은 차단 수. 보류 단계가 있으면 rs의 수는 아직 뜻이 없다
        let held = scenario.steps.iter().any(|step| step.pending.is_some());
        let number = |n: Option<usize>| n.map_or("-".to_string(), |n| n.to_string());
        let _ = writeln!(
            flows,
            "| {stem} | {} | {} | {} | {} |",
            number(py.as_ref().and_then(|py| py.merged)),
            number(rs.merged.filter(|_| !held)),
            number(py.as_ref().map(|py| lead_blocks(&scenario, &py.steps))),
            number(Some(lead_blocks(&scenario, &rs.steps)).filter(|_| !held)),
        );
    }

    let missing: Vec<&str> = rows
        .iter()
        .map(|row| row.id.as_str())
        .filter(|id| !covered.iter().any(|c| c == id))
        .collect();
    let count = |kind: &str| counts.get(kind).copied().unwrap_or(0);
    let held: usize = pending.values().sum();
    let by_bundle: Vec<String> = pending.iter().map(|(b, n)| format!("{b} {n}")).collect();
    let mark = |ok: bool| if ok { "합격" } else { "미달" };
    let mut report = String::new();
    let _ = write!(
        report,
        "# 비교 보고서: 1.24.0(py) 대 새 판(rs)\n\n\
         `cargo xtask compare`가 쓰는 파일이다. 손으로 고치지 않는다. 방법과 합격 기준은 설계 12.1절과 12.4절, 다시 돌리는 명령은 `docs/bench.md`에 있다.\n\n\
         - 시나리오 {}개, 단계 {}개.\n\
         - 기준선(py 구동기): {}.\n\
         - 견주는 것: hook 동사는 판정 종류, 흐름 동사(`lead_*`·`worker_*`)는 흐름(이어짐·멈춤). 문구와 파일 바이트는 견주지 않는다.\n\
         - 결과 칸: `같음`, `의도한 차이`(그 단계에 적은 `df`가 설계 12.3절에 있다. `df`를 적은 단계가 하나도 없는 시나리오 파일은 그 ID의 12.2절 행에 있는 DF로 가리고 \"행:\"이라고 적는다. 12.2절 행의 \"더 나음\"도 여기 든다), `기준선 없음`(1.24.0에 대응이 없거나 12.2절의 1.24.0 칸이 `-`다), `보류`(`pending`), `회귀`(그 밖의 차이).\n\
         - 단계의 `py`는 py 구동기가 내야 하는 판정이다. 다르면 결과 칸에 `py 기대와 다름`이 붙고 기대와 다른 단계로 센다.\n\n\
         ## 합격 점검 (설계 12.4절)\n\n\
         | 항목 | 값 | 판정 |\n|---|---|---|\n\
         | 회귀 | {}건 | {} |\n\
         | 시나리오의 기대와 다른 단계(rs의 `expect`, py의 `py`) | {wrong}건 | {} |\n\
         | 남은 `pending` | {held}건{} | {} |\n\
         | 시나리오 파일이 없는 12.2절 ID | {}개 | {} |\n\
         | py 구동기가 끝까지 돌지 못한 시나리오 | {}개 | {} |\n\n",
        files.len(),
        counts.values().sum::<usize>(),
        if baseline.is_some() {
            "돌렸다"
        } else {
            "건너뜀(환경 변수 `FULLOPS_PY_BASELINE`이 없다)"
        },
        count("회귀"),
        mark(count("회귀") == 0),
        mark(wrong == 0),
        if by_bundle.is_empty() {
            String::new()
        } else {
            format!("(묶음별: {})", by_bundle.join(", "))
        },
        mark(held == 0),
        missing.len(),
        mark(missing.is_empty()),
        broken.len(),
        mark(broken.is_empty()),
    );
    if !broken.is_empty() {
        let _ = writeln!(
            report,
            "py 구동기가 끝까지 돌지 못한 시나리오(그 단계들은 견주지 못했다): {}.\n",
            broken.join(", ")
        );
    }
    if !missing.is_empty() {
        let _ = writeln!(report, "시나리오 파일이 없는 ID: {}.\n", missing.join(", "));
    }
    let _ = write!(
        report,
        "같음 {}건, 의도한 차이 {}건(그 가운데 12.2절 행 단위로 가린 것 {coarse}건. 단계에 `df`를 적으면 단계 단위가 된다), 기준선 없음 {}건, 보류 {}건, 회귀 {}건.\n\n\
         ## 끝까지 간 정도\n\n\
         사람 입력 없이 병합된 과제 수는 통합 기준 ref에 들어간 `<KEY> 구현` 커밋의 키 수이고, lead가 받은 차단 수는 lead가 한 단계 가운데 판정이 차단·거부인 것이다. \
         K4의 합격은 병합된 과제 수가 py 이상이고 차단 수가 py 이하인 것이다. rs 칸의 `-`는 보류 단계가 남아 아직 셀 수 없다는 뜻이다.\n\n\
         | 시나리오 | 병합된 과제(py) | 병합된 과제(rs) | lead 차단(py) | lead 차단(rs) |\n|---|---|---|---|---|\n{flows}\n\
         ## 단계별 결과\n\n\
         | 시나리오 | 단계 | py 판정 | rs 판정 | py 흐름 | rs 흐름 | 결과 | py 시간 | rs 시간 |\n|---|---|---|---|---|---|---|---|---|\n{table}",
        count("같음"),
        count("의도한 차이"),
        count("기준선 없음"),
        count("보류"),
        count("회귀"),
    );
    std::fs::write(&out, report).map_err(|e| format!("{}: {e}", out.display()))?;
    println!(
        "{}: 회귀 {}건, 기대와 다름 {wrong}건, pending {held}건, 시나리오 없는 ID {}개, py 구동기 실패 {}개",
        out.display(),
        count("회귀"),
        missing.len(),
        broken.len()
    );
    if count("회귀") + wrong + broken.len() > 0 {
        return Err(
            "회귀, 기대와 다른 단계, py 구동기가 끝까지 돌지 못한 시나리오 가운데 하나가 있습니다. 보고서를 확인하세요."
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(verdict: &str) -> Out {
        Out {
            verdict: verdict.into(),
            ms: 0,
            note: String::new(),
        }
    }

    fn step(act: &str) -> Step {
        Step {
            act: act.into(),
            expect: None,
            pending: None,
            args: Default::default(),
        }
    }

    #[test]
    fn design_tables_and_step_classes() {
        let design = std::fs::read_to_string(workspace().join("docs/design.md")).unwrap();
        let rows = rows(&design);
        let known = dfs(section(&design, "### 12.3", "### 12.4"));
        let row = |id: &str| rows.iter().find(|row| row.id == id);
        assert!(rows.len() > 90 && row("A1").is_some() && row("R8").is_some());
        assert_eq!(row("K4").map(|row| row.baseline.as_str()), Some("-"));
        assert!(known.contains(&"DF-1".to_string()) && known.contains(&"DF-24".to_string()));

        let class = |act: &str, py: &str, rs: &str, id: &str| {
            classify(&step(act), Some(&out(py)), &out(rs), row(id), &known, false)
        };
        assert_eq!(class("runs_shell", "deny", "deny", "C1"), "같음");
        // 흐름 동사는 흐름만 견준다: hook 거부와 CLI 거절은 같은 결과다
        assert_eq!(class("lead_dispatches", "block", "deny", "D1"), "같음");
        assert_eq!(
            class("runs_shell", "pass", "deny", "C2"),
            "의도한 차이(12.2절 C2 행: DF-1)"
        );
        // 단계의 `df`는 그 단계만 가린다. `df`를 쓰는 파일에서는 행의 DF가 다른 단계의 차이를 가리지 않는다
        let stepwise = |df: Option<&str>| {
            let mut step = step("runs_shell");
            if let Some(df) = df {
                step.args.insert("df".into(), df.into());
            }
            let (py, rs) = (out("pass"), out("deny"));
            classify(&step, Some(&py), &rs, row("C2"), &known, true)
        };
        assert_eq!(stepwise(Some("DF-1")), "의도한 차이(DF-1)");
        assert_eq!(stepwise(Some("DF-99")), "회귀(DF-99는 12.3절에 없다)");
        assert_eq!(stepwise(None), "회귀");
        assert_eq!(
            class("session_starts", "warn", "context", "B1"),
            "의도한 차이(DF-12)"
        );
        assert!(class("lead_dispatches", "deny", "pass", "D4").contains("더 나음"));
        assert_eq!(
            class("stops", "pass", "block", "H4"),
            "기준선 없음(12.2절 H4)"
        );
        assert_eq!(class("runs_shell", "deny", "pass", "C1"), "회귀");
        assert_eq!(class("runs_fullops", "n/a", "pass", "C1"), "기준선 없음");
        let held = Out {
            note: "묶음 C".into(),
            ..out("pending")
        };
        assert_eq!(
            classify(&step("lead_plans"), None, &held, None, &known, false),
            "보류(묶음 C)"
        );
    }
}
