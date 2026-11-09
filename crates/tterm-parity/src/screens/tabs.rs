//! GPUI parity screens for the Tabs primitive.

use crate::color::{hsl_to_rgba, to_gpui};
use crate::layout::{ElementSizing, LayoutSink, PaddingSnapshot, StyleSnapshot};
use crate::manifest::ScreensManifest;
use crate::marker::{parity_child, parity_element, parity_text_for};
use gpui_kit::{Context, IntoElement, ParentElement, Styled, div, px};
use tterm_ui::components::tabs::{TriggerState, content, list, style, trigger};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::palette::{DARK, LIGHT};
use tterm_ui::tokens::theme::Theme;

pub const DEFAULT_SCREEN_ID: &str = "ui.tabs.default";
pub const SERVER_SCREEN_ID: &str = "ui.tabs.server-monitor";

pub struct TabsScreen {
    layout: LayoutSink,
    theme: Theme,
    server: bool,
    selected: usize,
}

impl TabsScreen {
    pub fn load(
        manifest: &ScreensManifest,
        layout: LayoutSink,
        server: bool,
    ) -> Result<Self, String> {
        let theme = match manifest.theme.as_str() {
            "dark" => DARK,
            "light" => LIGHT,
            other => return Err(format!("unsupported theme {other}")),
        };
        Ok(Self {
            layout,
            theme,
            server,
            selected: 0,
        })
    }

    fn style(&self, active: bool, focused: bool) -> StyleSnapshot {
        let colors = self.theme.colors;
        let geometry = style(self.server);
        StyleSnapshot {
            background_color: if active {
                hsl_to_rgba(colors.background)
            } else {
                [0, 0, 0, 0]
            },
            border_color: hsl_to_rgba(colors.background),
            border_width: 0.0,
            border_radius: geometry.radius,
            ring_spread: if focused { geometry.ring_spread } else { 0.0 },
            ring_spreads: if focused {
                geometry.ring_spreads().to_vec()
            } else {
                Vec::new()
            },
            font_family: "system-ui".into(),
            font_size: geometry.font_size,
            font_weight: 500,
            line_height: geometry.line_height,
            color: hsl_to_rgba(if active {
                colors.foreground
            } else {
                colors.muted_foreground
            }),
            padding: PaddingSnapshot {
                top: geometry.padding_y,
                right: geometry.padding_x,
                bottom: geometry.padding_y,
                left: geometry.padding_x,
            },
            declared_background_color: None,
            declared_border_color: None,
            opacity: 1.0,
        }
    }

    fn root_style(&self) -> StyleSnapshot {
        let colors = self.theme.colors;
        StyleSnapshot {
            background_color: hsl_to_rgba(colors.background),
            border_color: hsl_to_rgba(colors.background),
            border_width: 0.0,
            border_radius: 0.0,
            ring_spread: 0.0,
            ring_spreads: Vec::new(),
            font_family: "system-ui".into(),
            font_size: 13.0,
            font_weight: 400,
            line_height: 18.2,
            color: hsl_to_rgba(colors.foreground),
            padding: PaddingSnapshot::all(0.0),
            declared_background_color: None,
            declared_border_color: None,
            opacity: 1.0,
        }
    }

    fn list_style(&self) -> StyleSnapshot {
        let colors = self.theme.colors;
        let geometry = style(self.server);
        StyleSnapshot {
            background_color: hsl_to_rgba(colors.muted),
            border_color: hsl_to_rgba(colors.muted),
            border_width: 0.0,
            border_radius: geometry.list_radius,
            ring_spread: 0.0,
            ring_spreads: Vec::new(),
            font_family: "system-ui".into(),
            font_size: 13.0,
            font_weight: 400,
            line_height: 18.2,
            color: hsl_to_rgba(colors.muted_foreground),
            padding: PaddingSnapshot::all(geometry.list_padding),
            declared_background_color: None,
            declared_border_color: None,
            opacity: 1.0,
        }
    }

