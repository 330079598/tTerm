//! Connects one tab's session reports to its saved resume record.
//!
//! A controller outlives any single output chunk but never owns the session.
//! The report loop drops with the session, and the restore state only sends
//! what [`build_resume_command`] approves.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Weak;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui_kit::{AppContext, Context, Entity, WeakEntity};
use tokio::sync::mpsc;
use tterm_core::agent_status::{AgentResumeSession, parse_agent_report};
use tterm_core::core::agent_resume::{ResumeAgent, ResumeRequest, build_resume_command};
use tterm_core::events::ssh::ShellIntegrationStatus;
use tterm_core::terminal::pty::{LocalTerminalShellNameInput, local_terminal_shell_name_service};
use tterm_terminal::session_reports::SessionReport;

use super::super::Prototype;
use super::state::{Capture, Notice, NoticeReason, PROMPT_TIMEOUT, Restore};
use crate::config_state::ConfigState;
use crate::terminal_demo::binding::TabBinding;
use crate::terminal_demo::binding::scoped::{ConnectionState, SessionScopedState};
use crate::terminal_demo::session::Session;
use crate::terminal_demo::session::reports::ReportsSignal;

/// Everything a new controller reads from the tab that owns it.
pub(crate) struct AttachOptions {
    /// The window's internal id for the tab.
    pub tab: usize,
    /// The tab's current session binding.
    pub binding: Rc<TabBinding>,
    /// The generation whose reports this controller follows.
    pub session: Arc<Session>,
    /// The resume record the workspace restored, if any.
    pub restored: Option<AgentResumeSession>,
    /// Whether this launch may start an agent again.
    pub may_resume: bool,
    /// Whether the connection is SSH and needs a ready event first.
    pub is_ssh: bool,
    /// The local shell choice, for local tabs.
    pub local_shell: Option<String>,
    /// The local custom shell path, for custom local shells.
    pub local_custom_path: Option<String>,
    /// The live configuration that gates resume and command marks.
    pub config: Entity<ConfigState>,
}

/// One tab's capture and, when allowed, its restore.
pub(crate) struct Controller {
    core: Core,
    // These keep the tab and its generation alive as long as the controller
    // is in the window. The spawned tasks only hold weak handles.
    _session: Arc<Session>,
    _binding: Rc<TabBinding>,
}

impl Controller {
    /// Starts following `options.session` and waits to resume its saved agent.
    ///
    /// Capture always runs, including when the setting is off, so a session
    /// that starts after a manual launch becomes resumable too.
    pub(crate) fn attach(
        workspace: WeakEntity<Prototype>,
        options: AttachOptions,
        cx: &mut Context<Prototype>,
    ) -> Option<Controller> {
        workspace.upgrade()?;
        let AttachOptions {
            tab,
            binding,
            session,
            restored,
            may_resume,
            is_ssh,
            local_shell,
            local_custom_path,
            config,
        } = options;
        let (marks, resume_enabled) = {
            let settings = config.read(cx);
            let settings = settings.config();
            (settings.command_marks, settings.resume_agent_sessions)
        };

        let resume_pending = should_resume(restored.is_some(), may_resume, resume_enabled);
        let restore = if resume_pending {
            match restored.clone() {
                Some(resume) => Restore::waiting(resume),
                None => Restore::disabled(),
            }
        } else {
            Restore::disabled()
        };
        let capture = Capture::new(restored, None);
        let (backlog, reports, signal) = session.reports().attach();
        let state = Rc::new(RefCell::new(State {
            capture,
            restore,
            shell_ready: false,
            lost: false,
        }));
        let core = Core {
            tab,
            workspace,
            session: Arc::downgrade(&session),
            state: Rc::clone(&state),
            reports: signal,
        };
        if core.reports_lost() {
            core.lose();
        }

        let (wake, wake_receiver) = mpsc::unbounded_channel();
        let wake_sender = wake.clone();
        let observer = move |bytes: &[u8]| {
            if bytes.is_empty() || is_terminal_reply(bytes) {
                return;
            }
            let _ = wake_sender.send(bytes.contains(&b'\r'));
        };
        session.set_input_observer(Arc::new(observer));

        spawn_reports(core.clone(), backlog, reports, wake_receiver, cx);
        if core.resume_enabled() {
            spawn_readiness(
                core.clone(),
                &binding,
                Readiness {
                    session_nonce: session.session_nonce(),
                    marks,
                    is_ssh,
                    local_shell,
                    local_custom_path,
                },
                cx,
            );
        }

        Some(Controller {
            core,
            _session: session,
            _binding: binding,
        })
    }

