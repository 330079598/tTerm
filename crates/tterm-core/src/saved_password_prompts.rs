//! Saved-password offers at SSH sudo prompts.
//!
//! Prompt recognition is `tterm-terminal::sudo_prompt`. This module keeps the
//! per-session rules: a prompt is offered once, a password from the same
//! profile is never sent to another account, a refused password stays refused,
//! and asynchronous lookups cannot revive an offer for a later prompt.

use serde::{Deserialize, Serialize};
use tterm_terminal::sudo_prompt::{PasswordPromptMatch, match_password_prompt};
use tterm_terminal::terminal_command_capture::{
    TerminalCommandCaptureState, empty_command_capture_state,
};

use crate::password_feedback::next_typed_length;

/// The same prompt returning within this time after a fill means it was
/// refused.
pub const PASSWORD_REJECT_WINDOW_MS: u64 = 20_000;

/// Which saved password answers a sudo prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SudoPasswordSource {
    /// The profile's dedicated sudo password.
    Sudo,
    /// The SSH login password.
    Login,
}

impl SudoPasswordSource {
    /// Reads the value the backend returns, treating an empty or unknown
    /// value as no source, as JavaScript's truthiness check does.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "sudo" => Some(Self::Sudo),
            "login" => Some(Self::Login),
            _ => None,
        }
    }
}

/// The kind of local connection a saved prompt belongs to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PromptConnectionKind {
    /// An SSH session, where password prompts are recognized.
    Ssh,
    /// A saved local terminal, which can have a login script.
    Terminal,
    /// Another local connection.
    #[default]
    Other,
}

/// The connection fields a saved-password prompt reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PromptConnection {
    /// The connection kind.
    pub kind: PromptConnectionKind,
    /// The login account.
    pub username: Option<String>,
    /// The saved profile identifier.
    pub profile_id: Option<String>,
    /// The saved profile name.
    pub profile_name: Option<String>,
    /// The connection's login script.
    pub login_script: Option<String>,
}

/// What the saved-password bar and cursor overlay show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SavedPasswordPromptState {
    /// A saved password can answer this prompt.
    Available {
        /// The prompt text.
        prompt: String,
        /// The account named by the prompt.
        user: Option<String>,
        /// Where the saved password comes from.
        source: SudoPasswordSource,
    },
    /// The saved password was just refused.
    Rejected {
        /// The prompt text.
        prompt: String,
        /// The account named by the prompt.
        user: Option<String>,
    },
    /// The user types the password.
    Manual {
        /// The prompt text.
        prompt: String,
        /// The account named by the prompt.
        user: Option<String>,
        /// How many characters the user typed.
        typed: i64,
    },
}

/// A side effect the terminal view must perform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SavedPasswordPromptAction {
    /// Show or hide the state.
    Show(Option<SavedPasswordPromptState>),
    /// Mark or clear the tab's password-prompt badge.
    TabPrompt(bool),
    /// Tell the workspace which prompt has a saved-password offer.
    OfferChanged(Option<String>),
    /// Mark the tab as waiting on a sensitive prompt.
    SensitivePrompt,
    /// Look up the saved password source on a worker.
    LookupSource {
        /// The generation of this lookup.
        check_id: u64,
        /// The prompt text.
        prompt: String,
        /// The account named by the prompt.
        user: Option<String>,
        /// The saved profile identifier.
        profile_id: Option<String>,
        /// The saved profile name.
        profile_name: Option<String>,
    },
    /// Write the saved password through the session's input channel.
    FillThroughInput {
        /// The tab identifier.
        tab_id: String,
        /// The session generation.
        session_nonce: i64,
        /// The data the input channel receives.
        data: String,
        /// The input request kind.
        kind: String,
    },
    /// Ask the backend to write the saved password for this prompt.
    FillThroughCommand {
        /// The tab identifier.
        tab_id: String,
        /// The session generation.
        session_nonce: i64,
        /// The saved profile identifier.
        profile_id: Option<String>,
        /// The saved profile name.
        profile_name: Option<String>,
        /// The prompt text.
        prompt: String,
    },
    /// Tell the user the backend did not write the saved password.
    NotifyNotSent,
}

/// The result of a fill request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FillRequest {
    /// Whether a saved password was on offer and sent.
    pub filled: bool,
    /// The prompt the offer answered.
    pub prompt: Option<String>,
    /// Actions for the adapter.
    pub actions: Vec<SavedPasswordPromptAction>,
}

