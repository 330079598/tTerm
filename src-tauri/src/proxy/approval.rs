//! A proxy command runs a program on this device, so one that came from
//! elsewhere (synced from another device, restored from a backup or imported
//! from an SSH config file) runs only after the user allows it here. A
//! command typed in this device's settings is allowed as it is saved.
//!
//! What was allowed is kept in this device's database by the SHA-256 of the
//! command, outside sync and backups, so a changed command asks again.
use tterm_core::events::proxy::ProxyCommandPrompt;
use tterm_core::events::proxy::ProxyEvent;
use tterm_core::events::AppEvent;
use tterm_core::prompts::{
    PromptBroker, PromptId, PromptKind, PromptOptions, PromptOutcome, PromptReply, PromptRequest,
};

use crate::core::blocking::run_blocking;
use crate::db::sql_error;
use crate::ssh::types::SshConnectError;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

/// As long as the host key and sign-in prompts wait for an answer.
const PROMPT_TIMEOUT: Duration = Duration::from_secs(120);

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
    owner: Option<&tterm_core::prompts::PromptOwner>,
) -> Result<(), SshConnectError> {
    let checked = command.to_string();
    let allowed =
        run_blocking(move || crate::db::read(|connection| is_allowed(connection, &checked)))
            .await
            .map_err(SshConnectError::Permanent)?;
    if allowed {
        return Ok(());
    }
    let prompts = app.state::<Arc<PromptBroker>>();
    let fallback;
    let owner = match owner {
        Some(owner) => owner,
        None => {
            fallback = crate::prompt_scope::PromptScope::new(prompts.inner().clone(), None)
                .map_err(SshConnectError::Permanent)?;
            &fallback.owner
        }
    };
    let request_id = command_hash(command);
    let wait = prompts
        .begin(
            PromptRequest {
                id: PromptId(request_id.clone()),
                kind: PromptKind::ProxyCommand,
                event: AppEvent::Proxy(ProxyEvent::CommandPrompt(ProxyCommandPrompt {
                    request_id: request_id.clone(),
                    proxy_name: proxy_name.to_string(),
                    command: command.to_string(),
                })),
                expired_event: Some(AppEvent::Proxy(ProxyEvent::CommandPromptExpired(
                    request_id,
                ))),
            },
            PromptOptions {
                owner: owner.clone(),
                timeout: PROMPT_TIMEOUT,
            },
        )
        .map_err(SshConnectError::Permanent)?;
    if wait.is_first_waiter() {
        crate::prompt_scope::reveal(app);
    }
    match wait.wait().await.map_err(SshConnectError::Permanent)? {
        PromptOutcome::Answered(PromptReply::ProxyCommand(true)) if !owner.is_closed() => Ok(()),
        PromptOutcome::TimedOut => Err(SshConnectError::Permanent(format!(
            "The proxy command of \"{proxy_name}\" timed out waiting for you to allow it."
        ))),
        _ => Err(SshConnectError::Permanent(format!(
            "The proxy command of \"{proxy_name}\" was not allowed to run on this device."
        ))),
    }
}

/// The questions still waiting, for a window that opens after they were asked.
#[tauri::command]
pub fn pending_proxy_command_prompts(
    prompts: State<'_, Arc<PromptBroker>>,
) -> Vec<ProxyCommandPrompt> {
    prompts
        .pending_proxy()
        .into_iter()
        .filter_map(|request| match request.event {
            AppEvent::Proxy(ProxyEvent::CommandPrompt(prompt)) => Some(prompt),
            _ => None,
        })
        .collect()
}

/// Answers a question; `allow` also remembers the command on this device.
#[tauri::command]
pub async fn respond_proxy_command_prompt(
    request_id: String,
    allow: bool,
    prompts: State<'_, Arc<PromptBroker>>,
) -> Result<(), String> {
    let pending = prompts.claim_proxy(&request_id)?;
    let command = match &pending.request().event {
        AppEvent::Proxy(ProxyEvent::CommandPrompt(prompt)) => prompt.command.clone(),
        _ => return Err("The proxy command question has expired.".into()),
    };
    let saved = async move {
        if allow {
            run_blocking(move || crate::db::write(|transaction| self::allow(transaction, &command)))
                .await
        } else {
            Ok(())
        }
    };
    resolve_approval(pending, allow, saved).await
}

async fn resolve_approval(
    pending: tterm_core::prompts::proxy::ProxyResolution,
    allow: bool,
    saved: impl std::future::Future<Output = Result<(), String>>,
) -> Result<(), String> {
    let saved = saved.await;
    // When the command cannot be remembered, the waiting connections stop
    // rather than run it and ask again next time.
    let allowed = allow && saved.is_ok();
    pending.complete(allowed);
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
    fn pending() -> (Arc<PromptBroker>, tterm_core::prompts::PromptWait) {
        let broker = Arc::new(PromptBroker::new(Arc::new(
            tterm_core::events::recording::RecordingEventSink::default(),
        )));
        let owner = broker.register_owner(None).unwrap();
        let wait = broker
            .begin(
                PromptRequest {
                    id: PromptId("hash".into()),
                    kind: PromptKind::ProxyCommand,
                    event: AppEvent::Proxy(ProxyEvent::CommandPrompt(ProxyCommandPrompt {
                        request_id: "hash".into(),
                        proxy_name: "p1-05".into(),
                        command: "nc %h %p".into(),
                    })),
                    expired_event: None,
                },
                PromptOptions {
                    owner,
                    timeout: Duration::from_secs(120),
                },
            )
            .unwrap();
        (broker, wait)
    }

    #[tokio::test]
    async fn approval_waits_for_persistence_and_failure_denies_execution() {
        let (broker, wait) = pending();
        let claim = broker.claim_proxy("hash").unwrap();
        let (save, saved) = tokio::sync::oneshot::channel::<Result<(), String>>();
        let task = tokio::spawn(resolve_approval(claim, true, async {
            saved.await.unwrap()
        }));
        let waiter = tokio::spawn(wait.wait());
        tokio::task::yield_now().await;
        assert!(
            !waiter.is_finished(),
            "approval must not precede the DB result"
        );
        save.send(Err("database write failed".into())).unwrap();
        assert_eq!(task.await.unwrap().unwrap_err(), "database write failed");
        assert_eq!(
            waiter.await.unwrap().unwrap(),
            PromptOutcome::Answered(PromptReply::ProxyCommand(false))
        );
    }

    #[tokio::test]
    async fn abandoned_approval_rejects_and_success_releases_only_after_save() {
        let (broker, wait) = pending();
        let claim = broker.claim_proxy("hash").unwrap();
        let task = tokio::spawn(resolve_approval(claim, true, std::future::pending()));
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(
            wait.wait().await.unwrap(),
            PromptOutcome::Answered(PromptReply::ProxyCommand(false))
        );
        let (broker, wait) = pending();
        resolve_approval(broker.claim_proxy("hash").unwrap(), true, async { Ok(()) })
            .await
            .unwrap();
        assert_eq!(
            wait.wait().await.unwrap(),
            PromptOutcome::Answered(PromptReply::ProxyCommand(true))
        );
    }
}
