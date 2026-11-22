//! Persistence and in-memory tracking for the main window's geometry.
//!
//! The `gpui_window_state` value stores GPUI's logical window bounds. It is
//! separate from Tauri's `window_state`, whose `tterm_core` type promises
//! physical pixels. Keeping the units apart avoids applying a guessed scale
//! factor; GPUI's `PlatformDisplay` cannot expose a unified physical topology.

use std::sync::{Arc, Mutex};

use gpui_kit::{App, Bounds, Context, Subscription, Window, WindowBounds};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use tterm_core::config::window_background;
use tterm_core::core::blocking::run_blocking;
use tterm_core::db;
use tterm_core::db::meta;

use super::logical_bounds::{self, LogicalArea, LogicalBounds};

/// The GPUI-only `app_meta` key. Tauri's `window_state` is never read or
/// written by this module.
pub const KEY: &str = "gpui_window_state";

/// The background writer used by the serialized save queue.
pub type StateWriter = Arc<dyn Fn(SavedWindowState) -> Result<(), String> + Send + Sync>;

/// What the main window looked like when it last closed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedWindowState {
    /// Logical outer position and inner size, the values GPUI reports.
    pub bounds: LogicalBounds,
    /// The window was maximized; `bounds` is its restore size.
    pub maximized: bool,
}

impl SavedWindowState {
    pub(crate) fn sanitized(self) -> Self {
        Self {
            bounds: self.bounds.sanitized(),
            ..self
        }
    }
}

/// Values read before the GPUI window is opened.
#[derive(Clone, Copy, Debug, Default)]
pub struct InitialWindowState {
    pub saved: Option<SavedWindowState>,
    pub background: Option<(u8, u8, u8)>,
}

/// Reads both values through the existing blocking worker. Startup calls this
/// before `gpui_kit::application().run`, so no database work happens on a
/// window or move callback.
pub async fn load_initial() -> Result<InitialWindowState, String> {
    run_blocking(|| {
        db::read(|connection| {
            Ok(InitialWindowState {
                saved: read(connection)?.map(SavedWindowState::sanitized),
                background: window_background::read(connection)?,
            })
        })
    })
    .await
}

fn read(connection: &rusqlite::Connection) -> Result<Option<SavedWindowState>, String> {
    meta::get::<SavedWindowState>(connection, KEY)
}

fn write(connection: &rusqlite::Connection, state: SavedWindowState) -> Result<(), String> {
    meta::set(connection, KEY, &state)
}

/// The background writer used by [`SaveQueue`].
pub fn write_global(state: SavedWindowState) -> Result<(), String> {
    db::write(|transaction| write(transaction, state))
}

/// What one observation said about the window.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Observation {
    Fullscreen,
    Windowed(LogicalBounds),
    Maximized(LogicalBounds),
}

impl Observation {
    fn from_window_bounds(bounds: WindowBounds) -> Option<Self> {
        match bounds {
            WindowBounds::Fullscreen(_) => Some(Self::Fullscreen),
            WindowBounds::Windowed(bounds) => Some(Self::Windowed(
                LogicalBounds::from_bounds(bounds).sanitized(),
            )),
            WindowBounds::Maximized(bounds) => Some(Self::Maximized(
                LogicalBounds::from_bounds(bounds).sanitized(),
            )),
        }
    }
}

/// Keeps the last useful windowed geometry. Fullscreen and zero-size windowed
/// observations say nothing about where the window belongs; a maximized one
/// only says that it is maximized.
fn fold(
    previous: Option<SavedWindowState>,
    observation: Option<Observation>,
) -> Option<SavedWindowState> {
    let Some(observation) = observation else {
        return previous;
    };
    match observation {
        Observation::Fullscreen => previous,
        Observation::Windowed(bounds) => {
            if bounds.width == 0.0 || bounds.height == 0.0 {
                previous
            } else {
                Some(SavedWindowState {
                    bounds,
                    maximized: false,
                })
            }
        }
        Observation::Maximized(bounds) => Some(SavedWindowState {
            bounds: previous.map_or(bounds, |previous| previous.bounds),
            maximized: true,
        }),
    }
}

/// Turns the saved logical bounds into the value GPUI opens a window with. A
/// saved top edge that no longer touches a monitor is centered on the primary
/// visible area in GPUI's logical monitor space.
pub fn resolve_window_bounds(
    saved: SavedWindowState,
    monitors: &[LogicalArea],
    primary: Option<LogicalArea>,
    fallback: Bounds<gpui_kit::Pixels>,
) -> WindowBounds {
    let saved = saved.sanitized();
    if saved.bounds.width == 0.0 || saved.bounds.height == 0.0 {
        return WindowBounds::Windowed(fallback);
    }
    let bounds = if logical_bounds::top_edge_on_screen(saved.bounds, monitors) {
        saved.bounds
    } else if let Some(primary) = primary {
        logical_bounds::centered(saved.bounds, primary)
    } else {
        return WindowBounds::Windowed(fallback);
    };
    if saved.maximized {
        WindowBounds::Maximized(bounds.to_bounds())
    } else {
        WindowBounds::Windowed(bounds.to_bounds())
    }
}

/// The value the quit callback writes after all windows are gone.
pub fn shared_latest(shared: &Arc<Mutex<Option<SavedWindowState>>>) -> Option<SavedWindowState> {
    shared.lock().ok().and_then(|state| *state)
}

/// Startup state and the ordered writer. Keeping this in one place leaves
/// `window.rs` with only the window creation and root wiring.
pub struct WindowLifecycle {
    pub initial: InitialWindowState,
    pub geometry: Arc<Mutex<Option<SavedWindowState>>>,
    queue: SaveQueue,
}

