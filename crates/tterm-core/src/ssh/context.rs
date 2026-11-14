//! Concrete dependencies shared by SSH handshakes and session workers.
use crate::core::state::PtySession;
use crate::session_log::LogContext;
use crate::{events::EventSink, prompts::PromptBroker};
use std::collections::HashMap;
use std::sync::Arc;
use tterm_platform::window::MainWindow;

/// Capabilities used by a connection without access to the client application.
#[derive(Clone)]
pub struct ConnectionContext {
    pub events: Arc<dyn EventSink>,
    pub prompts: Arc<PromptBroker>,
    pub window: Arc<dyn MainWindow>,
    pub ptys: std::sync::Weak<tokio::sync::RwLock<HashMap<String, PtySession>>>,
    pub log: LogContext,
}

impl ConnectionContext {
    /// Resolves a live terminal generation without retaining its map in a worker.
    pub async fn prompt_scope(
        &self,
        tab_id: &str,
    ) -> Result<crate::prompt_scope::PromptScope, String> {
        let ptys = self.ptys.upgrade().ok_or("Prompt session is closed")?;
        crate::prompt_scope::PromptScope::for_tab(self.prompts.clone(), &ptys, tab_id).await
    }
}
