//! P1-05 operation lifetimes shared by host keys, authentication and proxies.

use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tterm_core::prompts::{PromptBroker, PromptOwner, SessionKey};

/// Cancels even when an outer timeout or task abort drops the connection future.
pub(crate) struct PromptScope {
    pub broker: Arc<PromptBroker>,
    pub owner: PromptOwner,
}

impl PromptScope {
    pub fn new(broker: Arc<PromptBroker>, session: Option<SessionKey>) -> Result<Self, String> {
        let owner = broker.register_owner(session)?;
        Ok(Self { broker, owner })
    }

    /// Side connections inherit a live terminal generation when one exists.
    pub async fn for_tab(app: &AppHandle, tab_id: &str) -> Result<Self, String> {
        let broker = app.state::<Arc<PromptBroker>>().inner().clone();
        if let Some(sessions) = app.try_state::<crate::core::state::PtyMap>() {
            let sessions = sessions.read().await;
            let session = sessions.get(tab_id).map(|session| SessionKey {
                tab_id: tab_id.to_string(),
                session_nonce: session.session_nonce,
            });
            return Self::new(broker, session);
        }
        Self::new(broker, None)
    }
}

impl Drop for PromptScope {
    fn drop(&mut self) {
        self.broker.cancel_owner(&self.owner);
    }
}

pub(crate) fn reveal(app: &AppHandle) {
    let window = app.state::<Arc<dyn tterm_platform::window::MainWindow>>();
    let _ = window.reveal_for_prompt();
}

/// Shares host-key request transport while callers retain verification and saving.
pub(crate) fn host_key(
    broker: &PromptBroker,
    owner: &PromptOwner,
    tab_id: &str,
    payload: tterm_core::events::ssh::SshHostKeyPromptPayload,
) -> Result<tterm_core::prompts::PromptWait, String> {
    broker.begin(
        tterm_core::prompts::PromptRequest {
            id: tterm_core::prompts::PromptId(payload.request_id.clone()),
            kind: tterm_core::prompts::PromptKind::HostKey,
            event: tterm_core::events::AppEvent::Ssh(
                tterm_core::events::ssh::SshEvent::HostKeyPrompt {
                    tab_id: tab_id.to_string(),
                    payload,
                },
            ),
            expired_event: None,
        },
        tterm_core::prompts::PromptOptions {
            owner: owner.clone(),
            timeout: crate::ssh::types::HOST_KEY_PROMPT_TIMEOUT,
        },
    )
}
