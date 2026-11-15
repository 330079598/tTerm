//! Ties a terminal view to a core-owned session without owning the session.
//!
//! The manager lives as long as the application, the binding lives as long as
//! its tab, and an attachment lives only while a view is on screen. Dropping
//! a view drops its attachment; the binding and the core session stay, and a
//! replacement view subscribes to the same model and parser.

pub mod scoped;

#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::prompts::SessionKey;
use tterm_core::services::AppServices;
use tterm_core::workspace::tab_attention::TabAttentionStore;
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{ClipboardPolicy, Terminal};

use self::scoped::{ConnectionState, SessionScopedState};
use super::session::{RefreshWatch, Session};

/// The application-owned sessions, one binding per tab.
pub struct SessionManager {
    services: Arc<AppServices>,
    attention: Rc<RefCell<TabAttentionStore>>,
    clipboard_policy: ClipboardPolicy,
    next_nonce: AtomicU32,
    tabs: Mutex<HashMap<String, Rc<TabBinding>>>,
}

impl SessionManager {
    /// Creates a manager whose tabs share the application's services and
    /// attention store.
    pub fn new(services: Arc<AppServices>, attention: Rc<RefCell<TabAttentionStore>>) -> Self {
        Self {
            services,
            attention,
            clipboard_policy: ClipboardPolicy::default(),
            next_nonce: AtomicU32::new(1),
            tabs: Mutex::new(HashMap::new()),
        }
    }

    /// Sets what programs may do with OSC 52 before any tab opens.
    pub fn with_clipboard_policy(mut self, clipboard_policy: ClipboardPolicy) -> Self {
        self.clipboard_policy = clipboard_policy;
        self
    }

    /// Opens a new generation for `tab_id`, replacing its old session.
    ///
    /// The returned binding exists even when opening fails, so the tab can
    /// keep its reset start-failure state and try again. An existing view is
    /// not attached to the new generation automatically; it attaches through
    /// [`TabBinding::attach`].
    pub fn open(
        &self,
        tab_id: impl Into<String>,
        connection: PtyConnectionOptions,
        grid: GridSize,
        cell: CellSize,
    ) -> Rc<TabBinding> {
        let tab_id = tab_id.into();
        let binding = self.tab_or_create(tab_id.clone());
        let session_nonce = self.next_nonce.fetch_add(1, Ordering::Relaxed);
        binding.begin_session(session_nonce);
        if let Some(previous) = binding.take_session() {
            previous.close();
        }
        match Session::spawn(
            Arc::clone(&self.services),
            tab_id,
            session_nonce,
            connection,
            grid,
            cell,
            self.clipboard_policy,
        ) {
            Ok(session) => binding.set_session(Arc::new(session)),
            Err(error) => {
                binding.set_start_error(session_nonce, Some(error));
            }
        }
        binding
    }

    /// The binding for a tab, when it has been opened.
    pub fn tab(&self, tab_id: &str) -> Option<Rc<TabBinding>> {
        self.tabs().get(tab_id).map(Rc::clone)
    }

    /// Closes a tab and releases every backend state item scoped to it.
    pub fn close_tab(&self, tab_id: &str) -> Result<(), String> {
        let binding = self.tabs().remove(tab_id);
        if let Some(binding) = binding {
            binding.close_session();
        }
        self.attention.borrow_mut().clear_tab_attention(tab_id);
        self.services.close_tab(tab_id)
    }

    /// Closes every tab, as application shutdown does.
    pub fn close_all(&self) {
        let tabs: Vec<(String, Rc<TabBinding>)> = self.tabs().drain().collect();
        for (tab_id, binding) in tabs {
            binding.close_session();
            self.attention.borrow_mut().clear_tab_attention(&tab_id);
            let _ = self.services.close_tab(&tab_id);
        }
    }

    fn tab_or_create(&self, tab_id: String) -> Rc<TabBinding> {
        let mut tabs = self.tabs();
        if let Some(binding) = tabs.get(&tab_id) {
            return Rc::clone(binding);
        }
        let binding = Rc::new(TabBinding::new(tab_id.clone(), Rc::clone(&self.attention)));
        tabs.insert(tab_id, Rc::clone(&binding));
        binding
    }

    fn tabs(&self) -> std::sync::MutexGuard<'_, HashMap<String, Rc<TabBinding>>> {
        self.tabs.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for SessionManager {
    fn drop(&mut self) {
        self.close_all();
    }
}

/// One tab's current session generation and its session-scoped state.
pub struct TabBinding {
    tab_id: String,
    session: Mutex<Option<Arc<Session>>>,
    scoped: Mutex<SessionScopedState>,
    attention: Rc<RefCell<TabAttentionStore>>,
    active: AtomicBool,
}

impl TabBinding {
    fn new(tab_id: String, attention: Rc<RefCell<TabAttentionStore>>) -> Self {
        Self {
            tab_id,
            session: Mutex::new(None),
            scoped: Mutex::new(SessionScopedState::new(0)),
            attention,
            active: AtomicBool::new(false),
        }
    }

