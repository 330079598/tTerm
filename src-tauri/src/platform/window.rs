use tauri::{AppHandle, Manager};
use tterm_platform::process::ProcessControl;
use tterm_platform::window::{MainWindow, NativeHandles, WindowHost};

pub(super) struct TauriWindow(pub AppHandle);
impl MainWindow for TauriWindow {
    fn save_before_update(&self) -> tterm_platform::PlatformFuture<'_, ()> {
        let app = self.0.clone();
        Box::pin(async move {
            crate::window_state::save_now(app).await;
            Ok(())
        })
    }
    fn send_to_background(&self, tray_present: bool) -> Result<(), String> {
        if let Some(window) = self.0.get_webview_window("main") {
            if tray_present {
                let _ = window.hide();
            } else {
                let _ = window.minimize();
            }
        }
        Ok(())
    }
    fn show_and_focus(&self) -> Result<(), String> {
        if let Some(window) = self.0.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
        Ok(())
    }
    fn reveal_for_prompt(&self) -> Result<(), String> {
        let app = self.0.clone();
        self.0
            .run_on_main_thread(move || {
                if let Some(window) = app.get_webview_window("main")
                    && (!window.is_visible().unwrap_or(true)
                        || window.is_minimized().unwrap_or(false))
                {
                    let _ = TauriWindow(app.clone()).show_and_focus();
                    if let Some(services) =
                        app.try_state::<std::sync::Arc<crate::services::AppServices>>()
                    {
                        tterm_core::notification::badge::restore(
                            services.shared.platform.badge.as_ref(),
                        );
                    }
                }
            })
            .map_err(|e| e.to_string())
    }
    fn set_attention(&self, attention: bool) -> Result<(), String> {
        crate::platform::tray_native::set_attention(&self.0, attention);
        Ok(())
    }
}
// SAFETY: Tauri retains the window throughout the callback, resolves handles inside
// its UI task, and never passes handles to another thread.
unsafe impl WindowHost for TauriWindow {
    fn dispatch(
        &self,
        task: Box<dyn FnOnce(Result<NativeHandles, String>) + Send>,
    ) -> Result<(), String> {
        let app = self.0.clone();
        self.0
            .run_on_main_thread(move || {
                let handles = (|| {
                    let window = app
                        .get_webview_window("main")
                        .ok_or("The main window is not available.")?;
                    #[cfg(target_os = "macos")]
                    return window
                        .ns_view()
                        .map(|view| NativeHandles {
                            view: view as usize,
                            hwnd: 0,
                        })
                        .map_err(|e| format!("Failed to get the window's view: {e}"));
                    #[cfg(windows)]
                    return window
                        .hwnd()
                        .map(|hwnd| NativeHandles {
                            view: 0,
                            hwnd: hwnd.0 as isize,
                        })
                        .map_err(|e| format!("Failed to get the window handle: {e}"));
                    #[cfg(not(any(target_os = "macos", windows)))]
                    {
                        let _ = window;
                        Err("Native dragging is not supported on this platform.".to_string())
                    }
                })();
                task(handles);
            })
            .map_err(|e| e.to_string())
    }
}
pub(super) struct TauriProcess(pub AppHandle);
impl ProcessControl for TauriProcess {
    fn exit(&self, code: i32) -> Result<(), String> {
        self.0.exit(code);
        Ok(())
    }
    fn restart(&self) -> Result<(), String> {
        self.0.restart()
    }
}
