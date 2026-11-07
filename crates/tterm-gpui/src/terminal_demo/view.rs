//! P0-14's local PTY terminal prototype.
//!
//! With no arguments it opens a window running the platform's default shell
//! in a real PTY. `--screenshot <png>` renders a fixed synthetic screen
//! headlessly, which makes the regression image reproducible without screen
//! recording permission. `--metrics` prints the measured cell size and the
//! row/column counts for 800x600 and 1200x800.
//!
//! ```sh
//! cargo run -p tterm-gpui --example terminal_view
//! cargo run -p tterm-gpui --example terminal_view -- \
//!   --screenshot docs/gpui-migration/reports/assets/P0-10-windows.png
//! cargo run -p tterm-gpui --example terminal_view -- --metrics
//! ```
//!
//! P0-13 connects keyboard, selection, clipboard and native text input to
//! the existing PTY writer. Copy/paste uses Command+C/V on macOS and
//! Control+Shift+C/V elsewhere.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::prelude::*;
use gpui_kit::{
    AnyWindowHandle, App, Bounds, Entity, TextSystem, Window, WindowBounds, WindowOptions, div,
    point, px, size,
};
use portable_pty::{PtySize, native_pty_system};
use tterm_terminal::grid::{CellSize, GridSize, WindowSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::font::{FontSet, TerminalFont, grid_for, measure_cell};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

mod command;
mod conpty;
mod output;
mod session;

/// The grid the demo starts with, before the first frame measures the font.
const INITIAL_COLUMNS: usize = 80;
const INITIAL_ROWS: usize = 24;

/// The fixed stream the headless screenshot draws: colors, every style,
/// true color, a 256-color cube, wide characters, emoji, box drawing and
/// Powerline.
fn screenshot_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"\x1b[2J\x1b[H\x1b[?25h");
    bytes.extend_from_slice(
        "\x1b[1mBold\x1b[0m \x1b[3mItalic\x1b[0m \x1b[4mUnderline\x1b[0m \
         \x1b[9mStrike\x1b[0m \x1b[2mDim\x1b[0m \x1b[7mInverse\x1b[0m \
         \x1b[8mHidden\x1b[0m\r\n"
            .as_bytes(),
    );
    bytes.extend_from_slice(
        b"\x1b[38;2;255;120;0mTruecolor\x1b[0m \x1b[38;5;46m256-color\x1b[0m \
          \x1b[48;5;24m shaded \x1b[0m\r\n",
    );
    bytes.extend_from_slice(
        b"\x1b[31mRed \x1b[32mGreen \x1b[34mBlue \x1b[35mMagenta \
          \x1b[36mCyan \x1b[33mYellow\x1b[0m\r\n",
    );
    bytes.extend_from_slice(
        b"\x1b[4:1mSingle\x1b[0m \x1b[4:2mDouble\x1b[0m \x1b[4:3mCurly\x1b[0m \
          \x1b[4:4mDotted\x1b[0m \x1b[4:5mDashed\x1b[0m\r\n",
    );
    bytes.extend_from_slice("Wide: 中文 全角 emoji: 😀 🚀 👨‍👩‍👧‍👦\r\n".as_bytes());
    bytes.extend_from_slice("Box: ┌───────┬───────┐  ╔═══════╦═══════╗\r\n".as_bytes());
    bytes.extend_from_slice("     │       │       │  ║       ║       ║\r\n".as_bytes());
    bytes.extend_from_slice("     └───────┴───────┘  ╚═══════╩═══════╝\r\n".as_bytes());
    bytes.extend_from_slice(
        "Blocks: ░░░ ▒▒▒ ▓▓▓ ███   Powerline: \u{e0b0}\u{e0b1}\u{e0b2}\u{e0b3}\r\n".as_bytes(),
    );
    bytes
        .extend_from_slice(b"\x1b[38;5;39mThe quick brown fox jumps over the lazy dog.\x1b[0m\r\n");
    bytes.extend_from_slice(b"\x1b[38;5;214m0123456789 !@#$%^&*()_+-=[]{};':\",./<>?\x1b[0m\r\n");
    // Park the cursor on the "q" of "quick", away from column 0, so the image
    // shows that the block cursor lands on its own cell.
    bytes.extend_from_slice(b"\x1b[10;5H\x1b[?25h");
    bytes
}