    /// The bar to show, or `None` while the restore is still silent.
    pub(crate) fn notice(&self) -> Option<Notice> {
        self.core.notice()
    }

    /// Sends the saved resume command after the user asked for it.
    pub(crate) fn resume_manually(&mut self, cx: &mut Context<Prototype>) {
        if self.core.send(true) {
            cx.notify();
        }
    }
}

struct State {
    capture: Capture,
    restore: Restore,
    shell_ready: bool,
    lost: bool,
}

#[derive(Clone)]
struct Core {
    tab: usize,
    workspace: WeakEntity<Prototype>,
    session: Weak<Session>,
    state: Rc<RefCell<State>>,
    reports: ReportsSignal,
}

impl Core {
    fn notice(&self) -> Option<Notice> {
        self.state.borrow().restore.notice().cloned()
    }

    fn resume_enabled(&self) -> bool {
        self.state.borrow().restore.is_enabled()
    }

    fn wants_send(&self) -> bool {
        self.state.borrow().restore.wants_send()
    }

    fn is_waiting(&self) -> bool {
        self.state.borrow().restore.is_waiting()
    }

    fn shell_ready(&self) -> bool {
        self.state.borrow().shell_ready
    }

    fn reports_lost(&self) -> bool {
        self.reports.lost()
    }

    /// Stops automatic resume after the report stream lost data.
    ///
    /// Capture keeps following the session, but the prompt state may no
    /// longer be current, so only an explicit button can send from here.
    fn lose(&self) -> bool {
        let mut state = self.state.borrow_mut();
        if state.lost {
            return false;
        }
        state.lost = true;
        if !state.restore.is_waiting() {
            return false;
        }
        let enabled = state.restore.shell().is_some();
        state.restore.failed(NoticeReason::Failed, enabled)
    }

    /// Applies one report from the session's mailbox.
    fn apply<C: AppContext>(&self, report: SessionReport, cx: &mut C) {
        if self.reports_lost() {
            self.lose();
        }
        match report {
            SessionReport::Cwd(cwd) => {
                self.state.borrow_mut().capture.set_cwd(cwd);
            }
            SessionReport::AgentReport(data) => {
                let Some(report) = parse_agent_report(&data) else {
                    return;
                };
                let changed = self
                    .state
                    .borrow_mut()
                    .capture
                    .report(&report, now_millis());
                if let Some(resume) = changed {
                    self.persist(resume, cx);
                }
            }
            SessionReport::Prompt => {
                let changed = {
                    let mut state = self.state.borrow_mut();
                    let changed = state.capture.prompt();
                    state.restore.prompt();
                    changed
                };
                if let Some(resume) = changed {
                    self.persist(resume, cx);
                }
                if self.wants_send() {
                    let _ = self.send(false);
                }
            }
            SessionReport::CommandStarted => {
                self.state.borrow_mut().restore.command_started();
            }
        }
        self.notify(cx);
    }

    /// Applies the detected shell once, returning whether the restore is
    /// still waiting and whether a prompt is already available.
    fn mark_ready(&self, shell: Option<String>, marks: bool) -> Option<(bool, bool)> {
        let mut state = self.state.borrow_mut();
        if state.shell_ready || !state.restore.is_waiting() {
            return None;
        }
        state.shell_ready = true;
        state.restore.ready(shell.as_deref(), marks);
        Some((state.restore.is_waiting(), state.restore.wants_send()))
    }

