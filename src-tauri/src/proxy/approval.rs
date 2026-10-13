//! A proxy command runs a program on this device, so one that came from
//! elsewhere (synced from another device, restored from a backup or imported
//! from an SSH config file) runs only after the user allows it here. A
//! command typed in this device's settings is allowed as it is saved.
//!
//! What was allowed is kept in this device's database by the SHA-256 of the
//! command, outside sync and backups, so a changed command asks again.

use crate::core::blocking::run_blocking;
use crate::db::sql_error;
use crate::ssh::types::SshConnectError;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::oneshot;

/// As long as the host key and sign-in prompts wait for an answer.
const PROMPT_TIMEOUT: Duration = Duration::from_secs(120);
const PROMPT_EVENT: &str = "proxy-command-prompt";
const PROMPT_EXPIRED_EVENT: &str = "proxy-command-prompt-expired";

/// A question the user is asked before a proxy command first runs here.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyCommandPrompt {
    /// The SHA-256 of the command, so that connections waiting on the same
    /// command share one question.
    pub request_id: String,
    pub proxy_name: String,
    pub command: String,
}

struct PendingPrompt {
    prompt: ProxyCommandPrompt,
    waiters: Vec<oneshot::Sender<bool>>,
}

/// The questions waiting for an answer, by request id.
#[derive(Default, Clone)]
pub struct ProxyCommandPrompts(Arc<Mutex<HashMap<String, PendingPrompt>>>);

impl ProxyCommandPrompts {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, PendingPrompt>> {
        self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }
}

fn command_hash(command: &str) -> String {
    Sha256::digest(command.as_bytes())
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

pub(crate) fn is_allowed(connection: &Connection, command: &str) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT 1 FROM trusted_proxy_commands WHERE command_sha256 = ?1",
            [command_hash(command)],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
        .map_err(sql_error("Failed to read allowed proxy commands"))
}

pub(crate) fn allow(connection: &Connection, command: &str) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO trusted_proxy_commands (command_sha256, trusted_at) VALUES (?1, ?2) \
             ON CONFLICT(command_sha256) DO NOTHING",
            params![command_hash(command), crate::ssh::store::now_unix_ms()],
        )
        .map_err(sql_error("Failed to save the allowed proxy command"))?;
    Ok(())
}

/// Returns once `command` may run, asking the user first when this device
/// has not allowed it yet.
pub(crate) async fn ensure_allowed(
    app: &AppHandle,
    proxy_name: &str,
    command: &str,
) -> Result<(), SshConnectError> {
    let checked = command.to_string();
    let allowed =
        run_blocking(move || crate::db::read(|connection| is_allowed(connection, &checked)))
            .await
            .map_err(SshConnectError::Permanent)?;
    if allowed {
        return Ok(());
    }
    let Some(prompts) = app.try_state::<ProxyCommandPrompts>() else {
        return Err(SshConnectError::Permanent(
            "The proxy command cannot be confirmed right now.".to_string(),
        ));
    };
    let prompts = prompts.inner().clone();
    let request_id = command_hash(command);
    let (sender, receiver) = oneshot::channel();
    let ask = {
        let mut pending = prompts.lock();
        let entry = pending
            .entry(request_id.clone())
            .or_insert_with(|| PendingPrompt {
                prompt: ProxyCommandPrompt {
                    request_id: request_id.clone(),
                    proxy_name: proxy_name.to_string(),
                    command: command.to_string(),
                },
                waiters: Vec::new(),
            });
        entry.waiters.push(sender);
        (entry.waiters.len() == 1).then(|| entry.prompt.clone())
    };
    if let Some(prompt) = ask {
        let _ = app.emit_to(tauri::EventTarget::any(), PROMPT_EVENT, prompt);
        crate::background::reveal_for_prompt(app);
    }

    match tokio::time::timeout(PROMPT_TIMEOUT, receiver).await {
        Ok(Ok(true)) => Ok(()),
        Ok(_) => Err(SshConnectError::Permanent(format!(
            "The proxy command of \"{proxy_name}\" was not allowed to run on this device."
        ))),
        Err(_) => {
            if prompts.lock().remove(&request_id).is_some() {
                let _ = app.emit_to(tauri::EventTarget::any(), PROMPT_EXPIRED_EVENT, &request_id);
            }
            Err(SshConnectError::Permanent(format!(
                "The proxy command of \"{proxy_name}\" timed out waiting for you to allow it."
            )))
        }
    }
}

/// The questions still waiting, for a window that opens after they were asked.
#[tauri::command]
pub fn pending_proxy_command_prompts(
    prompts: State<'_, ProxyCommandPrompts>,
) -> Vec<ProxyCommandPrompt> {
    prompts
        .lock()
        .values()
        .map(|pending| pending.prompt.clone())
        .collect()
}

/// Answers a question; `allow` also remembers the command on this device.
#[tauri::command]
pub async fn respond_proxy_command_prompt(
    request_id: String,
    allow: bool,
    prompts: State<'_, ProxyCommandPrompts>,
) -> Result<(), String> {
    let pending = prompts
        .lock()
        .remove(&request_id)
        .ok_or_else(|| "The proxy command question has expired.".to_string())?;
    let saved = if allow {
        let command = pending.prompt.command.clone();
        run_blocking(move || crate::db::write(|transaction| self::allow(transaction, &command)))
            .await
    } else {
        Ok(())
    };
    // When the command cannot be remembered, the waiting connections stop
    // rather than run it and ask again next time.
    let allowed = allow && saved.is_ok();
    for waiter in pending.waiters {
        let _ = waiter.send(allowed);
    }
    saved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_allowed_by_its_exact_text() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(include_str!(
                "../../migrations/0010_trusted_proxy_commands.sql"
            ))
            .expect("schema");
        let command = "cloudflared access ssh --hostname %h";
        assert!(!is_allowed(&connection, command).expect("read"));
        allow(&connection, command).expect("allow");
        allow(&connection, command).expect("allowing twice is harmless");
        assert!(is_allowed(&connection, command).expect("read"));
        assert!(!is_allowed(&connection, &format!("{command} ")).expect("read"));
        assert_eq!(command_hash("").len(), 64);
    }
}
