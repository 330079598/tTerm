//! Tauri tray icons and callbacks; domain menu policy lives in background::tray.
use crate::background::tray::TRAY_ID;
use crate::services::AppServices;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Once;
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
const DOT_RGB: [u8; 3] = [0xE8, 0x11, 0x23];
/// Installs the already-rendered native menu without reading domain state.
pub(crate) fn present_menu(app: &AppHandle, menu: Menu<tauri::Wry>) -> tauri::Result<()> {
    static MENU_HANDLER: Once = Once::new();
    MENU_HANDLER.call_once(|| app.on_menu_event(on_menu_event));
    let services = app.state::<Arc<AppServices>>();
    let state = &services.app.tray;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        return tray.set_menu(Some(menu));
    }
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("tTerm")
        .menu(&menu)
        // macOS opens the menu on a click, like every menu bar item; elsewhere
        // a click brings the window back and the menu is on right click.
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_tray_icon_event(on_tray_icon_event);
    if let Some(icon) = tray_icon(app, state.attention.load(Ordering::SeqCst)) {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

pub(crate) fn hide(app: &AppHandle) {
    app.remove_tray_by_id(TRAY_ID);
    *app.state::<Arc<AppServices>>()
        .app
        .tray
        .shown
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
}

/// Puts the red dot on the tray icon while notifications wait unseen, or
/// takes it off. Remembered for a tray icon added later.
pub fn set_attention(app: &AppHandle, attention: bool) {
    let Some(services) = app.try_state::<Arc<AppServices>>() else {
        return;
    };
    let state = &services.app.tray;
    if state.attention.swap(attention, Ordering::SeqCst) == attention {
        return;
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Err(error) = tray.set_icon(tray_icon(app, attention)) {
            eprintln!("Failed to update the tray icon: {error}");
        }
    }
}

fn tray_icon(app: &AppHandle, attention: bool) -> Option<Image<'_>> {
    let icon = app.default_window_icon()?;
    Some(if attention {
        with_dot(icon)
    } else {
        icon.clone()
    })
}

/// The icon with a red dot in its top right corner. A clear ring cuts the dot
/// out of the icon so it stands apart on any tray background.
fn with_dot(icon: &Image<'_>) -> Image<'static> {
    let (width, height) = (icon.width(), icon.height());
    let size = width.min(height) as f32;
    let radius = size * 0.15;
    let ring = (size * 0.05).max(1.0);
    let (center_x, center_y) = (width as f32 - radius - ring, radius + ring);
    let mut rgba = icon.rgba().to_vec();
    for (index, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let x = (index % width as usize) as f32 + 0.5;
        let y = (index / width as usize) as f32 + 0.5;
        let distance = (x - center_x).hypot(y - center_y);
        // How much of the pixel the dot, and the dot with its ring, cover.
        let dot = (radius + 0.5 - distance).clamp(0.0, 1.0);
        let cut = (radius + ring + 0.5 - distance).clamp(0.0, 1.0);
        let kept = f32::from(pixel[3]) / 255.0 * (1.0 - cut) * (1.0 - dot);
        let alpha = dot + kept;
        if alpha <= 0.0 {
            pixel.fill(0);
            continue;
        }
        for (channel, dot_channel) in pixel.iter_mut().zip(DOT_RGB) {
            let color = f32::from(dot_channel) * dot + f32::from(*channel) * kept;
            *channel = (color / alpha).round() as u8;
        }
        pixel[3] = (alpha * 255.0).round() as u8;
    }
    Image::new_owned(rgba, width, height)
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let Some(services) = app.try_state::<Arc<AppServices>>() else {
        return;
    };
    let context = crate::app_context::background(
        &services,
        crate::sftp::internal::api::external::unuploaded_files(&services.sftp.external_edit),
    );
    crate::background::tray::handle_action(
        context,
        crate::app_context::tunnel(&services),
        event.id().as_ref(),
    );
}

fn on_tray_icon_event(tray: &TrayIcon, event: TrayIconEvent) {
    let app = tray.app_handle();
    let Some(services) = app.try_state::<Arc<AppServices>>() else {
        return;
    };
    let context = crate::app_context::background(
        &services,
        crate::sftp::internal::api::external::unuploaded_files(&services.sftp.external_edit),
    );
    match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } if !cfg!(target_os = "macos") => {
            crate::background::show_main_window(context.window.as_ref(), context.badge.as_ref())
        }
        TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        } => crate::background::show_main_window(context.window.as_ref(), context.badge.as_ref()),
        // Hovering comes before the tooltip and the menu; bring both up to
        // date, including tunnels changed by a sync or an import.
        TrayIconEvent::Enter { .. } => {
            let _ = tray.set_tooltip(Some(crate::background::tray::tooltip(
                &services.session.ptys,
                &services.app.tunnels,
                &services.app.tray,
            )));
            crate::background::tray::refresh_tunnels(&context.tray);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dot_sits_in_the_top_right_corner_apart_from_the_icon() {
        let blue = [0, 0, 255, 255];
        let icon = Image::new_owned(blue.repeat(32 * 32), 32, 32);
        let dotted = with_dot(&icon);
        let pixel = |x: usize, y: usize| {
            let start = (y * 32 + x) * 4;
            dotted.rgba()[start..start + 4].to_vec()
        };
        assert_eq!(pixel(25, 6), [0xE8, 0x11, 0x23, 255]);
        // The ring between the dot and the icon is clear.
        assert_eq!(pixel(21, 10)[3], 0);
        assert_eq!(pixel(18, 6), blue);
        assert_eq!(pixel(4, 28), blue);
    }
}
