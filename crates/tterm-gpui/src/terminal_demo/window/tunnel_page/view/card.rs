//! Tunnel cards and the saved/live statistics they display.

use gpui_kit::prelude::*;
use gpui_kit::{BoxShadow, Context, Div, FontWeight, Role, Stateful, div, px};
use tterm_core::events::tunnel::TunnelState;
use tterm_core::i18n::keys;
use tterm_core::tunnel::types::TunnelKind;
use tterm_ui::components::icons::{ARROW_DOWN, ARROW_UP, TRIANGLE_ALERT, icon};
use tterm_ui::tokens::color::Hsl;
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::palette::Colors;

use super::super::data::TunnelRow;
use super::format::{format_bytes, format_date, format_uptime};
use super::{TUNNEL_PAGE_LIST_ID, TunnelPageView};

const CARD_MIN_WIDTH: f32 = 380.0;
const CARD_PADDING: f32 = 16.0;
const CARD_GAP: f32 = 12.0;
const CARD_LIST_GAP: f32 = 16.0;

impl TunnelPageView {
    /// Renders the saved rules as a wrapping grid of cards.
    pub(super) fn list(
        &self,
        rows: &[TunnelRow],
        now: u64,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let mut list = div()
            .id(TUNNEL_PAGE_LIST_ID)
            .debug_selector(|| TUNNEL_PAGE_LIST_ID.to_string())
            .role(Role::List)
            .flex()
            .flex_wrap()
            .gap(px(CARD_LIST_GAP));
        for row in rows {
            list = list.child(self.card(row, now, cx));
        }
        list
    }

