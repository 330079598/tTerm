//! Notifications programs ask for, and the rules for announcing them.

use std::sync::LazyLock;

use regress::Regex;
use serde::{Deserialize, Serialize};

use tterm_terminal::command_mark::CommandMark;
use tterm_terminal::js_encoding::decode_base64_utf8;
use tterm_terminal::js_text::{is_control, js_trim, utf16_len, utf16_slice, utf16_units};

use crate::agent_status::AgentAnnouncement;

/// OSC codes that carry a notification: iTerm2's 9, urxvt's 777, kitty's 99.
pub const NOTIFICATION_OSC_CODES: [i64; 3] = [9, 777, 99];

/// A notification request with its optional title.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalNotificationRequest {
    /// The title, when the request has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The message.
    pub body: String,
}

/// Something in a terminal that may deserve the user's attention.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum TerminalAttentionEvent {
    /// A command finished.
    Command {
        /// The command line, when it is known.
        #[serde(skip_serializing_if = "Option::is_none")]
        command: Option<String>,
        /// The exit status, when the shell reported one.
        #[serde(skip_serializing_if = "Option::is_none")]
        exit_code: Option<i64>,
        /// How long it ran.
        duration_ms: f64,
    },
    /// A program asked for a notification.
    Request {
        /// The requested title.
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// The requested message.
        body: String,
    },
    /// An agent reported a state worth announcing.
    Agent(AgentAnnouncement),
    /// The terminal bell rang.
    Bell,
}

const MAX_TITLE_LENGTH: usize = 256;
const MAX_BODY_LENGTH: usize = 1024;
static CONEMU_NUMBERED_OSC9: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^\d+(;|$)").ok());

fn clean(text: &str, max_length: usize) -> String {
    let mut visible = String::with_capacity(text.len());
    let mut in_control_run = false;
    for character in text.chars() {
        if is_control(character) {
            if !in_control_run {
                visible.push(' ');
                in_control_run = true;
            }
        } else {
            visible.push(character);
            in_control_run = false;
        }
    }
    let visible = js_trim(&visible).to_owned();
    if utf16_len(&visible) > max_length {
        format!("{}…", utf16_slice(&visible, 0, max_length - 1))
    } else {
        visible
    }
}

fn request(title: Option<&str>, body: &str) -> Option<TerminalNotificationRequest> {
    let clean_title = title
        .map(|title| clean(title, MAX_TITLE_LENGTH))
        .unwrap_or_default();
    let clean_body = clean(body, MAX_BODY_LENGTH);
    if clean_title.is_empty() && clean_body.is_empty() {
        return None;
    }
    if clean_body.is_empty() {
        return Some(TerminalNotificationRequest {
            title: None,
            body: clean_title,
        });
    }
    Some(TerminalNotificationRequest {
        title: (!clean_title.is_empty()).then_some(clean_title),
        body: clean_body,
    })
}

/// OSC 9 `message`.
///
/// ConEmu numbers its own OSC 9 commands (`9;4;…` progress, `9;9;…` working
/// directory), so a payload that starts with a number and a semicolon is one
/// of those, not a message.
pub fn parse_osc9_notification(data: &str) -> Option<TerminalNotificationRequest> {
    if CONEMU_NUMBERED_OSC9
        .as_ref()
        .and_then(|pattern| pattern.find_from_ucs2(&utf16_units(data), 0).next())
        .is_some()
    {
        return None;
    }
    request(None, data)
}

/// OSC 777 `notify;title;body`; the body may itself contain semicolons.
pub fn parse_osc777_notification(data: &str) -> Option<TerminalNotificationRequest> {
    let mut parts = data.split(';');
    if parts.next()? != "notify" {
        return None;
    }
    let title = parts.next().unwrap_or_default();
    let body: Vec<&str> = parts.collect();
    request(Some(title), &body.join(";"))
}

#[derive(Clone, Debug, Default)]
struct KittyPartial {
    title: String,
    body: String,
}

/// kitty's OSC 99 `metadata;payload`.
///
/// A notification may arrive in chunks that share an `i` id, all but the last
/// with `d=0`; `p` says whether a chunk is the title (the default) or the
/// body, `e=1` that it is base64. Queries, icons, buttons and close requests
/// are ignored.
#[derive(Default)]
pub struct KittyNotificationAssembler {
    partials: Vec<(String, KittyPartial)>,
}

impl KittyNotificationAssembler {
    /// Creates an assembler with no partial notifications.
    pub fn new() -> Self {
        Self::default()
    }

