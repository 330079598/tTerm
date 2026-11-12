//! What AI coding agents report through tTerm's hooks.
//!
//! The protocol is OSC 777 `tterm-agent;agent;state`, with optional
//! `;session;direct|broadcast;encoded-cwd` fields. The backend that emits it
//! lives in `src-tauri/src/agent_integration/`; the field and enum strings
//! here are copied from that format verbatim.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use serde::{Deserialize, Serialize};

use tterm_terminal::js_encoding::decode_uri_component;
use tterm_terminal::js_text::{contains_control, utf16_len};

/// The state an agent reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentReportState {
    /// Working on a turn.
    Processing,
    /// Waiting for the user.
    Waiting,
    /// Finished a turn.
    Done,
    /// Failed a turn.
    Error,
    /// Stopped by the user, nothing to announce.
    Idle,
    /// The agent quit.
    Ended,
}

impl AgentReportState {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "processing" => Some(Self::Processing),
            "waiting" => Some(Self::Waiting),
            "done" => Some(Self::Done),
            "error" => Some(Self::Error),
            "idle" => Some(Self::Idle),
            "ended" => Some(Self::Ended),
            _ => None,
        }
    }
}

/// Whether a session report came straight from its terminal or was broadcast
/// to every terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentScope {
    /// The report names the terminal it belongs to.
    Direct,
    /// The report went to every terminal.
    Broadcast,
}

/// A parsed OSC 777 agent report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentReport {
    /// The agent name, such as `claude` or `codex`.
    pub agent: String,
    /// The reported state.
    pub state: AgentReportState,
    /// The session the report names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Whether the report was direct or broadcast.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<AgentScope>,
    /// The directory reported by the hook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// A restored resume record for a tab.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResumeSession {
    /// The agent that reported the session: `claudeCode`, `codex`, or the
    /// other agent name as reported.
    pub agent: String,
    /// The agent's session identifier.
    pub session_id: String,
    /// The directory at agent start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// The payload of an OSC 777, which also carries `notify` and other
/// extensions.
pub fn parse_agent_report(data: &str) -> Option<AgentReport> {
    let mut parts = data.split(';');
    let command = parts.next()?;
    if command != "tterm-agent" {
        return None;
    }
    let agent = parts.next()?;
    let state = parts.next()?;
    let session = parts.next();
    let scope = parts.next();
    let encoded_cwd = parts.next();
    let rest: Vec<&str> = parts.collect();
    if !rest.is_empty() {
        return None;
    }
    if !valid_agent(agent) {
        return None;
    }
    let state = AgentReportState::from_str(state)?;

    let session = match session {
        None => None,
        Some(session) => {
            if !valid_session(session) {
                return None;
            }
            Some(session.to_owned())
        }
    };
    let scope = match scope {
        None => None,
        Some("direct") => Some(AgentScope::Direct),
        Some("broadcast") => Some(AgentScope::Broadcast),
        Some(_) => return None,
    };
    let cwd = match encoded_cwd {
        Some(encoded) if !encoded.is_empty() => {
            let decoded = decode_uri_component(encoded)?;
            if utf16_len(&decoded) > 16_384 || contains_control(&decoded) {
                return None;
            }
            (!decoded.is_empty()).then_some(decoded)
        }
        _ => None,
    };

    Some(AgentReport {
        agent: agent.to_owned(),
        state,
        session,
        scope,
        cwd,
    })
}

fn valid_agent(agent: &str) -> bool {
    let bytes = agent.as_bytes();
    let Some(first) = bytes.first() else {
        return false;
    };
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    bytes.len() <= 32
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn valid_session(session: &str) -> bool {
    let bytes = session.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
}

/// The name a tab shows for an agent.
pub fn agent_display_name(agent: &str) -> String {
    match agent {
        "claude" => "Claude Code",
        "codex" => "Codex",
        "opencode" => "OpenCode",
        "pi" => "pi",
        _ => agent,
    }
    .to_owned()
}

/// What a tab shows while an agent in it is busy or asking.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentStatus {
    /// The agent name.
    pub agent: String,
    /// What it is doing.
    pub activity: AgentActivity,
}

/// The activity an agent shows on its tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentActivity {
    /// Working on a turn.
    Processing,
    /// Waiting for the user.
    Waiting,
}

/// The states worth announcing, from an agent report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentAnnouncementState {
    /// The agent waits for the user.
    Waiting,
    /// The turn finished.
    Done,
    /// The turn failed.
    Error,
}

/// Something an agent did that may deserve the user's attention.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAnnouncement {
    /// The agent name.
    pub agent: String,
    /// What happened.
    pub state: AgentAnnouncementState,
}

