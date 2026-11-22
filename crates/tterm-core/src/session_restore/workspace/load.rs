//! Reading a workspace from the session row without treating malformed JSON
//! as an empty session.

use rusqlite::Connection;

use crate::core::blocking::run_blocking;
use crate::db::meta;
use crate::session_restore::{SESSION_KEY, SessionData};

use super::{WorkspaceLoad, decode_session};

pub(super) async fn service() -> Result<WorkspaceLoad, String> {
    run_blocking(|| crate::db::read(from_connection)).await
}

pub(super) fn from_connection(connection: &Connection) -> Result<WorkspaceLoad, String> {
    let Some(text) = meta::get_raw(connection, SESSION_KEY)? else {
        return Ok(WorkspaceLoad::Empty);
    };
    match serde_json::from_str::<SessionData>(&text) {
        Ok(session) => Ok(decode_session(session)),
        Err(error) => Ok(WorkspaceLoad::Unreadable {
            reason: format!("The saved session is not valid JSON: {error}"),
        }),
    }
}