    /// Builds and sends the command through the core's validated builder.
    ///
    /// Returns whether the restore state changed, so the caller can repaint.
    fn send(&self, by_button: bool) -> bool {
        let (resume, shell) = {
            let state = self.state.borrow();
            let restore = &state.restore;
            if by_button {
                let Some(notice) = restore.notice() else {
                    return false;
                };
                if !notice.enabled {
                    return false;
                }
            } else if !restore.wants_send() {
                return false;
            }
            let Some(resume) = restore.resume().cloned() else {
                return false;
            };
            let Some(shell) = restore.shell().map(str::to_owned) else {
                return false;
            };
            (resume, shell)
        };
        if !by_button {
            if self.reports_lost() {
                return self.lose();
            }
            if !self.reports.at_prompt() {
                return false;
            }
        }
        let cwd = resume
            .cwd
            .clone()
            .or_else(|| self.state.borrow().capture.cwd())
            .unwrap_or_default();
        let Some(agent) = resume_agent(&resume.agent) else {
            return self.fail(NoticeReason::Unsafe, false);
        };
        let command = build_resume_command(&ResumeRequest {
            agent,
            session_id: &resume.session_id,
            cwd: &cwd,
            shell: &shell,
        });
        let Some(command) = command else {
            return self.fail(NoticeReason::Unsafe, false);
        };
        let Some(session) = self.session.upgrade() else {
            return self.fail(NoticeReason::Failed, true);
        };
        let line = format!("{command}\r");
        if !by_button {
            if self.reports_lost() {
                return self.lose();
            }
            if !self.reports.at_prompt() {
                return false;
            }
        }
        match session.write_input(line.as_bytes()) {
            Ok(()) => {
                self.state.borrow_mut().restore.sent();
                true
            }
            Err(_) => self.fail(NoticeReason::Failed, true),
        }
    }

    fn fail(&self, reason: NoticeReason, enabled: bool) -> bool {
        self.state.borrow_mut().restore.failed(reason, enabled)
    }

    fn cancel(&self) -> bool {
        self.state.borrow_mut().restore.cancel()
    }

    fn submit(&self, now_ms: u64) {
        self.state.borrow_mut().capture.submit(now_ms);
    }

    fn persist<C: AppContext>(&self, resume: Option<AgentResumeSession>, cx: &mut C) {
        let tab = self.tab;
        let _ = self.workspace.update(cx, move |prototype, cx| {
            prototype.set_agent_resume(tab, resume, cx)
        });
    }

    fn notify<C: AppContext>(&self, cx: &mut C) {
        let _ = self.workspace.update(cx, |_, cx| cx.notify());
    }
}

fn spawn_reports(
    core: Core,
    backlog: Vec<SessionReport>,
    mut reports: mpsc::Receiver<SessionReport>,
    mut wake: mpsc::UnboundedReceiver<bool>,
    cx: &mut Context<Prototype>,
) {
    cx.spawn(async move |prototype, cx| {
        for report in backlog {
            core.apply(report, cx);
        }
        loop {
            tokio::select! {
                report = reports.recv() => {
                    let Some(report) = report else {
                        break;
                    };
                    core.apply(report, cx);
                }
                Some(submitted) = wake.recv() => {
                    if submitted {
                        core.submit(now_millis());
                    }
                    if core.cancel() {
                        let _ = prototype.update(cx, |_, cx| cx.notify());
                    }
                }
            }
        }
    })
    .detach();
}