struct Demo {
    view: Entity<TerminalView>,
    session: Option<Session>,
    title: Arc<Mutex<Option<String>>>,
    window_handle: Arc<Mutex<Option<AnyWindowHandle>>>,
}

impl Drop for Demo {
    fn drop(&mut self) {
        if let Some(session) = self.session.take() {
            session.shutdown();
        }
    }
}

impl Render for Demo {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if let Ok(mut title) = self.title.lock() {
            if let Some(title) = title.take() {
                window.set_window_title(&title);
            }
        }
        div().size_full().child(self.view.clone())
    }
}

use self::command::{CommandSpec, command_from_environment};
use self::conpty::prepare_conpty;
use self::output::handle_intent;
use self::session::{Session, WriteMessage};
pub fn run() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match arguments.first().map(String::as_str) {
        Some("--screenshot") => {
            if arguments.len() != 2 {
                eprintln!(
                    "usage: tterm-gpui [--screenshot <path> | --metrics | --idle-check | --ssh <target> [args...]]"
                );
                std::process::exit(2);
            }
            let path = &arguments[1];
            if let Err(error) = screenshot(Path::new(path)) {
                eprintln!("terminal_view: {error}");
                std::process::exit(1);
            }
            println!("terminal_view: wrote {path}");
        }
        Some("--metrics") if arguments.len() == 1 => print_metrics(),
        Some("--idle-check") if arguments.len() == 1 => run_idle_check(),
        Some("--ssh") => {
            let target = match arguments.get(1) {
                Some(target) => target.clone(),
                None => {
                    eprintln!("--ssh requires a user@wsl-ip target");
                    std::process::exit(2);
                }
            };
            run_window(CommandSpec::Ssh {
                target,
                args: arguments.into_iter().skip(2).collect(),
            });
        }
        None => run_window(command_from_environment()),
        _ => {
            eprintln!(
                "usage: tterm-gpui [--screenshot <path> | --metrics | --idle-check | --ssh <target> [args...]]"
            );
            std::process::exit(2);
        }
    }
}

fn new_terminal() -> Arc<Terminal> {
    Arc::new(Terminal::new(
        Config::default(),
        GridSize::new(INITIAL_COLUMNS, INITIAL_ROWS),
        CellSize::new(8, 16),
    ))
}

/// The platform's real text system.
///
/// A headless GPUI platform on Windows has no text system (it reports fixed
/// placeholder metrics and accepts every family name), so the measurement
/// asks the windowed platform's text system without opening a window.
fn real_text_system() -> TextSystem {
    let platform = gpui_kit::platform::current_platform(false);
    TextSystem::new(platform.text_system())
}

fn print_metrics() {
    let font = TerminalFont::default();
    let text_system = real_text_system();
    let fonts = FontSet::resolve(&font, &text_system);
    let font_id = text_system.resolve_font(fonts.regular());
    println!(
        "font={} (chain starts with {}) size={} line_height={} letter_spacing={}",
        fonts.family(),
        font.primary_family(),
        font.size,
        font.line_height,
        font.letter_spacing
    );
    println!(
        "metrics: units_per_em={} ascent={:.4} descent={:.4} advance(m)={:.4}",
        text_system.units_per_em(font_id),
        text_system.ascent(font_id, px(font.size)).as_f32(),
        text_system.descent(font_id, px(font.size)).as_f32(),
        text_system
            .advance(font_id, px(font.size), 'm')
            .map(|advance| advance.width.as_f32())
            .unwrap_or(f32::NAN)
    );
    for scale_factor in [1.0, 1.25, 1.5, 2.0] {
        let metrics = measure_cell(&text_system, &fonts, &font, scale_factor);
        println!(
            "dpr={scale_factor}: cell={:.4}x{:.4} logical",
            metrics.cell_width, metrics.cell_height
        );
        // The demo terminal keeps scrollback, so the gutter is reserved.
        for (width, height) in [(800.0, 600.0), (1200.0, 800.0)] {
            let grid = grid_for(px(width), px(height), &metrics, true);
            println!(
                "  {width}x{height}: {} columns x {} rows",
                grid.columns, grid.screen_lines
            );
        }
    }
}