impl WindowLifecycle {
    /// Loads persisted state on the existing runtime's blocking worker and
    /// starts the serialized save task.
    pub fn start(runtime: &Handle) -> Self {
        let initial = match runtime.block_on(load_initial()) {
            Ok(initial) => initial,
            Err(error) => {
                eprintln!("tterm-gpui: failed to load the window state: {error}");
                InitialWindowState::default()
            }
        };
        let geometry = Arc::new(Mutex::new(initial.saved));
        let writer: StateWriter = Arc::new(write_global);
        Self {
            initial,
            geometry,
            queue: SaveQueue::start(runtime, writer),
        }
    }

    pub fn queue(&self) -> SaveQueue {
        self.queue.clone()
    }

    /// Best-effort fallback for quit paths that bypass the window close gate.
    /// GPUI limits application quit handlers to 200 ms, so only the normal
    /// close gate guarantees that the final write finishes before removal.
    pub fn install_quit_flush(&self, cx: &mut App) {
        let geometry = Arc::clone(&self.geometry);
        let queue = self.queue.clone();
        cx.on_app_quit(move |_| {
            let latest = shared_latest(&geometry);
            let queue = queue.clone();
            async move {
                if let Some(latest) = latest
                    && let Err(error) = queue.flush(latest).await
                {
                    eprintln!("tterm-gpui: failed to save the window state: {error}");
                }
            }
        })
        .detach();
    }
}

/// Owns the latest observation, mirrors it into the shared slot, and gates
/// close on the final background write. Move and resize callbacks only
/// update memory.
pub struct WindowGeometry {
    latest: Option<SavedWindowState>,
    shared: Arc<Mutex<Option<SavedWindowState>>>,
    queue: SaveQueue,
    closing: bool,
    _bounds_subscription: Option<Subscription>,
}

impl WindowGeometry {
    pub fn new(
        initial: Option<SavedWindowState>,
        shared: Arc<Mutex<Option<SavedWindowState>>>,
        queue: SaveQueue,
    ) -> Self {
        let latest = initial.map(SavedWindowState::sanitized);
        let geometry = Self {
            latest,
            shared,
            queue,
            closing: false,
            _bounds_subscription: None,
        };
        geometry.publish();
        geometry
    }

    /// Takes the current value and starts following moves and resizes. Both
    /// platform callbacks go through `Window::bounds_changed`, so one
    /// observer covers both.
    pub fn observe(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.record(window);
        self._bounds_subscription =
            Some(cx.observe_window_bounds(window, |geometry, window, cx| {
                geometry.record(window);
                cx.notify();
            }));
    }

    /// Starts observing and installs the geometry-only close handler used by
    /// the lifecycle tests. Production installs one combined close handler
    /// from the workspace root so the workspace flush runs first.
    #[cfg(test)]
    pub fn attach(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.observe(window, cx);
        let entity = cx.entity();
        window.on_window_should_close(cx, move |window, cx| {
            entity.update(cx, |geometry, cx| geometry.request_close(window, cx))
        });
    }

    /// The last recorded geometry, used by the combined close gate.
    pub fn latest(&self) -> Option<SavedWindowState> {
        self.latest
    }

    /// A clone of the serialized geometry writer.
    pub fn queue(&self) -> SaveQueue {
        self.queue.clone()
    }

    /// Updates the in-memory value; intentionally performs no database work.
    pub fn record(&mut self, window: &Window) {
        let observation = Observation::from_window_bounds(window.window_bounds());
        self.latest = fold(self.latest, observation);
        self.publish();
    }

    /// Closes the window only after the final state has been written. The
    /// first close request returns `false` to the platform while the async
    /// flush runs; a second request is also refused until the window is
    /// removed. A failed write is logged and the window still closes, so the
    /// user is not trapped by a full disk.
    pub fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.closing {
            return false;
        }
        self.closing = true;
        self.record(window);
        let latest = self.latest;
        let queue = self.queue.clone();
        let window_handle = window.window_handle();
        cx.spawn(async move |_, cx| {
            if let Some(latest) = latest
                && let Err(error) = queue.flush(latest).await
            {
                eprintln!("tterm-gpui: failed to save the window state: {error}");
            }
            cx.update(|cx| {
                let _ = window_handle.update(cx, |_, window, _| window.remove_window());
            });
        })
        .detach();
        false
    }

    fn publish(&self) {
        if let Ok(mut shared) = self.shared.lock() {
            *shared = self.latest;
        }
    }
}

struct SaveRequest {
    state: SavedWindowState,
    acknowledge: oneshot::Sender<Result<(), String>>,
}

/// Serializes writes on one background task. A flush waits for its own write,
/// so all earlier writes are complete before the returned future resolves.
#[derive(Clone)]
pub struct SaveQueue {
    sender: mpsc::UnboundedSender<SaveRequest>,
}

impl SaveQueue {
    pub fn start(runtime: &Handle, writer: StateWriter) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel::<SaveRequest>();
        runtime.spawn(async move {
            while let Some(request) = receiver.recv().await {
                let writer = Arc::clone(&writer);
                let result = run_blocking(move || writer(request.state)).await;
                let _ = request.acknowledge.send(result);
            }
        });
        Self { sender }
    }

    /// Writes `state` and waits for that write. The queue is FIFO, so a final
    /// flush cannot overtake an earlier save.
    pub async fn flush(&self, state: SavedWindowState) -> Result<(), String> {
        let (acknowledge, outcome) = oneshot::channel();
        self.sender
            .send(SaveRequest { state, acknowledge })
            .map_err(|_| "The window state saver has stopped".to_string())?;
        outcome
            .await
            .map_err(|_| "The window state saver dropped the save".to_string())?
    }
}

#[cfg(test)]
mod tests;
