//! 시험 도구(설계 12절). 임시 레포·임시 홈 빌더, 가짜 실행 파일(fake-orca, fake-host, fake-curl), `ScriptJudge`,
//! 시나리오 형식과 `rs` 구동기. 배포본에 들어가지 않는다. 다른 크레이트는 dev-dependency로만 쓴다.

pub mod driver;
pub mod fake;
pub mod host_shapes;
pub mod judge;
pub mod orca_shapes;
pub mod sandbox;
pub mod scenario;

pub use driver::{Driver, StepResult, VERBS, run_scenario};
pub use judge::ScriptJudge;
pub use sandbox::{Run, Sandbox, Setup, bin};
pub use scenario::{Scenario, Step};