fn run_window(command: CommandSpec) {
    prepare_conpty();
    gpui_kit::application().run(|cx: &mut App| {
        gpui_kit::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        if let Err(error) = open_demo_window(cx, command) {
            eprintln!("terminal_view: {error}");
            cx.quit();
        }
    });
}

fn open_demo_window(cx: &mut App, command: CommandSpec) -> Result<(), String> {
    let bounds = Bounds {
        origin: point(px(120.0), px(120.0)),
        size: size(px(1200.0), px(800.0)),
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..WindowOptions::default()
    };
    let (handle, demo) = gpui_kit::open_window(options, cx, move |_, cx| {
        let command = command.clone();
        cx.new(|cx| Demo::new(cx, command))
    })
    .map_err(|error| error.to_string())?;
    let focus = demo.read(cx).view.read(cx).focus_handle();
    let window_handle: AnyWindowHandle = handle.into();
    demo.update(cx, |demo, _| {
        if let Ok(mut current) = demo.window_handle.lock() {
            *current = Some(window_handle);
        }
    });
    handle
        .update(cx, |_, window, cx| {
            window.focus(&focus, cx);
            window.activate_window();
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

impl Demo {
    fn new(cx: &mut Context<Self>, command: CommandSpec) -> Self {
        match Self::spawn(cx, command) {
            Ok(demo) => demo,
            Err(error) => {
                eprintln!("terminal_view: could not start a shell: {error}");
                let view = cx
                    .new(|cx| TerminalView::new(cx, new_terminal(), TerminalViewConfig::default()));
                Self {
                    view,
                    session: None,
                    title: Arc::new(Mutex::new(None)),
                    window_handle: Arc::new(Mutex::new(None)),
                }
            }
        }
    }

    fn spawn(cx: &mut Context<Self>, command: CommandSpec) -> Result<Self, String> {
        let terminal = new_terminal();
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: INITIAL_ROWS as u16,
                cols: INITIAL_COLUMNS as u16,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| error.to_string())?;
        let child = pair
            .slave
            .spawn_command(command.builder())
            .map_err(|error| error.to_string())?;
        drop(pair.slave);
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| error.to_string())?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| error.to_string())?;
        let master = Arc::new(Mutex::new(Some(pair.master)));
        let child = Arc::new(Mutex::new(child));
        let alive = Arc::new(AtomicBool::new(true));
        let (bytes_tx, bytes_rx) = mpsc::sync_channel::<Vec<u8>>(8);
        let (write_tx, write_rx) = mpsc::channel::<WriteMessage>();
        let (wake_tx, wake_rx) = async_channel::bounded::<()>(1);
        let parser_terminal = Arc::clone(&terminal);
        let parser_title = Arc::new(Mutex::new(None));
        let parser_title_for_thread = Arc::clone(&parser_title);
        let parser_writer = write_tx.clone();
        let parser_size = Arc::new(Mutex::new(WindowSize {
            grid: GridSize::new(INITIAL_COLUMNS, INITIAL_ROWS),
            cell: CellSize::new(8, 16),
        }));
        let parser_size_for_thread = Arc::clone(&parser_size);
        let reader_tx = bytes_tx.clone();
        let reader_thread = std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) if reader_tx.send(buffer[..count].to_vec()).is_err() => break,
                    Ok(_) => {}
                }
            }
        });
        let parser_wake = wake_tx.clone();
        let parser_thread = std::thread::spawn(move || {
            while let Ok(first) = bytes_rx.recv() {
                let mut batch = first;
                while batch.len() < 64 * 1024 {
                    match bytes_rx.try_recv() {
                        Ok(bytes) => batch.extend_from_slice(&bytes),
                        Err(_) => break,
                    }
                }
                parser_terminal.write(&batch);
                for intent in parser_terminal.drain_intents() {
                    handle_intent(
                        intent,
                        &parser_terminal,
                        &parser_writer,
                        &parser_size_for_thread,
                        &parser_title_for_thread,
                    );
                }
                let _ = parser_wake.try_send(());
            }
        });
        let writer_thread = std::thread::spawn(move || {
            let mut writer = writer;
            while let Ok(message) = write_rx.recv() {
                match message {
                    WriteMessage::Bytes(bytes) => {
                        if writer.write_all(&bytes).is_err() {
                            break;
                        }
                    }
                    WriteMessage::Close => break,
                }
            }
        });
        let child_for_monitor = Arc::clone(&child);
        let monitor_bytes = bytes_tx.clone();
        let monitor_wake = wake_tx.clone();
        let monitor_alive = Arc::clone(&alive);
        let child_monitor = std::thread::spawn(move || {
            loop {
                let exited = child_for_monitor
                    .lock()
                    .ok()
                    .and_then(|mut child| child.try_wait().ok().flatten())
                    .is_some();
                if exited {
                    monitor_alive.store(false, Ordering::Release);
                    let _ = monitor_bytes.send(b"\r\n\x1b[33m[Process exited]\x1b[0m\r\n".to_vec());
                    let _ = monitor_wake.try_send(());
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        });

        let input_writer = write_tx.clone();
        let input_alive = Arc::clone(&alive);
        let resize_master = Arc::clone(&master);
        let config = TerminalViewConfig {
            input: Some(Rc::new(move |bytes: &[u8]| {
                if input_alive.load(Ordering::Acquire) {
                    let _ = input_writer.send(WriteMessage::Bytes(bytes.to_vec()));
                }
            })),
            resize: Some(Rc::new(move |grid: GridSize, cell: CellSize| {
                if let Ok(mut size) = parser_size.lock() {
                    *size = WindowSize { grid, cell };
                }
                if let Ok(mut master) = resize_master.lock() {
                    if let Some(master) = master.as_mut() {
                        let _ = master.resize(PtySize {
                            rows: grid.screen_lines as u16,
                            cols: grid.columns as u16,
                            pixel_width: (cell.width as usize * grid.columns) as u16,
                            pixel_height: (cell.height as usize * grid.screen_lines) as u16,
                        });
                    }
                }
            })),
            ..TerminalViewConfig::default()
        };
        let view = cx.new(|cx| TerminalView::new(cx, terminal, config));

        let output_view = view.downgrade();
        let output_title = Arc::clone(&parser_title);
        let output_window = Arc::new(Mutex::new(None));
        let title_window = Arc::clone(&output_window);
        cx.spawn(async move |_demo, cx| {
            while wake_rx.recv().await.is_ok() {
                if output_view
                    .update(cx, |view, cx| view.notify_output(cx))
                    .is_err()
                {
                    break;
                }
                if let Ok(mut title) = output_title.lock() {
                    if let Some(title) = title.take() {
                        if let Ok(current) = title_window.lock() {
                            if let Some(handle) = *current {
                                let _ = cx.update_window(handle, |_, window, _| {
                                    window.set_window_title(&title)
                                });
                            }
                        }
                    }
                }
            }
        })
        .detach();

        Ok(Self {
            view,
            session: Some(Session {
                child: Some(child),
                master: Some(master),
                writer: Some(write_tx),
                reader: Some(reader_thread),
                parser: Some(parser_thread),
                writer_thread: Some(writer_thread),
                child_monitor: Some(child_monitor),
                alive,
            }),
            title: parser_title,
            window_handle: output_window,
        })
    }
}

