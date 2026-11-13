//! fullops의 순수 로직. 파일·프로세스·시계에 직접 닿지 않는다(설계 10.1절).

pub mod config;
pub mod derive;
pub mod facts;
pub mod flow;
pub mod frontmatter;
pub mod lint_rules;
pub mod messages;
pub mod model;
pub mod patch;
pub mod plan;
pub mod review;
pub mod rules;
pub mod shell;

pub use config::{PolicyError, PolicyKind, is_code};
pub use derive::{ready, stage_of};
pub use facts::Facts;
pub use flow::{check_dispatch, check_task_new, metrics, record_completion};
pub use frontmatter::parse_front_matter;
pub use plan::parse_plan;
pub use review::review_need;
pub use rules::{Effect, Outcome, decide};
