//! P0-11 synthetic recording player. No PTY, application data or real input.
//!
//! `cargo run --workspace --example terminal_replay -- unicode --original`
//! `cargo run --workspace --example terminal_replay -- attributes --fast --screenshot target/replay.png`

#[path = "../../tterm-terminal/tests/support/recording.rs"]
mod recording;
#[path = "../../tterm-terminal/tests/support/screen.rs"]
mod screen;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, Entity, Window, WindowBounds, WindowOptions, div, point, px, size};
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::font::{FontSet, SCROLLBAR_GUTTER, TerminalFont, measure_cell};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

struct Options {
    name: String,
    original: bool,
    screenshot: Option<PathBuf>,
    frames: Vec<recording::Frame>,
}

fn options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let name = args.next().ok_or("expected fixture name")?;
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
    {
        return Err("invalid fixture name".into());
    }
    let mut original = false;
    let mut screenshot = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--original" => original = true,
            "--fast" => original = false,
            "--screenshot" => {
                screenshot = Some(PathBuf::from(args.next().ok_or("expected PNG path")?))
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tterm-terminal/tests/fixtures/replay")
        .join(format!("{name}.bin"));
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    Ok(Options {
        name,
        original,
        screenshot,
        frames: recording::decode(&bytes)?,
    })
}

struct Player {
    name: String,
    view: Entity<TerminalView>,
    terminal: Arc<Terminal>,
    grid: GridSize,
    original: bool,
    frames: Option<Vec<recording::Frame>>,
    screenshot: Option<PathBuf>,
}

impl Player {
    fn new(options: Options, cx: &mut Context<Self>) -> Self {
        let terminal = Arc::new(Terminal::new(
            Config {
                scrollback: 12,
                ..Config::default()
            },
            GridSize::new(40, 8),
            CellSize::new(8, 19),
        ));
        let view = cx
            .new(|cx| TerminalView::new(cx, Arc::clone(&terminal), TerminalViewConfig::default()));
        Self {
            name: options.name,
            view,
            terminal,
            grid: GridSize::new(40, 8),
            original: options.original,
            frames: Some(options.frames),
            screenshot: options.screenshot,
        }
    }

    fn verify(&self) -> Result<(), String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tterm-terminal/tests/fixtures/replay");
        let reference = std::fs::read_to_string(root.join(format!("{}.screen", self.name)))
            .map_err(|error| error.to_string())?;
        let expected = std::fs::read_to_string(root.join(format!("{}.diff", self.name)))
            .map_err(|error| error.to_string())?;
        let actual = screen::screen(&self.terminal);
        let diff = screen::differences(&reference, &actual);
        if diff != expected {
            return Err(format!(
                "window replay differs from reviewed model result:\n{diff}"
            ));
        }
        println!(
            "window comparison: {} changed records, exact documented result",
            diff.lines().count() / 2
        );
        Ok(())
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(frames) = self.frames.take() else {
            return;
        };
        let (sender, receiver) = async_channel::bounded(16);
        let (acknowledge, acknowledged) = async_channel::bounded(1);
        let terminal = Arc::clone(&self.terminal);
        let original = self.original;
        std::thread::spawn(move || {
            if acknowledged.recv_blocking().is_err() {
                return;
            }
            let start = Instant::now();
            for frame in frames {
                if original {
                    std::thread::sleep(frame.at.saturating_sub(start.elapsed()));
                }
                frame.apply(&terminal);
                let grid = match frame.action {
                    recording::Action::Resize(grid) => Some(grid),
                    recording::Action::Output(_) => None,
                };
                if sender.send_blocking(grid).is_err() {
                    return;
                }
                if acknowledged.recv_blocking().is_err() {
                    return;
                }
            }
        });
        let handle = window.window_handle();
        cx.spawn(async move |weak, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
            let _ = acknowledge.send(()).await;
            while let Ok(grid) = receiver.recv().await {
                if handle
                    .update(cx, |_, window, cx| {
                        let _ = weak.update(cx, |player, cx| {
                            if let Some(grid) = grid {
                                player.grid = grid;
                            }
                            player.view.update(cx, |view, cx| view.notify_output(cx));
                            cx.notify();
                        });
                        window.refresh();
                        let _ = window.draw(cx);
                    })
                    .is_err()
                {
                    return;
                }
                let _ = acknowledge.send(()).await;
            }
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
            let _ = handle.update(cx, |_, window, cx| {
                window.refresh();
                let _ = window.draw(cx);
                let path = weak
                    .update(cx, |player, _| {
                        if let Err(error) = player.verify() {
                            eprintln!("terminal_replay: {error}");
                            std::process::exit(1);
                        }
                        let grid = player.terminal.snapshot().grid();
                        println!(
                            "replay complete: {}x{}, mode={}",
                            grid.columns,
                            grid.screen_lines,
                            if player.original { "original" } else { "fast" }
                        );
                        player.screenshot.clone()
                    })
                    .ok()
                    .flatten();
                if let Some(path) = path {
                    let result = window
                        .render_to_image()
                        .map_err(|error| error.to_string())
                        .and_then(|image| image.save(&path).map_err(|error| error.to_string()));
                    if let Err(error) = result {
                        eprintln!("replay screenshot failed: {error}");
                        std::process::exit(1);
                    }
                    println!("screenshot: {}", path.display());
                    cx.quit();
                }
            });
        })
        .detach();
    }
}

impl Render for Player {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let font = TerminalFont::default();
        let fonts = FontSet::resolve(&font, window.text_system());
        let geometry = measure_cell(window.text_system(), &fonts, &font, window.scale_factor());
        // Match the recording grid exactly, leaving unused window area outside the view.
        let width = (self.grid.columns as f32 * geometry.cell_width + SCROLLBAR_GUTTER).ceil();
        let height = (self.grid.screen_lines as f32 * geometry.cell_height).ceil();
        self.start(window, cx);
        div()
            .size_full()
            .bg(gpui_kit::rgb(0x181818))
            .child(div().w(px(width)).h(px(height)).child(self.view.clone()))
    }
}

fn main() {
    let options = match options() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("terminal_replay: {error}");
            std::process::exit(2);
        }
    };
    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(100.0), px(100.0)),
                size: size(px(640.0), px(360.0)),
            })),
            focus: false,
            ..WindowOptions::default()
        };
        if let Err(error) = gpui_kit::open_window(window_options, cx, |window, cx| {
            window.set_window_title(&format!("P0-11 replay: {}", options.name));
            cx.new(|cx| Player::new(options, cx))
        }) {
            eprintln!("terminal_replay: {error}");
            std::process::exit(1);
        }
    });
}
