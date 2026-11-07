//! The prototype's window: one terminal view running a local shell, or the
//! system `ssh`, in a real PTY.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, Entity, Window, WindowBounds, WindowOptions, div, point, px, size};
use tterm_terminal::grid::{CellSize, GridSize, WindowSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

use super::command::CommandSpec;
use super::conpty::{log_conpty_source, prepare_conpty};
use super::intents::DEFAULT_TITLE;
use super::pipeline::SessionState;
use super::session::{Session, pty_size};

/// The grid and cell size the session starts with. The view measures the
/// font on its first frame and resizes both the model and the PTY.
const INITIAL_GRID: GridSize = GridSize {
    columns: 80,
    screen_lines: 24,
};
const INITIAL_CELL: CellSize = CellSize {
    width: 8,
    height: 16,
};

/// Runs the application with one prototype window until it closes.
pub(super) fn run_window(command: CommandSpec) {
    prepare_conpty();
    gpui_kit::application().run(|cx: &mut App| {
        gpui_kit::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        if let Err(error) = open_window(cx, command) {
            eprintln!("tterm-gpui: {error}");
            cx.quit();
        }
    });
}

fn open_window(cx: &mut App, command: CommandSpec) -> Result<(), String> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(120.0), px(120.0)),
            size: size(px(1200.0), px(800.0)),
        })),
        ..WindowOptions::default()
    };
    let (handle, prototype) = gpui_kit::open_window(options, cx, move |_, cx| {
        cx.new(|cx| Prototype::new(cx, &command))
    })
    .map_err(|error| error.to_string())?;
    let focus = prototype.read(cx).view.read(cx).focus_handle();
    handle
        .update(cx, |_, window, cx| {
            window.set_window_title(DEFAULT_TITLE);
            window.focus(&focus, cx);
            window.activate_window();
        })
        .map_err(|error| error.to_string())
}

/// The window's root view: the terminal and the session that feeds it.
struct Prototype {
    view: Entity<TerminalView>,
    session: Option<Session>,
    /// The latest title the program set, which the next render shows.
    title: Arc<Mutex<Option<String>>>,
}

impl Prototype {
    fn new(cx: &mut Context<Self>, command: &CommandSpec) -> Self {
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
        let started = Session::spawn(
            command.builder(),
            pty_size(INITIAL_GRID, INITIAL_CELL),
            state,
        );
        let (session, wake) = match started {
            Ok(started) => {
                log_conpty_source();
                started
            }
            Err(error) => {
                eprintln!("tterm-gpui: {error}");
                terminal.write(format!("\x1b[31m[{error}]\x1b[0m\r\n").as_bytes());
                let view = cx.new(|cx| TerminalView::new(cx, terminal, Default::default()));
                return Self {
                    view,
                    session: None,
                    title,
                };
            }
        };
        let config = TerminalViewConfig {
            input: Some(Rc::new(session.input())),
            resize: Some(Rc::new(session.resizer(size))),
            ..TerminalViewConfig::default()
        };
        let view = cx.new(|cx| TerminalView::new(cx, terminal, config));
        cx.spawn(async move |prototype, cx| {
            while wake.recv().await.is_ok() {
                let updated = prototype.update(cx, |prototype, cx| {
                    prototype.view.update(cx, |view, cx| view.notify_output(cx));
                    // The title is drawn by this view, not the terminal view.
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
        Self {
            view,
            session: Some(session),
            title,
        }
    }
}

impl Drop for Prototype {
    fn drop(&mut self) {
        if let Some(session) = self.session.take() {
            // Ends the program and closes the PTY now; the waiting happens on
            // a background thread nobody needs to join.
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
        }
        div().size_full().child(self.view.clone())
    }
}
