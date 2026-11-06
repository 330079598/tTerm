//! P0-10's terminal element demo.
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
//! Input is P0-13's job: this example displays output and scrolls with the
//! wheel, but does not forward typing.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::prelude::*;
use gpui_kit::{
    App, Bounds, Entity, TextSystem, Window, WindowBounds, WindowOptions, div, point, px, size,
};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::font::{FontSet, TerminalFont, grid_for, measure_cell};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

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
    bytes.extend_from_slice(b"\x1b[10;1H\x1b[?25h");
    bytes
}

struct Demo {
    view: Entity<TerminalView>,
    _child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    _master: Option<Arc<Mutex<Box<dyn MasterPty + Send>>>>,
    _reader: Option<std::thread::JoinHandle<()>>,
}

impl Render for Demo {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.view.clone())
    }
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    match (arguments.next().as_deref(), arguments.next()) {
        (Some("--screenshot"), Some(path)) => {
            if let Err(error) = screenshot(Path::new(&path)) {
                eprintln!("terminal_view: {error}");
                std::process::exit(1);
            }
            println!("terminal_view: wrote {path}");
        }
        (Some("--metrics"), None) => print_metrics(),
        (Some("--idle-check"), None) => run_idle_check(),
        (None, None) => run_window(),
        _ => {
            eprintln!("usage: terminal_view [--screenshot <path> | --metrics | --idle-check]");
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

fn measure(font: &TerminalFont) -> (FontSet, tterm_terminal_view::view::font::CellGeometry) {
    let platform = gpui_kit::platform::current_platform(true);
    let text_system = TextSystem::new(platform.text_system());
    let fonts = FontSet::new(font);
    let metrics = measure_cell(&text_system, &fonts, font, 1.0);
    let font_id = text_system.resolve_font(fonts.regular());
    println!(
        "metrics: units_per_em={} ascent={:.4} descent={:.4} cap_height={:.4}",
        text_system.units_per_em(font_id),
        text_system.ascent(font_id, px(font.size)).as_f32(),
        text_system.descent(font_id, px(font.size)).as_f32(),
        text_system.cap_height(font_id, px(font.size)).as_f32()
    );
    (fonts, metrics)
}

fn print_metrics() {
    let font = TerminalFont::default();
    let (_fonts, metrics) = measure(&font);
    println!(
        "font={} size={} line_height={} letter_spacing={}",
        font.primary_family(),
        font.size,
        font.line_height,
        font.letter_spacing
    );
    println!(
        "cell={:.4}x{:.4} logical, dpr={}",
        metrics.cell_width, metrics.cell_height, metrics.scale_factor
    );
    for (width, height) in [(800.0, 600.0), (1200.0, 800.0)] {
        let grid = grid_for(px(width), px(height), &metrics);
        println!(
            "{width}x{height}: {} columns x {} rows",
            grid.columns, grid.screen_lines
        );
    }
}

fn run_window() {
    gpui_kit::application().run(|cx: &mut App| {
        gpui_kit::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        if let Err(error) = open_demo_window(cx) {
            eprintln!("terminal_view: {error}");
            cx.quit();
        }
    });
}

fn open_demo_window(cx: &mut App) -> Result<(), String> {
    let bounds = Bounds {
        origin: point(px(120.0), px(120.0)),
        size: size(px(1200.0), px(800.0)),
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..WindowOptions::default()
    };
    let (handle, demo) = gpui_kit::open_window(options, cx, |_, cx| cx.new(Demo::new))
        .map_err(|error| error.to_string())?;
    let focus = demo.read(cx).view.read(cx).focus_handle();
    handle
        .update(cx, |_, window, cx| {
            window.focus(&focus, cx);
            window.activate_window();
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        match Self::spawn(cx) {
            Ok(demo) => demo,
            Err(error) => {
                eprintln!("terminal_view: could not start a shell: {error}");
                let view = cx
                    .new(|cx| TerminalView::new(cx, new_terminal(), TerminalViewConfig::default()));
                Self {
                    view,
                    _child: None,
                    _master: None,
                    _reader: None,
                }
            }
        }
    }

    fn spawn(cx: &mut Context<Self>) -> Result<Self, String> {
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
            .spawn_command(CommandBuilder::new_default_prog())
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
        let master = Arc::new(Mutex::new(pair.master));
        let writer = Arc::new(Mutex::new(writer));

        let (sender, receiver) = async_channel::bounded::<()>(1);
        let input_terminal = Arc::clone(&terminal);
        let reader_thread = std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        input_terminal.write(&buffer[..count]);
                        let _ = sender.try_send(());
                    }
                }
            }
        });

        let input_writer = Arc::clone(&writer);
        let resize_master = Arc::clone(&master);
        let config = TerminalViewConfig {
            input: Some(Rc::new(move |bytes: &[u8]| {
                if let Ok(mut writer) = input_writer.lock() {
                    let _ = writer.write_all(bytes);
                }
            })),
            resize: Some(Rc::new(move |grid: GridSize, cell: CellSize| {
                if let Ok(master) = resize_master.lock() {
                    let _ = master.resize(PtySize {
                        rows: grid.screen_lines as u16,
                        cols: grid.columns as u16,
                        pixel_width: (cell.width as usize * grid.columns) as u16,
                        pixel_height: (cell.height as usize * grid.screen_lines) as u16,
                    });
                }
            })),
            ..TerminalViewConfig::default()
        };
        let view = cx.new(|cx| TerminalView::new(cx, terminal, config));

        let output_view = view.downgrade();
        cx.spawn(async move |_demo, cx| {
            while receiver.recv().await.is_ok() {
                if output_view
                    .update(cx, |view, cx| view.notify_output(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        Ok(Self {
            view,
            _child: Some(child),
            _master: Some(master),
            _reader: Some(reader_thread),
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
