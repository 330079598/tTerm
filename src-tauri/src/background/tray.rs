//! The tray icon (the menu bar on macOS) shown while closing the window can
//! leave tTerm running: it brings the window back, starts and stops tunnels,
//! and quits. It carries a red dot while system notifications wait unseen.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};

use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::core::state::PtyMap;
use crate::tunnel::commands::TunnelManager;
use tterm_core::events::tunnel::TunnelState;

pub(crate) const TRAY_ID: &str = "main";
const MENU_SHOW: &str = "tray-show";
const MENU_QUIT: &str = "tray-quit";
const MENU_CHECK_UPDATES: &str = "tray-updates";
const MENU_TUNNELS_PAGE: &str = "tray-tunnels-page";
const MENU_TUNNEL_PREFIX: &str = "tray-tunnel:";
/// Asks the UI to open the Port Forwarding page.
const OPEN_TUNNELS_EVENT: &str = "tray-open-tunnels";
/// Tells the UI that an update check asked for from the tray is waiting in
/// `take_tray_update_check`.
const CHECK_UPDATES_EVENT: &str = "tray-check-updates";
/// The dot's red, the same as the count on the Windows taskbar.
const DOT_RGB: [u8; 3] = [0xE8, 0x11, 0x23];

#[derive(Default)]
pub struct TrayState {
    language: Mutex<String>,
    /// What the current menu shows; it is rebuilt only when this changes.
    shown: Mutex<Option<Vec<MenuTunnel>>>,
    refresh_pending: AtomicBool,
    /// Whether the icon shows the dot for unseen notifications.
    attention: AtomicBool,
    /// An update check asked for from the tray that the UI has not taken yet.
    /// It is kept because a click before the UI listens would otherwise be lost.
    update_check_requested: AtomicBool,
}

/// A tunnel as one menu entry shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MenuTunnel {
    id: String,
    name: String,
    state: TunnelState,
}

#[derive(Clone, Copy)]
struct Labels {
    show: &'static str,
    check_updates: &'static str,
    quit: &'static str,
    tunnels: &'static str,
    no_tunnels: &'static str,
    open_tunnels: &'static str,
    connecting: &'static str,
    error: &'static str,
    needs_password: &'static str,
}

fn labels(language: &str) -> Labels {
    if language == "zh" {
        Labels {
            show: "显示 tTerm",
            check_updates: "检查更新…",
            quit: "退出",
            tunnels: "端口转发",
            no_tunnels: "没有端口转发规则",
            open_tunnels: "打开端口转发页面",
            connecting: "（连接中…）",
            error: "（出错）",
            needs_password: "（需要密码）",
        }
    } else {
        Labels {
            show: "Show tTerm",
            check_updates: "Check for updates…",
            quit: "Quit",
            tunnels: "Port Forwarding",
            no_tunnels: "No tunnels",
            open_tunnels: "Open Port Forwarding",
            connecting: " (connecting…)",
            error: " (error)",
            needs_password: " (needs password)",
        }
    }
}

/// Adds the tray icon, or rebuilds its menu in `language`.
pub(crate) fn show(app: &AppHandle, language: &str) -> Result<(), String> {
    // A tray's own menu handler outlives the tray, so one built again after
    // being removed would answer each click twice; this one is app-wide.
    static MENU_HANDLER: Once = Once::new();
    MENU_HANDLER.call_once(|| app.on_menu_event(on_menu_event));

    let state = app.state::<TrayState>();
    *state.language.lock().unwrap_or_else(|e| e.into_inner()) = language.to_string();
    let tunnels = menu_tunnels(app);
    *state.shown.lock().unwrap_or_else(|e| e.into_inner()) = Some(tunnels.clone());
    app.state::<std::sync::Arc<dyn tterm_platform::tray::Tray>>()
        .present(tterm_platform::tray::TrayModel {
            entries: menu_model(labels(language), &tunnels),
        })
}

/// Installs the already-rendered native menu without reading domain state.
pub(crate) fn present_menu(app: &AppHandle, menu: Menu<tauri::Wry>) -> tauri::Result<()> {
    let state = app.state::<TrayState>();
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
    *app.state::<TrayState>()
        .shown
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
}

