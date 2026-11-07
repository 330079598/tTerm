//! One local PTY session: the child process, the PTY master and the pipeline
//! threads. A prototype shortcut that P1-12 replaces with `AppServices`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use tterm_terminal::grid::{CellSize, GridSize, WindowSize};

use super::pipeline::{self, SessionState, WriteMessage};

/// What the terminal shows when the program ends by itself, as the Tauri
/// version does for a local tab.
pub(super) const EXIT_NOTICE: &[u8] = b"\r\n\x1b[33m[Process exited]\x1b[0m\r\n";

/// How often the monitor asks whether the program has ended. The reader
/// cannot tell: on Windows it only sees end of file once the ConPTY closes.
const EXIT_POLL: Duration = Duration::from_millis(50);

type SharedChild = Arc<Mutex<Box<dyn Child + Send + Sync>>>;

/// The PTY master, shared with the view's resize callback. Closing the
/// session takes it out, so the callback cannot keep the ConPTY open.
type MasterSlot = Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>;

/// A running program in a PTY and the threads around it.
pub(super) struct Session {
    child: SharedChild,
    master: MasterSlot,
    writes: Sender<WriteMessage>,
    /// False once the program ended or the session closed; input stops then.
    alive: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl Session {
    /// Starts `command` in a new PTY and returns the session with the
    /// channel that wakes the view when new output was applied.
    pub fn spawn(
        command: CommandBuilder,
        size: PtySize,
        state: SessionState,
    ) -> Result<(Self, async_channel::Receiver<()>), String> {
        let pair = native_pty_system()
            .openpty(size)
            .map_err(|error| format!("Failed to open a PTY: {error}"))?;
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("Failed to start the shell: {error}"))?;
        drop(pair.slave);
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("Failed to read the PTY: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("Failed to write the PTY: {error}"))?;

        let pipeline = pipeline::start(reader, writer, state);
        let child: SharedChild = Arc::new(Mutex::new(child));
        let alive = Arc::new(AtomicBool::new(true));
        let monitor = spawn_monitor(Arc::clone(&child), pipeline.queue, Arc::clone(&alive));
        let session = Self {
            child,
            master: Arc::new(Mutex::new(Some(pair.master))),
            writes: pipeline.writes,
            alive,
            threads: vec![pipeline.reader, pipeline.parser, pipeline.writer, monitor],
        };
        Ok((session, pipeline.wake))
    }

    /// Whether the program still runs and input still reaches it.
    #[cfg(test)]
    pub fn is_running(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    /// The view's input callback. It queues bytes for the writer while the
    /// program runs and drops them once it has ended.
    pub fn input(&self) -> impl Fn(&[u8]) + 'static {
        let writes = self.writes.clone();
        let alive = Arc::clone(&self.alive);
        move |bytes| {
            if alive.load(Ordering::Acquire) {
                let _ = writes.send(WriteMessage::Bytes(bytes.to_vec()));
            }
        }
    }

    /// The view's resize callback: records the size for `CSI 14 t` replies
    /// and resizes the PTY.
    pub fn resizer(&self, size: Arc<Mutex<WindowSize>>) -> impl Fn(GridSize, CellSize) + 'static {
        let master = Arc::clone(&self.master);
        move |grid, cell| {
            if let Ok(mut size) = size.lock() {
                *size = WindowSize { grid, cell };
            }
            if let Ok(master) = master.lock()
                && let Some(master) = master.as_ref()
            {
                let _ = master.resize(pty_size(grid, cell));
            }
        }
    }

    /// Ends the program and closes the PTY at once, then waits for the
    /// process and the threads on a background thread, whose handle it
    /// returns. Nothing here blocks the caller, which is the UI thread when
    /// a window closes.
    pub fn shutdown(self) -> JoinHandle<()> {
        self.alive.store(false, Ordering::Release);
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
        // On Windows the reader only sees end of file once the ConPTY closes,
        // which happens when the master is dropped.
        if let Ok(mut master) = self.master.lock() {
            master.take();
        }
        let _ = self.writes.send(WriteMessage::Close);
        std::thread::spawn(move || {
            if let Ok(mut child) = self.child.lock() {
                let _ = child.wait();
            }
            for thread in self.threads {
                let _ = thread.join();
            }
        })
    }
}

/// The PTY size for a grid of cells.
pub(super) fn pty_size(grid: GridSize, cell: CellSize) -> PtySize {
    let clamp = |value: usize| u16::try_from(value).unwrap_or(u16::MAX);
    PtySize {
        rows: clamp(grid.screen_lines),
        cols: clamp(grid.columns),
        pixel_width: clamp(usize::from(cell.width) * grid.columns),
        pixel_height: clamp(usize::from(cell.height) * grid.screen_lines),
    }
}

/// Watches for the program to end. When it ends by itself, input stops and
/// the exit notice is queued behind the output already read.
fn spawn_monitor(
    child: SharedChild,
    queue: SyncSender<Vec<u8>>,
    alive: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            let ended = match child.lock() {
                Ok(mut child) => !matches!(child.try_wait(), Ok(None)),
                Err(_) => true,
            };
            if ended {
                // False when the session closed and killed the program.
                if alive.swap(false, Ordering::AcqRel) {
                    let _ = queue.send(EXIT_NOTICE.to_vec());
                }
                return;
            }
            std::thread::sleep(EXIT_POLL);
        }
    })
}
