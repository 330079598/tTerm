//! The Port Forwarding entry in the tab strip.

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, MouseButton, Stateful, div, px};
use tterm_ui::components::icons::{WAYPOINTS, icon};
use tterm_ui::tokens::gpui::hsla;

use super::{CLOSE_BUTTON_ID, TAB_ID};
use crate::terminal_demo::window::Prototype;

impl Prototype {
    /// Draws the singleton page as a tab beside the session tabs. Selecting it
    /// again only refreshes the route; it never adds a second page.
    pub(in crate::terminal_demo::window) fn tunnel_page_tab(
        &self,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let colors = self.ui.theme.colors;
        let active = self.tunnel_page_is_open();
        let label = self.tunnel_page_title(cx);
        let group = "tunnel-page-tab-group";
        div()
            .id(TAB_ID)
            .debug_selector(|| TAB_ID.to_string())
            .group(group)
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(6.0))
            .h(px(30.0))
            .pl(px(9.0))
            .pr(px(7.0))
            .rounded(px(5.0))
            .min_w(px(112.0))
            .max_w(px(220.0))
            .cursor_pointer()
            .bg(hsla(
                if active {
                    colors.tab_active
                } else {
                    colors.tab_background
                },
                1.0,
            ))
            .when(!active, |tab| {
                tab.hover(|style| style.bg(hsla(colors.tab_hover, 1.0)))
            })
            .when(active, |tab| {
                tab.child(
                    div()
                        .absolute()
                        .top_0()
                        .left(px(5.0))
                        .right(px(5.0))
                        .h(px(2.5))
                        .bg(hsla(colors.tab_active_border, 1.0)),
                )
            })
            .child(icon(WAYPOINTS, hsla(colors.foreground, 0.72), 13.0))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .truncate()
                    .text_size(px(13.0))
                    .child(label.clone()),
            )
            .child(self.tunnel_page_close_button(active, group, label, cx))
            .on_click(cx.listener(|this, _, window, cx| this.open_tunnel_page(window, cx)))
    }

    fn tunnel_page_close_button(
        &self,
        active: bool,
        group: &'static str,
        label: String,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let muted = self.ui.theme.colors.muted;
        div()
            .id(CLOSE_BUTTON_ID)
            .debug_selector(|| CLOSE_BUTTON_ID.to_string())
            .aria_label(label)
            .size(px(18.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.0))
            .when(!active, |button| {
                button
                    .opacity(0.0)
                    .group_hover(group, |style| style.opacity(1.0))
            })
            .hover(move |style| style.bg(hsla(muted, 0.82)))
            .child("×")
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_click(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.close_tunnel_page(window, cx);
            }))
    }
}
