//! The main window's creation plan: geometry, title bar and platform options.
//!
//! The plan is built before `gpui_kit::open_window`; it contains no database
//! or window work. Appearance is the saved-background bootstrap only — blur,
//! acrylic and radius are not claimed here.

use super::Platform;
use super::logical_bounds::LogicalArea;
use super::state::{InitialWindowState, SavedWindowState};
use gpui_kit::{
    App, Bounds, Pixels, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowOptions, px, size,
};

/// The first-run size. It preserves the prototype's existing window.
pub const DEFAULT_WIDTH: f32 = 1200.0;
pub const DEFAULT_HEIGHT: f32 = 800.0;
/// The smallest useful window: room for the title bar and a terminal.
pub const MIN_WIDTH: f32 = 640.0;
pub const MIN_HEIGHT: f32 = 400.0;

/// Everything `open_window` needs before it creates the window.
pub struct MainWindowPlan {
    pub options: WindowOptions,
    pub saved: Option<SavedWindowState>,
}

/// Builds the plan from GPUI's current display list. This keeps monitor and
/// first-run bounds work out of `window.rs`.
pub fn plan_for(
    cx: &App,
    initial: &InitialWindowState,
    app_id: &str,
    platform: Platform,
) -> MainWindowPlan {
    let default_bounds = Bounds::centered(None, size(px(DEFAULT_WIDTH), px(DEFAULT_HEIGHT)), cx);
    let monitors: Vec<LogicalArea> = cx
        .displays()
        .iter()
        .map(|display| LogicalArea::from_bounds(display.visible_bounds()))
        .collect();
    let primary = cx
        .primary_display()
        .map(|display| LogicalArea::from_bounds(display.visible_bounds()));
    plan(
        initial,
        &monitors,
        primary,
        default_bounds,
        app_id,
        platform,
    )
}

/// Builds the options. `monitors` are GPUI logical visible areas and
/// `default_bounds` is GPUI's first-run centered bounds.
pub fn plan(
    initial: &InitialWindowState,
    monitors: &[LogicalArea],
    primary: Option<LogicalArea>,
    default_bounds: Bounds<Pixels>,
    app_id: &str,
    platform: Platform,
) -> MainWindowPlan {
    let saved = initial.saved.map(SavedWindowState::sanitized);
    let window_bounds = saved.map_or(WindowBounds::Windowed(default_bounds), |saved| {
        super::state::resolve_window_bounds(saved, monitors, primary, default_bounds)
    });
    let options = WindowOptions {
        window_bounds: Some(window_bounds),
        titlebar: titlebar_options(platform),
        window_background: WindowBackgroundAppearance::Opaque,
        app_id: Some(app_id.to_string()),
        window_min_size: Some(size(px(MIN_WIDTH), px(MIN_HEIGHT))),
        window_decorations: decorations(platform),
        app_owns_titlebar_drag: matches!(platform, Platform::MacOs),
        ..WindowOptions::default()
    };
    MainWindowPlan { options, saved }
}

fn titlebar_options(platform: Platform) -> Option<TitlebarOptions> {
    match platform {
        Platform::MacOs | Platform::Windows => Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        // Linux uses window decorations, not the macOS/Windows title-bar flag.
        Platform::Linux => None,
    }
}

fn decorations(platform: Platform) -> Option<WindowDecorations> {
    match platform {
        Platform::Linux => Some(WindowDecorations::Client),
        Platform::MacOs | Platform::Windows => None,
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Bounds, WindowBounds, WindowDecorations, point, px, size};

    use super::{MIN_HEIGHT, MIN_WIDTH, plan};
    use crate::app_window::Platform;
    use crate::app_window::logical_bounds::{LogicalArea, LogicalBounds};
    use crate::app_window::state::{InitialWindowState, SavedWindowState};

    fn default_bounds() -> Bounds<gpui_kit::Pixels> {
        Bounds {
            origin: point(px(120.0), px(120.0)),
            size: size(px(1200.0), px(800.0)),
        }
    }

    fn initial() -> InitialWindowState {
        InitialWindowState {
            saved: None,
            background: Some((0x1b, 0x1d, 0x23)),
        }
    }

    #[test]
    fn a_first_run_is_windowed_and_centered_by_the_caller() {
        let window = plan(
            &initial(),
            &[],
            None,
            default_bounds(),
            "com.example.gpui",
            Platform::Windows,
        );
        assert!(matches!(
            window.options.window_bounds,
            Some(WindowBounds::Windowed(bounds)) if bounds == default_bounds()
        ));
    }

    #[test]
    fn the_minimum_size_is_set_for_every_platform() {
        for platform in [Platform::MacOs, Platform::Windows, Platform::Linux] {
            let window = plan(
                &initial(),
                &[],
                None,
                default_bounds(),
                "com.example.gpui",
                platform,
            );
            let minimum = window.options.window_min_size.expect("minimum size");
            assert_eq!(minimum.width.as_f32(), MIN_WIDTH);
            assert_eq!(minimum.height.as_f32(), MIN_HEIGHT);
            assert_eq!(window.options.app_id.as_deref(), Some("com.example.gpui"));
        }
    }

    #[test]
    fn mac_and_windows_hide_the_native_title_bar() {
        for platform in [Platform::MacOs, Platform::Windows] {
            let window = plan(
                &initial(),
                &[],
                None,
                default_bounds(),
                "com.example.gpui",
                platform,
            );
            let titlebar = window.options.titlebar.expect("custom title bar");
            assert!(titlebar.appears_transparent);
            assert_eq!(titlebar.traffic_light_position, None);
            assert_eq!(window.options.window_decorations, None);
        }
    }

    #[test]
    fn linux_requests_client_decorations() {
        let window = plan(
            &initial(),
            &[],
            None,
            default_bounds(),
            "com.example.gpui",
            Platform::Linux,
        );
        assert_eq!(
            window.options.window_decorations,
            Some(WindowDecorations::Client)
        );
        assert!(window.options.titlebar.is_none());
    }

    #[test]
    fn saved_logical_bounds_are_restored_without_guessing_a_scale() {
        let saved = SavedWindowState {
            bounds: LogicalBounds {
                x: 300.0,
                y: 200.0,
                width: 800.0,
                height: 600.0,
            },
            maximized: true,
        };
        let state = InitialWindowState {
            saved: Some(saved),
            background: Some((0xff, 0xff, 0xff)),
        };
        let monitor = LogicalArea {
            x: 0.0,
            y: 0.0,
            width: 2880.0,
            height: 1620.0,
        };
        let window = plan(
            &state,
            &[monitor],
            Some(monitor),
            default_bounds(),
            "com.example.gpui",
            Platform::Windows,
        );
        match window.options.window_bounds.expect("restored bounds") {
            WindowBounds::Maximized(bounds) => {
                assert_eq!(bounds.origin.x.as_f32(), 300.0);
                assert_eq!(bounds.size.width.as_f32(), 800.0);
            }
            other => panic!("maximized state expected, got {other:?}"),
        }
    }
}
