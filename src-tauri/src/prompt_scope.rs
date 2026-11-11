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
        let session = if let Some(sessions) = app.try_state::<crate::core::state::PtyMap>() {
            sessions.read().await.get(tab_id).map(|session| SessionKey {
                tab_id: tab_id.to_string(),
                session_nonce: session.session_nonce,
            })
        } else {
            None
        };
        Self::new(app.state::<Arc<PromptBroker>>().inner().clone(), session)
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
