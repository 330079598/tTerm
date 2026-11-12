//! Marks on tabs the user is not looking at, from `src/lib/tabAttention.ts`.
//!
//! The TypeScript module keeps its marks in module globals and exposes React
//! hooks. Native code owns one [`TabAttentionStore`] and queries it directly;
//! the GPUI workspace view subscribes through its own update mechanism.

use indexmap::IndexMap;

/// How strongly a tab asks for attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TabAttentionLevel {
    /// The tab has new output.
    Activity,
    /// The terminal bell rang in the tab.
    Bell,
    /// The tab raised a system notification.
    Notification,
}

fn attention_rank(level: TabAttentionLevel) -> u8 {
    match level {
        TabAttentionLevel::Activity => 1,
        TabAttentionLevel::Bell => 2,
        TabAttentionLevel::Notification => 3,
    }
}

/// The stronger of two attention levels.
pub fn stronger_attention(
    current: Option<TabAttentionLevel>,
    next: TabAttentionLevel,
) -> TabAttentionLevel {
    match current {
        Some(current) if attention_rank(current) >= attention_rank(next) => current,
        _ => next,
    }
}

/// What an AI agent is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentActivity {
    /// The agent is working.
    Processing,
    /// The agent waits for the user.
    Waiting,
}

/// What a tab shows while an agent in it is busy or asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatus {
    /// The agent's name, such as `claude` or `codex`.
    pub agent: String,
    /// What the agent is doing.
    pub activity: AgentActivity,
}

/// How long a command must run before its tab shows it.
pub const RUNNING_COMMAND_DELAY_MS: u64 = 1000;

#[derive(Debug, Clone)]
struct PendingCommand {
    command: String,
    due_ms: u64,
}

type NotificationListener = Box<dyn FnMut(u64)>;

/// Marks, notifications and agent state kept for the whole workspace.
#[derive(Default)]
pub struct TabAttentionStore {
    marks: IndexMap<String, TabAttentionLevel>,
    unseen_notifications: IndexMap<String, u64>,
    agent_statuses: IndexMap<String, AgentStatus>,
    running_agents: IndexMap<String, String>,
    running_commands: IndexMap<String, String>,
    pending_commands: IndexMap<String, PendingCommand>,
    password_prompts: IndexMap<String, ()>,
    listeners: Vec<(u64, NotificationListener)>,
    next_listener_id: u64,
}

