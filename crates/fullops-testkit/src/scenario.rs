//! 시나리오 파일 형식(설계 12.1절). `scenarios/<ID 소문자>.toml` 하나가 시나리오 하나다(경우가 많으면 `<id>-<경우>.toml`).
//! 상황과 단계를 의도 수준의 동사로 적고 상태 파일을 직접 주입하지 않는다.

use crate::sandbox::Setup;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub name: String,
    /// 근거(예: `flow-graph E4`, 12.2절의 ID).
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub setup: Setup,
    #[serde(default, rename = "step")]
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Step {
    /// 동사. 목록은 `driver::VERBS`.
    pub act: String,
    #[serde(default)]
    pub expect: Option<Expect>,
    /// 그 단계가 기대는 다른 묶음의 글자. 있으면 구동기가 그 단계를 건너뛰고 "보류"로 적는다.
    #[serde(default)]
    pub pending: Option<String>,
    /// 동사의 인자.
    #[serde(flatten)]
    pub args: toml::Table,
}

/// 단계의 기대. 적은 항목만 비교한다.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    /// hook: `pass`·`context`·`warn`·`deny`·`block`. CLI: `pass`(0)·`deny`(2)·`human`(3)·`external`(4)·`error`(1).
    pub verdict: Option<String>,
    /// 출력의 `FullOps[<ID>]` 가운데 이 ID가 있어야 한다.
    pub rule: Option<String>,
    /// `continues`(통과·안내·경고) 또는 `stopped`.
    pub flow: Option<String>,
    /// CLI 종료 코드.
    pub exit: Option<i32>,
    /// 출력에 이 글(탈출구 명령 이름)이 있어야 한다. `rs` 구동기에서만 단언한다.
    pub escape: Option<String>,
}

impl Step {
    pub fn str(&self, name: &str) -> Option<&str> {
        self.args.get(name).and_then(|v| v.as_str())
    }

    pub fn req(&self, name: &str) -> Result<&str, String> {
        self.str(name)
            .ok_or_else(|| format!("동사 {}에 `{name}` 인자가 필요합니다", self.act))
    }

    /// 문자열 배열 인자. 없으면 빈 목록이다.
    pub fn list(&self, name: &str) -> Vec<String> {
        self.args
            .get(name)
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }
}

pub fn parse(text: &str) -> Result<Scenario, String> {
    toml::from_str(text).map_err(|e| e.to_string())
}

pub fn load(path: &Path) -> Result<Scenario, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}
