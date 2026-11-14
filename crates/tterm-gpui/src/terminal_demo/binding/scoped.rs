//! State that belongs to one terminal session generation.

use tterm_core::events::ssh::{SshConnectionProgressPayload, SshHostKeyPromptPayload};

/// The connection status shown for a tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConnectionState {
    /// The session is being opened.
    #[default]
    Connecting,
    /// The shell or SSH channel is ready.
    Connected,
    /// The session ended.
    Disconnected,
    /// An automatic reconnect is in progress.
    Reconnecting,
    /// Opening the session failed.
    Error,
}

/// What a session starts over on, keyed by its generation number.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionScopedState {
    session_nonce: u32,
    host_key_prompt: Option<SshHostKeyPromptPayload>,
    connection_state: ConnectionState,
    connection_progress: Option<SshConnectionProgressPayload>,
    start_error: Option<String>,
}

impl SessionScopedState {
    /// The default state of a session generation.
    pub fn new(session_nonce: u32) -> Self {
        Self {
            session_nonce,
            host_key_prompt: None,
            connection_state: ConnectionState::Connecting,
            connection_progress: None,
            start_error: None,
        }
    }

    /// Starts over when the session generation changes.
    ///
    /// A view may receive a late prompt or progress event from the generation
    /// it just left. Setters check the generation, and this reset clears what
    /// the old generation left behind.
    pub fn begin(&mut self, session_nonce: u32) -> bool {
        if self.session_nonce == session_nonce {
            return false;
        }
        *self = Self::new(session_nonce);
        true
    }

    /// The generation these values belong to.
    pub fn session_nonce(&self) -> u32 {
        self.session_nonce
    }

    /// The host-key question waiting for an answer.
    pub fn host_key_prompt(&self) -> Option<&SshHostKeyPromptPayload> {
        self.host_key_prompt.as_ref()
    }

    /// The current connection status.
    pub fn connection_state(&self) -> ConnectionState {
        self.connection_state
    }

    /// The latest SSH progress message.
    pub fn connection_progress(&self) -> Option<&SshConnectionProgressPayload> {
        self.connection_progress.as_ref()
    }

    /// Why a local shell could not start.
    pub fn start_error(&self) -> Option<&str> {
        self.start_error.as_deref()
    }

    /// Sets or clears the host-key question for this generation.
    pub fn set_host_key_prompt(
        &mut self,
        session_nonce: u32,
        prompt: Option<SshHostKeyPromptPayload>,
    ) -> bool {
        if session_nonce != self.session_nonce {
            return false;
        }
        self.host_key_prompt = prompt;
        true
    }

    /// Sets the connection status for this generation.
    pub fn set_connection_state(&mut self, session_nonce: u32, state: ConnectionState) -> bool {
        if session_nonce != self.session_nonce {
            return false;
        }
        self.connection_state = state;
        true
    }

    /// Sets or clears the SSH progress message for this generation.
    pub fn set_connection_progress(
        &mut self,
        session_nonce: u32,
        progress: Option<SshConnectionProgressPayload>,
    ) -> bool {
        if session_nonce != self.session_nonce {
            return false;
        }
        self.connection_progress = progress;
        true
    }

    /// Sets or clears the local shell start failure for this generation.
    pub fn set_start_error(&mut self, session_nonce: u32, error: Option<String>) -> bool {
        if session_nonce != self.session_nonce {
            return false;
        }
        self.start_error = error;
        true
    }
}
