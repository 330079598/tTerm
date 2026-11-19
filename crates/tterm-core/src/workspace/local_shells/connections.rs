//! The connection a local tab opens with, from `src/lib/localShells.ts`.
//!
//! A connection keeps its shell id, path and arguments as canonical saved
//! fields, so an id no client writes is read and written unchanged instead of
//! being reduced to a known shell kind.

use serde_json::Value;
use tterm_terminal::js_text::js_trim;

use crate::config::local_shells::LocalShell;
use crate::workspace::tab::{ConnectionType, Tab, TabConnection};

use super::AUTO_SHELL_KEY;
use super::same_path;
use super::types::LocalShellChoice;

/// The canonical field a connection keeps the shell kind in.
const SHELL_FIELD: &str = "terminalShell";
/// The canonical field a `custom` connection keeps its executable in.
const SHELL_PATH_FIELD: &str = "terminalShellCustomPath";
/// The canonical field a `custom` connection keeps its arguments in.
const SHELL_ARGS_FIELD: &str = "terminalShellCustomArgs";
/// The canonical field a `custom` connection keeps the user's shell id in.
const SHELL_ID_FIELD: &str = "terminalShellId";
/// The canonical field a `wsl` connection keeps its distribution in.
const SHELL_WSL_DISTRO_FIELD: &str = "terminalShellWslDistro";
/// The canonical field a tab keeps the generation of the session it opens in.
const SESSION_NONCE_FIELD: &str = "sessionNonce";
/// The canonical field a connection keeps the directory it starts in.
const CWD_FIELD: &str = "cwd";

/// A saved text field as JavaScript reads it: `fallback` when the key is
/// missing or null, and nothing when the key holds a value no text can equal.
fn saved_text<'a>(
    connection: Option<&'a TabConnection>,
    key: &str,
    fallback: &'a str,
) -> Option<&'a str> {
    match connection.and_then(|connection| connection.saved_field(key)) {
        None | Some(Value::Null) => Some(fallback),
        Some(Value::String(text)) => Some(text),
        Some(_) => None,
    }
}

/// `value >>> 0` for a JavaScript number: truncate toward zero, then keep the
/// remainder in the unsigned 32-bit range.
fn js_uint32(value: f64) -> u32 {
    let remainder = value.trunc() % 4_294_967_296.0;
    let unsigned = if remainder < 0.0 {
        remainder + 4_294_967_296.0
    } else {
        remainder
    };
    unsigned as u32
}

/// The session counter a tab saved, as JavaScript `tab.sessionNonce ?? 0`.
fn saved_session_nonce(tab: &Tab) -> f64 {
    match tab.saved_field(SESSION_NONCE_FIELD) {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// The directory a saved connection starts in, when it is text.
fn saved_cwd(tab: &Tab) -> Option<String> {
    tab.connection
        .as_ref()
        .and_then(|connection| connection.saved_field(CWD_FIELD))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// A local tab's connection that starts `choice` in `cwd`.
pub fn local_shell_connection(choice: &LocalShellChoice, cwd: Option<&str>) -> TabConnection {
    let mut connection = TabConnection::default();
    connection.set_connection_type(Some(ConnectionType::Terminal));
    connection.set_cwd(cwd.map(str::to_owned));
    connection.set_saved_field(SHELL_FIELD, Some(Value::String(choice.shell.clone())));
    if choice.shell == "custom" {
        connection.set_terminal_shell_custom_path(choice.custom_path.clone());
        connection.set_terminal_shell_custom_args(choice.custom_args.clone());
        connection.set_terminal_shell_id(choice.shell_id.clone());
    }
    if choice.shell == "wsl" {
        connection.set_terminal_shell_wsl_distro(choice.wsl_distro.clone());
    }
    connection
}

/// A saved local tab's connection with the current path and arguments of the
/// user's shell it starts. One whose shell was since removed keeps the path it
/// was saved with.
pub fn with_current_local_shell(
    connection: Option<&TabConnection>,
    shells: &[LocalShell],
) -> Option<TabConnection> {
    let connection = connection?;
    let id = saved_text(Some(connection), SHELL_ID_FIELD, "").unwrap_or_default();
    let shell = if id.is_empty() {
        None
    } else {
        shells.iter().find(|shell| shell.id == id)
    };
    let Some(shell) = shell else {
        return Some(connection.clone());
    };
    let mut current = connection.clone();
    current.set_saved_field(SHELL_FIELD, Some(Value::String("custom".to_string())));
    current.set_terminal_shell_custom_path(Some(js_trim(&shell.path).to_string()));
    current.set_terminal_shell_custom_args(
        Some(js_trim(&shell.args).to_string()).filter(|args| !args.is_empty()),
    );
    Some(current)
}

/// Whether a local tab's connection starts the same shell as `choice`.
pub fn starts_shell(connection: Option<&TabConnection>, choice: &LocalShellChoice) -> bool {
    let Some(shell) = saved_text(connection, SHELL_FIELD, AUTO_SHELL_KEY) else {
        return false;
    };
    if shell != choice.shell {
        return false;
    }
    if shell == "wsl" {
        let saved = saved_text(connection, SHELL_WSL_DISTRO_FIELD, "");
        return saved
            .is_some_and(|saved| saved == choice.wsl_distro.as_deref().unwrap_or_default());
    }
    if shell != "custom" {
        return true;
    }
    let saved_id = saved_text(connection, SHELL_ID_FIELD, "");
    let choice_id = choice.shell_id.as_deref().unwrap_or_default();
    match saved_id {
        Some("") if choice_id.is_empty() => {
            let saved_path = saved_text(connection, SHELL_PATH_FIELD, "");
            saved_path.is_some_and(|saved| {
                same_path(saved, choice.custom_path.as_deref().unwrap_or_default())
            })
        }
        Some(saved_id) => saved_id == choice_id,
        None => false,
    }
}

/// Whether a local tab's connection starts `shell` with its saved path and
/// arguments.
pub fn starts_as_saved(connection: Option<&TabConnection>, shell: &LocalShell) -> bool {
    let saved_path = saved_text(connection, SHELL_PATH_FIELD, "");
    let saved_args = saved_text(connection, SHELL_ARGS_FIELD, "");
    saved_path.is_some_and(|saved| same_path(saved, &shell.path))
        && saved_args.is_some_and(|saved| js_trim(saved) == js_trim(&shell.args))
}

/// A local tab started again with `choice`, in the directory its shell was in.
pub fn with_local_shell(tab: &Tab, choice: &LocalShellChoice) -> Tab {
    let cwd = saved_cwd(tab);
    let mut restarted = tab.clone();
    restarted.set_connection(Some(local_shell_connection(choice, cwd.as_deref())));
    restarted.set_session_nonce(Some(js_uint32(saved_session_nonce(tab) + 1.0)));
    restarted
}