/// A real window with an empty terminal and no focus. After ten seconds it
/// prints the frame count and quits; a non-zero increase means something in
/// the element schedules frames while idle.
struct IdleProbe {
    view: Entity<TerminalView>,
}

impl IdleProbe {
    fn new(cx: &mut Context<Self>) -> Self {
        let view =
            cx.new(|cx| TerminalView::new(cx, new_terminal(), TerminalViewConfig::default()));
        let probe = Self { view };
        cx.spawn(async move |weak, cx| {
            // Let the first window frames settle, then measure the next 9
            // seconds. A visible, unfocused, unchanged terminal must add no
            // frames at all.
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let start = weak
                .update(cx, |probe, cx| probe.view.read(cx).stats().frames)
                .unwrap_or(0);
            cx.background_executor().timer(Duration::from_secs(9)).await;
            let _ = weak.update(cx, |probe, cx| {
                let stats = probe.view.read(cx).stats();
                println!(
                    "idle_check: start_frames={start} end_frames={} delta={} redraw_requests={} blink_toggles={} output_notifications={}",
                    stats.frames,
                    stats.frames.saturating_sub(start),
                    stats.redraw_requests,
                    stats.blink_toggles,
                    stats.output_notifications
                );
            });
            cx.update(|app| app.quit());
        })
        .detach();
        probe
    }
}