#[derive(Clone, Debug)]
struct OfferedPrompt {
    prompt: String,
}

#[derive(Clone, Debug)]
struct PasswordFill {
    prompt: String,
    at: u64,
}

/// One session's saved-password prompt state.
pub struct SavedPasswordPrompts {
    tab_id: String,
    session_nonce: i64,
    disposed: bool,
    password_prompt_check_id: u64,
    line_feed_count: u64,
    current_prompt_key: Option<String>,
    answered_prompt_key: Option<String>,
    offered_prompt: Option<OfferedPrompt>,
    saved_password_rejected: bool,
    current_prompt: Option<PasswordPromptMatch>,
    shown: Option<SavedPasswordPromptState>,
    connection: Option<PromptConnection>,
    has_input_callback: bool,
    last_password_fill: Option<PasswordFill>,
    pending_fill_through_input: Option<bool>,
    command_capture_suspended: bool,
    command_capture_state: TerminalCommandCaptureState,
}

impl SavedPasswordPrompts {
    /// Creates the prompt state for one terminal session.
    pub fn new(
        tab_id: String,
        session_nonce: i64,
        connection: Option<PromptConnection>,
        has_input_callback: bool,
    ) -> Self {
        Self {
            tab_id,
            session_nonce,
            disposed: false,
            password_prompt_check_id: 0,
            line_feed_count: 0,
            current_prompt_key: None,
            answered_prompt_key: None,
            offered_prompt: None,
            saved_password_rejected: false,
            current_prompt: None,
            shown: None,
            connection,
            has_input_callback,
            last_password_fill: None,
            pending_fill_through_input: None,
            command_capture_suspended: false,
            command_capture_state: empty_command_capture_state(),
        }
    }

    /// Replaces the connection whose password and login script are used.
    pub fn set_connection(&mut self, connection: Option<PromptConnection>) {
        self.connection = connection;
    }

    /// Sets whether the adapter has an input callback instead of the
    /// backend command.
    pub fn set_has_input_callback(&mut self, has_input_callback: bool) {
        self.has_input_callback = has_input_callback;
    }

    /// Notes a line feed in the session.
    pub fn note_line_feed(&mut self) {
        self.line_feed_count += 1;
    }

    /// Marks the session disposed so late callbacks are ignored.
    pub fn dispose(&mut self) {
        self.disposed = true;
    }

    /// The prompt key under the cursor, when one waits.
    pub fn current_prompt_key(&self) -> Option<&str> {
        self.current_prompt_key.as_deref()
    }

    /// Whether a password prompt currently waits.
    pub fn is_at_password_prompt(&self) -> bool {
        self.current_prompt_key.is_some()
    }

    /// The state most recently shown.
    pub fn shown(&self) -> Option<&SavedPasswordPromptState> {
        self.shown.as_ref()
    }

    /// The prompt answered by the most recent fill, while it may still be
    /// refused by the remote.
    pub fn last_password_fill(&self) -> Option<(&str, u64)> {
        self.last_password_fill
            .as_ref()
            .map(|fill| (fill.prompt.as_str(), fill.at))
    }

    /// Whether command-line capture is paused while a password prompt waits.
    pub fn command_capture_suspended(&self) -> bool {
        self.command_capture_suspended
    }

    /// The command-line capture state left after a password prompt or fill.
    pub fn command_capture_state(&self) -> &TerminalCommandCaptureState {
        &self.command_capture_state
    }

    /// Checks the line under the cursor after output was parsed.
    pub fn check_password_prompt(
        &mut self,
        line: &str,
        custom_patterns: &[regress::Regex],
        now_ms: u64,
    ) -> Vec<SavedPasswordPromptAction> {
        if self.disposed
            || self
                .connection
                .as_ref()
                .is_none_or(|connection| connection.kind != PromptConnectionKind::Ssh)
        {
            return Vec::new();
        }
        let known_user = self
            .connection
            .as_ref()
            .and_then(|connection| connection.username.as_deref());
        let matched = match_password_prompt(line, known_user, custom_patterns);
        let key = matched
            .as_ref()
            .map(|found| format!("{}:{}", self.line_feed_count, found.prompt));
        if key == self.current_prompt_key {
            return Vec::new();
        }
        self.current_prompt_key = key.clone();
        self.current_prompt = matched.clone();
        let mut actions = vec![SavedPasswordPromptAction::TabPrompt(matched.is_some())];
        actions.extend(self.clear_saved_password_offer());
        if let Some(matched) = matched
            && key.as_deref() != self.answered_prompt_key.as_deref()
        {
            self.handle_password_prompt(matched, now_ms, &mut actions);
        }
        actions
    }