fn spawn_readiness(
    core: Core,
    binding: &Rc<TabBinding>,
    options: Readiness,
    cx: &mut Context<Prototype>,
) {
    let weak_binding = Rc::downgrade(binding);
    let mut watch = binding.subscribe_scoped();
    cx.spawn(async move |prototype, cx| {
        loop {
            if core.reports_lost() {
                if core.lose() {
                    let _ = prototype.update(cx, |_, cx| cx.notify());
                }
                break;
            }
            let Some(binding) = weak_binding.upgrade() else {
                break;
            };
            let scoped = binding.scoped_state();
            if scoped.session_nonce() != options.session_nonce {
                if core.cancel() {
                    let _ = prototype.update(cx, |_, cx| cx.notify());
                }
                break;
            }
            let connection = scoped.connection_state();
            if matches!(
                connection,
                ConnectionState::Error | ConnectionState::Disconnected
            ) && core.cancel()
            {
                let _ = prototype.update(cx, |_, cx| cx.notify());
            } else if core.is_waiting()
                && !core.shell_ready()
                && ready_to_resume(options.is_ssh, connection)
            {
                let detected = if options.is_ssh {
                    ssh_shell(&scoped)
                } else {
                    detect_local_shell(
                        options.local_shell.clone(),
                        options.local_custom_path.clone(),
                        cx,
                    )
                    .await
                };
                if let Some((waiting, send_now)) = core.mark_ready(detected, options.marks) {
                    let _ = prototype.update(cx, |_, cx| cx.notify());
                    if waiting {
                        spawn_timeout(core.clone(), cx);
                    }
                    if send_now {
                        let _ = core.send(false);
                        let _ = prototype.update(cx, |_, cx| cx.notify());
                    }
                }
            }
            drop(scoped);
            drop(binding);
            if watch.changed().await.is_err() {
                break;
            }
        }
    })
    .detach();
}

/// Whether a session has opened far enough to detect its shell.
///
/// SSH must have finished authentication and opened the shell channel. A
/// local PTY has no authentication step, but an ended or failed one must not
/// be treated as ready.
pub(super) fn ready_to_resume(is_ssh: bool, connection: ConnectionState) -> bool {
    match connection {
        ConnectionState::Connected => true,
        ConnectionState::Connecting => !is_ssh,
        ConnectionState::Reconnecting | ConnectionState::Error | ConnectionState::Disconnected => {
            false
        }
    }
}

struct Readiness {
    session_nonce: u32,
    marks: bool,
    is_ssh: bool,
    local_shell: Option<String>,
    local_custom_path: Option<String>,
}

fn spawn_timeout(core: Core, cx: &mut gpui_kit::AsyncApp) {
    cx.spawn(async move |cx| {
        cx.background_executor().timer(PROMPT_TIMEOUT).await;
        if core.state.borrow_mut().restore.timeout().is_some() {
            let _ = core.workspace.update(cx, |_, cx| cx.notify());
        }
    })
    .detach();
}

fn ssh_shell(scoped: &SessionScopedState) -> Option<String> {
    let progress = scoped.connection_progress()?;
    match &progress.shell_integration {
        Some(ShellIntegrationStatus::Active { shell, .. }) => shell.clone(),
        _ => None,
    }
}

async fn detect_local_shell(
    shell: Option<String>,
    custom_path: Option<String>,
    cx: &mut gpui_kit::AsyncApp,
) -> Option<String> {
    cx.background_executor()
        .spawn(async move {
            local_terminal_shell_name_service(LocalTerminalShellNameInput { shell, custom_path })
        })
        .await
}

/// Whether a restored tab may run its saved resume command.
pub(super) fn should_resume(restored: bool, may_resume: bool, enabled: bool) -> bool {
    restored && may_resume && enabled
}

/// Maps the saved agent name to a resume command the core builder supports.
pub(super) fn resume_agent(agent: &str) -> Option<ResumeAgent> {
    match agent {
        "claude" | "claudeCode" => Some(ResumeAgent::ClaudeCode),
        "codex" => Some(ResumeAgent::Codex),
        _ => None,
    }
}

/// Whether bytes are a terminal reply rather than the user typing.
pub(super) fn is_terminal_reply(bytes: &[u8]) -> bool {
    bytes == b"\x1b[I"
        || bytes == b"\x1b[O"
        || bytes.starts_with(b"\x1b[<")
        || bytes.starts_with(b"\x1b[M")
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}