impl Render for IdleProbe {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.view.clone())
    }
}

fn run_idle_check() {
    gpui_kit::application().run(|cx: &mut App| {
        gpui_kit::init(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(120.0), px(120.0)),
                size: size(px(1200.0), px(800.0)),
            })),
            focus: false,
            show: true,
            ..WindowOptions::default()
        };
        if let Err(error) = gpui_kit::open_window(options, cx, |_, cx| cx.new(IdleProbe::new)) {
            eprintln!("terminal_view: {error}");
            cx.quit();
        }
    });
}

fn screenshot(path: &Path) -> Result<(), String> {
    // GPUI's platform-neutral headless renderer is macOS/Linux only. On
    // Windows the same `Window::render_to_image` path is reachable through a
    // real window placed off-screen and rendered into a texture, so the
    // capture never involves the display or screen-recording permission.
    let path = path.to_path_buf();
    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        match capture_window(cx, &path) {
            Ok(()) => {
                cx.quit();
            }
            Err(error) => {
                eprintln!("terminal_view: {error}");
                cx.quit();
            }
        }
    });
    Ok(())
}

fn capture_window(cx: &mut App, path: &PathBuf) -> Result<(), String> {
    let terminal = new_terminal();
    let bounds = Bounds {
        origin: point(px(-10_000.0), px(-10_000.0)),
        size: size(px(1200.0), px(800.0)),
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        focus: true,
        show: true,
        ..WindowOptions::default()
    };
    let (handle, view) = gpui_kit::open_window(options, cx, |_, cx| {
        cx.new(|cx| TerminalView::new(cx, Arc::clone(&terminal), TerminalViewConfig::default()))
    })
    .map_err(|error| error.to_string())?;
    let focus = view.read(cx).focus_handle();
    handle
        .update(cx, |_, window, _| {
            println!(
                "terminal_view: scale_factor={} bounds={:?} viewport={:?}",
                window.scale_factor(),
                window.bounds(),
                window.viewport_size()
            );
        })
        .map_err(|error| error.to_string())?;

    // First frame: measure the font and resize the model to the window.
    handle
        .update(cx, |_, window, cx| {
            let _ = window.draw(cx);
        })
        .map_err(|error| error.to_string())?;
    terminal.write(&screenshot_bytes());

    let image = handle
        .update(cx, |_, window, cx| {
            window.focus(&focus, cx);
            let _ = window.draw(cx);
            window.render_to_image()
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    image.save(path).map_err(|error| error.to_string())
}
