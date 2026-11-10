//! UI operations requested by domain services.

/// The main window is bound by the client; methods dispatch without blocking its UI thread.
pub trait MainWindow: Send + Sync {
    /// Unminimizes, shows, and focuses the bound main window.
    fn show_and_focus(&self) -> Result<(), String>;
    /// Shows the main window only when backgrounded or minimized.
    fn reveal_for_prompt(&self) -> Result<(), String>;
    /// Updates the existing tray attention indication.
    fn set_attention(&self, attention: bool) -> Result<(), String>;
}

/// Native handles valid only for the duration of a task dispatched by WindowHost.
#[derive(Clone, Copy)]
pub struct NativeHandles {
    pub view: usize,
    pub hwnd: isize,
}

/// Dispatches work while the owning native window is alive.
///
/// # Safety
/// Implementations must invoke tasks on the owning UI thread with live native handles.
/// Tasks must not retain a handle beyond the callback, or invoke this dispatcher while
/// synchronously waiting on the UI thread itself.
pub unsafe trait WindowHost: Send + Sync {
    /// Runs a callback while its native window handles are valid on the UI thread.
    fn dispatch(
        &self,
        task: Box<dyn FnOnce(Result<NativeHandles, String>) + Send>,
    ) -> Result<(), String>;
}
