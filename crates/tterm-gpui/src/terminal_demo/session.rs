//! One core-owned GPUI terminal session and its output parser.
//!
//! A view subscribes to this object; it never owns or stops it. The session
//! keeps the model and parser across view destruction, so a reattached view
//! sees every byte parsed while it was gone.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
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
use tterm_terminal::palette::TerminalPalette;
use tterm_terminal::snapshot::CursorShape;
use tterm_terminal::terminal::{ClipboardPolicy, Config, Terminal};

use super::clipboard::store::ClipboardWrites;
use super::pipeline::mouse_guard::{ForegroundProbe, MouseGuard};
use super::pipeline::{self, QueueSender, RefreshSignal, SessionState};
use super::prompt::PromptControl;

pub(crate) mod reports;
use reports::Reports;

/// Bytes the user typed or pasted, before they reach the session.
pub(crate) type InputObserver = Arc<dyn Fn(&[u8]) + Send + Sync>;

/// What the terminal shows when a program ends by itself.
pub(super) const EXIT_NOTICE: &[u8] = b"\r\n\x1b[33m[Process exited]\x1b[0m\r\n";

/// The only clipboard policy the OSC 52 setting selects: stores, never reads.
pub(crate) fn clipboard_write_policy(enabled: bool) -> ClipboardPolicy {
    if enabled {
        ClipboardPolicy::CopyOnly
    } else {
        ClipboardPolicy::Disabled
    }
}

/// A Tauri-era snapshot replayed into a new model before its transport starts.
pub(crate) struct RestoredHistory {
    /// The serialized escape sequences the old terminal showed.
    pub(crate) content: String,
    /// The localized line that marks where the restored content ends.
    pub(crate) banner: String,
}

/// What one session generation inherits from its tab when it opens.
///
/// The palette stays out of [`Config`] on purpose: the model never stores
/// theme colors, while the parser needs the same palette the view draws with
/// to answer color queries.
pub(super) struct SessionOptions {
    /// The transport and connection details of the new generation.
    pub(super) connection: PtyConnectionOptions,
    /// The grid and cell size the generation starts with.
    pub(super) size: WindowSize,
    /// The model options inherited from the application settings.
    pub(super) config: Config,
    /// The palette the attached view draws with.
    pub(super) palette: TerminalPalette,
    /// A Tauri-era snapshot to replay before the transport starts.
    pub(super) restored: Option<RestoredHistory>,
}

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
    reports: Arc<Reports>,
    queue: Mutex<Option<QueueSender>>,
    parser: Mutex<Option<JoinHandle<()>>>,
    alive: Arc<AtomicBool>,
    visible: Arc<AtomicBool>,
    input_open: Arc<AtomicBool>,
    input_observer: Arc<Mutex<Option<InputObserver>>>,
    refresh: Arc<RefreshSignal>,
    clipboard: Arc<ClipboardWrites>,
}