impl TabAttentionStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    fn emit(&mut self) {
        let total = self.unseen_notification_total();
        for (_, listener) in &mut self.listeners {
            listener(total);
        }
    }

    /// Raises a tab's mark to at least `level`. Returns whether the mark
    /// changed.
    pub fn mark_tab_attention(&mut self, tab_id: &str, level: TabAttentionLevel) -> bool {
        let current = self.marks.get(tab_id).copied();
        let next = stronger_attention(current, level);
        if current == Some(next) {
            return false;
        }
        self.marks.insert(tab_id.to_owned(), next);
        self.emit();
        true
    }

    /// Answers a tab's mark and unseen notifications because the user looked
    /// at it. Returns whether anything was cleared.
    pub fn clear_tab_attention(&mut self, tab_id: &str) -> bool {
        if !self.marks.contains_key(tab_id) && !self.unseen_notifications.contains_key(tab_id) {
            return false;
        }
        self.marks.shift_remove(tab_id);
        self.unseen_notifications.shift_remove(tab_id);
        self.emit();
        true
    }

    /// Counts a system notification a tab raised while the user looked away.
    /// Returns whether the count changed, which is always true.
    pub fn count_tab_notification(&mut self, tab_id: &str) -> bool {
        *self
            .unseen_notifications
            .entry(tab_id.to_owned())
            .or_insert(0) += 1;
        self.emit();
        true
    }

    /// The number of notifications no one has looked at yet.
    pub fn unseen_notification_total(&self) -> u64 {
        self.unseen_notifications.values().sum()
    }

    /// Hears the unseen notification total now and whenever it changes.
    /// Returns an id for [`Self::stop_watching_unseen_notification_total`].
    pub fn watch_unseen_notification_total(&mut self, listener: impl Fn(u64) + 'static) -> u64 {
        let mut last = self.unseen_notification_total();
        listener(last);
        let id = self.next_listener_id;
        self.next_listener_id += 1;
        self.listeners.push((
            id,
            Box::new(move |total| {
                if total == last {
                    return;
                }
                last = total;
                listener(total);
            }),
        ));
        id
    }

    /// Stops a notification-total listener.
    pub fn stop_watching_unseen_notification_total(&mut self, id: u64) {
        self.listeners.retain(|(listener_id, _)| *listener_id != id);
    }

    /// The mark on a tab, if any.
    pub fn get_tab_attention(&self, tab_id: &str) -> Option<TabAttentionLevel> {
        self.marks.get(tab_id).copied()
    }

    /// Sets or clears what an agent in a tab is doing. Returns whether the
    /// store emitted a change; like the TypeScript setter, setting a status
    /// emits even when it is equal to the current one.
    pub fn set_tab_agent_status(&mut self, tab_id: &str, status: Option<AgentStatus>) -> bool {
        match status {
            Some(status) => {
                self.agent_statuses.insert(tab_id.to_owned(), status);
            }
            None => {
                if self.agent_statuses.shift_remove(tab_id).is_none() {
                    return false;
                }
            }
        }
        self.emit();
        true
    }

    /// Sets or clears the AI agent running in a tab. Returns whether the
    /// store emitted a change; the comparison uses the raw value, as the
    /// TypeScript setter does.
    pub fn set_tab_running_agent(&mut self, tab_id: &str, agent: Option<&str>) -> bool {
        let current = self.running_agents.get(tab_id).map(String::as_str);
        if current == agent {
            return false;
        }
        match agent.filter(|agent| !agent.is_empty()) {
            Some(agent) => {
                self.running_agents
                    .insert(tab_id.to_owned(), agent.to_owned());
            }
            None => {
                self.running_agents.shift_remove(tab_id);
            }
        }
        self.emit();
        true
    }

    /// Starts or clears the command a tab's shell runs. The command appears
    /// once [`RUNNING_COMMAND_DELAY_MS`] has passed; call [`Self::advance_to`]
    /// from the view's clock. Returns whether any stored state changed,
    /// including scheduling or replacing a pending command.
    pub fn set_tab_running_command(
        &mut self,
        tab_id: &str,
        command: Option<&str>,
        now_ms: u64,
    ) -> bool {
        let pending_changed = self.pending_commands.shift_remove(tab_id).is_some();
        let scheduled = if let Some(command) = command.filter(|command| !command.is_empty()) {
            self.pending_commands.insert(
                tab_id.to_owned(),
                PendingCommand {
                    command: command.to_owned(),
                    due_ms: now_ms.saturating_add(RUNNING_COMMAND_DELAY_MS),
                },
            );
            true
        } else {
            false
        };
        let running_changed = self.running_commands.shift_remove(tab_id).is_some();
        if running_changed {
            self.emit();
        }
        pending_changed || scheduled || running_changed
    }

    /// The earliest time a pending command becomes visible, if any. A view
    /// can schedule one wake-up at this time instead of polling.
    pub fn next_due_ms(&self) -> Option<u64> {
        self.pending_commands
            .values()
            .map(|pending| pending.due_ms)
            .min()
    }

    /// Applies pending command timers up to `now_ms`. Returns whether any
    /// command became visible.
    pub fn advance_to(&mut self, now_ms: u64) -> bool {
        let due: Vec<(String, String)> = self
            .pending_commands
            .iter()
            .filter(|(_, pending)| pending.due_ms <= now_ms)
            .map(|(tab_id, pending)| (tab_id.clone(), pending.command.clone()))
            .collect();
        let changed = !due.is_empty();
        for (tab_id, command) in due {
            self.pending_commands.shift_remove(&tab_id);
            self.running_commands.insert(tab_id, command);
            self.emit();
        }
        changed
    }

    /// Whether the tab's session waits at a password prompt. Returns whether
    /// the state changed.
    pub fn set_tab_password_prompt(&mut self, tab_id: &str, waiting: bool) -> bool {
        let current = self.password_prompts.contains_key(tab_id);
        if current == waiting {
            return false;
        }
        if waiting {
            self.password_prompts.insert(tab_id.to_owned(), ());
        } else {
            self.password_prompts.shift_remove(tab_id);
        }
        self.emit();
        true
    }

    /// The strongest mark on any of the tabs.
    pub fn tab_attention(&self, tab_ids: &[String]) -> Option<TabAttentionLevel> {
        tab_ids.iter().fold(None, |level, tab_id| {
            match self.marks.get(tab_id).copied() {
                Some(mark) => Some(stronger_attention(level, mark)),
                None => level,
            }
        })
    }

    /// What an agent does in the first tab that has one.
    pub fn tab_agent_status(&self, tab_ids: &[String]) -> Option<&AgentStatus> {
        tab_ids
            .iter()
            .find_map(|tab_id| self.agent_statuses.get(tab_id))
    }

    /// The AI agent running in the first tab that has one.
    pub fn tab_running_agent(&self, tab_ids: &[String]) -> Option<&str> {
        tab_ids
            .iter()
            .find_map(|tab_id| self.running_agents.get(tab_id).map(String::as_str))
    }

    /// Whether any of the tabs waits at a password prompt.
    pub fn tab_password_prompt(&self, tab_ids: &[String]) -> bool {
        tab_ids
            .iter()
            .any(|tab_id| self.password_prompts.contains_key(tab_id))
    }

    /// The command each tab's shell runs, keyed by tab id.
    pub fn running_commands(&self) -> &IndexMap<String, String> {
        &self.running_commands
    }
}
