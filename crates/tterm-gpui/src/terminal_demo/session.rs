//! One core-owned GPUI terminal session and its output parser.
//!
//! A view subscribes to this object; it never owns or stops it. The session
//! keeps the model and parser across view destruction, so a reattached view
//! sees every byte parsed while it was gone.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use tokio::sync::watch;
use tterm_core::core::create::PtyIdentity;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::events::ssh::{SshConnectionProgressPayload, SshEvent};
use tterm_core::events::terminal::TerminalEvent;
use tterm_core::events::{AppEvent, EventSink};
use tterm_core::prompts::SessionKey;
use tterm_core::services::AppServices;
use tterm_core::session::sink::SessionSink;
use tterm_core::terminal::io_batcher::OutputFlow;
use tterm_core::terminal::output::TerminalOutput;
use tterm_terminal::grid::{CellSize, GridSize, WindowSize};
use tterm_terminal::terminal::{ClipboardPolicy, Config, Terminal};

use super::clipboard::store::ClipboardWrites;
use super::pipeline::mouse_guard::{ForegroundProbe, MouseGuard};
use super::pipeline::{self, QueueSender, RefreshSignal, SessionState};
use super::prompt::PromptControl;

/// What the terminal shows when a program ends by itself.
pub(super) const EXIT_NOTICE: &[u8] = b"\r\n\x1b[33m[Process exited]\x1b[0m\r\n";

/// A running core session, its model and the parser that feeds the model.
///
/// Clones are not made; the session manager and attachments share it through
/// an `Arc`. Dropping an attachment therefore has no effect on the session.
pub struct Session {
    services: Arc<AppServices>,
    key: SessionKey,
    state: SessionState,
    prompt: Arc<PromptControl>,
    flow: Arc<OutputFlow>,
    queue: Mutex<Option<QueueSender>>,
    parser: Mutex<Option<JoinHandle<()>>>,
    alive: Arc<AtomicBool>,
    visible: Arc<AtomicBool>,
    input_open: Arc<AtomicBool>,
    refresh: Arc<RefreshSignal>,
    clipboard: Arc<ClipboardWrites>,
}

impl Session {
    /// Opens `connection` through core and starts parsing its output.
    pub(super) fn spawn(
        services: Arc<AppServices>,
        tab_id: String,
        session_nonce: u32,
        mut connection: PtyConnectionOptions,
        grid: GridSize,
        cell: CellSize,
        clipboard_policy: ClipboardPolicy,
    ) -> Result<Self, String> {
        prepare_connection(&services, &mut connection)?;
        // Only a local term session owns a process this client can inspect.
        // SSH, serial and unknown kinds must not run a host process check.
        let local = matches!(
            connection.connection_type.as_deref(),
            None | Some("terminal")
        );
        let key = SessionKey {
            tab_id,
            session_nonce,
        };
        let clipboard = Arc::new(ClipboardWrites::new(matches!(
            clipboard_policy,
            ClipboardPolicy::CopyOnly | ClipboardPolicy::CopyAndPaste
        )));
        let terminal = Arc::new(Terminal::new(
            Config {
                clipboard: clipboard_policy,
                ..Config::default()
            },
            grid,
            cell,
        ));
        let alive = Arc::new(AtomicBool::new(true));
        let visible = Arc::new(AtomicBool::new(false));
        let input_open = Arc::new(AtomicBool::new(true));
        let flow = Arc::new(OutputFlow::default());
        let refresh = Arc::new(RefreshSignal::new());
        let state = SessionState {
            terminal: Arc::clone(&terminal),
            size: Arc::new(Mutex::new(WindowSize { grid, cell })),
            title: Arc::new(Mutex::new(None)),
            session_nonce,
            alive: Arc::clone(&alive),
            visible: Arc::clone(&visible),
            refresh: Arc::clone(&refresh),
            clipboard: Arc::clone(&clipboard),
        };
        let write_services = Arc::clone(&services);
        let write_key = key.clone();
        let write = Arc::new(move |bytes: Vec<u8>| {
            let _ = write_services.write_pty(&write_key, &bytes);
        });
        let acknowledge_flow = Arc::clone(&flow);
        let acknowledge = Arc::new(move |bytes: usize| acknowledge_flow.ack(bytes));
        let foreground = local.then(|| ForegroundProbe::Local {
            ptys: services.session.ptys.clone(),
            tab_id: key.tab_id.clone(),
            session_nonce: key.session_nonce,
        });
        let guard = Arc::new(MouseGuard::new(
            state.clone(),
            services.shared.runtime.clone(),
            foreground,
        ));
        let pipeline = pipeline::start(state.clone(), write, acknowledge, Some(guard))?;
        let queue_for_output = pipeline.queue.clone();
        let output = TerminalOutput::new(
            Arc::new(move |bytes| queue_for_output.send(bytes)),
            Arc::clone(&flow),
        );
        let sink: Arc<dyn SessionSink> = Arc::new(GpuiSessionSink {
            tab_id: key.tab_id.clone(),
            events: Arc::clone(&services.shared.events),
            output: output.clone(),
            queue: pipeline.queue.clone(),
            input_open: Arc::clone(&input_open),
        });

        if let Err(error) = services.create_pty_blocking(
            PtyIdentity {
                tab_id: key.tab_id.clone(),
                session_nonce: key.session_nonce,
                rows: grid_rows(grid),
                cols: grid_cols(grid),
            },
            Some(connection),
            output,
            sink,
        ) {
            alive.store(false, Ordering::Release);
            flow.close();
            drop(pipeline.queue);
            let _ = pipeline.parser.join();
            return Err(error);
        }

        let prompt = PromptControl::new(
            Arc::clone(&terminal),
            Arc::clone(&services.shared.prompts),
            Arc::clone(&refresh),
        );
        Ok(Self {
            services,
            key,
            state,
            prompt,
            flow,
            queue: Mutex::new(Some(pipeline.queue)),
            parser: Mutex::new(Some(pipeline.parser)),
            alive,
            visible,
            input_open,
            refresh,
            clipboard,
        })
    }

