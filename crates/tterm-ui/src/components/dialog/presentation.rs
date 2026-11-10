//! Shared painting declarations and optional prepaint observation.
use super::text::DialogText;
use super::{Dialog, DialogAction, DialogKind};
use crate::components::icons::{INFO, TRIANGLE_ALERT, X, icon};
use crate::components::input::{InputOptions, input, style as input_style};
use crate::components::surface::{css_color, shadow_lg};
use crate::tokens::color::Hsl;
use crate::tokens::gpui::hsla;
use crate::tokens::theme::Theme;
use gpui_kit::base::FocusTrapElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Bounds, Context, Div, Focusable, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, Window, div, px,
};
use std::rc::Rc;

/// ui-parity.md §4.1: a denser same-colour scrim replaces element backdrop blur.
pub const OVERLAY_OPACITY: f32 = 0.9;

/// Paint declarations supplied with each observed data-slot equivalent.
#[derive(Clone, Debug)]
pub struct PartStyle {
    pub background: Hsl,
    pub background_alpha: f32,
    pub border: Hsl,
    pub border_width: f32,
    pub radius: f32,
    pub color: Hsl,
    pub font_size: f32,
    pub line_height: f32,
    pub weight: u16,
    pub padding: [f32; 4],
    pub opacity: f32,
    pub ring_spread: f32,
    pub centered: bool,
}
/// A diagnostic hook; coordinates come from the rendered child, never a fixture.
pub type PartObserver = Rc<dyn Fn(&str, Bounds<Pixels>, &PartStyle)>;
/// Actual text child bounds, observed through the same layout as painting.
pub type TextObserver = Rc<dyn Fn(&str, Bounds<Pixels>)>;

impl PartStyle {
    pub(super) fn base(theme: Theme) -> Self {
        Self {
            background: theme.colors.background,
            background_alpha: 0.0,
            border: theme.colors.border,
            border_width: 0.0,
            radius: 0.0,
            color: theme.colors.foreground,
            font_size: 13.0,
            line_height: 18.2,
            weight: 400,
            padding: [0.0; 4],
            opacity: 1.0,
            ring_spread: 0.0,
            centered: false,
        }
    }
    pub(super) fn apply(&self, view: Div) -> Div {
        view.when(self.centered, |view| view.text_center())
            .when(!self.centered, |view| view.text_left())
            .font_family(".SystemUIFont")
            .text_size(px(self.font_size))
            .line_height(px(self.line_height))
            .font_weight(FontWeight(f32::from(self.weight)))
            .text_color(hsla(self.color, 1.0))
            .bg(css_color(self.background, self.background_alpha))
            .border(px(self.border_width))
            .border_color(hsla(self.border, 1.0))
            .rounded(px(self.radius))
            .pt(px(self.padding[0]))
            .pr(px(self.padding[1]))
            .pb(px(self.padding[2]))
            .pl(px(self.padding[3]))
            .opacity(self.opacity)
    }
}

