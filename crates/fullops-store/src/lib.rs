//! 파일 계층(설계 6절). 바깥 프로세스를 띄우지 않고 `fullops-ext`를 모른다.

pub mod files;
pub mod lock;
pub mod repo;
pub mod session;
pub mod state;
pub mod trace;
pub mod turns;

pub use files::{
    append_archive, append_line, archive_keys, load_config, now, parse_config, read_archive,
    read_current, read_inbox, read_json, read_lint_pass, read_plan, read_route, read_text,
    write_atomic, write_current, write_json, write_route,
};
pub use lock::{CLI_WAIT, HOOK_WAIT, LockGuard, StoreError};
pub use repo::{Repo, discover, head};
pub use session::{create_session, read_session, update_session};
pub use state::{quarantine_state, read_state, update_state};
pub use trace::append_trace;
pub use turns::{append_turn, read_turns};