/// A wait is announced only once it lasts this long.
pub const AGENT_WAIT_ANNOUNCE_DELAY_MS: u64 = 3000;
/// How long after its last report an agent's own notifications count as
/// repeats.
const AGENT_ECHO_WINDOW_MS: u64 = 10_000;
/// How long after Enter a terminal may take a session it hears about.
pub const AGENT_CLAIM_WINDOW_MS: u64 = 10_000;

const AGENTS_REPORTING_STOPS: [&str; 3] = ["codex", "opencode", "pi"];

/// The callbacks and restored state a tracker starts with.
pub struct AgentTrackerCallbacks {
    /// A restored record already owned by this tab.
    pub initial_session: Option<AgentResumeSession>,
    /// The directory reported by the live shell at tool start.
    pub shell_cwd: Option<Box<dyn Fn() -> Option<String>>>,
    /// Receives status changes.
    pub on_status: Box<dyn FnMut(Option<AgentStatus>)>,
    /// Receives the running agent, or `None` once it ends.
    pub on_agent: Box<dyn FnMut(Option<String>)>,
    /// Receives announcements.
    pub on_announce: Box<dyn FnMut(AgentAnnouncement)>,
}

/// The process-wide session ownership shared by all trackers.
pub struct AgentTrackerRegistry {
    inner: Mutex<AgentTrackerRegistryInner>,
    next_id: AtomicU64,
}

#[derive(Default)]
struct AgentTrackerRegistryInner {
    session_owners: HashMap<String, u64>,
    recent_submissions: HashMap<u64, u64>,
}

impl AgentTrackerRegistry {
    /// Creates an isolated registry, used by tests that need separate state.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(AgentTrackerRegistryInner::default()),
            next_id: AtomicU64::new(0),
        }
    }

    fn lock(&self) -> MutexGuard<'_, AgentTrackerRegistryInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn next_tracker_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

impl Default for AgentTrackerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn global_registry() -> Arc<AgentTrackerRegistry> {
    static GLOBAL: OnceLock<Arc<AgentTrackerRegistry>> = OnceLock::new();
    GLOBAL
        .get_or_init(|| Arc::new(AgentTrackerRegistry::new()))
        .clone()
}

/// Follows the reports of the agent running in one terminal.
pub struct AgentTracker {
    id: u64,
    registry: Arc<AgentTrackerRegistry>,
    callbacks: AgentTrackerCallbacks,
    wait_delay_ms: u64,
    status: Option<AgentStatus>,
    agent: Option<String>,
    last_report_at: Option<u64>,
    wait_deadline: Option<u64>,
    wait_agent: Option<String>,
    resume: Option<AgentResumeSession>,
    submitted_at: Option<u64>,
    sessions: HashSet<String>,
    resume_claims: HashSet<String>,
}

impl AgentTracker {
    /// Creates a tracker on the process-wide registry.
    pub fn new(callbacks: AgentTrackerCallbacks, wait_delay_ms: u64) -> Self {
        Self::with_registry(global_registry(), callbacks, wait_delay_ms)
    }

    /// Creates a tracker on `registry`; tests use a fresh registry per case.
    pub fn with_registry(
        registry: Arc<AgentTrackerRegistry>,
        callbacks: AgentTrackerCallbacks,
        wait_delay_ms: u64,
    ) -> Self {
        let id = registry.next_tracker_id();
        let mut tracker = Self {
            id,
            registry,
            callbacks,
            wait_delay_ms,
            status: None,
            agent: None,
            last_report_at: None,
            wait_deadline: None,
            wait_agent: None,
            resume: None,
            submitted_at: None,
            sessions: HashSet::new(),
            resume_claims: HashSet::new(),
        };
        tracker.resume = tracker.callbacks.initial_session.clone();
        if let Some(resume) = &tracker.resume {
            let mut state = tracker.registry.lock();
            if !state.session_owners.contains_key(&resume.session_id) {
                state
                    .session_owners
                    .insert(resume.session_id.clone(), tracker.id);
                tracker.sessions.insert(resume.session_id.clone());
            }
        }
        tracker
    }

    /// An agent is at work, waiting, or has just reported: notifications it
    /// sends itself repeat its reports.
    pub fn speaks_for_agent(&self, now: u64) -> bool {
        if self.status.is_some() {
            return true;
        }
        self.last_report_at
            .is_some_and(|at| now.saturating_sub(at) < AGENT_ECHO_WINDOW_MS)
    }

    /// Enter typed in the terminal: it may take the next new session it hears
    /// of.
    pub fn submit(&mut self, now: u64) {
        self.submitted_at = Some(now);
        self.registry.lock().recent_submissions.insert(self.id, now);
    }