    /// Applies a saved-password source lookup result.
    pub fn resolve_saved_password_source(
        &mut self,
        check_id: u64,
        source: Option<SudoPasswordSource>,
    ) -> Vec<SavedPasswordPromptAction> {
        if self.disposed || check_id != self.password_prompt_check_id {
            return Vec::new();
        }
        let Some(matched) = self.current_prompt.clone() else {
            return Vec::new();
        };
        let Some(source) = source else {
            return self.type_manually(matched);
        };
        self.offered_prompt = Some(OfferedPrompt {
            prompt: matched.prompt.clone(),
        });
        let state = SavedPasswordPromptState::Available {
            prompt: matched.prompt.clone(),
            user: matched.user,
            source,
        };
        self.shown = Some(state.clone());
        vec![
            SavedPasswordPromptAction::OfferChanged(Some(matched.prompt)),
            SavedPasswordPromptAction::Show(Some(state)),
        ]
    }

    /// Applies a raw source value from the backend.
    ///
    /// An absent, empty or unknown value takes the manual-password path,
    /// matching the frontend's `if (!source)` check.
    pub fn resolve_saved_password_source_wire(
        &mut self,
        check_id: u64,
        source: Option<&str>,
    ) -> Vec<SavedPasswordPromptAction> {
        self.resolve_saved_password_source(check_id, source.and_then(SudoPasswordSource::from_wire))
    }

    /// Applies a failed saved-password source lookup.
    pub fn fail_saved_password_source(&mut self, check_id: u64) -> Vec<SavedPasswordPromptAction> {
        if self.disposed || check_id != self.password_prompt_check_id {
            return Vec::new();
        }
        match self.current_prompt.clone() {
            Some(matched) => self.type_manually(matched),
            None => Vec::new(),
        }
    }

    /// Types `data` at the current prompt and counts the hidden characters.
    pub fn type_at_prompt(&mut self, data: &str) -> Vec<SavedPasswordPromptAction> {
        let matched = self.current_prompt.clone();
        let typed_before = match (matched.as_ref(), self.shown.as_ref()) {
            (Some(_), Some(SavedPasswordPromptState::Manual { typed, .. })) => *typed,
            _ => 0,
        };
        let typed = matched
            .as_ref()
            .and_then(|_| next_typed_length(typed_before, data));
        let mut actions = self.answer_current_prompt();
        if let Some(matched) = matched {
            match typed {
                Some(typed) => actions.extend(self.show_typed(&matched, typed)),
                None => {
                    self.current_prompt = None;
                    actions.push(SavedPasswordPromptAction::TabPrompt(false));
                }
            }
        }
        actions
    }

    /// Closes the offer and leaves the user to type.
    pub fn dismiss(&mut self) -> Vec<SavedPasswordPromptAction> {
        let matched = self.current_prompt.clone();
        let mut actions = self.answer_current_prompt();
        if let Some(matched) = matched {
            actions.extend(self.show_typed(&matched, 0));
        }
        actions
    }

    /// Sends the offered saved password.
    pub fn fill(&mut self) -> FillRequest {
        let Some(offer) = self.offered_prompt.clone() else {
            return FillRequest {
                filled: false,
                prompt: None,
                actions: Vec::new(),
            };
        };
        let mut actions = self.answer_current_prompt();
        self.command_capture_state = empty_command_capture_state();
        self.command_capture_suspended = false;
        let connection = self.connection.clone().unwrap_or_default();
        self.pending_fill_through_input = Some(self.has_input_callback);
        if self.has_input_callback {
            actions.push(SavedPasswordPromptAction::FillThroughInput {
                tab_id: self.tab_id.clone(),
                session_nonce: self.session_nonce,
                data: offer.prompt.clone(),
                kind: "saved-password".to_string(),
            });
        } else {
            actions.push(SavedPasswordPromptAction::FillThroughCommand {
                tab_id: self.tab_id.clone(),
                session_nonce: self.session_nonce,
                profile_id: connection.profile_id,
                profile_name: connection.profile_name,
                prompt: offer.prompt.clone(),
            });
        }
        FillRequest {
            filled: true,
            prompt: Some(offer.prompt),
            actions,
        }
    }