impl Session {
    /// Opens `connection` through core and starts parsing its output.
    pub(super) fn spawn(
        services: Arc<AppServices>,
        tab_id: String,
        session_nonce: u32,
        options: SessionOptions,
    ) -> Result<Self, String> {
        let SessionOptions {
            mut connection,
            size,
            config,
            palette,
            restored,
        } = options;
        let WindowSize { grid, cell } = size;
        prepare_connection(&mut connection);
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
            config.clipboard,
            ClipboardPolicy::CopyOnly | ClipboardPolicy::CopyAndPaste
        )));
        let terminal = Arc::new(Terminal::new(config, grid, cell));
        // Replay before the parser or transport exists, so the old content can
        // never land below output from the new session.
        if let Some(restored) = restored {
            if cfg!(windows) && local {
                terminal.restore_before_redraw(&restored.content, &restored.banner);
            } else {
                terminal.restore_legacy_history(&restored.content, &restored.banner);
            }
        }
        let alive = Arc::new(AtomicBool::new(true));
        let visible = Arc::new(AtomicBool::new(false));
        let input_open = Arc::new(AtomicBool::new(true));
        let flow = Arc::new(OutputFlow::default());
        let refresh = Arc::new(RefreshSignal::new());
        let reports = Arc::new(Reports::new());
        let state = SessionState {
            terminal: Arc::clone(&terminal),
            size: Arc::new(Mutex::new(WindowSize { grid, cell })),
            title: Arc::new(Mutex::new(None)),
            palette: Arc::new(Mutex::new(palette)),
            session_nonce,
            alive: Arc::clone(&alive),
            visible: Arc::clone(&visible),
            refresh: Arc::clone(&refresh),
            clipboard: Arc::clone(&clipboard),
            reports: Arc::clone(&reports),
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
        let ssh_queries =
            (connection.connection_type.as_deref() == Some("ssh")).then(Default::default);
        let pipeline =
            pipeline::start(state.clone(), write, acknowledge, Some(guard), ssh_queries)?;
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
            reports,
            queue: Mutex::new(Some(pipeline.queue)),
            parser: Mutex::new(Some(pipeline.parser)),
            alive,
            visible,
            input_open,
            input_observer: Arc::new(Mutex::new(None)),
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

    /// Replaces the palette color queries are answered with.
    ///
    /// The attached view must be given the same palette so its pixels and the
    /// replies agree. The model, parser and transport keep running.
    pub fn set_palette(&self, palette: TerminalPalette) {
        self.state.set_palette(palette);
    }

    /// The palette color queries are answered with.
    pub fn palette(&self) -> TerminalPalette {
        self.state.palette()
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

    /// The prompt, directory and agent reports parsed from this generation.
    pub(crate) fn reports(&self) -> Arc<Reports> {
        Arc::clone(&self.reports)
    }

    /// Calls `observer` for bytes the user typed or pasted.
    ///
    /// The resume controller uses this to stop before its command reaches a
    /// prompt the user already started using. It never sees replies the model
    /// sends to a program, because those go through the parser write path.
    pub(crate) fn set_input_observer(&self, observer: InputObserver) {
        *self
            .input_observer
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(observer);
    }

    /// Writes bytes the client constructed itself, bypassing prompt input.
    ///
    /// Returns an error when the generation is gone or a host-key or
    /// authentication prompt still owns the keyboard.
    pub(crate) fn write_input(&self, bytes: &[u8]) -> Result<(), String> {
        if !self.is_live() {
            return Err("The session is no longer running".to_string());
        }
        if !self.prompt.is_idle() {
            return Err("The session is waiting for an SSH prompt answer".to_string());
        }
        self.services.write_pty(&self.key, bytes)
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
        if visible {
            self.wake();
        }
    }

    /// Changes the scrollback capacity of this session's model.
    ///
    /// The session keeps its generation and parser. A visible view is woken
    /// because shrinking the capacity can evict history and clamp the viewport
    /// without another PTY batch arriving to request the frame.
    pub fn set_scrollback(&self, lines: usize) {
        let model = self.model();
        if model.scrollback() == lines {
            return;
        }
        model.set_scrollback(lines);
        self.wake();
    }

    /// Changes the shape a reset cursor returns to in this session's model.
    pub fn set_default_cursor_shape(&self, shape: CursorShape) {
        let model = self.model();
        model.set_default_cursor_shape(shape);
        self.wake();
    }

    /// Turns shell-integration command marks on or off for this generation.
    ///
    /// Disabling drops the positions already collected, so the overview ruler
    /// stops drawing them; the parser, PTY and model identity stay the same.
    /// A visible view is woken because no new output has to arrive for its
    /// scrollbar to change.
    pub fn set_command_marks_enabled(&self, enabled: bool) {
        self.model().set_command_marks_enabled(enabled);
        self.wake();
    }

    /// Applies a committed OSC 52 write setting to this generation.
    ///
    /// The model's bounded scanner starts or stops recognizing stores, and the
    /// UI slot starts or stops applying them. Disabling revokes a store that
    /// has not reached the clipboard yet; the parser and the model identity
    /// stay the same, and a query never becomes a clipboard read.
    pub fn set_clipboard_write_enabled(&self, enabled: bool) {
        self.model().set_clipboard_write_enabled(enabled);
        self.clipboard.set_enabled(enabled);
    }

    /// Requests one frame when the session is both visible and live.
    fn wake(&self) {
        if self.visible.load(Ordering::Acquire) && self.alive.load(Ordering::Acquire) {
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
        let input_observer = Arc::clone(&self.input_observer);
        move |bytes| {
            let observer = input_observer
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            if let Some(observer) = &observer {
                observer(bytes);
            }
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

/// Supplies the default only when an SSH connection names no method.
///
/// A password profile keeps `password` even without a saved password: core
/// asks once after the host key is verified, then sends the standard password
/// method. Interactive, key, agent and auto profiles stay as configured.
fn prepare_connection(connection: &mut PtyConnectionOptions) {
    if connection.connection_type.as_deref() == Some("ssh") && connection.auth_method.is_none() {
        connection.auth_method = Some("auto".to_string());
    }
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

    fn owns_ssh_queries(&self) -> bool {
        true
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
    use super::{GpuiSessionSink, prepare_connection};

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

    fn ssh_options(auth_method: Option<&str>) -> PtyConnectionOptions {
        PtyConnectionOptions {
            connection_type: Some("ssh".to_string()),
            auth_method: auth_method.map(str::to_string),
            profile_name: Some("Production".to_string()),
            ..PtyConnectionOptions::default()
        }
    }

    #[test]
    fn a_password_profile_without_a_saved_password_keeps_password_auth() {
        let mut options = ssh_options(Some("password"));
        prepare_connection(&mut options);
        assert_eq!(options.auth_method.as_deref(), Some("password"));
    }

    #[test]
    fn explicit_ssh_auth_methods_are_preserved() {
        for method in ["interactive", "key", "agent", "auto"] {
            let mut options = ssh_options(Some(method));
            prepare_connection(&mut options);
            assert_eq!(options.auth_method.as_deref(), Some(method));
        }
    }

    #[test]
    fn an_unset_ssh_auth_method_defaults_to_auto() {
        let mut options = ssh_options(None);
        prepare_connection(&mut options);
        assert_eq!(options.auth_method.as_deref(), Some("auto"));
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
            palette: Arc::new(Mutex::new(
                tterm_terminal::palette::TerminalPalette::default(),
            )),
            clipboard: Arc::new(
                crate::terminal_demo::clipboard::store::ClipboardWrites::disabled(),
            ),
            session_nonce: 1,
            alive: Arc::new(AtomicBool::new(true)),
            visible: Arc::new(AtomicBool::new(false)),
            refresh: Arc::new(RefreshSignal::new()),
            reports: Arc::new(super::reports::Reports::new()),
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
