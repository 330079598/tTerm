//! Shared painting declarations and optional prepaint observation.
use super::{Dialog, DialogAction, DialogKind};
use crate::components::button::{ButtonSize, ButtonState, Variant, button, style as button_style};
use crate::components::icons::{TRIANGLE_ALERT, X, icon};
use crate::components::input::{InputOptions, input, style as input_style};
use crate::components::surface::{css_color, shadow_lg};
use crate::tokens::color::Hsl;
use crate::tokens::gpui::hsla;
use crate::tokens::theme::Theme;
use gpui_kit::{
    Bounds, Context, Div, Focusable, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, Window, div, px,
};
use std::rc::Rc;

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
}
/// A diagnostic hook; coordinates come from the rendered child, never a fixture.
pub type PartObserver = Rc<dyn Fn(&str, Bounds<Pixels>, &PartStyle)>;

impl PartStyle {
    fn base(theme: Theme) -> Self {
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
        }
    }
    fn apply(&self, view: Div) -> Div {
        view.font_family(".SystemUIFont")
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
    fn part(&self, slot: &'static str, style: PartStyle, child: impl IntoElement) -> Div {
        let observer = self.observer.clone();
        div()
            .on_children_prepainted(move |bounds, _, _| {
                if let (Some(observer), Some(bounds)) = (&observer, bounds.first()) {
                    observer(slot, *bounds, &style);
                }
            })
            .child(child)
    }
    fn header(&self) -> Div {
        let base = PartStyle::base(self.theme);
        let mut title_style = base.clone();
        title_style.font_size = 18.0;
        title_style.line_height = 18.0;
        title_style.weight = 600;
        let title = title_style.apply(div()).flex().items_center().gap(px(8.0));
        let title = if self.options.destructive {
            title.child(icon(
                TRIANGLE_ALERT,
                hsla(self.theme.colors.destructive, 1.0),
                16.0,
            ))
        } else {
            title
        };
        let mut header = base
            .apply(div())
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(self.part(
                "dialog-title",
                title_style,
                title.child(self.options.title.clone()),
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
                        .h(px(style.line_height.floor()))
                        .child(description.clone()),
                ),
            );
        }
        self.part("dialog-header", base, header)
    }
    fn action_button(&self, confirm: bool, window: &Window, cx: &mut Context<Self>) -> Div {
        let index = usize::from(confirm);
        let variant = if !confirm {
            Variant::Outline
        } else if self.options.destructive {
            Variant::Destructive
        } else {
            Variant::Default
        };
        let state = ButtonState {
            focused: self.controls[index].is_focused(window),
            ..ButtonState::default()
        };
        let computed = button_style(variant, ButtonSize::Default, state, self.theme);
        let mut style = PartStyle::base(self.theme);
        style.background = computed.background;
        style.background_alpha = computed.background_alpha;
        style.border = computed.border;
        style.border_width = computed.border_width;
        style.radius = 6.0;
        style.color = computed.foreground;
        style.font_size = computed.font_size;
        style.line_height = computed.line_height;
        style.weight = 500;
        style.padding = [
            computed.padding_y,
            computed.padding_x,
            computed.padding_y,
            computed.padding_x,
        ];
        let label = if confirm {
            &self.options.confirm_text
        } else {
            &self.options.cancel_text
        };
        let measure = |weight| {
            window
                .text_system()
                .shape_line(
                    label.clone().into(),
                    px(computed.font_size),
                    &[gpui_kit::TextRun {
                        len: label.len(),
                        font: gpui_kit::Font {
                            family: ".SystemUIFont".into(),
                            weight: FontWeight(weight),
                            ..Default::default()
                        },
                        color: hsla(computed.foreground, 1.0),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    None,
                )
                .width
        };
        // CoreText's family matcher exposes Regular and Semibold on macOS.
        // Interpolate the requested medium weight for intrinsic sizing rather
        // than truncating its advance to Regular's integer layout width.
        let text_width = (measure(400.0) + measure(600.0)) / 2.0;
        let view = button(
            if confirm {
                "dialog-confirm"
            } else {
                "dialog-cancel"
            },
            label.clone(),
            variant,
            ButtonSize::Default,
            state,
            self.theme,
        )
        .w(text_width + px(computed.padding_x * 2.0 + computed.border_width * 2.0))
        .track_focus(&self.controls[index])
        .on_click(cx.listener(move |this, _, window, cx| {
            this.close(
                Some(if confirm {
                    DialogAction::Confirm
                } else {
                    DialogAction::Cancel
                }),
                window,
                cx,
            )
        }));
        self.part(if confirm { "confirm" } else { "cancel" }, style, view)
    }
    fn footer(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let base = PartStyle::base(self.theme);
        let view = base
            .apply(div())
            .flex()
            .justify_end()
            .gap(px(8.0))
            .child(self.action_button(false, window, cx))
            .child(self.action_button(true, window, cx));
        self.part("dialog-footer", base, view)
    }
    fn field(&self, window: &Window, cx: &Context<Self>) -> Div {
        let base = PartStyle::base(self.theme);
        let mut field = div().flex().flex_col().gap(px(8.0));
        if let Some(label) = &self.options.label {
            field = field.child(base.apply(div()).child(label.clone()));
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
    fn content(&self, width: f32, window: &Window, cx: &mut Context<Self>) -> Div {
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
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::handle_key))
            .on_mouse_down_out(cx.listener(Self::outside))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.header());
        let content = if self.options.kind == DialogKind::Prompt {
            content.child(self.field(window, cx))
        } else {
            content
        };
        let mut close_style = base;
        close_style.radius = 2.0;
        close_style.opacity = 0.7;
        let close = close_style
            .apply(div())
            .id("dialog-close")
            .size(px(16.0))
            .track_focus(&self.controls[2])
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
            content.child(self.footer(window, cx)).child(close),
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
                input.update(cx, |state, cx| state.focus(window, cx));
            } else {
                self.focus.focus(window, cx);
            }
        }
        let viewport = window.viewport_size();
        let width = 448.0_f32.min(f32::from(viewport.width) - 32.0);
        let mut style = PartStyle::base(self.theme);
        style.background_alpha = 0.8;
        // CSS quantizes the opaque token before premultiplying it. Keep the
        // integer operation here so channels on an integer boundary do not
        // lose a level through a floating point floor.
        let rgba = gpui_kit::Rgba::from(hsla(style.background, 1.0));
        let channel = |value: f32| ((value * 255.0).round() as u32 * 204 / 255) as f32 / 204.0;
        let overlay_color = gpui_kit::Rgba {
            r: channel(rgba.r),
            g: channel(rgba.g),
            b: channel(rgba.b),
            a: 0.8,
        };
        let overlay = style
            .apply(div())
            .bg(overlay_color)
            .id("dialog-overlay")
            .absolute()
            .inset_0();
        let overlay = self
            .part("dialog-overlay", style, overlay)
            .absolute()
            .inset_0();
        div()
            .w(viewport.width)
            .h(viewport.height)
            .relative()
            .child(overlay)
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(self.content(width, window, cx)),
            )
    }
}