/// Puts the red dot on the tray icon while notifications wait unseen, or
/// takes it off. Remembered for a tray icon added later.
pub fn set_attention(app: &AppHandle, attention: bool) {
    let Some(state) = app.try_state::<TrayState>() else {
        return;
    };
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

/// Brings the tunnel entries up to date after a tunnel started, stopped or
/// changed. Callable from any thread; the menu is rebuilt on the main thread,
/// and several changes in a row rebuild it once.
pub fn refresh_tunnels(app: &AppHandle) {
    let Some(state) = app.try_state::<TrayState>() else {
        return;
    };
    if app.tray_by_id(TRAY_ID).is_none() || state.refresh_pending.swap(true, Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    let scheduled = app.run_on_main_thread(move || {
        handle
            .state::<TrayState>()
            .refresh_pending
            .store(false, Ordering::SeqCst);
        if let Err(error) = rebuild_if_changed(&handle) {
            eprintln!("Failed to update the tray menu: {error}");
        }
    });
    if scheduled.is_err() {
        state.refresh_pending.store(false, Ordering::SeqCst);
    }
}

fn rebuild_if_changed(app: &AppHandle) -> tauri::Result<()> {
    if app.tray_by_id(TRAY_ID).is_none() {
        return Ok(());
    }
    let state = app.state::<TrayState>();
    let tunnels = menu_tunnels(app);
    let mut shown = state.shown.lock().unwrap_or_else(|e| e.into_inner());
    if shown.as_ref() == Some(&tunnels) {
        return Ok(());
    }
    let language = state
        .language
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    app.state::<std::sync::Arc<dyn tterm_platform::tray::Tray>>()
        .present(tterm_platform::tray::TrayModel {
            entries: menu_model(labels(&language), &tunnels),
        })
        .map_err(|e| tauri::Error::Io(std::io::Error::other(e)))?;
    *shown = Some(tunnels);
    Ok(())
}

/// Forgets what the menu shows so the next refresh rebuilds it. A check item
/// flips itself when clicked, before the tunnel has changed at all.
fn invalidate(app: &AppHandle) {
    *app.state::<TrayState>()
        .shown
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
    refresh_tunnels(app);
}

fn menu_tunnels(app: &AppHandle) -> Vec<MenuTunnel> {
    let manager = app.state::<TunnelManager>();
    crate::tunnel::storage::load_tunnels()
        .unwrap_or_else(|error| {
            eprintln!("Failed to list tunnels for the tray menu: {error}");
            Vec::new()
        })
        .into_iter()
        .map(|rule| MenuTunnel {
            state: manager.state_of(&rule.id),
            id: rule.id,
            name: rule.name,
        })
        .collect()
}

fn menu_model(labels: Labels, tunnels: &[MenuTunnel]) -> Vec<tterm_platform::tray::MenuEntry> {
    use tterm_platform::tray::MenuEntry;
    let item = |id: &str, label: &str| MenuEntry::Item {
        id: id.into(),
        label: label.into(),
        enabled: true,
        checked: None,
    };
    let mut entries = Vec::new();
    if tunnels.is_empty() {
        entries.push(MenuEntry::Item {
            id: String::new(),
            label: labels.no_tunnels.into(),
            enabled: false,
            checked: None,
        });
    }
    for tunnel in tunnels {
        entries.push(MenuEntry::Item {
            id: format!("{MENU_TUNNEL_PREFIX}{}", tunnel.id),
            label: tunnel_label(labels, tunnel),
            enabled: true,
            checked: Some(tunnel.state.is_active()),
        });
    }
    entries.extend([
        MenuEntry::Separator,
        item(MENU_TUNNELS_PAGE, labels.open_tunnels),
    ]);
    vec![
        item(MENU_SHOW, labels.show),
        MenuEntry::Separator,
        MenuEntry::Submenu {
            label: labels.tunnels.into(),
            entries,
        },
        MenuEntry::Separator,
        item(MENU_CHECK_UPDATES, labels.check_updates),
        item(MENU_QUIT, labels.quit),
    ]
}

fn tunnel_label(labels: Labels, tunnel: &MenuTunnel) -> String {
    // `&` marks a keyboard accelerator in Windows menus.
    let name = if cfg!(windows) {
        tunnel.name.replace('&', "&&")
    } else {
        tunnel.name.clone()
    };
    let suffix = match tunnel.state {
        TunnelState::Starting | TunnelState::Reconnecting => labels.connecting,
        TunnelState::Error => labels.error,
        TunnelState::NeedsCredentials => labels.needs_password,
        TunnelState::Running | TunnelState::Stopped => "",
    };
    format!("{name}{suffix}")
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        MENU_SHOW => super::show_main_window(app),
        MENU_QUIT => super::request_quit(app),
        MENU_TUNNELS_PAGE => open_tunnels_page(app),
        MENU_CHECK_UPDATES => check_updates(app),
        id => {
            if let Some(tunnel_id) = id.strip_prefix(MENU_TUNNEL_PREFIX) {
                toggle_tunnel(app, tunnel_id.to_string());
            }
        }
    }
}

fn open_tunnels_page(app: &AppHandle) {
    super::show_main_window(app);
    let _ = app.emit(OPEN_TUNNELS_EVENT, ());
}

/// Shows the window first, so the result of the check is in sight.
fn check_updates(app: &AppHandle) {
    super::show_main_window(app);
    app.state::<TrayState>()
        .update_check_requested
        .store(true, Ordering::SeqCst);
    let _ = app.emit(CHECK_UPDATES_EVENT, ());
}

/// Hands the UI an update check asked for from the tray, once. The UI asks
/// both when told about a click and when it starts listening.
#[tauri::command]
pub fn take_tray_update_check(state: tauri::State<'_, TrayState>) -> bool {
    state.update_check_requested.swap(false, Ordering::SeqCst)
}

/// Stops a running tunnel or starts a stopped one. A start that needs a
/// password, or fails, opens the Port Forwarding page, where the tunnel shows
/// what is missing.
fn toggle_tunnel(app: &AppHandle, id: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let manager = app.state::<TunnelManager>();
        if manager.state_of(&id).is_active() {
            manager.stop(&id).await;
        } else if !crate::tunnel::commands::start_unattended(&app, &id).await {
            open_tunnels_page(&app);
        }
        invalidate(&app);
    });
}

