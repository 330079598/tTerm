//! The tray icon (the menu bar on macOS) shown while closing the window can
//! leave tTerm running: it brings the window back, starts and stops tunnels,
//! and quits. It carries a red dot while system notifications wait unseen.

use std::sync::Arc;
use tterm_core::events::background::BackgroundEvent;
use tterm_core::events::tunnel::TunnelState;
use tterm_core::events::{AppEvent, EventSink};

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::core::state::PtyMap;
use crate::tunnel::manager::TunnelManager;

pub(crate) const TRAY_ID: &str = "main";
const MENU_SHOW: &str = "tray-show";
const MENU_QUIT: &str = "tray-quit";
const MENU_CHECK_UPDATES: &str = "tray-updates";
const MENU_TUNNELS_PAGE: &str = "tray-tunnels-page";
const MENU_TUNNEL_PREFIX: &str = "tray-tunnel:";

#[derive(Default)]
pub struct TrayState {
    pub(crate) language: Mutex<String>,
    /// What the current menu shows; it is rebuilt only when this changes.
    pub(crate) shown: Mutex<Option<Vec<MenuTunnel>>>,
    refresh_pending: AtomicBool,
    /// Whether the icon shows the dot for unseen notifications.
    pub(crate) attention: AtomicBool,
    /// An update check asked for from the tray that the UI has not taken yet.
    /// It is kept because a click before the UI listens would otherwise be lost.
    update_check_requested: AtomicBool,
}

impl TrayState {
    fn take_update_check(&self) -> bool {
        self.update_check_requested.swap(false, Ordering::SeqCst)
    }
}

/// A tunnel as one menu entry shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MenuTunnel {
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

/// Dependencies used to rebuild tray policy without owning the tunnel manager.
#[derive(Clone)]
pub(crate) struct TrayContext {
    pub state: Arc<TrayState>,
    pub native: Arc<dyn tterm_platform::tray::Tray>,
    pub tunnels: std::sync::Weak<TunnelManager>,
}

/// Adds the tray icon, or rebuilds its menu in the saved language.
pub(crate) fn show(context: &TrayContext, language: &str) -> Result<(), String> {
    *context
        .state
        .language
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = language.to_string();
    let tunnels = menu_tunnels(context);
    *context
        .state
        .shown
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(tunnels.clone());
    context.native.present(tterm_platform::tray::TrayModel {
        entries: menu_model(labels(language), &tunnels),
    })
}

/// Coalesces refreshes and rebuilds the menu on the native owning thread.
pub(crate) fn refresh_tunnels(context: &TrayContext) {
    if !context.native.is_present() || context.state.refresh_pending.swap(true, Ordering::SeqCst) {
        return;
    }
    let captured = context.clone();
    if context
        .native
        .dispatch(Box::new(move || {
            captured
                .state
                .refresh_pending
                .store(false, Ordering::SeqCst);
            if let Err(error) = rebuild_if_changed(&captured) {
                eprintln!("Failed to update the tray menu: {error}");
            }
        }))
        .is_err()
    {
        context.state.refresh_pending.store(false, Ordering::SeqCst);
    }
}

fn rebuild_if_changed(context: &TrayContext) -> Result<(), String> {
    if !context.native.is_present() {
        return Ok(());
    }
    let tunnels = menu_tunnels(context);
    let mut shown = context
        .state
        .shown
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if shown.as_ref() == Some(&tunnels) {
        return Ok(());
    }
    let language = context
        .state
        .language
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    context.native.present(tterm_platform::tray::TrayModel {
        entries: menu_model(labels(&language), &tunnels),
    })?;
    *shown = Some(tunnels);
    Ok(())
}

pub(crate) fn invalidate(context: &TrayContext) {
    *context
        .state
        .shown
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
    refresh_tunnels(context);
}

fn menu_tunnels(context: &TrayContext) -> Vec<MenuTunnel> {
    let Some(manager) = context.tunnels.upgrade() else {
        return Vec::new();
    };
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

/// Applies a native menu action after the entry adapter resolves dependencies.
pub(crate) fn handle_action(
    context: super::BackgroundContext,
    tunnel: crate::tunnel::manager::TunnelContext,
    id: &str,
) {
    match id {
        MENU_SHOW => super::show_main_window(context.window.as_ref(), context.badge.as_ref()),
        MENU_QUIT => super::request_quit(&context),
        MENU_TUNNELS_PAGE => open_tunnels_page(
            context.window.as_ref(),
            context.badge.as_ref(),
            context.events.as_ref(),
        ),
        MENU_CHECK_UPDATES => check_updates(
            &context.tray.state,
            context.window.as_ref(),
            context.badge.as_ref(),
            context.events.as_ref(),
        ),
        id => {
            if let Some(id) = id.strip_prefix(MENU_TUNNEL_PREFIX) {
                let id = id.to_string();
                context.quit.runtime.clone().spawn(async move {
                    if context.tunnels.state_of(&id).is_active() {
                        context.tunnels.stop(&id).await;
                    } else if !crate::tunnel::manager::start_unattended(
                        &tunnel,
                        &context.tunnels,
                        &id,
                    )
                    .await
                    {
                        open_tunnels_page(
                            context.window.as_ref(),
                            context.badge.as_ref(),
                            context.events.as_ref(),
                        );
                    }
                    invalidate(&context.tray);
                });
            }
        }
    }
}

pub(crate) fn open_tunnels_page(
    window: &dyn tterm_platform::window::MainWindow,
    badge: &dyn tterm_platform::notifications::AppBadge,
    events: &dyn EventSink,
) {
    super::show_main_window(window, badge);
    let _ = events.emit(AppEvent::Background(BackgroundEvent::OpenTunnels));
}

pub(crate) fn check_updates(
    state: &TrayState,
    window: &dyn tterm_platform::window::MainWindow,
    badge: &dyn tterm_platform::notifications::AppBadge,
    events: &dyn EventSink,
) {
    super::show_main_window(window, badge);
    state.update_check_requested.store(true, Ordering::SeqCst);
    let _ = events.emit(AppEvent::Background(BackgroundEvent::CheckUpdates));
}

/// Hands the UI an update check asked for from the tray, once.
#[tauri::command]
pub fn take_tray_update_check(
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) -> bool {
    services.app.tray.take_update_check()
}

pub(crate) fn tooltip(ptys: &PtyMap, tunnels: &TunnelManager, state: &TrayState) -> String {
    let sessions = ptys.try_read().map(|sessions| sessions.len()).ok();
    let language = state
        .language
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    tooltip_text(&language, sessions, tunnels.active_count())
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
    fn model_keeps_actions_and_tunnel_checked_state_across_rebuilds() {
        use tterm_platform::tray::MenuEntry;
        let model = menu_model(labels("en"), &[tunnel("db", TunnelState::Running)]);
        assert!(matches!(&model[0], MenuEntry::Item { id, .. } if id == MENU_SHOW));
        assert!(matches!(&model[5], MenuEntry::Item { id, .. } if id == MENU_QUIT));
        let MenuEntry::Submenu { entries, .. } = &model[2] else {
            panic!("missing tunnel submenu");
        };
        assert!(
            matches!(&entries[0], MenuEntry::Item { id, checked: Some(true), .. } if id == "tray-tunnel:t1")
        );
        let stopped = menu_model(labels("zh"), &[tunnel("db", TunnelState::Stopped)]);
        let MenuEntry::Submenu { entries, .. } = &stopped[2] else {
            panic!("missing tunnel submenu");
        };
        assert!(matches!(
            &entries[0],
            MenuEntry::Item {
                checked: Some(false),
                ..
            }
        ));
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