    /// Applies one report; returns whether the terminal that sent it owns it.
    pub fn report(&mut self, report: &AgentReport, now: u64) -> bool {
        if let Some(session) = &report.session {
            let owner = self.registry.lock().session_owners.get(session).copied();
            if owner.is_some_and(|owner| owner != self.id) {
                return false;
            }
            if report.scope == Some(AgentScope::Direct) {
                if let Some(resume) = &self.resume
                    && resume.session_id != *session
                {
                    let old = resume.session_id.clone();
                    self.release_session(&old);
                }
                self.registry
                    .lock()
                    .session_owners
                    .insert(session.clone(), self.id);
                self.sessions.insert(session.clone());
                if report.agent == "claude" || report.agent == "codex" {
                    let mapped = resume_agent_name(&report.agent);
                    let same = self.resume.as_ref().is_some_and(|resume| {
                        resume.session_id == *session && resume.agent == mapped
                    });
                    let start_cwd = if same {
                        self.resume.as_ref().and_then(|resume| resume.cwd.clone())
                    } else {
                        self.callbacks
                            .shell_cwd
                            .as_ref()
                            .and_then(|read| read())
                            .or_else(|| report.cwd.clone())
                    };
                    self.resume = Some(AgentResumeSession {
                        agent: mapped,
                        session_id: session.clone(),
                        cwd: start_cwd.filter(|cwd| !cwd.is_empty()),
                    });
                }
            } else {
                let mapped = resume_agent_name(&report.agent);
                let restored = self
                    .resume
                    .as_ref()
                    .is_some_and(|resume| resume.session_id == *session && resume.agent == mapped);
                if !restored && !self.claim(session, report.state, now) {
                    return false;
                }
                if self
                    .resume
                    .as_ref()
                    .is_some_and(|resume| resume.session_id == *session)
                    && !restored
                {
                    return false;
                }
                let shell_cwd = self.callbacks.shell_cwd.as_ref().and_then(|read| read());
                if !restored
                    && owner.is_none()
                    && self.resume_claims.contains(session)
                    && report.agent == "codex"
                    && agent_resume_cwd_matches(report.cwd.as_deref(), shell_cwd.as_deref())
                {
                    self.resume = Some(AgentResumeSession {
                        agent: mapped,
                        session_id: session.clone(),
                        cwd: shell_cwd,
                    });
                }
            }
        }

        if report.state == AgentReportState::Idle {
            self.resume = None;
            if let Some(session) = report
                .session
                .as_ref()
                .filter(|session| !session.is_empty())
            {
                self.resume_claims.remove(session);
            }
        }
        self.cancel_wait();
        if report.state == AgentReportState::Ended {
            self.reset();
            return true;
        }
        self.last_report_at = Some(now);
        self.set_agent(Some(report.agent.clone()));
        match report.state {
            AgentReportState::Idle => {
                self.set_status(None);
            }
            AgentReportState::Processing | AgentReportState::Waiting => {
                let activity = if report.state == AgentReportState::Waiting {
                    AgentActivity::Waiting
                } else {
                    AgentActivity::Processing
                };
                self.set_status(Some(AgentStatus {
                    agent: report.agent.clone(),
                    activity,
                }));
                if activity == AgentActivity::Waiting {
                    self.wait_deadline = Some(now + self.wait_delay_ms);
                    self.wait_agent = Some(report.agent.clone());
                }
            }
            AgentReportState::Done | AgentReportState::Error => {
                self.set_status(None);
                let state = if report.state == AgentReportState::Done {
                    AgentAnnouncementState::Done
                } else {
                    AgentAnnouncementState::Error
                };
                (self.callbacks.on_announce)(AgentAnnouncement {
                    agent: report.agent.clone(),
                    state,
                });
            }
            AgentReportState::Ended => {}
        }
        true
    }

    /// The resume record a report names, when it is the exact restored
    /// session.
    pub fn resume_session(&self, report: &AgentReport) -> Option<AgentResumeSession> {
        let session = report
            .session
            .as_ref()
            .filter(|session| !session.is_empty())?;
        if matches!(
            report.state,
            AgentReportState::Ended | AgentReportState::Idle
        ) {
            return None;
        }
        let saved = self.resume.as_ref()?;
        if saved.session_id != *session || saved.agent != resume_agent_name(&report.agent) {
            return None;
        }
        Some(saved.clone())
    }

    /// Esc or Ctrl+C typed in the terminal: an agent at work that does not
    /// report its stops stops and waits for the user.
    pub fn interrupt(&mut self) {
        let Some(status) = &self.status else {
            return;
        };
        if AGENTS_REPORTING_STOPS.contains(&status.agent.as_str()) {
            return;
        }
        self.cancel_wait();
        self.set_status(None);
    }

