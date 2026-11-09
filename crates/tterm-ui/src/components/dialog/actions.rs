//! Production action buttons, including measured intrinsic widths and hover state.
use super::presentation::PartStyle;
use super::{Dialog, DialogAction};
use crate::components::button::{ButtonSize, ButtonState, Variant, button, style as button_style};
use crate::tokens::gpui::hsla;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    Context, Div, FontWeight, InteractiveElement, StatefulInteractiveElement, Styled, Window, px,
};
impl Dialog {
    pub(super) fn action_button(
        &self,
        confirm: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let index = usize::from(confirm);
        let variant = if !confirm {
            Variant::Outline
        } else if self.options.destructive {
            Variant::Destructive
        } else {
            Variant::Default
        };
        let state = ButtonState {
            hovered: self.hovered[index],
            focused: self.controls[index].is_focused(window),
            ..ButtonState::default()
        };
        let computed = button_style(variant, ButtonSize::Default, state, self.theme);
        let mut style = PartStyle::base(self.theme);
        style.background = computed.background;
        style.background_alpha = computed.background_alpha;
        style.border = computed.border;
        style.border_width = computed.border_width;
        style.radius = computed.radius;
        style.color = computed.foreground;
        style.font_size = computed.font_size;
        style.line_height = computed.line_height;
        style.weight = computed.weight;
        style.ring_spread = computed.ring_spread;
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
        #[cfg(target_os = "macos")]
        let text_width = (measure(400.0) + measure(600.0)) / 2.0;
        #[cfg(not(target_os = "macos"))]
        let text_width = measure(f32::from(computed.weight));
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
        .when(f32::from(window.viewport_size().width) < 640.0, |view| {
            view.w_full()
        })
        .track_focus(&self.controls[index])
        .tab_index(0)
        .on_hover(cx.listener(move |this, hovered, _, cx| {
            this.hovered[index] = *hovered;
            cx.notify();
        }))
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
}
