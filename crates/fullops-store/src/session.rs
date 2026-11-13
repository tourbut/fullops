//! 세션 파일 읽기·쓰기(설계 6.3·6.5절). 잠금 파일은 `sessions/<sid>.json.lock`이다.

use crate::files::{read_json, write_json};
use crate::lock::{StoreError, acquire};
use crate::repo::Repo;
use fullops_core::model::{Known, SESSION_V, Session, SessionId};
use std::path::PathBuf;
use std::time::Duration;

fn lock_path(repo: &Repo, sid: &SessionId) -> PathBuf {
    let mut path = repo.session_path(sid).into_os_string();
    path.push(".lock");
    path.into()
}

/// 잠그지 않고 읽는다. 없으면 `Yes(None)`, 파손이거나 `v`가 더 크면 `Unknown`.
pub fn read_session(repo: &Repo, sid: &SessionId) -> Known<Option<Session>> {
    match read_json::<Session>(&repo.session_path(sid)) {
        Known::Yes(Some(session)) if session.v > SESSION_V => Known::Unknown("session_newer"),
        other => other,
    }
}

/// 세션 파일이 없을 때만 만든다(S-02). 만들었으면 `true`. `anchor`는 이때 한 번만 쓰인다.
pub fn create_session(repo: &Repo, session: &Session, wait: Duration) -> Result<bool, StoreError> {
    let _guard = acquire(&lock_path(repo, &session.sid), wait)?;
    let path = repo.session_path(&session.sid);
    if path.exists() {
        return Ok(false);
    }
    write_json(&path, session)?;
    Ok(true)
}

/// 잠금 안에서 다시 읽어 고치고 원자 교체한다. 세션 파일이 없으면 `StoreError::Missing`이다.
pub fn update_session<T>(
    repo: &Repo,
    sid: &SessionId,
    wait: Duration,
    f: impl FnOnce(&mut Session) -> T,
) -> Result<T, StoreError> {
    let _guard = acquire(&lock_path(repo, sid), wait)?;
    let mut session = match read_session(repo, sid) {
        Known::Yes(Some(session)) => session,
        Known::Yes(None) => return Err(StoreError::Missing),
        Known::Unknown(why) => return Err(StoreError::Corrupt(why)),
    };
    let out = f(&mut session);
    write_json(&repo.session_path(sid), &session)?;
    Ok(out)
}
