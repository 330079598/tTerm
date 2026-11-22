//! Pure capture and restore state for agent sessions.
//!
//! The state here has no GPUI or session handles. It decides when a live
//! report becomes the tab's saved resume record, and when a restored tab has
//! seen enough of a new shell to receive the command built for it.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tterm_core::agent_status::{
    AgentReport, AgentReportState, AgentResumeSession, AgentTracker, AgentTrackerCallbacks,
    AgentTrackerRegistry,
};

/// How long a restored tab waits for a fresh prompt before offering the
/// manual button.
pub(super) const PROMPT_TIMEOUT: Duration = Duration::from_secs(15);

/// Why automatic resume stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoticeReason {
    Timeout,
    UnknownShell,
    Unavailable,
    Unsafe,
    Failed,
}

/// The bar a tab shows after automatic resume cannot continue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Notice {
    pub reason: NoticeReason,
    /// Whether the button may send the command.
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Disabled,
    Waiting,
    Manual,
    Sent,
    Cancelled,
}

/// The restore half of one tab.
pub(super) struct Restore {
    phase: Phase,
    resume: Option<AgentResumeSession>,
    shell: Option<String>,
    at_prompt: bool,
    notice: Option<Notice>,
}

impl Restore {
    /// A restore that never runs because the setting, command line or saved
    /// workspace does not allow it.
    pub(super) fn disabled() -> Self {
        Self {
            phase: Phase::Disabled,
            resume: None,
            shell: None,
            at_prompt: false,
            notice: None,
        }
    }

    /// A restore waiting for the restored session's shell.
    pub(super) fn waiting(resume: AgentResumeSession) -> Self {
        Self {
            phase: Phase::Waiting,
            resume: Some(resume),
            shell: None,
            at_prompt: false,
            notice: None,
        }
    }

    /// The saved record this restore would send, if any.
    pub(super) fn resume(&self) -> Option<&AgentResumeSession> {
        self.resume.as_ref()
    }

    /// The shell detected for the restored session.
    pub(super) fn shell(&self) -> Option<&str> {
        self.shell.as_deref()
    }

    /// The current notice, if automatic resume stopped.
    pub(super) fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    /// Whether a prompt can now carry the resume command.
    ///
    /// A known shell is required as well, so a prompt seen before the
    /// connection authenticated or the shell was identified cannot start the
    /// command on its own.
    pub(super) fn wants_send(&self) -> bool {
        self.phase == Phase::Waiting
            && self.at_prompt
            && self.shell.is_some()
            && self.notice.is_none()
    }

    /// Whether automatic resume is still waiting for a prompt.
    pub(super) fn is_waiting(&self) -> bool {
        self.phase == Phase::Waiting
    }

    /// Whether this tab has any restore behavior at all.
    pub(super) fn is_enabled(&self) -> bool {
        self.phase != Phase::Disabled
    }

    /// Applies the shell the session authenticated and opened.
    ///
    /// A missing shell or a shell without command marks cannot be resumed
    /// automatically. The returned notice is the one the tab should show.
    pub(super) fn ready(&mut self, shell: Option<&str>, marks: bool) -> Option<Notice> {
        if self.phase != Phase::Waiting {
            return None;
        }
        self.shell = shell.map(str::to_owned);
        let Some(shell) = self.shell.clone() else {
            return self.stop(NoticeReason::UnknownShell, false);
        };
        if !marks || shell.eq_ignore_ascii_case("cmd") {
            return self.stop(NoticeReason::Unavailable, true);
        }
        None
    }

    /// A prompt mark means the shell can accept a command.
    pub(super) fn prompt(&mut self) {
        self.at_prompt = true;
    }

    /// Output started, so the shell is no longer at the prompt.
    pub(super) fn command_started(&mut self) {
        self.at_prompt = false;
    }

    /// The first prompt did not arrive in time.
    pub(super) fn timeout(&mut self) -> Option<Notice> {
        if self.phase != Phase::Waiting || self.at_prompt {
            return None;
        }
        self.stop(NoticeReason::Timeout, true)
    }