    /// The generation this session belongs to.
    pub fn key(&self) -> &SessionKey {
        &self.key
    }

    /// The generation number this session was created with.
    pub fn session_nonce(&self) -> u32 {
        self.key.session_nonce
    }

    /// The model every attached view draws.
    pub fn model(&self) -> Arc<Terminal> {
        Arc::clone(&self.state.terminal)
    }

    /// The latest title a program set, if the window has not shown it yet.
    pub fn title(&self) -> Arc<Mutex<Option<String>>> {
        Arc::clone(&self.state.title)
    }

    /// The watch channel that wakes every attached view for new output.
    pub fn subscribe(&self) -> RefreshWatch {
        RefreshWatch {
            receiver: self.refresh.subscribe(),
            refresh: Arc::clone(&self.refresh),
        }
    }

    /// Whether a view currently shows this session.
    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Acquire)
    }

    /// Whether this generation can still receive input and clipboard work.
    pub fn is_live(&self) -> bool {
        self.alive.load(Ordering::Acquire) && self.input_open.load(Ordering::Acquire)
    }

    /// The bounded OSC 52 write slot for this generation.
    pub(crate) fn clipboard_writes(&self) -> Arc<ClipboardWrites> {
        Arc::clone(&self.clipboard)
    }

    /// Shows or hides the session.
    ///
    /// Hiding only stops frame wake-ups; the parser keeps consuming and
    /// acknowledging output so the scrollback stays current and the reader is
    /// never blocked by a hidden tab. Showing requests one frame so the view
    /// catches up with everything parsed while it was hidden.
    pub fn set_visible(&self, visible: bool) {
        if self.visible.swap(visible, Ordering::AcqRel) == visible {
            return;
        }
        if visible && self.alive.load(Ordering::Acquire) {
            self.refresh.notify();
        }
    }

    /// Sends bytes through the session's local, SSH or serial channel.
    pub fn input(&self) -> impl Fn(&[u8]) + 'static {
        let services = Arc::clone(&self.services);
        let key = self.key.clone();
        let alive = Arc::clone(&self.alive);
        let input_open = Arc::clone(&self.input_open);
        let prompt = Arc::clone(&self.prompt);
        move |bytes| {
            if !alive.load(Ordering::Acquire) || !input_open.load(Ordering::Acquire) {
                return;
            }
            if !prompt.handle_input(bytes) {
                let _ = services.write_pty(&key, bytes);
            }
        }
    }

    /// Records the latest model size and resizes the core session.
    pub fn resizer(&self) -> impl Fn(GridSize, CellSize) + 'static {
        let services = Arc::clone(&self.services);
        let key = self.key.clone();
        let size = Arc::clone(&self.state.size);
        move |grid, cell| {
            if let Ok(mut current) = size.lock() {
                *current = WindowSize { grid, cell };
            }
            let _ = services.resize_pty(&key, grid_rows(grid), grid_cols(grid));
        }
    }

    /// Applies output only when it belongs to this live generation.
    ///
    /// The parser uses the same check internally. Callers that deliver
    /// already-routed output, such as a late cold-path event, get the same
    /// nonce gate here.
    pub fn accept_output(&self, session_nonce: u32, bytes: &[u8]) -> bool {
        self.state.apply(session_nonce, bytes)
    }

    /// Shows a host-key question in this session's model.
    pub fn show_host_key(&self, payload: tterm_core::events::ssh::SshHostKeyPromptPayload) {
        self.prompt.show_host_key(payload);
    }

    /// Dismisses a host-key question that expired.
    pub fn expire_host_key(&self, request_id: &str) {
        self.prompt.expire(request_id, false);
    }

    /// Shows an authentication question in this session's model.
    pub fn show_auth(&self, payload: tterm_core::events::ssh::AuthPromptPayload) {
        self.prompt.show_auth(payload);
    }

    /// Dismisses an authentication question that expired.
    pub fn expire_auth(&self, request_id: &str) {
        self.prompt.expire(request_id, true);
    }

    /// Stops the core session and the parser. Safe to call more than once.
    pub fn close(&self) {
        self.alive.store(false, Ordering::Release);
        self.visible.store(false, Ordering::Release);
        self.input_open.store(false, Ordering::Release);
        let _ = self.services.kill_pty(&self.key);
        self.flow.close();
        if let Ok(mut queue) = self.queue.lock() {
            queue.take();
        }
        if let Ok(mut parser) = self.parser.lock()
            && let Some(parser) = parser.take()
        {
            std::thread::spawn(move || {
                let _ = parser.join();
            });
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

/// A view's subscription to the session's coalesced frame wake-ups.
pub struct RefreshWatch {
    receiver: watch::Receiver<u64>,
    refresh: Arc<RefreshSignal>,
}

impl RefreshWatch {
    /// Waits for the next refresh, returning false once the session is gone.
    pub async fn changed(&mut self) -> bool {
        self.receiver.changed().await.is_ok()
    }

    /// Marks the current refresh as handled, so the next parsed batch can
    /// request another frame.
    pub fn acknowledge(&self) {
        self.refresh.acknowledge();
    }
}

/// Makes a password profile ask in the GPUI window when no saved password is
/// available. Keyboard-interactive reaches the same server password prompt
/// over servers that disable the password method itself.
fn prepare_connection(
    services: &AppServices,
    connection: &mut PtyConnectionOptions,
) -> Result<(), String> {
    if connection.connection_type.as_deref() != Some("ssh") {
        return Ok(());
    }
    if connection.auth_method.is_none() {
        connection.auth_method = Some("auto".to_string());
    }
    let saved = !connection.ignore_saved_password
        && tterm_core::core::session::load_saved_ssh_password(
            &services.shared.secrets,
            connection.profile_id.as_deref(),
            connection.profile_name.as_deref(),
        )
        .is_ok_and(|password| password.is_some());
    if password_needs_interactive_prompt(connection, saved) {
        connection.auth_method = Some("interactive".to_string());
    }
    Ok(())
}

fn password_needs_interactive_prompt(
    connection: &PtyConnectionOptions,
    saved_password: bool,
) -> bool {
    connection.auth_method.as_deref() == Some("password")
        && connection.password.is_none()
        && !saved_password
}

struct GpuiSessionSink {
    tab_id: String,
    events: Arc<dyn EventSink>,
    output: TerminalOutput,
    queue: QueueSender,
    input_open: Arc<AtomicBool>,
}

impl EventSink for GpuiSessionSink {
    fn emit(&self, event: AppEvent) -> Result<(), String> {
        self.events.emit(event)
    }
}

impl SessionSink for GpuiSessionSink {
    fn send_bytes(&self, bytes: Vec<u8>) -> Result<(), String> {
        self.output.send(bytes)
    }

    fn state_changed(&self, payload: SshConnectionProgressPayload) -> Result<(), String> {
        self.emit(AppEvent::Ssh(SshEvent::ConnectionProgress {
            tab_id: self.tab_id.clone(),
            payload,
        }))
    }

    fn exited(&self, reason: Option<String>) -> Result<(), String> {
        if self.input_open.swap(false, Ordering::AcqRel) {
            let _ = self.queue.send_synthetic(EXIT_NOTICE.to_vec());
        }
        self.emit(AppEvent::Terminal(TerminalEvent::Exit {
            tab_id: self.tab_id.clone(),
            reason,
        }))
    }

    fn ack_output(&self, bytes: usize) {
        self.output.flow().ack(bytes);
    }

    fn close_output(&self) {
        self.output.flow().close();
    }

    fn note_input(&self) {
        self.output.note_input();
    }

    fn answers_input(&self) -> bool {
        self.output.answers_input()
    }
}

fn grid_rows(grid: GridSize) -> u16 {
    u16::try_from(grid.screen_lines).unwrap_or(u16::MAX)
}

fn grid_cols(grid: GridSize) -> u16 {
    u16::try_from(grid.columns).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use tterm_core::core::session::types::PtyConnectionOptions;
    use tterm_core::events::{AppEvent, EventSink};
    use tterm_core::session::sink::SessionSink;
    use tterm_core::terminal::io_batcher::OutputFlow;
    use tterm_core::terminal::output::TerminalOutput;
    use tterm_terminal::grid::{CellSize, GridSize, WindowSize};
    use tterm_terminal::terminal::{Config, Terminal};

    use super::pipeline::{self, RefreshSignal, SessionState};
    use super::{GpuiSessionSink, password_needs_interactive_prompt};

    const GRID: GridSize = GridSize {
        columns: 80,
        screen_lines: 24,
    };
    const CELL: CellSize = CellSize {
        width: 8,
        height: 16,
    };

    struct NoopEvents;

    impl EventSink for NoopEvents {
        fn emit(&self, _event: AppEvent) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn a_password_profile_without_a_saved_password_uses_interactive_prompts() {
        let options = PtyConnectionOptions {
            connection_type: Some("ssh".to_string()),
            auth_method: Some("password".to_string()),
            profile_name: Some("Production".to_string()),
            ..PtyConnectionOptions::default()
        };
        assert!(password_needs_interactive_prompt(&options, false));
        assert!(!password_needs_interactive_prompt(&options, true));
    }

    #[test]
    fn an_explicitly_ignored_password_is_typed_in_the_window() {
        let options = PtyConnectionOptions {
            connection_type: Some("ssh".to_string()),
            auth_method: Some("password".to_string()),
            ignore_saved_password: true,
            ..PtyConnectionOptions::default()
        };
        assert!(password_needs_interactive_prompt(&options, false));
    }

    #[test]
    fn the_exit_notice_does_not_consume_pty_flow_control_credit() {
        let model = Arc::new(Terminal::new(Config::default(), GRID, CELL));
        let state = SessionState {
            terminal: Arc::clone(&model),
            size: Arc::new(Mutex::new(WindowSize {
                grid: GRID,
                cell: CELL,
            })),
            title: Arc::new(Mutex::new(None)),
            clipboard: Arc::new(
                crate::terminal_demo::clipboard::store::ClipboardWrites::disabled(),
            ),
            session_nonce: 1,
            alive: Arc::new(AtomicBool::new(true)),
            visible: Arc::new(AtomicBool::new(false)),
            refresh: Arc::new(RefreshSignal::new()),
        };
        let acknowledged = Arc::new(AtomicUsize::new(0));
        let acknowledge = Arc::clone(&acknowledged);
        let pipeline = pipeline::start(
            state,
            Arc::new(|_| {}),
            Arc::new(move |bytes| {
                acknowledge.fetch_add(bytes, Ordering::Relaxed);
            }),
            None,
        )
        .expect("start parser");
        let output = TerminalOutput::new(Arc::new(|_| Ok(())), Arc::new(OutputFlow::default()));
        let sink = GpuiSessionSink {
            tab_id: "p2-02-exit".to_string(),
            events: Arc::new(NoopEvents),
            output,
            queue: pipeline.queue.clone(),
            input_open: Arc::new(AtomicBool::new(true)),
        };

        sink.exited(None).expect("emit exit");
        drop(sink);
        drop(pipeline.queue);
        pipeline.parser.join().expect("parser exits");

        assert_eq!(
            acknowledged.load(Ordering::Acquire),
            0,
            "the synthetic exit notice has no PTY reservation to acknowledge"
        );
        let screen: String = model
            .snapshot()
            .rows()
            .iter()
            .flat_map(|row| row.cells())
            .map(|cell| cell.ch)
            .collect();
        assert!(screen.contains("[Process exited]"));
    }
}