    /// Typed in the terminal, not by the terminal itself: an agent waiting
    /// for the user has its answer and works on.
    pub fn answer(&mut self) {
        let Some(status) = self.status.clone() else {
            return;
        };
        if status.activity != AgentActivity::Waiting
            || !AGENTS_REPORTING_STOPS.contains(&status.agent.as_str())
        {
            return;
        }
        self.cancel_wait();
        self.set_status(Some(AgentStatus {
            agent: status.agent,
            activity: AgentActivity::Processing,
        }));
    }

    /// The shell's prompt is back or the session closed: no agent runs.
    pub fn reset(&mut self) {
        self.cancel_wait();
        self.release_sessions();
        self.resume = None;
        self.last_report_at = None;
        self.set_status(None);
        self.set_agent(None);
    }

    /// Releases the tracker's sessions without changing its callbacks.
    pub fn dispose(&mut self) {
        self.cancel_wait();
        self.release_sessions();
    }

    /// Fires the waiting announcement when `now` reaches its deadline.
    pub fn poll(&mut self, now: u64) {
        let Some(deadline) = self.wait_deadline else {
            return;
        };
        if now < deadline {
            return;
        }
        self.wait_deadline = None;
        if let Some(agent) = self.wait_agent.take() {
            (self.callbacks.on_announce)(AgentAnnouncement {
                agent,
                state: AgentAnnouncementState::Waiting,
            });
        }
    }

    fn claim(&mut self, session: &str, state: AgentReportState, now: u64) -> bool {
        let owner = self.registry.lock().session_owners.get(session).copied();
        if owner == Some(self.id) {
            if state == AgentReportState::Ended {
                self.release_session(session);
            }
            return true;
        }
        if owner.is_some() || state == AgentReportState::Ended {
            return false;
        }
        let Some(submitted_at) = self.submitted_at else {
            return false;
        };
        if now.saturating_sub(submitted_at) > AGENT_CLAIM_WINDOW_MS {
            return false;
        }
        self.submitted_at = None;
        let contenders = {
            let state = self.registry.lock();
            state
                .recent_submissions
                .values()
                .filter(|at| now.saturating_sub(**at) <= AGENT_CLAIM_WINDOW_MS)
                .count()
        };
        if contenders == 1 {
            self.resume_claims.insert(session.to_owned());
        }
        self.registry
            .lock()
            .session_owners
            .insert(session.to_owned(), self.id);
        self.sessions.insert(session.to_owned());
        true
    }

    fn release_session(&mut self, session: &str) {
        {
            let mut state = self.registry.lock();
            if state.session_owners.get(session) == Some(&self.id) {
                state.session_owners.remove(session);
            }
        }
        self.sessions.remove(session);
        self.resume_claims.remove(session);
    }

    fn release_sessions(&mut self) {
        let sessions: Vec<String> = self.sessions.iter().cloned().collect();
        for session in sessions {
            self.release_session(&session);
        }
        self.registry.lock().recent_submissions.remove(&self.id);
    }

    fn cancel_wait(&mut self) {
        self.wait_deadline = None;
        self.wait_agent = None;
    }

    fn set_agent(&mut self, agent: Option<String>) {
        if self.agent == agent {
            return;
        }
        self.agent = agent.clone();
        (self.callbacks.on_agent)(agent);
    }

    fn set_status(&mut self, status: Option<AgentStatus>) {
        if self.status == status {
            return;
        }
        self.status = status.clone();
        (self.callbacks.on_status)(status);
    }
}

fn resume_agent_name(agent: &str) -> String {
    if agent == "claude" {
        "claudeCode".to_owned()
    } else {
        agent.to_owned()
    }
}

fn agent_resume_cwd_matches(hook_cwd: Option<&str>, shell_cwd: Option<&str>) -> bool {
    let (Some(hook_cwd), Some(shell_cwd)) = (hook_cwd, shell_cwd) else {
        return false;
    };
    if hook_cwd.is_empty() || shell_cwd.is_empty() {
        return false;
    }
    if contains_control(&format!("{hook_cwd}{shell_cwd}")) {
        return false;
    }
    normalize_resume_cwd(hook_cwd) == normalize_resume_cwd(shell_cwd)
}

fn normalize_resume_cwd(path: &str) -> String {
    let slashes = path.replace('\\', "/");
    let unprefixed = if let Some(rest) = slashes.strip_prefix("//?/") {
        if rest
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("UNC/"))
        {
            format!("//{}", &rest[4..])
        } else {
            rest.to_owned()
        }
    } else {
        slashes
    };
    let bytes = unprefixed.as_bytes();
    let windows_drive =
        bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/';
    if windows_drive || unprefixed.starts_with("//") {
        unprefixed.to_lowercase()
    } else {
        path.to_owned()
    }
}

#[cfg(test)]
mod tests;
