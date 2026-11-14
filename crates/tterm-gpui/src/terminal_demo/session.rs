//! One GPUI session backed by the same `AppServices` used by the Tauri client.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

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

use super::pipeline::{self, SessionState};

/// What the terminal shows when a program ends by itself.
pub(super) const EXIT_NOTICE: &[u8] = b"\r\n\x1b[33m[Process exited]\x1b[0m\r\n";

static NEXT_SESSION_NONCE: AtomicU32 = AtomicU32::new(1);

/// A running core session and the GPUI output parser attached to it.
pub(super) struct Session {
    services: Arc<AppServices>,
    key: SessionKey,
    queue: SyncSender<Vec<u8>>,
    parser: JoinHandle<()>,
    alive: Arc<AtomicBool>,
}

impl Session {
    /// Opens `connection` through core and returns the session plus the
    /// channel that wakes the view when parsed output or prompts arrive.
    pub fn spawn(
        services: Arc<AppServices>,
        mut connection: PtyConnectionOptions,
        state: SessionState,
        rows: u16,
        cols: u16,
        wake: tokio::sync::mpsc::Sender<()>,
    ) -> Result<Self, String> {
        prepare_connection(&services, &mut connection)?;
        let key = SessionKey {
            tab_id: "gpui-main".to_string(),
            session_nonce: NEXT_SESSION_NONCE.fetch_add(1, Ordering::Relaxed),
        };
        let flow = Arc::new(OutputFlow::default());
        let pipeline = pipeline::start(
            state,
            Arc::clone(&services),
            key.clone(),
            Arc::clone(&flow),
            wake,
        )?;
        let queue_for_output = pipeline.queue.clone();
        let output = TerminalOutput::new(
            Arc::new(move |bytes| {
                queue_for_output
                    .send(bytes)
                    .map_err(|_| "The GPUI output parser has closed".to_string())
            }),
            Arc::clone(&flow),
        );
        let alive = Arc::new(AtomicBool::new(true));
        let sink: Arc<dyn SessionSink> = Arc::new(GpuiSessionSink {
            tab_id: key.tab_id.clone(),
            events: Arc::clone(&services.shared.events),
            output: output.clone(),
            queue: pipeline.queue.clone(),
            alive: Arc::clone(&alive),
        });

        if let Err(error) = services.create_pty_blocking(
            PtyIdentity {
                tab_id: key.tab_id.clone(),
                session_nonce: key.session_nonce,
                rows,
                cols,
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

        Ok(Self {
            services,
            key,
            queue: pipeline.queue,
            parser: pipeline.parser,
            alive,
        })
    }

    /// Sends bytes through the session's local, SSH or serial channel.
    pub fn input(&self) -> impl Fn(&[u8]) + 'static {
        let services = Arc::clone(&self.services);
        let key = self.key.clone();
        let alive = Arc::clone(&self.alive);
        move |bytes| {
            if alive.load(Ordering::Acquire) {
                let _ = services.write_pty(&key, bytes);
            }
        }
    }

    /// Records the latest model size and resizes the core session.
    pub fn resizer(&self, size: Arc<Mutex<WindowSize>>) -> impl Fn(GridSize, CellSize) + 'static {
        let services = Arc::clone(&self.services);
        let key = self.key.clone();
        move |grid, cell| {
            if let Ok(mut current) = size.lock() {
                *current = WindowSize { grid, cell };
            }
            let rows = u16::try_from(grid.screen_lines).unwrap_or(u16::MAX);
            let cols = u16::try_from(grid.columns).unwrap_or(u16::MAX);
            let _ = services.resize_pty(&key, rows, cols);
        }
    }

    /// Stops the core session and joins its parser off the UI thread.
    pub fn shutdown(self) -> JoinHandle<()> {
        self.alive.store(false, Ordering::Release);
        let _ = self.services.kill_pty(&self.key);
        drop(self.queue);
        std::thread::spawn(move || {
            let _ = self.parser.join();
        })
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
    queue: SyncSender<Vec<u8>>,
    alive: Arc<AtomicBool>,
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
        if self.alive.swap(false, Ordering::AcqRel) {
            let _ = self.queue.send(EXIT_NOTICE.to_vec());
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use tterm_core::core::session::types::PtyConnectionOptions;
    use tterm_terminal::grid::{CellSize, GridSize, WindowSize};
    use tterm_terminal::terminal::{Config, Terminal};

    use super::{Session, password_needs_interactive_prompt};
    use crate::terminal_demo::command::CommandSpec;
    use crate::terminal_demo::pipeline::SessionState;
    use crate::terminal_demo::runtime;

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
    fn a_local_session_routes_window_input_and_output_through_app_services() {
        let runtime = runtime::test_runtime();
        let model = Arc::new(Terminal::new(
            Config::default(),
            GridSize {
                columns: 80,
                screen_lines: 24,
            },
            CellSize {
                width: 8,
                height: 16,
            },
        ));
        let state = SessionState {
            terminal: Arc::clone(&model),
            size: Arc::new(Mutex::new(WindowSize {
                grid: GridSize {
                    columns: 80,
                    screen_lines: 24,
                },
                cell: CellSize {
                    width: 8,
                    height: 16,
                },
            })),
            title: Arc::new(Mutex::new(None)),
        };
        #[cfg(windows)]
        let command = CommandSpec::Program {
            program: std::env::var("COMSPEC")
                .unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".to_string()),
            args: vec!["/Q".to_string()],
        };
        #[cfg(not(windows))]
        let command = CommandSpec::Program {
            program: "/bin/sh".to_string(),
            args: vec!["-i".to_string()],
        };
        let connection = command
            .connection_options()
            .expect("local connection options");
        let (wake, _wake_receiver) = tokio::sync::mpsc::channel(1);
        let session = Session::spawn(
            Arc::clone(&runtime.services),
            connection,
            state,
            24,
            80,
            wake,
        )
        .expect("start local session");

        let marker = "GPUI-APPSERVICES-ROUNDTRIP";
        let input = session.input();
        #[cfg(windows)]
        input(format!("echo {marker}\r\n").as_bytes());
        #[cfg(not(windows))]
        input(format!("printf '{marker}\\n'\n").as_bytes());
        assert!(
            wait_for_text(&model, marker, Duration::from_secs(15)),
            "local output reaches the GPUI model"
        );

        #[cfg(windows)]
        input(b"exit\r\n");
        #[cfg(not(windows))]
        input(b"exit\n");
        let closed = session.shutdown();
        let deadline = Instant::now() + Duration::from_secs(15);
        while !closed.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(closed.is_finished(), "the parser and session tasks finish");
        closed.join().expect("join parser");
    }

    fn wait_for_text(model: &Terminal, needle: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            let frame = model.snapshot();
            let text: String = frame
                .rows()
                .iter()
                .flat_map(|row| row.cells())
                .map(|cell| cell.ch)
                .collect();
            if text.contains(needle) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
