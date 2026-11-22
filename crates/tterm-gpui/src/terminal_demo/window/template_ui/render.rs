//! The modal layer and dialog body for the layout picker.

use gpui_kit::base::FocusTrapElement;
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Div, MouseButton, Role, Stateful, Window, div, px};
use tterm_core::i18n::keys;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::dialog::presentation::OVERLAY_OPACITY;
use tterm_ui::tokens::gpui::hsla;

use super::{
    CANCEL_BUTTON_ID, DIALOG_ID, DIALOG_PADDING, DIALOG_RADIUS, DIALOG_WIDTH, TemplateUi,
    feedback_line,
};

impl TemplateUi {
    pub(super) fn dialog(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let title = self.text(cx, keys::LAYOUT_TEMPLATES_TITLE);
        let description = self.text(cx, keys::LAYOUT_TEMPLATES_DESCRIPTION);
        let cancel = self.text(cx, keys::COMMON_CANCEL);
        let mut content = div()
            .id(DIALOG_ID)
            .debug_selector(|| DIALOG_ID.to_string())
            .role(Role::Dialog)
            .aria_label(title.clone())
            .aria_description(description.clone())
            .w(px(DIALOG_WIDTH))
            .max_h(px(720.0))
            .p(px(DIALOG_PADDING))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .overflow_y_scroll()
            .rounded(px(DIALOG_RADIUS))
            .border(px(1.0))
            .border_color(hsla(theme.colors.border, 1.0))
            .bg(hsla(theme.colors.background, 1.0))
            .text_color(hsla(theme.colors.foreground, 1.0))
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(gpui_kit::FontWeight(600.0))
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(hsla(theme.colors.muted_foreground, 1.0))
                    .child(description),
            )
            .child(self.builtin_section(cx))
            .child(self.saved_section(cx))
            .child(self.save_section(cx));
        if let Some(feedback) = &self.feedback {
            content = content.child(feedback_line(feedback.clone(), theme));
        }
        content.child(
            div().flex().justify_end().child(
                button(
                    CANCEL_BUTTON_ID,
                    cancel.clone(),
                    Variant::Outline,
                    ButtonSize::Default,
                    ButtonState::default(),
                    theme,
                )
                .debug_selector(|| CANCEL_BUTTON_ID.to_string())
                .role(Role::Button)
                .aria_label(cancel)
                .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
            ),
        )
    }

    pub(super) fn open_button_label(&self, cx: &App) -> String {
        self.text(cx, keys::LAYOUT_TEMPLATES_OPEN)
    }
}

impl Render for TemplateUi {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open || self.target.is_none() {
            return div();
        }
        let dialog = self.dialog(cx);
        let trapped = dialog.focus_trap("layout-template-focus-trap", &self.focus_handle);
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(hsla(self.theme.colors.background, 1.0).opacity(OVERLAY_OPACITY))
            .on_any_mouse_down(cx.listener(|this, _, window, cx| this.close(window, cx)))
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(trapped),
            )
    }
}
