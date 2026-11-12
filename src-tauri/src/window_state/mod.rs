//! Remembers the main window's size, position and maximized state, so it
//! comes back the way it was left after a quit, a restart or an update.
//!
//! It is kept in `app_meta` like the window background: per device and left
//! out of backups and sync, so one machine's window layout never lands on
//! another machine's screens.

mod placement;
mod state;
#[cfg(test)]
mod tests;

use placement::{Area, centered, top_edge_on_screen};
use serde::{Deserialize, Serialize};
use state::Bounds;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, WindowEvent};
use tterm_core::db::meta;

const KEY: &str = "window_state";

/// How long the window must stay still before its state is taken. Maximizing
/// and restoring animate through moves and resizes that are neither the old
/// bounds nor the new ones; once the window settles it reports whether it
/// ended up maximized.
const SETTLE: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SavedWindowState {
    /// Where the window sits when it is not maximized. A maximized window
    /// keeps the bounds it had before, so un-maximizing puts it back there.
    bounds: Bounds,
    maximized: bool,
}

/// What the window reports about itself at one moment.
#[derive(Clone, Copy, Debug)]
struct Observation {
    minimized: bool,
    fullscreen: bool,
    maximized: bool,
    bounds: Bounds,
}

/// The state to keep after `observation`, given what was kept before.
/// Minimized and fullscreen windows say nothing about where the window
/// belongs, and a maximized one only that it is maximized.
fn fold(latest: Option<SavedWindowState>, observation: Observation) -> Option<SavedWindowState> {
    if observation.minimized || observation.fullscreen {
        return latest;
    }
    if observation.maximized {
        let bounds = latest.map_or(observation.bounds, |latest| latest.bounds);
        return Some(SavedWindowState {
            bounds,
            maximized: true,
        });
    }
    if observation.bounds.width == 0 || observation.bounds.height == 0 {
        return latest;
    }
    Some(SavedWindowState {
        bounds: observation.bounds,
        maximized: false,
    })
}

/// The latest state of the main window, ready to be written at exit.
#[derive(Default)]
struct Tracker {
    latest: Mutex<Option<SavedWindowState>>,
    /// Bumped on every move and resize, so only the last one of a burst is
    /// taken.
    generation: AtomicU64,
}

impl Tracker {
    fn latest(&self) -> Option<SavedWindowState> {
        self.latest.lock().ok().and_then(|latest| *latest)
    }

    /// Takes the window's present state. The lock is not held while the
    /// window is asked: off the main thread that waits on the main thread.
    fn refresh(&self, window: &WebviewWindow) -> Option<SavedWindowState> {
        let Ok(observation) = observe(window) else {
            return self.latest();
        };
        let Ok(mut latest) = self.latest.lock() else {
            return None;
        };
        *latest = fold(*latest, observation);
        *latest
    }
}

fn observe(window: &WebviewWindow) -> tauri::Result<Observation> {
    let position = window.outer_position()?;
    let size = window.inner_size()?;
    Ok(Observation {
        minimized: window.is_minimized()?,
        fullscreen: window.is_fullscreen()?,
        maximized: window.is_maximized()?,
        bounds: Bounds {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
        },
    })
}

/// Puts the main window back the way it was left and starts following its
/// moves and resizes. Called on the hidden window before it is first shown.
pub fn restore(window: &WebviewWindow) {
    let saved = tterm_core::db::read(|connection| meta::get::<SavedWindowState>(connection, KEY))
        .unwrap_or_else(|error| {
            eprintln!("Failed to read the window state: {error}");
            None
        });
    if let Some(saved) = saved
        && let Err(error) = apply(window, saved)
    {
        eprintln!("Failed to restore the window state: {error}");
    }
    let tracker = Tracker {
        latest: Mutex::new(saved),
        ..Tracker::default()
    };
    if saved.is_none() {
        tracker.refresh(window);
    }
    window.manage(tracker);
    let handle = window.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
            refresh_when_settled(&handle);
        }
    });
}

fn apply(window: &WebviewWindow, saved: SavedWindowState) -> tauri::Result<()> {
    let monitors: Vec<Area> = window
        .available_monitors()?
        .iter()
        .map(|monitor| Area {
            x: monitor.position().x,
            y: monitor.position().y,
            width: monitor.size().width,
            height: monitor.size().height,
        })
        .collect();
    // A window left on a monitor that is gone would open where it cannot be
    // reached; it opens centered on the main monitor instead, no larger than
    // it. The center is worked out here because macOS applies a new size
    // later, and `center()` would still measure the old one.
    let bounds = if top_edge_on_screen(saved.bounds, &monitors) {
        saved.bounds
    } else if let Some(monitor) = window.primary_monitor()? {
        let work_area = monitor.work_area();
        centered(
            saved.bounds,
            Area {
                x: work_area.position.x,
                y: work_area.position.y,
                width: work_area.size.width,
                height: work_area.size.height,
            },
        )
    } else {
        return Ok(());
    };
    window.set_position(PhysicalPosition::new(bounds.x, bounds.y))?;
    if bounds.width > 0 && bounds.height > 0 {
        window.set_size(PhysicalSize::new(bounds.width, bounds.height))?;
    }
    if saved.maximized {
        window.maximize()?;
    }
    Ok(())
}

fn refresh_when_settled(window: &WebviewWindow) {
    let Some(tracker) = window.try_state::<Tracker>() else {
        return;
    };
    let generation = tracker.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SETTLE).await;
        let Some(tracker) = window.try_state::<Tracker>() else {
            return;
        };
        if tracker.generation.load(Ordering::SeqCst) == generation {
            tracker.refresh(&window);
        }
    });
}

/// Writes the main window's state. Called when the app exits.
pub fn save(app: &AppHandle) {
    let Some(tracker) = app.try_state::<Tracker>() else {
        return;
    };
    let state = match app.get_webview_window("main") {
        Some(window) => tracker.refresh(&window),
        None => tracker.latest(),
    };
    let Some(state) = state else {
        return;
    };
    if let Err(error) = tterm_core::db::write(|transaction| meta::set(transaction, KEY, &state)) {
        eprintln!("Failed to save the window state: {error}");
    }
}

/// Writes the main window's state before an update is installed: on Windows
/// the installer ends the process without the exit [`save`] runs on.
pub async fn save_now(app: AppHandle) {
    let saved = tterm_core::core::blocking::run_blocking(move || {
        save(&app);
        Ok(())
    })
    .await;
    if let Err(error) = saved {
        eprintln!("Failed to save the window state: {error}");
    }
}
