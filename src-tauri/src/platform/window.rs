use tauri::{AppHandle, Manager};
use tterm_platform::process::ProcessControl;
use tterm_platform::window::{MainWindow, NativeHandles, WindowHost};

pub(super) struct TauriWindow(pub AppHandle);
impl MainWindow for TauriWindow {
    fn show_and_focus(&self) -> Result<(), String> {
        crate::background::show_main_window(&self.0);
        Ok(())
    }
    fn reveal_for_prompt(&self) -> Result<(), String> {
        crate::background::reveal_for_prompt(&self.0);
        Ok(())
    }
    fn set_attention(&self, attention: bool) -> Result<(), String> {
        crate::background::tray::set_attention(&self.0, attention);
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
