//! Tabs presentation, with gpui-base Button supplying native activation and focus.

use super::surface::{css_color, offset_ring, shadow_sm};
use crate::tokens::theme::Theme;
use gpui_kit::base::Button;
use gpui_kit::{
    App, ClickEvent, Div, ElementId, Font, FontWeight, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, TextRun, Window, div, px,
};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TriggerState {
    pub active: bool,
    /// Forces the focus-visible state for visual fixtures.
    pub focused: bool,
    pub disabled: bool,
}

/// Geometry resolved from the primitive or ServerMonitorPanel class names.
#[derive(Clone, Copy, Debug)]
pub struct TabsStyle {
    pub list_height: f32,
    pub list_padding: f32,
    pub list_radius: f32,
    pub gap: f32,
    pub trigger_height: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub font_size: f32,
    pub line_height: f32,
    pub radius: f32,
    pub ring_spread: f32,
    pub content_margin: f32,
    pub content_padding: (f32, f32),
}

/// Production default and the overrides used by TerminalTab/ServerMonitorPanel.
pub fn style(server_monitor: bool) -> TabsStyle {
    TabsStyle {
        list_height: if server_monitor { 28.0 } else { 40.0 },
        list_padding: if server_monitor { 2.0 } else { 4.0 },
        list_radius: if server_monitor { 5.0 } else { 6.0 },
        gap: if server_monitor { 2.0 } else { 0.0 },
        trigger_height: if server_monitor { 24.0 } else { 28.0 },
        padding_x: if server_monitor { 10.0 } else { 12.0 },
        padding_y: if server_monitor { 0.0 } else { 6.0 },
        font_size: if server_monitor { 11.0 } else { 14.0 },
        line_height: if server_monitor { 13.2 } else { 16.8 },
        radius: 4.0,
        ring_spread: 4.0,
        content_margin: if server_monitor { 0.0 } else { 8.0 },
        content_padding: if server_monitor {
            (14.0, 16.0)
        } else {
            (0.0, 0.0)
        },
    }
}

/// The shrink-to-content `inline-flex` list; its owner controls placement.
pub fn list(theme: Theme, geometry: TabsStyle, children: impl IntoElement) -> Div {
    div()
        .h(px(geometry.list_height))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .self_start()
        .rounded(px(geometry.list_radius))
        .p(px(geometry.list_padding))
        .gap(px(geometry.gap))
        .bg(css_color(theme.colors.muted, 1.0))
        .text_color(css_color(theme.colors.muted_foreground, 1.0))
        .child(children)
}

type ChangeHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Trigger {
    id: ElementId,
    label: SharedString,
    state: TriggerState,
    theme: Theme,
    geometry: TabsStyle,
    on_change: ChangeHandler,
}

/// A pointer and keyboard accessible trigger. Selection belongs to its owner.
pub fn trigger(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    state: TriggerState,
    theme: Theme,
    on_change: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Trigger {
    Trigger {
        id: id.into(),
        label: label.into(),
        state,
        theme,
        geometry: style(false),
        on_change: Rc::new(on_change),
    }
}

impl Trigger {
    pub fn geometry(mut self, geometry: TabsStyle) -> Self {
        self.geometry = geometry;
        self
    }
}

impl RenderOnce for Trigger {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let colors = self.theme.colors;
        let geometry = self.geometry;
        let foreground = if self.state.active {
            colors.foreground
        } else {
            colors.muted_foreground
        };
        // CoreText's Medium trait is 0.23; font-kit's CSS mapping reports it
        // above 500. Its CSS 400..500 matching rule otherwise selects Regular.
        // Crossing that boundary selects the actual system Medium face.
        let font = Font {
            family: ".SystemUIFont".into(),
            weight: FontWeight(if cfg!(target_os = "macos") {
                501.0
            } else {
                500.0
            }),
            ..Font::default()
        };
        // GPUI's text element rounds intrinsic widths upward. Use the shaped
        // advance, as CSS inline-flex does, so sibling origins do not accumulate
        // a whole logical pixel of rounding per label.
        let line = window.text_system().shape_line(
            self.label.clone(),
            px(geometry.font_size),
            &[TextRun {
                len: self.label.len(),
                font: font.clone(),
                color: css_color(foreground, 1.0),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        );
        let on_change = self.on_change;
        let mut button = Button::new(self.id)
            .track_focus(&focus)
            .h(px(geometry.trigger_height))
            .w(line.width + px(2.0 * geometry.padding_x))
            .flex_shrink_0()
            .px(px(geometry.padding_x))
            .py(px(geometry.padding_y))
            .rounded(px(geometry.radius))
            .font_family(font.family)
            .text_size(px(geometry.font_size))
            .line_height(px(geometry.line_height))
            .font_weight(font.weight)
            .text_color(css_color(foreground, 1.0))
            .disabled(self.state.disabled)
            .on_click(move |event, window, cx| on_change(event, window, cx))
            .relative();
        if self.state.active {
            button = button
                .bg(css_color(colors.background, 1.0))
                .shadow(shadow_sm(1.0));
        }
        if self.state.focused || (focus.is_focused(window) && window.last_input_was_keyboard()) {
            // CSS draws the ring outside its background-coloured 2px offset.
            button = button.child(offset_ring(
                css_color(colors.background, 1.0),
                css_color(colors.ring, 1.0),
                geometry.radius,
                geometry.ring_spread,
            ));
        }
        button.child(self.label)
    }
}

/// The content wrapper follows `mt-2`, or the production panel overrides.
pub fn content(geometry: TabsStyle, children: impl IntoElement) -> Div {
    div()
        .w_full()
        .mt(px(geometry.content_margin))
        .py(px(geometry.content_padding.0))
        .px(px(geometry.content_padding.1))
        .child(children)
}

#[cfg(test)]
mod tests;
