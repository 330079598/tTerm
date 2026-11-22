//! The three-node route diagram inside each tunnel card.

use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Div, FontWeight, Role, Stateful, div, px};
use tterm_core::events::tunnel::TunnelState;
use tterm_core::i18n::keys;
use tterm_core::tunnel::types::{TunnelKind, TunnelRule};
use tterm_ui::components::icons::{CHEVRON_RIGHT, LAPTOP, SERVER, TARGET, icon};
use tterm_ui::tokens::color::Hsl;
use tterm_ui::tokens::gpui::hsla;

use super::super::data::TunnelRow;
use super::TunnelPageView;

const ROUTE_CONNECTOR_WIDTH: f32 = 34.0;

impl TunnelPageView {
    /// Draws the device, SSH host and destination nodes in flow order.
    pub(super) fn route(&self, row: &TunnelRow, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let host = self.server_label(row);
        let nodes = route_nodes(&row.rule, &host, cx, self);
        let flow = flow_state(row.status.state);
        let mut route = div()
            .id(format!("tunnel-route-{}", row.rule.id))
            .debug_selector({
                let id = row.rule.id.clone();
                move || format!("tunnel-route-{id}")
            })
            .role(Role::Group)
            .flex()
            .items_center()
            .w_full();
        for (index, node) in nodes.iter().enumerate() {
            if index > 0 {
                route = route.child(connector(
                    flow,
                    colors.border,
                    colors.success,
                    colors.warning,
                    colors.destructive,
                ));
            }
            route = route.child(self.route_node(
                node,
                row.rule.kind,
                flow,
                row.rule.id.as_str(),
                index,
                cx,
            ));
        }
        route
    }

    fn route_node(
        &self,
        node: &RouteNode,
        kind: TunnelKind,
        flow: FlowState,
        rule_id: &str,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let colors = self.theme.colors;
        let title = match node.kind {
            RouteNodeKind::Device if kind == TunnelKind::Dynamic => {
                self.text(cx, keys::TUNNELS_ROUTE_PROXY)
            }
            RouteNodeKind::Device => self.text(cx, keys::TUNNELS_ROUTE_DEVICE),
            RouteNodeKind::Server => node
                .title
                .clone()
                .unwrap_or_else(|| self.text(cx, keys::TUNNELS_ROUTE_SERVER)),
            RouteNodeKind::Target => self.text(cx, keys::TUNNELS_ROUTE_TARGET),
        };
        let detail = node.detail.clone().or_else(|| {
            (node.kind == RouteNodeKind::Target && kind == TunnelKind::Dynamic)
                .then(|| self.text(cx, keys::TUNNELS_ROUTE_ANY_DESTINATION))
        });
        let icon_data = match node.kind {
            RouteNodeKind::Device => LAPTOP,
            RouteNodeKind::Server => SERVER,
            RouteNodeKind::Target => TARGET,
        };
        let id = format!("tunnel-route-node-{rule_id}-{index}");
        div()
            .id(format!("tunnel-route-node-{rule_id}-{index}"))
            .debug_selector(move || id)
            .flex()
            .flex_1()
            .flex_basis(px(0.0))
            .min_w(px(0.0))
            .flex_col()
            .items_center()
            .gap(px(4.0))
            .px(px(8.0))
            .py(px(8.0))
            .rounded(px(6.0))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .bg(hsla(colors.background, 1.0))
            .text_center()
            .when(flow == FlowState::Idle, |node| node.opacity(0.6))
            .child(icon(icon_data, hsla(colors.muted_foreground, 1.0), 16.0))
            .child(
                div()
                    .w_full()
                    .truncate()
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .font_weight(FontWeight(500.0))
                    .child(title),
            )
            .child(
                div()
                    .w_full()
                    .line_clamp(2)
                    .min_h(px(16.0))
                    .text_size(px(11.0))
                    .line_height(px(15.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(detail.unwrap_or_default()),
            )
    }

    fn server_label(&self, row: &TunnelRow) -> String {
        if row.host_label.is_empty() {
            // The caller uses this fallback whenever a profile was deleted.
            String::new()
        } else {
            row.host_label.clone()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FlowState {
    Active,
    Pending,
    Error,
    Idle,
}

fn flow_state(state: TunnelState) -> FlowState {
    match state {
        TunnelState::Running => FlowState::Active,
        TunnelState::Starting | TunnelState::Reconnecting => FlowState::Pending,
        TunnelState::Error => FlowState::Error,
        TunnelState::Stopped | TunnelState::NeedsCredentials => FlowState::Idle,
    }
}

fn connector(flow: FlowState, border: Hsl, success: Hsl, warning: Hsl, destructive: Hsl) -> Div {
    let (color, alpha) = match flow {
        FlowState::Active => (success, 1.0),
        FlowState::Pending => (warning, 1.0),
        FlowState::Error => (destructive, 0.6),
        FlowState::Idle => (border, 1.0),
    };
    let color = hsla(color, alpha);
    div()
        .relative()
        .w(px(ROUTE_CONNECTOR_WIDTH))
        .h(px(2.0))
        .mx(px(2.0))
        .flex_shrink_0()
        .rounded(px(1.0))
        .bg(color)
        .child(div().absolute().top(px(-3.0)).right(px(-2.0)).child(icon(
            CHEVRON_RIGHT,
            color,
            8.0,
        )))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RouteNodeKind {
    Device,
    Server,
    Target,
}

struct RouteNode {
    kind: RouteNodeKind,
    title: Option<String>,
    detail: Option<String>,
}

fn route_nodes(rule: &TunnelRule, host: &str, cx: &App, view: &TunnelPageView) -> [RouteNode; 3] {
    let bind = endpoint(&rule.bind_host, rule.bind_port);
    let destination = endpoint(&rule.dest_host, rule.dest_port);
    let server = if host.is_empty() {
        view.text(cx, keys::TUNNELS_ROUTE_SERVER)
    } else {
        host.to_string()
    };
    match rule.kind {
        TunnelKind::Local => [
            RouteNode {
                kind: RouteNodeKind::Device,
                title: None,
                detail: Some(bind),
            },
            RouteNode {
                kind: RouteNodeKind::Server,
                title: Some(server),
                detail: None,
            },
            RouteNode {
                kind: RouteNodeKind::Target,
                title: None,
                detail: Some(destination),
            },
        ],
        TunnelKind::Remote => [
            RouteNode {
                kind: RouteNodeKind::Server,
                title: Some(server),
                detail: Some(bind),
            },
            RouteNode {
                kind: RouteNodeKind::Device,
                title: None,
                detail: None,
            },
            RouteNode {
                kind: RouteNodeKind::Target,
                title: None,
                detail: Some(destination),
            },
        ],
        TunnelKind::Dynamic => [
            RouteNode {
                kind: RouteNodeKind::Device,
                title: None,
                detail: Some(bind),
            },
            RouteNode {
                kind: RouteNodeKind::Server,
                title: Some(server),
                detail: None,
            },
            RouteNode {
                kind: RouteNodeKind::Target,
                title: None,
                detail: None,
            },
        ],
    }
}

fn endpoint(host: &str, port: u16) -> String {
    let host = host.trim();
    let value = if host.is_empty() { "…" } else { host };
    let rendered = if value.contains(':') && !value.starts_with('[') {
        format!("[{value}]")
    } else {
        value.to_string()
    };
    let port = if port == 0 {
        "…".to_string()
    } else {
        port.to_string()
    };
    format!("{rendered}:{port}")
}