    /// Handles one OSC 99 payload.
    pub fn handle(&mut self, data: &str) -> Option<TerminalNotificationRequest> {
        let (metadata_text, payload_text) = data.split_once(';')?;
        let mut metadata: Vec<(String, String)> = Vec::new();
        for pair in metadata_text.split(':') {
            if let Some((key, value)) = pair.split_once('=')
                && !key.is_empty()
            {
                metadata.push((key.to_owned(), value.to_owned()));
            }
        }
        let lookup = |name: &str| {
            metadata
                .iter()
                .rev()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        let id = lookup("i").unwrap_or_default();
        let part = lookup("p").unwrap_or("title");
        let mut payload = payload_text.to_owned();
        if lookup("e") == Some("1") {
            payload = decode_base64_utf8(&payload)?;
        }

        let mut partial = self
            .partials
            .iter()
            .find(|(key, _)| key == id)
            .map(|(_, partial)| partial.clone())
            .unwrap_or_default();
        if part == "title" {
            partial.title.push_str(&payload);
        } else if part == "body" {
            partial.body.push_str(&payload);
        } else {
            return None;
        }

        if lookup("d") == Some("0") {
            // Bounded: a program that never finishes its chunks cannot grow
            // this.
            if !self.partials.iter().any(|(key, _)| key == id) && self.partials.len() >= 16 {
                self.partials.remove(0);
            }
            if let Some(entry) = self.partials.iter_mut().find(|(key, _)| key == id) {
                entry.1 = partial;
            } else {
                self.partials.push((id.to_owned(), partial));
            }
            return None;
        }
        self.partials.retain(|(key, _)| key != id);
        if partial.body.is_empty() {
            request(None, &partial.title)
        } else {
            request(Some(&partial.title), &partial.body)
        }
    }
}

/// A command the timer finished.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishedCommand {
    /// The command line, when it is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The exit status, when the shell reported one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    /// How long the command ran, never negative.
    pub duration_ms: i64,
}

/// Times commands from shell integration marks.
///
/// A command starts at Enter on its prompt or at its output mark (C),
/// whichever comes first, and finishes at its done mark (D). A new prompt (A)
/// without a done mark forgets it.
type RunningCallback = Box<dyn FnMut(Option<&str>)>;

#[derive(Default)]
pub struct CommandTimer {
    started_at: Option<i64>,
    command: Option<String>,
    running: Option<String>,
    on_running: Option<RunningCallback>,
}

impl CommandTimer {
    /// Creates a timer without a running-command callback.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a timer whose callback hears the command the shell starts, and
    /// `None` once it is over.
    pub fn with_on_running(on_running: RunningCallback) -> Self {
        Self {
            on_running: Some(on_running),
            ..Self::default()
        }
    }

    /// Records that the shell is back at its prompt.
    pub fn prompt(&mut self) {
        self.started_at = None;
        self.command = None;
        if self.running.take().is_some()
            && let Some(callback) = self.on_running.as_mut()
        {
            callback(None);
        }
    }

    /// Records the start of a command.
    pub fn start(&mut self, now: i64) {
        if self.started_at.is_none() {
            self.started_at = Some(now);
        }
        self.report_running();
    }

    /// Takes the command line the shell reports it runs (OSC 633 E).
    pub fn set_command(&mut self, command: &str) {
        self.command = Some(command.to_owned());
        if self.running.is_none() || self.running.as_deref() == Some(command) {
            self.report_running();
            return;
        }
        self.running = Some(command.to_owned());
        if let Some(callback) = self.on_running.as_mut() {
            callback(Some(command));
        }
    }

    /// Records a line typed at the prompt.
    pub fn set_typed_command(&mut self, command: &str) {
        if self.started_at.is_some() {
            return;
        }
        self.command = Some(command.to_owned());
        self.report_running();
    }

    /// Follows a shell integration mark; a done mark returns the command it
    /// ends.
    pub fn mark(&mut self, mark: CommandMark, now: i64) -> Option<FinishedCommand> {
        match mark {
            CommandMark::Done { exit_code } => self.finish(exit_code, now),
            CommandMark::Prompt => {
                self.prompt();
                None
            }
            CommandMark::Output => {
                self.start(now);
                None
            }
        }
    }

    /// Ends the running command at `now`.
    pub fn finish(&mut self, exit_code: Option<i64>, now: i64) -> Option<FinishedCommand> {
        let started_at = self.started_at;
        let command = self.command.clone();
        self.prompt();
        let started_at = started_at?;
        Some(FinishedCommand {
            command,
            exit_code,
            duration_ms: (now - started_at).max(0),
        })
    }

    fn report_running(&mut self) {
        if self.running.is_some()
            || self.started_at.is_none()
            || !self
                .command
                .as_deref()
                .is_some_and(|command| !command.is_empty())
        {
            return;
        }
        let command = self.command.clone().unwrap_or_default();
        self.running = Some(command.clone());
        if let Some(callback) = self.on_running.as_mut() {
            callback(Some(&command));
        }
    }
}

