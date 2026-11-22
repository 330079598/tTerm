//! The saved-profile dialog and its focus-trapping overlay.

use gpui_kit::base::FocusTrapElement;
use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, MouseButton, Role, Stateful, Window, div, px};
use tterm_core::i18n::keys;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::dialog::presentation::OVERLAY_OPACITY;
use tterm_ui::tokens::color::Hsl;
use tterm_ui::tokens::gpui::hsla;

use super::{
    CANCEL_BUTTON_ID, CONFIRM_BUTTON_ID, DIALOG_ID, DIALOG_PADDING, DIALOG_RADIUS, DIALOG_WIDTH,
    EMPTY_ID, ERROR_ID, LIST_ID, LIST_MAX_HEIGHT, LOADING_ID, ROW_ID_PREFIX, SavedConnectionsUi,
};

impl SavedConnectionsUi {
    pub(super) fn dialog(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let colors = theme.colors;
        let title = self.text(cx, keys::SAVED_CONNECTIONS_TITLE);
        let description = self.text(cx, keys::SAVED_CONNECTIONS_DESCRIPTION);
        div()
            .id(DIALOG_ID)
            .debug_selector(|| DIALOG_ID.to_string())
            .role(Role::Dialog)
            .aria_label(title.clone())
            .aria_description(description.clone())
            .w(px(DIALOG_WIDTH))
            .max_h(px(540.0))
            .p(px(DIALOG_PADDING))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .rounded(px(DIALOG_RADIUS))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .bg(hsla(colors.background, 1.0))
            .text_color(hsla(colors.foreground, 1.0))
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
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(description),
            )
            .child(self.list_body(cx))
            .child(self.footer(cx))
    }

    fn list_body(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .id(LIST_ID)
            .debug_selector(|| LIST_ID.to_string())
            .max_h(px(LIST_MAX_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&self.scroll_handle);
        if self.loading {
            body = body.child(status_line(
                LOADING_ID,
                self.text(cx, keys::SAVED_CONNECTIONS_LOADING),
                colors.muted_foreground,
                Role::Status,
            ));
        } else {
            if let Some(error) = &self.error {
                body = body.child(status_line(
                    ERROR_ID,
                    error.clone(),
                    colors.destructive,
                    Role::Status,
                ));
            }
            if !self.profiles.is_empty() {
                for (index, profile) in self.profiles.iter().enumerate() {
                    let selector = format!("{ROW_ID_PREFIX}{index}");
                    let selected = self.selected_id.as_deref() == Some(profile.id.as_str());
                    let select_label = self.select_label(&profile.name, cx);
                    let id = profile.id.clone();
                    body = body.child(
                        div()
                            .id(("saved-connections-row", index))
                            .debug_selector(move || selector)
                            .role(Role::Button)
                            .aria_label(select_label)
                            .flex()
                            .flex_col()
                            .gap(px(3.0))
                            .px(px(12.0))
                            .py(px(9.0))
                            .rounded(px(6.0))
                            .cursor_pointer()
                            .when(selected, |row| row.bg(hsla(colors.accent, 1.0)))
                            .when(!selected, |row| {
                                row.hover(|style| style.bg(hsla(colors.accent, 0.65)))
                            })
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(gpui_kit::FontWeight(500.0))
                                    .child(profile.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(hsla(colors.muted_foreground, 1.0))
                                    .child(profile.endpoint()),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_profile(&id, cx);
                            })),
                    );
                }
            } else if self.error.is_none() {
                body = body.child(status_line(
                    EMPTY_ID,
                    self.text(cx, keys::SAVED_CONNECTIONS_EMPTY),
                    colors.muted_foreground,
                    Role::Status,
                ));
            }
        }
        body
    }

    fn footer(&self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let colors = theme.colors;
        let cancel = self.text(cx, keys::COMMON_CANCEL);
        let open_label = if self.opening {
            self.text(cx, keys::SAVED_CONNECTIONS_OPENING)
        } else {
            self.text(cx, keys::SAVED_CONNECTIONS_OPEN_ACTION)
        };
        let open_disabled = self.loading || self.opening || self.selected_id.is_none();
        let cancel_button = button(
            CANCEL_BUTTON_ID,
            cancel.clone(),
            Variant::Outline,
            ButtonSize::Default,
            ButtonState {
                disabled: self.opening,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| CANCEL_BUTTON_ID.to_string())
        .role(Role::Button)
        .aria_label(cancel)
        .when(!self.opening, |cancel_button| {
            cancel_button.on_click(cx.listener(|this, _, window, cx| this.close(window, cx)))
        });
        let open_button = button(
            CONFIRM_BUTTON_ID,
            open_label.clone(),
            Variant::Default,
            ButtonSize::Default,
            ButtonState {
                disabled: open_disabled,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| CONFIRM_BUTTON_ID.to_string())
        .role(Role::Button)
        .aria_label(open_label)
        .when(!open_disabled, |open| {
            open.on_click(cx.listener(|this, _, window, cx| {
                this.confirm_selection(window, cx);
            }))
        });
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .truncate()
                    .text_size(px(12.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::SAVED_CONNECTIONS_HINT)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(cancel_button)
                    .child(open_button),
            )
    }

    fn select_label(&self, name: &str, cx: &Context<Self>) -> String {
        let arguments = [(
            "name",
            tterm_core::i18n::translate::ArgumentValue::Text(name),
        )];
        self.locale
            .read(cx)
            .translate(
                tterm_core::i18n::translate::TranslationRequest::new(
                    keys::SAVED_CONNECTIONS_SELECT,
                )
                .arguments(&arguments),
            )
            .unwrap_or_else(|_| name.to_string())
    }
}

fn status_line(id: &'static str, message: String, color: Hsl, role: Role) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .role(role)
        .aria_label(message.clone())
        .py(px(8.0))
        .text_size(px(13.0))
        .text_color(hsla(color, 1.0))
        .child(message)
}

impl Render for SavedConnectionsUi {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div();
        }
        let dialog = self.dialog(cx);
        let trapped = dialog.focus_trap("saved-connections-focus-trap", &self.focus_handle);
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(hsla(self.theme.colors.background, 1.0).opacity(OVERLAY_OPACITY))
            .capture_key_down(
                cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                    this.handle_key(event, window, cx);
                }),
            )
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
