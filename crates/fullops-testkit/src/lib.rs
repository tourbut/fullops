//! 시험 도구(설계 12절). 임시 레포 빌더, fake-orca, 시나리오 형식과 `rs` 구동기.
//! 배포본에 들어가지 않는다. 다른 크레이트는 dev-dependency로만 쓴다.

pub mod driver;
pub mod orca_shapes;
pub mod sandbox;
pub mod scenario;

pub use driver::{Driver, StepResult, VERBS, run_scenario};
pub use sandbox::{Run, Sandbox, Setup, bin};
pub use scenario::{Scenario, Step};
