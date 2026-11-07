//! The prototype's window: one terminal view running a local shell, or the
//! system `ssh`, in a real PTY.

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::prelude::*;
use gpui_kit::{
    AnyWindowHandle, App, Bounds, Entity, Window, WindowBounds, WindowOptions, div, point, px, size,
};
use portable_pty::{PtySize, native_pty_system};
use tterm_terminal::grid::{CellSize, GridSize, WindowSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

use super::command::CommandSpec;
use super::conpty::prepare_conpty;
use super::output::handle_intent;
use super::session::{Session, WriteMessage};

/// The grid the demo starts with, before the first frame measures the font.
const INITIAL_COLUMNS: usize = 80;
const INITIAL_ROWS: usize = 24;

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
        if let Ok(mut title) = self.title.lock()
            && let Some(title) = title.take()
        {
            window.set_window_title(&title);
        }
        div().size_full().child(self.view.clone())
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
    let window_handle: AnyWindowHandle = handle;
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

fn start_writer(
    mut writer: Box<dyn std::io::Write + Send>,
    receiver: mpsc::Receiver<WriteMessage>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while let Ok(message) = receiver.recv() {
            match message {
                WriteMessage::Bytes(bytes) => {
                    if writer.write_all(&bytes).is_err() {
                        break;
                    }
                }
                WriteMessage::Close => break,
            }
        }
    })
}

fn monitor_child(
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
    bytes: mpsc::SyncSender<Vec<u8>>,
    wake: async_channel::Sender<()>,
    alive: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            let exited = child
                .lock()
                .ok()
                .and_then(|mut child| child.try_wait().ok().flatten())
                .is_some();
            if exited {
                alive.store(false, Ordering::Release);
                let _ = bytes.send(b"\r\n\x1b[33m[Process exited]\x1b[0m\r\n".to_vec());
                let _ = wake.try_send(());
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    })
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
        let writer_thread = start_writer(writer, write_rx);

        let child_monitor = monitor_child(
            Arc::clone(&child),
            bytes_tx.clone(),
            wake_tx.clone(),
            Arc::clone(&alive),
        );

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
                if let Ok(mut master) = resize_master.lock()
                    && let Some(master) = master.as_mut()
                {
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
                if let Ok(mut title) = output_title.lock()
                    && let Some(title) = title.take()
                    && let Ok(current) = title_window.lock()
                    && let Some(handle) = *current
                {
                    let _ =
                        cx.update_window(handle, |_, window, _| window.set_window_title(&title));
                }
            }
        })
        .detach();

        Ok(Self {
            view,
            session: Some(Session {
                child: Some(child),
                master,
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
