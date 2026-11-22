//! The singleton, read-only Port Forwarding page.
//!
//! The page is a tab in the strip, not part of the pane tree: its data loads
//! only while it is open, it never owns a PTY, and it is never written into
//! the saved workspace. Closing it returns to the pane that had focus.

use std::sync::Arc;

use gpui_kit::{App, AppContext, Context, Entity, FocusHandle, Window};
use tterm_core::events::AppEvent;
use tterm_core::events::tunnel::{TunnelEvent, TunnelStatus};
use tterm_core::services::AppServices;
use tterm_ui::tokens::theme::Theme;

use super::{Locale, Prototype};

pub(super) mod data;
mod route;
mod view;
mod wiring;

pub(super) const OPEN_BUTTON_ID: &str = "tunnel-page-open";
pub(super) const TAB_ID: &str = "tunnel-page-tab";
pub(super) const CLOSE_BUTTON_ID: &str = "tunnel-page-close";

/// The page's data and view entities, plus the route state kept outside the
/// workspace's tabs and splits.
pub(super) struct TunnelPage {
    data: Entity<data::TunnelPageData>,
    view: Entity<view::TunnelPageView>,
    open: bool,
    restore_focus: Option<FocusHandle>,
}

impl TunnelPage {
    pub(super) fn new(
        services: Arc<AppServices>,
        locale: Entity<Locale>,
        theme: Theme,
        cx: &mut Context<Prototype>,
    ) -> Self {
        let data = cx.new(|cx| data::TunnelPageData::new(services, cx));
        let view = cx.new(|_| {
            let mut view = view::TunnelPageView::new(locale, theme);
            view.set_theme(theme);
            view
        });
        cx.observe(&data, |_, _, cx| cx.notify()).detach();
        Self {
            data,
            view,
            open: false,
            restore_focus: None,
        }
    }

    pub(super) fn apply_status(&mut self, status: TunnelStatus, cx: &mut Context<Prototype>) {
        self.data
            .update(cx, |data, cx| data.apply_status(status, cx));
    }

    /// Applies a window event that belongs to the page, and consumes it.
    pub(super) fn handle_event(&mut self, event: &AppEvent, cx: &mut Context<Prototype>) -> bool {
        let AppEvent::Tunnel(TunnelEvent::Status(status)) = event else {
            return false;
        };
        if !self.data.read(cx).is_visible() {
            return false;
        }
        self.apply_status(status.clone(), cx);
        true
    }
}

impl Prototype {
    /// Opens the singleton page without touching any session or pane.
    pub(super) fn open_tunnel_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tunnel_page_is_open() {
            self.ui.tunnel_page.data.update(cx, |data, cx| {
                data.open(cx);
                data.refresh(cx);
            });
            return;
        }
        if self.settings_is_open() && !self.close_settings(window, cx) {
            return;
        }
        self.picker = None;
        self.ui
            .template_ui
            .update(cx, |template_ui, cx| template_ui.dismiss(cx));
        self.ui
            .saved_connections
            .update(cx, |saved_connections, cx| saved_connections.dismiss(cx));
        let focus = self.tabs.active().view.read(cx).focus_handle();
        self.ui.tunnel_page.restore_focus = Some(focus);
        self.ui.tunnel_page.open = true;
        self.ui
            .tunnel_page
            .data
            .update(cx, |data, cx| data.open(cx));
        cx.notify();
    }

    /// Closes the page and restores the exact pane focus it replaced.
    pub(super) fn close_tunnel_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tunnel_page_is_open() {
            return;
        }
        self.ui.tunnel_page.open = false;
        self.ui
            .tunnel_page
            .data
            .update(cx, |data, cx| data.close(cx));
        let focus = self
            .ui
            .tunnel_page
            .restore_focus
            .take()
            .unwrap_or_else(|| self.tabs.active().view.read(cx).focus_handle());
        focus.focus(window, cx);
        cx.notify();
    }

    pub(super) fn tunnel_page_is_open(&self) -> bool {
        self.ui.tunnel_page.open
    }

    /// Renders the page body while the title bar and tab strip stay mounted.
    pub(super) fn tunnel_page(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let model = self.ui.tunnel_page.data.read(cx).model().clone();
        self.ui
            .tunnel_page
            .view
            .update(cx, |view, cx| view.render_page(&model, cx))
    }

    pub(super) fn tunnel_page_title(&self, cx: &App) -> String {
        self.ui
            .locale
            .read(cx)
            .text(tterm_core::i18n::keys::TUNNELS_TITLE)
            .unwrap_or_else(|error| {
                eprintln!("tterm-gpui: failed to translate the tunnels title: {error}");
                tterm_core::i18n::keys::TUNNELS_TITLE.as_str().to_string()
            })
    }
}
