//! 파일 계층(설계 6절). 바깥 프로세스를 띄우지 않고 `fullops-ext`를 모른다.

pub mod files;
pub mod lock;
pub mod repo;
pub mod search;
pub mod session;
pub mod state;
pub mod trace;
pub mod turns;

pub use files::{
    DepsError, append_archive, append_line, archive_keys, archive_landed, baseline, fixed_bin,
    fullops_home, home, load_config, load_deps, now, parse_config, parse_deps, read_archive,
    read_current, read_deps_state, read_inbox, read_json, read_lint_pass, read_plan, read_route,
    read_text, write_atomic, write_current, write_deps_state, write_json, write_route,
};
pub use fullops_core::model::{DepsState, Derived, HostInstall, path_text, same_path};
pub use lock::{CLI_WAIT, HOOK_WAIT, LockGuard, StoreError};
pub use repo::{Repo, discover, head, legacy_marker, legacy_root};
pub use search::{update_index, write_explore_result};
pub use session::{create_session, read_session, update_session};
pub use state::{quarantine_state, read_state, update_state};
pub use trace::append_trace;
pub use turns::{append_turn, read_turns};