impl Dialog {
    pub(super) fn part(
        &self,
        slot: &'static str,
        style: PartStyle,
        child: impl IntoElement,
    ) -> Div {
        let observer = self.observer.clone();
        div()
            .on_children_prepainted(move |bounds, _, _| {
                if let (Some(observer), Some(bounds)) = (&observer, bounds.first()) {
                    observer(slot, *bounds, &style);
                }
            })
            .child(child)
    }
    fn text(&self, slot: &'static str, text: String) -> DialogText {
        DialogText::new(slot, text.into(), self.text_observer.clone(), false)
    }
    fn header(&self, narrow: bool) -> Div {
        let mut base = PartStyle::base(self.theme);
        base.centered = narrow;
        let mut title_style = base.clone();
        title_style.centered = narrow && self.options.kind == DialogKind::Prompt;
        title_style.font_size = 18.0;
        title_style.line_height = 18.0;
        title_style.weight = 600;
        let title = title_style
            .apply(div())
            .when(self.options.kind != DialogKind::Prompt, |view| {
                view.flex().items_center().gap(px(8.0))
            });
        let title = if self.options.destructive {
            title.child(icon(
                TRIANGLE_ALERT,
                hsla(self.theme.colors.destructive, 1.0),
                16.0,
            ))
        } else if self.options.kind == DialogKind::Info {
            title.child(icon(INFO, hsla(self.theme.colors.primary, 1.0), 16.0))
        } else {
            title
        };
        let mut header = base
            .apply(div())
            .flex()
            .flex_col()
            .gap(px(8.0))
            .when(narrow, |view| view.text_center())
            .child(self.part(
                "dialog-title",
                title_style,
                title.child(div().flex_1().min_w_0().child(DialogText::new(
                    "title",
                    self.options.title.clone().into(),
                    self.text_observer.clone(),
                    narrow,
                ))),
            ));
        if let Some(description) = &self.options.description {
            let mut style = base.clone();
            style.color = self.theme.colors.muted_foreground;
            style.font_size = 14.0;
            style.line_height = 16.8;
            header = header.child(
                self.part(
                    "dialog-description",
                    style.clone(),
                    style
                        .apply(div())
                        .w_full()
                        .whitespace_normal()
                        .child(self.text("description", description.clone())),
                ),
            );
        }
        self.part("dialog-header", base, header)
    }
    fn footer(&self, narrow: bool, window: &Window, cx: &mut Context<Self>) -> Div {
        let base = PartStyle::base(self.theme);
        let view = base
            .apply(div())
            .flex()
            .when(narrow, |view| view.flex_col_reverse())
            .when(!narrow, |view| view.justify_end())
            .gap(px(8.0))
            .when(self.options.kind != DialogKind::Info, |view| {
                view.child(self.action_button(false, window, cx))
            })
            .child(self.action_button(true, window, cx));
        self.part("dialog-footer", base, view)
    }
    fn field(&self, window: &Window, cx: &Context<Self>) -> Div {
        let base = PartStyle::base(self.theme);
        let mut field = div().flex().flex_col().gap(px(8.0));
        if let Some(label) = &self.options.label {
            let mut style = base.clone();
            style.font_size = 14.0;
            style.line_height = 14.0;
            style.weight = 500;
            field = field.child(self.part(
                "label",
                style.clone(),
                style.apply(div()).child(label.clone()),
            ));
        }
        if let Some(state) = &self.input {
            let options = InputOptions {
                theme: Some(self.theme),
                focused: state.read(cx).focus_handle(cx).is_focused(window),
                ..InputOptions::default()
            };
            let computed = input_style(options);
            let mut style = base;
            style.background = computed.background;
            style.background_alpha = computed.background_alpha;
            style.border = computed.border;
            style.border_width = 1.0;
            style.radius = 6.0;
            style.color = computed.foreground;
            style.font_size = 14.0;
            style.line_height = 16.8;
            style.padding = [4.0, 12.0, 4.0, 12.0];
            field = field.child(self.part("input", style, input(state, options)));
        }
        field
    }
    fn content(&self, width: f32, narrow: bool, window: &Window, cx: &mut Context<Self>) -> Div {
        let base = PartStyle::base(self.theme);
        let mut style = base.clone();
        style.background_alpha = 1.0;
        style.border_width = 1.0;
        style.radius = 8.0;
        style.padding = [24.0; 4];
        let content = style
            .apply(div())
            .id("dialog-content")
            .w(px(width))
            .relative()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .shadow(shadow_lg())
            .occlude()
            .capture_key_down(cx.listener(Self::handle_key))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.header(narrow));
        let content = if self.options.kind == DialogKind::Prompt {
            content.child(self.field(window, cx))
        } else {
            content
        };
        let mut close_style = base;
        close_style.radius = 2.0;
        close_style.opacity = if self.hovered[2] { 1.0 } else { 0.7 };
        let close = close_style
            .apply(div())
            .id("dialog-close")
            .size(px(16.0))
            .track_focus(&self.controls[2])
            .tab_index(0)
            .on_hover(cx.listener(|this, hovered, _, cx| {
                this.hovered[2] = *hovered;
                cx.notify();
            }))
            .on_click(cx.listener(|this, _, window, cx| this.close(None, window, cx)))
            .child(icon(X, hsla(self.theme.colors.foreground, 1.0), 16.0));
        let close = self
            .part("dialog-close", close_style, close)
            .absolute()
            .top(px(16.0))
            .right(px(16.0));
        self.part(
            "dialog-content",
            style,
            content
                .child(self.footer(narrow, window, cx))
                .child(close)
                .focus_trap("dialog-trap", &self.focus),
        )
    }
}
impl Render for Dialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div();
        }
        if !self.initialized_focus {
            self.initialized_focus = true;
            if let Some(input) = &self.input {
                input.update(cx, |state, cx| {
                    state.set_placeholder(self.options.placeholder.clone(), window, cx);
                    state.focus(window, cx);
                });
            } else if let Some(action) = self.options.default_action {
                self.controls[usize::from(action == DialogAction::Confirm)].focus(window, cx);
            } else {
                self.focus.focus(window, cx);
            }
        }
        let viewport = window.viewport_size();
        let narrow = f32::from(viewport.width) < 640.0;
        let width = if narrow {
            f32::from(viewport.width) - 32.0
        } else {
            448.0
        };
        let mut style = PartStyle::base(self.theme);
        style.background_alpha = OVERLAY_OPACITY;
        let overlay = style
            .apply(div())
            .bg(css_color(style.background, 1.0).opacity(OVERLAY_OPACITY))
            .id("dialog-overlay")
            .occlude()
            .on_any_mouse_down(cx.listener(|this, event, window, cx| {
                cx.stop_propagation();
                this.outside(event, window, cx);
            }))
            .absolute()
            .inset_0();
        let overlay = self
            .part("dialog-overlay", style, overlay)
            .absolute()
            .inset_0();
        div()
            .absolute()
            .top_0()
            .left_0()
            .w(viewport.width)
            .h(viewport.height)
            .child(overlay)
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(self.content(width, narrow, window, cx)),
            )
    }
}
