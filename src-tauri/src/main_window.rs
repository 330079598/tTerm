//! The main window: built at startup with the user's settings, and the
//! devtools toggle.

use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::{Manager, WebviewWindowBuilder};

#[tauri::command]
pub fn toggle_devtools(app: tauri::AppHandle, enable: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Main window not found")?;

    if enable {
        window.open_devtools();
    } else {
        window.close_devtools();
    }

    Ok(())
}

fn background_throttling_policy(value: &str) -> BackgroundThrottlingPolicy {
    match value {
        "disabled" => BackgroundThrottlingPolicy::Disabled,
        "suspend" => BackgroundThrottlingPolicy::Suspend,
        _ => BackgroundThrottlingPolicy::Throttle,
    }
}

/// The main window is declared with `create: false` so it is built here: the
/// background throttling policy is a user setting and WebKit only takes it
/// when the webview is created. The first frame is painted in the last theme
/// background; `index.html` picks it up through `--boot-bg`. With the blur on
/// (macOS, Windows) that background is a translucent tint over it, and the
/// page learns its opacity from `__TTERM_WINDOW_BLUR__` before the config
/// loads.
pub fn create_main_window(
    app: &tauri::App,
    cfg: Option<&crate::config::AppConfig>,
) -> tauri::Result<()> {
    let window_config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
        .cloned()
        .expect("main window missing from tauri.conf.json");
    let background_throttling = cfg.map_or("throttle", |cfg| cfg.background_throttling.as_str());
    let mut builder = WebviewWindowBuilder::from_config(app.handle(), &window_config)?
        .background_throttling(background_throttling_policy(background_throttling));
    let background = crate::config::window_background::load_window_background();
    let blur_cfg = cfg.filter(|cfg| cfg!(any(target_os = "macos", windows)) && cfg.window_blur);
    let blur = blur_cfg.map(|cfg| {
        let dark = background.is_none_or(crate::window_blur::is_dark_background);
        crate::window_blur::WindowBlur::from_config(cfg, dark)
    });
    let blur_opacity = blur_cfg.map(|cfg| f64::from(cfg.window_opacity_percent) / 100.0);
    if let Some(opacity) = blur_opacity {
        builder = builder.initialization_script(format!("window.__TTERM_WINDOW_BLUR__={opacity};"));
    }
    #[cfg(target_os = "macos")]
    if blur.is_some() {
        builder = builder.background_color(tauri::window::Color(0, 0, 0, 0));
    }
    #[cfg(windows)]
    if let Some(blur) = blur {
        builder = builder
            .effects(crate::window_blur::windows_effects(blur))
            .theme(Some(crate::window_blur::windows_theme(blur)));
    }
    if let Some((r, g, b)) = background {
        let alpha = blur_opacity.map_or(String::new(), |opacity| format!("/{opacity}"));
        builder = builder.initialization_script(format!(
            "try{{const s=new CSSStyleSheet();s.replaceSync(':root{{--boot-bg:rgb({r} {g} {b}{alpha})}}');\
             document.adoptedStyleSheets=[...document.adoptedStyleSheets,s]}}catch{{}}"
        ));
        // The Windows window is transparent and must stay that way.
        #[cfg(not(target_os = "windows"))]
        if blur.is_none() {
            builder = builder.background_color(tauri::window::Color(r, g, b, 255));
        }
    }
    let window = builder.build()?;
    // The macOS blur is set on the built window; Windows took it above.
    #[cfg(target_os = "macos")]
    if blur.is_some() {
        crate::window_blur::apply_window_blur(&window, blur)?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
    Ok(())
}

#[cfg(test)]
mod background_throttling_tests {
    use super::{background_throttling_policy, BackgroundThrottlingPolicy};

    #[test]
    fn unknown_values_fall_back_to_throttle() {
        assert!(matches!(
            background_throttling_policy("disabled"),
            BackgroundThrottlingPolicy::Disabled
        ));
        assert!(matches!(
            background_throttling_policy("suspend"),
            BackgroundThrottlingPolicy::Suspend
        ));
        assert!(matches!(
            background_throttling_policy("throttle"),
            BackgroundThrottlingPolicy::Throttle
        ));
        assert!(matches!(
            background_throttling_policy(""),
            BackgroundThrottlingPolicy::Throttle
        ));
    }
}