    /// Records whether the backend accepted the fill.
    ///
    /// A refused write over the backend command path also needs the user to be
    /// told; the input callback path reports its own failures.
    pub fn note_password_written(
        &mut self,
        written: bool,
        now_ms: u64,
        prompt: &str,
    ) -> Vec<SavedPasswordPromptAction> {
        let through_input = self.pending_fill_through_input.take();
        if written && !self.disposed {
            self.last_password_fill = Some(PasswordFill {
                prompt: prompt.to_string(),
                at: now_ms,
            });
        }
        if !written && through_input == Some(false) {
            return vec![SavedPasswordPromptAction::NotifyNotSent];
        }
        Vec::new()
    }

    /// The login script commands to type, if this connection has any.
    pub fn login_script_lines(&self) -> Option<Vec<String>> {
        let connection = self.connection.as_ref()?;
        if !matches!(
            connection.kind,
            PromptConnectionKind::Ssh | PromptConnectionKind::Terminal
        ) {
            return None;
        }
        let lines = crate::login_script::parse_login_script(connection.login_script.as_deref());
        (!lines.is_empty()).then_some(lines)
    }

    fn handle_password_prompt(
        &mut self,
        matched: PasswordPromptMatch,
        now_ms: u64,
        actions: &mut Vec<SavedPasswordPromptAction>,
    ) {
        self.command_capture_suspended = true;
        self.command_capture_state = empty_command_capture_state();
        if let Some(fill) = self.last_password_fill.take()
            && fill.prompt == matched.prompt
            && now_ms.saturating_sub(fill.at) < PASSWORD_REJECT_WINDOW_MS
        {
            self.saved_password_rejected = true;
            self.shown = Some(SavedPasswordPromptState::Rejected {
                prompt: matched.prompt.clone(),
                user: matched.user.clone(),
            });
            actions.push(SavedPasswordPromptAction::Show(self.shown.clone()));
            actions.push(SavedPasswordPromptAction::SensitivePrompt);
            return;
        }
        let connection = self.connection.clone().unwrap_or_default();
        let user_matches = match matched.user.as_deref() {
            None => true,
            Some(user) => connection.username.as_deref() == Some(user),
        };
        let has_profile = connection
            .profile_id
            .as_deref()
            .is_some_and(|value| !value.is_empty())
            || connection
                .profile_name
                .as_deref()
                .is_some_and(|value| !value.is_empty());
        if self.saved_password_rejected || !user_matches || !has_profile {
            actions.extend(self.type_manually(matched));
            return;
        }
        self.password_prompt_check_id += 1;
        actions.push(SavedPasswordPromptAction::LookupSource {
            check_id: self.password_prompt_check_id,
            prompt: matched.prompt,
            user: matched.user,
            profile_id: connection.profile_id,
            profile_name: connection.profile_name,
        });
    }

    fn type_manually(&mut self, matched: PasswordPromptMatch) -> Vec<SavedPasswordPromptAction> {
        let mut actions = vec![SavedPasswordPromptAction::SensitivePrompt];
        actions.extend(self.show_typed(&matched, 0));
        actions
    }

    fn show_typed(
        &mut self,
        matched: &PasswordPromptMatch,
        typed: i64,
    ) -> Vec<SavedPasswordPromptAction> {
        let state = SavedPasswordPromptState::Manual {
            prompt: matched.prompt.clone(),
            user: matched.user.clone(),
            typed,
        };
        self.shown = Some(state.clone());
        vec![SavedPasswordPromptAction::Show(Some(state))]
    }

    fn clear_saved_password_offer(&mut self) -> Vec<SavedPasswordPromptAction> {
        self.password_prompt_check_id += 1;
        let mut actions = Vec::new();
        if self.offered_prompt.take().is_some() {
            actions.push(SavedPasswordPromptAction::OfferChanged(None));
        }
        self.shown = None;
        actions.push(SavedPasswordPromptAction::Show(None));
        actions
    }

    fn answer_current_prompt(&mut self) -> Vec<SavedPasswordPromptAction> {
        if self.current_prompt_key.is_some() {
            self.answered_prompt_key = self.current_prompt_key.clone();
        }
        self.clear_saved_password_offer()
    }
}

#[cfg(test)]
mod tests;
