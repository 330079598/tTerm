use tterm_platform::native::verification::platform;
use tterm_platform::verification::UserVerification;

pub(super) struct TauriVerification(pub tauri::AppHandle);
impl UserVerification for TauriVerification {
    fn available(&self) -> bool {
        platform::available()
    }
    fn verify(&self, message: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            use tauri::Manager;
            let window = self
                .0
                .get_webview_window("main")
                .ok_or("The main window is not available.")?;
            let hwnd = window
                .hwnd()
                .map_err(|e| format!("Failed to get the window handle: {e}"))?;
            platform::verify(hwnd.0 as isize, message)
        }
        #[cfg(not(windows))]
        {
            let _ = &self.0;
            platform::verify(message)
        }
    }
}