fn on_tray_icon_event(tray: &TrayIcon, event: TrayIconEvent) {
    let app = tray.app_handle();
    match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } if !cfg!(target_os = "macos") => super::show_main_window(app),
        TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        } => super::show_main_window(app),
        // Hovering comes before the tooltip and the menu; bring both up to
        // date, including tunnels changed by a sync or an import.
        TrayIconEvent::Enter { .. } => {
            let _ = tray.set_tooltip(Some(tooltip(app)));
            refresh_tunnels(app);
        }
        _ => {}
    }
}

fn tooltip(app: &AppHandle) -> String {
    // This runs on the main thread; a busy map just leaves the count out.
    let sessions = app
        .state::<PtyMap>()
        .try_read()
        .map(|sessions| sessions.len())
        .ok();
    let tunnels = app.state::<TunnelManager>().active_count();
    let language = app
        .state::<TrayState>()
        .language
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    tooltip_text(&language, sessions, tunnels)
}

fn tooltip_text(language: &str, sessions: Option<usize>, tunnels: usize) -> String {
    let zh = language == "zh";
    let mut parts = vec!["tTerm".to_string()];
    if let Some(sessions) = sessions {
        parts.push(if zh {
            format!("{sessions} 个会话")
        } else if sessions == 1 {
            "1 session".to_string()
        } else {
            format!("{sessions} sessions")
        });
    }
    parts.push(if zh {
        format!("{tunnels} 个隧道运行中")
    } else if tunnels == 1 {
        "1 tunnel running".to_string()
    } else {
        format!("{tunnels} tunnels running")
    });
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tunnel(name: &str, state: TunnelState) -> MenuTunnel {
        MenuTunnel {
            id: "t1".to_string(),
            name: name.to_string(),
            state,
        }
    }

    #[test]
    fn tunnel_labels_say_what_is_not_plainly_on_or_off() {
        let en = labels("en");
        assert_eq!(tunnel_label(en, &tunnel("db", TunnelState::Running)), "db");
        assert_eq!(tunnel_label(en, &tunnel("db", TunnelState::Stopped)), "db");
        assert_eq!(
            tunnel_label(en, &tunnel("db", TunnelState::Reconnecting)),
            "db (connecting…)"
        );
        assert_eq!(
            tunnel_label(labels("zh"), &tunnel("db", TunnelState::NeedsCredentials)),
            "db（需要密码）"
        );
    }

    #[test]
    fn ampersands_stay_literal_in_windows_menus() {
        let label = tunnel_label(labels("en"), &tunnel("a & b", TunnelState::Stopped));
        assert_eq!(label, if cfg!(windows) { "a && b" } else { "a & b" });
    }

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

    #[test]
    fn tooltip_counts_sessions_and_tunnels() {
        assert_eq!(
            tooltip_text("en", Some(1), 2),
            "tTerm · 1 session · 2 tunnels running"
        );
        assert_eq!(tooltip_text("en", None, 1), "tTerm · 1 tunnel running");
        assert_eq!(
            tooltip_text("zh", Some(3), 0),
            "tTerm · 3 个会话 · 0 个隧道运行中"
        );
    }
}
