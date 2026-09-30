//! Blur behind the main window; the page paints the theme tint on top.
//!
//! - macOS: the window server blurs whatever is behind the window's
//!   transparent pixels, the way iTerm2 and other terminals do it. The window
//!   is always created transparent so the blur can be switched at runtime;
//!   without it the window is painted in the theme background.
//! - Windows: a DWM backdrop on the (always transparent) window. Acrylic blurs
//!   what is behind it; Mica tints it with the wallpaper (Windows 11). Both
//!   follow the window's light or dark theme, which is pinned to the page's.
//! - Linux: blur belongs to the compositor; not supported.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowBlur {
    /// Points; macOS only.
    pub radius: u32,
    /// Mica instead of acrylic; Windows only.
    pub mica: bool,
    /// The page's theme is dark; Windows only.
    pub dark: bool,
}

impl WindowBlur {
    pub fn from_config(cfg: &crate::config::AppConfig, dark: bool) -> Self {
        Self {
            radius: u32::from(cfg.window_blur_radius),
            mica: cfg.window_blur_material == "mica",
            dark,
        }
    }
}

/// Same threshold the frontend uses to tell light themes from dark ones.
pub fn is_dark_background((r, g, b): (u8, u8, u8)) -> bool {
    0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b) <= 128.0
}

#[cfg(windows)]
pub fn windows_effects(blur: WindowBlur) -> tauri::utils::config::WindowEffectsConfig {
    use tauri::window::{Effect, EffectsBuilder};

    let effect = match (blur.mica, blur.dark) {
        (true, true) => Effect::MicaDark,
        (true, false) => Effect::MicaLight,
        (false, _) => Effect::Acrylic,
    };
    EffectsBuilder::new().effect(effect).build()
}

#[cfg(windows)]
pub fn windows_theme(blur: WindowBlur) -> tauri::Theme {
    if blur.dark {
        tauri::Theme::Dark
    } else {
        tauri::Theme::Light
    }
}

/// Turns the blur on with `blur`, or off with `None`.
pub fn apply_window_blur(
    window: &tauri::WebviewWindow,
    blur: Option<WindowBlur>,
) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let target = window.clone();
        let radius = blur.map_or(0, |blur| blur.radius);
        window.run_on_main_thread(move || {
            if let Err(error) = macos::set_blur_radius(&target, radius) {
                eprintln!("Failed to set window blur: {error}");
            }
        })?;
    }
    #[cfg(windows)]
    {
        window.set_effects(blur.map(windows_effects))?;
        window.set_theme(blur.map(windows_theme))?;
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = (window, blur);
    Ok(())
}

#[tauri::command]
pub fn set_window_blur(
    window: tauri::WebviewWindow,
    enabled: bool,
    radius: u32,
    material: String,
    dark: bool,
) -> Result<(), String> {
    let blur = enabled.then_some(WindowBlur {
        radius,
        mica: material == "mica",
        dark,
    });
    apply_window_blur(&window, blur).map_err(|error| error.to_string())
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

#[cfg(test)]
mod tests {
    use super::is_dark_background;

    #[test]
    fn classifies_theme_backgrounds() {
        assert!(is_dark_background((0x1b, 0x1d, 0x23)));
        assert!(!is_dark_background((0xff, 0xff, 0xff)));
    }
}
