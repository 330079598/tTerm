//! GPUI parity screens for the Tabs primitive.

use crate::color::{hsl_to_rgba, to_gpui};
use crate::layout::{ElementSizing, LayoutSink, PaddingSnapshot, StyleSnapshot};
use crate::manifest::ScreensManifest;
use crate::marker::{parity_child, parity_element, parity_text_for};
use gpui_kit::{IntoElement, ParentElement, Styled, div, px};
use tterm_ui::components::tabs::{TriggerState, list, trigger};
use tterm_ui::tokens::palette::{DARK, LIGHT};
use tterm_ui::tokens::theme::Theme;

pub const DEFAULT_SCREEN_ID: &str = "ui.tabs.default";
pub const SERVER_SCREEN_ID: &str = "ui.tabs.server-monitor";

pub struct TabsScreen {
    layout: LayoutSink,
    theme: Theme,
    server: bool,
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
        })
    }

    fn style(&self, active: bool, focused: bool) -> StyleSnapshot {
        let colors = self.theme.colors;
        StyleSnapshot {
            background_color: if active {
                hsl_to_rgba(colors.background)
            } else {
                [0, 0, 0, 0]
            },
            border_color: hsl_to_rgba(colors.background),
            border_width: 0.0,
            border_radius: 4.0,
            ring_spread: if focused { 4.0 } else { 0.0 },
            font_family: "system-ui".into(),
            font_size: if self.server { 11.0 } else { 14.0 },
            font_weight: 500,
            line_height: if self.server { 13.0 } else { 16.8 },
            color: hsl_to_rgba(if active {
                colors.foreground
            } else {
                colors.muted_foreground
            }),
            padding: PaddingSnapshot {
                top: if self.server { 0.0 } else { 6.0 },
                right: if self.server { 10.0 } else { 12.0 },
                bottom: if self.server { 0.0 } else { 6.0 },
                left: if self.server { 10.0 } else { 12.0 },
            },
            declared_background_color: None,
            declared_border_color: None,
            opacity: 1.0,
        }
    }

    pub fn render(&self) -> impl IntoElement {
        let colors = self.theme.colors;
        let labels = ["Overview", "CPU", "Memory"];
        let prefix = if self.server {
            "ui.tabs.server"
        } else {
            "ui.tabs"
        };
        let list_style = StyleSnapshot {
            background_color: hsl_to_rgba(colors.muted),
            border_color: hsl_to_rgba(colors.muted),
            border_width: 0.0,
            border_radius: 6.0,
            ring_spread: 0.0,
            font_family: "system-ui".into(),
            font_size: 13.0,
            font_weight: 400,
            line_height: 18.2,
            color: hsl_to_rgba(colors.muted_foreground),
            padding: PaddingSnapshot::all(4.0),
            declared_background_color: None,
            declared_border_color: None,
            opacity: 1.0,
        };
        let triggers = labels
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                let active = index == 0;
                let focused = index == 2;
                let id = format!("{prefix}.{}", ["active", "inactive", "focus"][index]);
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
                        |_event, _window, _cx| {},
                    ),
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
            font_family: "system-ui".into(),
            font_size: 13.0,
            font_weight: 400,
            line_height: 18.2,
            color: hsl_to_rgba(colors.foreground),
            padding: PaddingSnapshot::all(0.0),
            declared_background_color: None,
            declared_border_color: None,
            opacity: 1.0,
        };
        let content = parity_element(
            &self.layout,
            content_id,
            ElementSizing::Fixed,
            content_style.clone(),
            |view| {
                view.child(parity_text_for(
                    &self.layout,
                    "tabs-content-text",
                    "Active overview content",
                    &content_style,
                ))
            },
        );
        let root_style = StyleSnapshot {
            background_color: hsl_to_rgba(colors.background),
            border_color: hsl_to_rgba(colors.background),
            border_width: 0.0,
            border_radius: 0.0,
            ring_spread: 0.0,
            font_family: "system-ui".into(),
            font_size: 13.0,
            font_weight: 400,
            line_height: 18.2,
            color: hsl_to_rgba(colors.foreground),
            padding: PaddingSnapshot::all(0.0),
            declared_background_color: None,
            declared_border_color: None,
            opacity: 1.0,
        };
        let root_id = if self.server {
            "ui.tabs.server"
        } else {
            "ui.tabs.default"
        };
        parity_element(
            &self.layout,
            root_id,
            ElementSizing::Fixed,
            root_style,
            |view| {
                view.size_full()
                    .p(px(32.0))
                    .bg(to_gpui(hsl_to_rgba(colors.background)))
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .flex_col()
                            .child(parity_child(
                                &self.layout,
                                list_id,
                                ElementSizing::Fixed,
                                list_style,
                                list(self.theme, div().flex().children(triggers)),
                            ))
                            .child(content),
                    )
            },
        )
    }
}
