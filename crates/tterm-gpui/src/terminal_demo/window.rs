//! The prototype's window: one terminal fed by an `AppServices` session.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, Entity, Window, WindowBounds, WindowOptions, div, point, px, size};
use tokio::sync::mpsc::UnboundedReceiver;
use tterm_core::events::AppEvent;
use tterm_core::events::ssh::SshEvent;
use tterm_core::events::terminal::TerminalEvent;
use tterm_core::services::AppServices;
use tterm_terminal::grid::{CellSize, GridSize, WindowSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

use super::command::CommandSpec;
use super::conpty::{log_conpty_source, prepare_conpty};
use super::intents::DEFAULT_TITLE;
use super::pipeline::SessionState;
use super::prompt::PromptControl;
use super::session::Session;

const INITIAL_GRID: GridSize = GridSize {
    columns: 80,
    screen_lines: 24,
};
const INITIAL_CELL: CellSize = CellSize {
    width: 8,
    height: 16,
};

/// Runs the application with one prototype window until it closes.
pub(super) fn run_window(
    command: CommandSpec,
    services: Arc<AppServices>,
    events: UnboundedReceiver<AppEvent>,
) {
    prepare_conpty();
    gpui_kit::application().run(|cx: &mut App| {
        gpui_kit::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        if let Err(error) = open_window(cx, command, services, events) {
            eprintln!("tterm-gpui: {error}");
            cx.quit();
        }
    });
}

fn open_window(
    cx: &mut App,
    command: CommandSpec,
    services: Arc<AppServices>,
    events: UnboundedReceiver<AppEvent>,
) -> Result<(), String> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(120.0), px(120.0)),
            size: size(px(1200.0), px(800.0)),
        })),
        ..WindowOptions::default()
    };
    let (handle, prototype) = gpui_kit::open_window(options, cx, move |_, cx| {
        cx.new(|cx| Prototype::new(cx, &command, services, events))
    })
    .map_err(|error| error.to_string())?;
    let focus = prototype.read(cx).view.read(cx).focus_handle();
    handle
        .update(cx, |_, window, cx| {
            window.set_window_title(DEFAULT_TITLE);
            window.focus(&focus, cx);
            window.activate_window();
        })
        .map_err(|error| error.to_string())?;

    #[cfg(all(feature = "automation", debug_assertions))]
    super::automation::start(handle, prototype, cx);
    Ok(())
}

/// The window's root view: the terminal and the core session feeding it.
pub(super) struct Prototype {
    view: Entity<TerminalView>,
    session: Option<Session>,
    title: Arc<Mutex<Option<String>>>,
    current_title: String,
}

impl Prototype {
    #[cfg(all(feature = "automation", debug_assertions))]
    pub(super) fn automation_state(&self) -> String {
        format!("title={}", self.current_title)
    }

    fn new(
        cx: &mut Context<Self>,
        command: &CommandSpec,
        services: Arc<AppServices>,
        mut events: UnboundedReceiver<AppEvent>,
    ) -> Self {
        let terminal = Arc::new(Terminal::new(Config::default(), INITIAL_GRID, INITIAL_CELL));
        let state = SessionState {
            terminal: Arc::clone(&terminal),
            size: Arc::new(Mutex::new(WindowSize {
                grid: INITIAL_GRID,
                cell: INITIAL_CELL,
            })),
            title: Arc::new(Mutex::new(None)),
        };
        let title = Arc::clone(&state.title);
        let size = Arc::clone(&state.size);
        let (wake, mut wake_rx) = tokio::sync::mpsc::channel(1);
        let started = command.connection_options().and_then(|connection| {
            Session::spawn(
                Arc::clone(&services),
                connection,
                state.clone(),
                grid_rows(INITIAL_GRID),
                grid_cols(INITIAL_GRID),
                wake.clone(),
            )
        });
        let session = match started {
            Ok(session) => {
                log_conpty_source();
                session
            }
            Err(error) => {
                eprintln!("tterm-gpui: {error}");
                terminal.write(format!("\x1b[31m[{error}]\x1b[0m\r\n").as_bytes());
                let view = cx.new(|cx| TerminalView::new(cx, terminal, Default::default()));
                return Self {
                    view,
                    session: None,
                    title,
                    current_title: DEFAULT_TITLE.to_string(),
                };
            }
        };
        let prompt = PromptControl::new(
            Arc::clone(&terminal),
            Arc::clone(&services.shared.prompts),
            wake,
        );
        let session_input = session.input();
        let prompt_input = Arc::clone(&prompt);
        let config = TerminalViewConfig {
            input: Some(Rc::new(move |bytes: &[u8]| {
                if !prompt_input.handle_input(bytes) {
                    session_input(bytes);
                }
            })),
            resize: Some(Rc::new(session.resizer(size))),
            ..TerminalViewConfig::default()
        };
        let view = cx.new(|cx| TerminalView::new(cx, Arc::clone(&terminal), config));

        cx.spawn(async move |prototype, cx| {
            while wake_rx.recv().await.is_some() {
                let updated = prototype.update(cx, |prototype, cx| {
                    prototype.view.update(cx, |view, cx| view.notify_output(cx));
                    if prototype.title.lock().is_ok_and(|title| title.is_some()) {
                        cx.notify();
                    }
                });
                if updated.is_err() {
                    break;
                }
            }
        })
        .detach();

        cx.spawn(async move |prototype, cx| {
            while let Some(event) = events.recv().await {
                match event {
                    AppEvent::Ssh(SshEvent::HostKeyPrompt { payload, .. }) => {
                        prompt.show_host_key(payload);
                    }
                    AppEvent::Ssh(SshEvent::HostKeyPromptExpired { request_id, .. }) => {
                        prompt.expire(&request_id, false);
                    }
                    AppEvent::Ssh(SshEvent::AuthPrompt { payload, .. }) => {
                        prompt.show_auth(payload);
                    }
                    AppEvent::Ssh(SshEvent::AuthPromptExpired { request_id, .. }) => {
                        prompt.expire(&request_id, true);
                    }
                    AppEvent::Terminal(TerminalEvent::Output { text, .. }) => {
                        terminal.write(text.as_bytes());
                    }
                    _ => {}
                }
                if prototype
                    .update(cx, |prototype, cx| {
                        prototype.view.update(cx, |view, cx| view.notify_output(cx));
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        Self {
            view,
            session: Some(session),
            title,
            current_title: DEFAULT_TITLE.to_string(),
        }
    }
}

impl Drop for Prototype {
    fn drop(&mut self) {
        if let Some(session) = self.session.take() {
            drop(session.shutdown());
        }
    }
}

impl Render for Prototype {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if let Ok(mut title) = self.title.lock()
            && let Some(title) = title.take()
        {
            window.set_window_title(&title);
            self.current_title = title;
        }
        div().size_full().child(self.view.clone())
    }
}

fn grid_rows(grid: GridSize) -> u16 {
    u16::try_from(grid.screen_lines).unwrap_or(u16::MAX)
}

fn grid_cols(grid: GridSize) -> u16 {
    u16::try_from(grid.columns).unwrap_or(u16::MAX)
}
