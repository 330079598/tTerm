//! Main-window startup: build the persistence entity, open the window with
//! the saved options, and attach the gated close handler.

use std::sync::{Arc, Mutex};

use gpui_kit::{AnyWindowHandle, App, AppContext, Entity, Render};

use super::Platform;
use super::config;
use super::state::{InitialWindowState, SaveQueue, SavedWindowState, WindowGeometry};

/// The result of opening the main window.
pub struct Opened<V> {
    pub handle: AnyWindowHandle,
    pub root: Entity<V>,
    pub geometry: Entity<WindowGeometry>,
}

/// Opens the main window and leaves title, activation and app-specific view
/// wiring to the caller.
pub fn open<V: Render + 'static>(
    cx: &mut App,
    initial: InitialWindowState,
    shared_geometry: Arc<Mutex<Option<SavedWindowState>>>,
    queue: SaveQueue,
    app_id: &str,
    build: impl FnOnce(&mut App, Entity<WindowGeometry>) -> Entity<V> + 'static,
) -> Result<Opened<V>, String> {
    let plan = config::plan_for(cx, &initial, app_id, Platform::current());
    let geometry = cx.new(|_| WindowGeometry::new(plan.saved, shared_geometry, queue));
    let geometry_for_build = geometry.clone();
    let (handle, root) =
        gpui_kit::open_window(plan.options, cx, move |_, cx| build(cx, geometry_for_build))
            .map_err(|error| error.to_string())?;
    handle
        .update(cx, |_, window, cx| {
            geometry.update(cx, |geometry, cx| geometry.attach(window, cx));
        })
        .map_err(|error| error.to_string())?;
    Ok(Opened {
        handle,
        root,
        geometry,
    })
}