    fn card(&self, row: &TunnelRow, now: u64, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let id = row.rule.id.clone();
        let card_selector = format!("tunnel-card-{id}");
        let name_selector = format!("tunnel-card-name-{id}");
        let host_missing_selector = format!("tunnel-card-host-missing-{id}");
        let status_selector = format!("tunnel-card-status-{id}");
        let kind_selector = format!("tunnel-card-kind-{id}");
        let footer_selector = format!("tunnel-card-footer-{id}");
        let card_key = format!("tunnel-card-{id}");
        let mut card = div()
            .id(card_key)
            .debug_selector(move || card_selector)
            .role(Role::Article)
            .aria_label(row.rule.name.clone())
            .flex()
            .flex_col()
            .gap(px(CARD_GAP))
            .flex_basis(px(CARD_MIN_WIDTH))
            .flex_shrink_0()
            .flex_grow(1.0)
            .min_w(px(CARD_MIN_WIDTH))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .bg(hsla(colors.card, 1.0))
            .p(px(CARD_PADDING))
            .shadow(card_shadows());
        card = card
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(12.0))
                    .child(
                        div()
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .id(format!("tunnel-card-name-{id}"))
                                    .debug_selector(move || name_selector)
                                    .truncate()
                                    .text_size(px(14.0))
                                    .line_height(px(20.0))
                                    .font_weight(FontWeight(600.0))
                                    .child(row.rule.name.clone()),
                            )
                            .child(self.card_metadata(
                                row,
                                now,
                                kind_selector,
                                status_selector,
                                cx,
                            )),
                    ),
            )
            .child(self.route(row, cx));
        let footer = div()
            .id(format!("tunnel-card-footer-{id}"))
            .debug_selector(move || footer_selector)
            .min_h(px(16.0))
            .text_size(px(12.0))
            .line_height(px(16.0))
            .text_color(hsla(colors.muted_foreground, 1.0));
        let footer = if row.host_missing {
            footer.child(self.host_missing(host_missing_selector, cx))
        } else {
            self.card_footer(row, now, cx, footer)
        };
        card.child(footer)
    }

    fn card_metadata(
        &self,
        row: &TunnelRow,
        now: u64,
        kind_selector: String,
        status_selector: String,
        cx: &mut Context<Self>,
    ) -> Div {
        let colors = self.theme.colors;
        let kind = self.kind_label(row.rule.kind, cx);
        let state = self.state_label(row.status.state, cx);
        let dot = status_color(row.status.state, colors);
        let mut status = div()
            .id(format!("tunnel-card-status-{}", row.rule.id))
            .debug_selector(move || status_selector)
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .size(px(8.0))
                    .flex_shrink_0()
                    .rounded(px(4.0))
                    .bg(hsla(dot.0, dot.1)),
            )
            .child(state);
        if row.status.state == TunnelState::Running
            && let Some(connected_at) = row.status.connected_at
        {
            status = status.child(format!("· {}", format_uptime(connected_at, now)));
        }
        div()
            .mt(px(4.0))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.0))
            .text_size(px(12.0))
            .line_height(px(16.0))
            .text_color(hsla(colors.muted_foreground, 1.0))
            .child(
                div()
                    .id(format!("tunnel-card-kind-{}", row.rule.id))
                    .debug_selector(move || kind_selector)
                    .rounded(px(999.0))
                    .border(px(1.0))
                    .border_color(hsla(colors.border, 1.0))
                    .px(px(6.0))
                    .py(px(1.0))
                    .text_size(px(10.0))
                    .line_height(px(14.0))
                    .font_weight(FontWeight(600.0))
                    .child(kind.to_uppercase()),
            )
            .child(status)
    }

    fn host_missing(&self, selector: String, cx: &mut Context<Self>) -> Div {
        let colors = self.theme.colors;
        div()
            .debug_selector(move || selector)
            .text_color(hsla(colors.destructive, 1.0))
            .child(self.text(cx, keys::TUNNELS_HOST_MISSING))
    }

    fn card_footer(
        &self,
        row: &TunnelRow,
        now: u64,
        cx: &mut Context<Self>,
        mut footer: Stateful<Div>,
    ) -> Stateful<Div> {
        let colors = self.theme.colors;
        match row.status.state {
            TunnelState::NeedsCredentials => {
                footer = footer.child(
                    div()
                        .text_color(hsla(colors.warning, 1.0))
                        .child(self.text(cx, keys::TUNNELS_NEEDS_CREDENTIALS_HINT)),
                );
            }
            TunnelState::Error | TunnelState::Reconnecting
                if row
                    .status
                    .message
                    .as_deref()
                    .is_some_and(|message| !message.is_empty()) =>
            {
                let color = if row.status.state == TunnelState::Error {
                    colors.destructive
                } else {
                    colors.warning
                };
                footer = footer.child(
                    div()
                        .text_color(hsla(color, 1.0))
                        .child(row.status.message.clone().unwrap_or_default()),
                );
            }
            TunnelState::Running => {
                footer = footer.child(self.running_stats(row, now, cx));
            }
            _ => {
                if let Some(lifetime) = self.lifetime(row, cx) {
                    footer = footer.child(lifetime);
                }
            }
        }
        footer
    }

    fn running_stats(&self, row: &TunnelRow, now: u64, cx: &mut Context<Self>) -> Div {
        let colors = self.theme.colors;
        let active = self.text_count(
            cx,
            keys::TUNNELS_STATS_CONNECTIONS,
            row.status.active_connections,
        );
        let total = self.text_count(cx, keys::TUNNELS_STATS_TOTAL, row.status.total_connections);
        let mut stats = div().flex().flex_col().gap(px(4.0)).child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(12.0))
                .child(div().child(format!("{active} · {total}")))
                .child(self.traffic_amounts(row.status.bytes_up, row.status.bytes_down)),
        );
        if let Some(lifetime) = self.lifetime(row, cx) {
            stats = stats.child(lifetime);
        }
        if let Some(failure) = &row.status.last_failure {
            let failed = self.text_count(
                cx,
                keys::TUNNELS_STATS_FAILED,
                row.status.failed_connections,
            );
            let ago = format_uptime(failure.at, now);
            let ago = self.text_arg(cx, keys::TUNNELS_STATS_AGO, "time", &ago);
            stats = stats.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(4.0))
                    .text_color(hsla(colors.warning, 1.0))
                    .child(div().mt(px(2.0)).flex_shrink_0().child(icon(
                        TRIANGLE_ALERT,
                        hsla(colors.warning, 1.0),
                        12.0,
                    )))
                    .child(
                        div()
                            .min_w(px(0.0))
                            .line_clamp(2)
                            .child(format!("{failed} · {} · {ago}", failure.message)),
                    ),
            );
        }
        stats
    }

    fn lifetime(&self, row: &TunnelRow, cx: &mut Context<Self>) -> Option<Div> {
        let lifetime = row.status.lifetime;
        let has_lifetime = lifetime.since.is_some()
            || lifetime.bytes_up > 0
            || lifetime.bytes_down > 0
            || lifetime.connections > 0;
        if !has_lifetime {
            return None;
        }
        let colors = self.theme.colors;
        let label = self.text(cx, keys::TUNNELS_STATS_LIFETIME);
        let connections = self.text_count(
            cx,
            keys::TUNNELS_STATS_LIFETIME_CONNECTIONS,
            lifetime.connections,
        );
        let mut line = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(12.0))
            .text_color(hsla(colors.muted_foreground, 1.0))
            .child(label)
            .child(self.traffic_amounts(lifetime.bytes_up, lifetime.bytes_down))
            .child(connections);
        if let Some(since) = lifetime.since {
            let language = self.locale.read(cx).language();
            let date = format_date(since, language);
            line = line.child(self.text_arg(cx, keys::TUNNELS_STATS_SINCE, "date", &date));
        }
        Some(line)
    }

    fn traffic_amounts(&self, up: u64, down: u64) -> Div {
        let colors = self.theme.colors;
        let muted = hsla(colors.muted_foreground, 1.0);
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(icon(ARROW_UP, muted, 12.0))
                    .child(format_bytes(up)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(icon(ARROW_DOWN, muted, 12.0))
                    .child(format_bytes(down)),
            )
    }

    fn kind_label(&self, kind: TunnelKind, cx: &mut Context<Self>) -> String {
        let key = match kind {
            TunnelKind::Local => keys::TUNNELS_KIND_LOCAL,
            TunnelKind::Remote => keys::TUNNELS_KIND_REMOTE,
            TunnelKind::Dynamic => keys::TUNNELS_KIND_DYNAMIC,
        };
        self.text(cx, key)
    }

    fn state_label(&self, state: TunnelState, cx: &mut Context<Self>) -> String {
        let key = match state {
            TunnelState::Stopped => keys::TUNNELS_STATE_STOPPED,
            TunnelState::Starting => keys::TUNNELS_STATE_STARTING,
            TunnelState::Running => keys::TUNNELS_STATE_RUNNING,
            TunnelState::Reconnecting => keys::TUNNELS_STATE_RECONNECTING,
            TunnelState::Error => keys::TUNNELS_STATE_ERROR,
            TunnelState::NeedsCredentials => keys::TUNNELS_STATE_NEEDS_CREDENTIALS,
        };
        self.text(cx, key)
    }
}

fn card_shadows() -> Vec<BoxShadow> {
    let color = gpui_kit::black().opacity(0.1);
    vec![
        BoxShadow::new(px(0.0), px(1.0), color).blur_radius(px(1.5)),
        BoxShadow::new(px(0.0), px(1.0), color)
            .blur_radius(px(1.0))
            .spread_radius(px(-1.0)),
    ]
}

fn status_color(state: TunnelState, colors: Colors) -> (Hsl, f32) {
    match state {
        TunnelState::Stopped => (colors.muted_foreground, 0.4),
        TunnelState::Starting | TunnelState::Reconnecting => (colors.warning, 1.0),
        TunnelState::Running => (colors.success, 1.0),
        TunnelState::Error => (colors.destructive, 1.0),
        TunnelState::NeedsCredentials => (colors.warning, 1.0),
    }
}
