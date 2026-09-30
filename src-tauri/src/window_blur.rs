//! Blur behind the main window (macOS). The window server blurs whatever is
//! behind the window's transparent pixels, the way iTerm2 and other terminals
//! do it; the page paints the theme tint on top. The macOS window is always
//! created transparent so the blur can be switched at runtime; without it the
//! window is painted in the theme background and looks opaque.

/// Sets the blur radius in points; 0 turns the blur off.
pub fn apply_window_blur(window: &tauri::WebviewWindow, radius: u32) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let target = window.clone();
        window.run_on_main_thread(move || {
            if let Err(error) = macos::set_blur_radius(&target, radius) {
                eprintln!("Failed to set window blur: {error}");
            }
        })?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (window, radius);
    Ok(())
}

#[tauri::command]
pub fn set_window_blur(window: tauri::WebviewWindow, radius: u32) -> Result<(), String> {
    apply_window_blur(&window, radius).map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;

    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    // Private WindowServer calls, the same ones iTerm2, Alacritty, WezTerm and
    // Ghostty use; there is no public API for a plain background blur.
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGSMainConnectionID() -> *mut c_void;
        fn CGSSetWindowBackgroundBlurRadius(
            connection: *mut c_void,
            window_number: isize,
            radius: i64,
        ) -> i32;
    }

    pub fn set_blur_radius(window: &tauri::WebviewWindow, radius: u32) -> Result<(), String> {
        let ns_window = window.ns_window().map_err(|error| error.to_string())?;
        if ns_window.is_null() {
            return Err("window has no NSWindow".to_string());
        }
        // SAFETY: called on the main thread with the live NSWindow of `window`.
        let status = unsafe {
            let ns_window = &*ns_window.cast::<AnyObject>();
            let window_number: isize = msg_send![ns_window, windowNumber];
            CGSSetWindowBackgroundBlurRadius(
                CGSMainConnectionID(),
                window_number,
                i64::from(radius),
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "CGSSetWindowBackgroundBlurRadius returned {status}"
            ))
        }
    }
}