    /// The user started typing or the session went away.
    ///
    /// Returns whether anything changed and the tab should repaint.
    pub(super) fn cancel(&mut self) -> bool {
        if matches!(self.phase, Phase::Disabled | Phase::Sent | Phase::Cancelled) {
            return false;
        }
        self.phase = Phase::Cancelled;
        self.notice = None;
        true
    }

    /// The command was handed to the session.
    pub(super) fn sent(&mut self) {
        self.phase = Phase::Sent;
        self.notice = None;
    }

    /// The command could not be built or written, or reports were lost.
    ///
    /// Returns whether the tab should repaint.
    pub(super) fn failed(&mut self, reason: NoticeReason, enabled: bool) -> bool {
        self.stop(reason, enabled).is_some()
    }

    fn stop(&mut self, reason: NoticeReason, enabled: bool) -> Option<Notice> {
        if matches!(self.phase, Phase::Disabled | Phase::Cancelled | Phase::Sent) {
            return None;
        }
        self.phase = Phase::Manual;
        self.at_prompt = false;
        let notice = Notice { reason, enabled };
        self.notice = Some(notice.clone());
        Some(notice)
    }
}

/// The live half of one tab: ownership and the last valid resume record.
pub(super) struct Capture {
    tracker: AgentTracker,
    shell_cwd: Arc<Mutex<Option<String>>>,
    saved: Option<AgentResumeSession>,
}

impl Capture {
    /// Creates a tracker that starts from `initial`.
    pub(super) fn new(
        initial: Option<AgentResumeSession>,
        registry: Option<Arc<AgentTrackerRegistry>>,
    ) -> Self {
        let shell_cwd = Arc::new(Mutex::new(None));
        let read = Arc::clone(&shell_cwd);
        let callbacks = AgentTrackerCallbacks {
            initial_session: initial.clone(),
            shell_cwd: Some(Box::new(move || {
                read.lock().unwrap_or_else(PoisonError::into_inner).clone()
            })),
            on_status: Box::new(|_| {}),
            on_agent: Box::new(|_| {}),
            on_announce: Box::new(|_| {}),
        };
        let tracker = match registry {
            Some(registry) => AgentTracker::with_registry(registry, callbacks, 0),
            None => AgentTracker::new(callbacks, 0),
        };
        Self {
            tracker,
            shell_cwd,
            saved: initial,
        }
    }

    /// The record a save should write.
    #[cfg(test)]
    pub(super) fn saved(&self) -> Option<&AgentResumeSession> {
        self.saved.as_ref()
    }

    /// Records the directory the live shell reported.
    pub(super) fn set_cwd(&self, cwd: String) {
        *self
            .shell_cwd
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(cwd);
    }

    /// Records Enter in this tab, which lets it claim a new broadcast
    /// session the shared agent reports.
    pub(super) fn submit(&mut self, now_ms: u64) {
        self.tracker.submit(now_ms);
    }

    /// The directory the live shell last reported.
    pub(super) fn cwd(&self) -> Option<String> {
        self.shell_cwd
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Applies one agent report.
    ///
    /// `None` means the tab's record does not change. `Some(None)` clears it,
    /// and `Some(Some(_))` saves the exact session the report names.
    pub(super) fn report(
        &mut self,
        report: &AgentReport,
        now_ms: u64,
    ) -> Option<Option<AgentResumeSession>> {
        if !self.tracker.report(report, now_ms) {
            return None;
        }
        if let Some(session) = self.tracker.resume_session(report) {
            return self.store(Some(session));
        }
        if matches!(
            report.state,
            AgentReportState::Ended | AgentReportState::Idle
        ) {
            return self.store(None);
        }
        None
    }

    /// The shell prompt is back, so no agent owns a resumable session.
    pub(super) fn prompt(&mut self) -> Option<Option<AgentResumeSession>> {
        self.tracker.reset();
        self.store(None)
    }

    fn store(&mut self, next: Option<AgentResumeSession>) -> Option<Option<AgentResumeSession>> {
        if self.saved == next {
            return None;
        }
        self.saved = next.clone();
        Some(next)
    }
}
