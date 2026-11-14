//! P1-05 operation lifetimes shared by host keys, authentication and proxies.

use crate::core::state::PtyMap;
use crate::prompts::{PromptBroker, PromptOwner, SessionKey};
use std::sync::Arc;
use tterm_platform::window::MainWindow;

/// Cancels even when an outer timeout or task abort drops the connection future.
pub struct PromptScope {
    pub broker: Arc<PromptBroker>,
    pub owner: PromptOwner,
}

impl PromptScope {
    pub fn new(broker: Arc<PromptBroker>, session: Option<SessionKey>) -> Result<Self, String> {
        let owner = broker.register_owner(session)?;
        Ok(Self { broker, owner })
    }

    /// Side connections require a live terminal generation; standalone operations use new.
    pub async fn for_tab(
        broker: Arc<PromptBroker>,
        sessions: &PtyMap,
        tab_id: &str,
    ) -> Result<Self, String> {
        let sessions = sessions.read().await;
        let session = sessions.get(tab_id).ok_or("Prompt session is closed")?;
        Self::new(
            broker,
            Some(SessionKey {
                tab_id: tab_id.to_string(),
                session_nonce: session.session_nonce,
            }),
        )
    }
}

impl Drop for PromptScope {
    fn drop(&mut self) {
        self.broker.cancel_owner(&self.owner);
    }
}

pub(crate) fn reveal(window: &dyn MainWindow) {
    let _ = window.reveal_for_prompt();
}

/// Shares host-key request transport while callers retain verification and saving.
pub fn host_key(
    broker: &PromptBroker,
    owner: &PromptOwner,
    tab_id: &str,
    payload: crate::events::ssh::SshHostKeyPromptPayload,
) -> Result<crate::prompts::PromptWait, String> {
    let request_id = payload.request_id.clone();
    broker.begin(
        crate::prompts::PromptRequest {
            id: crate::prompts::PromptId(payload.request_id.clone()),
            kind: crate::prompts::PromptKind::HostKey,
            event: crate::events::AppEvent::Ssh(crate::events::ssh::SshEvent::HostKeyPrompt {
                tab_id: tab_id.to_string(),
                payload,
            }),
            expired_event: Some(crate::events::AppEvent::Ssh(
                crate::events::ssh::SshEvent::HostKeyPromptExpired {
                    tab_id: tab_id.to_string(),
                    request_id,
                },
            )),
        },
        crate::prompts::PromptOptions {
            owner: owner.clone(),
            timeout: crate::ssh::types::HOST_KEY_PROMPT_TIMEOUT,
        },
    )
}