/// How much a tab wants to be looked at, shown on its tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TabAttentionLevel {
    /// Output arrived.
    Activity,
    /// The bell rang.
    Bell,
    /// A notification arrived.
    Notification,
}

fn attention_rank(level: TabAttentionLevel) -> u8 {
    match level {
        TabAttentionLevel::Activity => 1,
        TabAttentionLevel::Bell => 2,
        TabAttentionLevel::Notification => 3,
    }
}

/// Keeps the strongest attention mark.
pub fn stronger_attention(
    current: Option<TabAttentionLevel>,
    next: TabAttentionLevel,
) -> TabAttentionLevel {
    match current {
        Some(current) if attention_rank(current) >= attention_rank(next) => current,
        _ => next,
    }
}

/// The shell-marking settings that decide whether finished commands can be
/// announced.
#[derive(Clone, Copy)]
pub struct LocalShellMarkConfig {
    /// Whether command marks are on.
    pub command_marks: bool,
    /// Whether tTerm's shell integration is on for macOS and Linux shells.
    pub local_unix_shell_integration: bool,
}

/// Whether local shells mark where their commands start and end.
pub fn local_shells_mark_commands(config: &LocalShellMarkConfig, platform: &str) -> bool {
    config.command_marks && (platform == "windows" || config.local_unix_shell_integration)
}

/// The settings that decide whether and how an event is announced.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NotificationSettings {
    /// Whether any system notification or toast may be shown.
    pub notifications_enabled: bool,
    /// Whether finished commands may be announced.
    pub notify_command_finished: bool,
    /// The shortest finished command to announce, in seconds.
    pub notify_command_min_secs: f64,
    /// Whether terminal notification requests may be announced.
    pub notify_terminal_requests: bool,
    /// Whether an agent waiting may be announced.
    pub notify_agent_waiting: bool,
    /// Whether an agent finishing may be announced.
    pub notify_agent_done: bool,
    /// Whether the bell may be announced.
    pub bell_notify: bool,
}

/// What an attention event leads to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttentionDecision {
    /// Mark on the tab, when it is out of sight.
    pub mark: Option<TabAttentionLevel>,
    /// A system notification: the window is not in front.
    pub system: bool,
    /// An in-app toast: the window is in front but the user works elsewhere.
    pub toast: bool,
}

const NOTHING: AttentionDecision = AttentionDecision {
    mark: None,
    system: false,
    toast: false,
};

/// What an event leads to.
pub fn decide_attention(
    event: &TerminalAttentionEvent,
    settings: &NotificationSettings,
    window_focused: bool,
    tab_visible: bool,
    tab_focused: bool,
) -> AttentionDecision {
    if window_focused && tab_focused {
        return NOTHING;
    }
    let mark = if tab_visible {
        None
    } else if matches!(event, TerminalAttentionEvent::Bell) {
        Some(TabAttentionLevel::Bell)
    } else {
        Some(TabAttentionLevel::Notification)
    };

    let announce = match event {
        TerminalAttentionEvent::Command { duration_ms, .. } => {
            if !settings.notify_command_finished
                || *duration_ms < settings.notify_command_min_secs * 1000.0
            {
                return NOTHING;
            }
            settings.notifications_enabled
        }
        TerminalAttentionEvent::Request { .. } => {
            if !settings.notify_terminal_requests {
                return NOTHING;
            }
            settings.notifications_enabled
        }
        TerminalAttentionEvent::Agent(announcement) => {
            let wanted =
                if announcement.state == crate::agent_status::AgentAnnouncementState::Waiting {
                    settings.notify_agent_waiting
                } else {
                    settings.notify_agent_done
                };
            if !wanted {
                return NOTHING;
            }
            settings.notifications_enabled
        }
        TerminalAttentionEvent::Bell => settings.notifications_enabled && settings.bell_notify,
    };

    AttentionDecision {
        mark,
        system: announce && !window_focused,
        toast: announce && window_focused && !matches!(event, TerminalAttentionEvent::Bell),
    }
}

/// Formats a command duration as `1h 2m`, `3m 5s` or `42s`.
pub fn format_command_duration(duration_ms: f64) -> String {
    let total_seconds = js_math_round(duration_ms / 1000.0);
    if total_seconds.is_nan() {
        return "NaNs".to_owned();
    }
    if total_seconds.is_infinite() {
        return if total_seconds.is_sign_positive() {
            "Infinityh NaNm".to_owned()
        } else {
            "NaNs".to_owned()
        };
    }
    let total_seconds = total_seconds as i64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

fn js_math_round(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    (value + 0.5).floor()
}

#[cfg(test)]
mod tests;