    /// The tab these values belong to.
    pub fn tab_id(&self) -> &str {
        &self.tab_id
    }

    /// The current session, if opening it succeeded.
    pub fn session(&self) -> Option<Arc<Session>> {
        self.session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(Arc::clone)
    }

    /// Subscribes a view to the current session.
    pub fn attach(&self) -> Option<SessionAttachment> {
        self.session().map(|session| SessionAttachment { session })
    }

    /// Whether the tab is in sight.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Enters the tab's view: clears what the tab wanted attention for.
    ///
    /// Returns whether this was the transition from hidden to in sight.
    pub fn enter(&self) -> bool {
        if self.active.swap(true, Ordering::AcqRel) {
            return false;
        }
        if let Some(session) = self.session() {
            session.set_visible(true);
        }
        self.attention
            .borrow_mut()
            .clear_tab_attention(&self.tab_id);
        true
    }

    /// Leaves the tab's view so it stops view-specific activity.
    pub fn leave(&self) {
        self.active.store(false, Ordering::Release);
        if let Some(session) = self.session() {
            session.set_visible(false);
        }
    }

    /// The session-scoped state of the current generation.
    pub fn scoped_state(&self) -> SessionScopedState {
        self.scoped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Sets or clears the host-key question for `session_nonce`.
    pub fn set_host_key_prompt(
        &self,
        session_nonce: u32,
        prompt: Option<tterm_core::events::ssh::SshHostKeyPromptPayload>,
    ) -> bool {
        self.scoped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .set_host_key_prompt(session_nonce, prompt)
    }

    /// Sets the connection status for `session_nonce`.
    pub fn set_connection_state(&self, session_nonce: u32, state: ConnectionState) -> bool {
        self.scoped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .set_connection_state(session_nonce, state)
    }

    /// Sets or clears the SSH progress message for `session_nonce`.
    pub fn set_connection_progress(
        &self,
        session_nonce: u32,
        progress: Option<tterm_core::events::ssh::SshConnectionProgressPayload>,
    ) -> bool {
        self.scoped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .set_connection_progress(session_nonce, progress)
    }

    /// Sets or clears the local-shell start failure for `session_nonce`.
    pub fn set_start_error(&self, session_nonce: u32, error: Option<String>) -> bool {
        self.scoped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .set_start_error(session_nonce, error)
    }

    fn begin_session(&self, session_nonce: u32) {
        self.scoped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .begin(session_nonce);
    }

    fn take_session(&self) -> Option<Arc<Session>> {
        self.session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    fn set_session(&self, session: Arc<Session>) {
        session.set_visible(self.active.load(Ordering::Acquire));
        *self.session.lock().unwrap_or_else(PoisonError::into_inner) = Some(session);
    }

    fn close_session(&self) {
        if let Some(session) = self.take_session() {
            session.close();
        }
    }
}

impl Drop for TabBinding {
    fn drop(&mut self) {
        self.close_session();
    }
}

/// A view's subscription to one session generation.
///
/// The attachment does not own the session. Dropping it leaves the session in
/// its binding, so the model keeps parsing and a later attachment sees the
/// complete content.
pub struct SessionAttachment {
    session: Arc<Session>,
}

impl SessionAttachment {
    /// The attached session.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// The generation this attachment belongs to.
    pub fn key(&self) -> &SessionKey {
        self.session.key()
    }

    /// The generation number.
    pub fn session_nonce(&self) -> u32 {
        self.session.session_nonce()
    }

    /// The model a new view draws.
    pub fn model(&self) -> Arc<Terminal> {
        self.session.model()
    }

    /// The ordered input path shared by keyboard, clipboard and prompts.
    pub fn input(&self) -> impl Fn(&[u8]) + 'static {
        self.session.input()
    }

    /// The resize path that reaches the PTY or SSH channel.
    pub fn resizer(&self) -> impl Fn(GridSize, CellSize) + 'static {
        self.session.resizer()
    }

    /// Wake-ups for new output.
    pub fn subscribe(&self) -> RefreshWatch {
        self.session.subscribe()
    }

    /// The latest title a program set.
    pub fn title(&self) -> Arc<Mutex<Option<String>>> {
        self.session.title()
    }

    /// Applies already-routed output only from this live generation.
    pub fn accept_output(&self, session_nonce: u32, bytes: &[u8]) -> bool {
        self.session.accept_output(session_nonce, bytes)
    }
}