    pub fn render(&self, cx: &mut Context<super::ScreenView>) -> impl IntoElement {
        let geometry = style(self.server);
        let colors = self.theme.colors;
        let labels = ["Overview", "CPU", "Memory"];
        let prefix = if self.server {
            "ui.tabs.server"
        } else {
            "ui.tabs"
        };
        let list_style = self.list_style();
        let triggers = labels
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                let active = index == self.selected;
                let focused = index == 2;
                let id = if self.server {
                    format!("{prefix}-{}", ["active", "inactive", "focus"][index])
                } else {
                    format!("{prefix}.{}", ["active", "inactive", "focus"][index])
                };
                parity_child(
                    &self.layout,
                    id,
                    ElementSizing::Fixed,
                    self.style(active, focused),
                    trigger(
                        format!("{prefix}-trigger-{index}"),
                        label,
                        TriggerState {
                            active,
                            focused,
                            disabled: false,
                        },
                        self.theme,
                        cx.listener(move |view, _, _, cx| {
                            if let super::ScreenView::TabsDefault(tabs)
                            | super::ScreenView::TabsServer(tabs) = view
                            {
                                tabs.selected = index;
                                cx.notify();
                            }
                        }),
                    )
                    .geometry(geometry),
                )
                .into_any_element()
            })
            .collect::<Vec<_>>();
        let list_id = if self.server {
            "ui.tabs.server-list"
        } else {
            "ui.tabs.list"
        };
        let content_id = if self.server {
            "ui.tabs.server-content"
        } else {
            "ui.tabs.content"
        };
        let content_style = StyleSnapshot {
            background_color: hsl_to_rgba(colors.background),
            border_color: hsl_to_rgba(colors.background),
            border_width: 0.0,
            border_radius: 0.0,
            ring_spread: 0.0,
            ring_spreads: Vec::new(),
            font_family: "system-ui".into(),
            font_size: 13.0,
            font_weight: 400,
            line_height: 18.2,
            color: hsl_to_rgba(colors.foreground),
            padding: PaddingSnapshot {
                top: geometry.content_padding.0,
                bottom: geometry.content_padding.0,
                left: geometry.content_padding.1,
                right: geometry.content_padding.1,
            },
            declared_background_color: None,
            declared_border_color: None,
            opacity: 1.0,
        };
        let text = ["Active overview content", "CPU content", "Memory content"][self.selected];
        let content_view = parity_child(
            &self.layout,
            content_id,
            ElementSizing::Fixed,
            content_style.clone(),
            content(
                geometry,
                parity_text_for(&self.layout, "tabs-content-text", text, &content_style),
            )
            .h(px(if self.server { 498.0 } else { 18.0 })),
        )
        .w_full();
        let list_view = parity_child(
            &self.layout,
            list_id,
            ElementSizing::Fixed,
            list_style,
            list(
                self.theme,
                geometry,
                div().flex().gap(px(geometry.gap)).children(triggers),
            ),
        );
        let list_view = if self.server {
            div()
                .w_full()
                .h(px(38.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_between()
                .pt(px(5.0))
                .px(px(8.0))
                .border_b(px(1.0))
                .border_color(hsla(colors.border, 0.72))
                .bg(to_gpui(hsl_to_rgba(colors.titlebar)))
                .child(div().w(px(28.0)).h(px(28.0)).flex_none())
                .child(list_view)
        } else {
            list_view
        };
        let root_style = self.root_style();
        let root_id = if self.server {
            "ui.tabs.server"
        } else {
            "ui.tabs.default"
        };
        div()
            .size_full()
            .p(px(32.0))
            .bg(to_gpui(hsl_to_rgba(colors.background)))
            .child(parity_element(
                &self.layout,
                root_id,
                ElementSizing::Fixed,
                root_style,
                |view| {
                    view.w_full()
                        .h(px(if self.server { 536.0 } else { 66.0 }))
                        .flex()
                        .items_start()
                        .flex_col()
                        .child(list_view)
                        .child(content_view)
                },
            ))
    }
}
