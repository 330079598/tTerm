//! The saved SSH connection editor dialog.

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, Entity, FontWeight, Role, Stateful, div, px};
use tterm_core::i18n::keys;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::input::{InputOptions, input};
use tterm_ui::tokens::color::Hsl;
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use super::super::profile::AuthKind;
use super::super::{
    EDITOR_AUTH_KEY_ID, EDITOR_AUTH_PASSWORD_ID, EDITOR_CANCEL_ID, EDITOR_DIALOG_ID,
    EDITOR_ERROR_ID, EDITOR_HOST_ID, EDITOR_KEY_PATH_ID, EDITOR_LOADING_ID, EDITOR_NAME_ID,
    EDITOR_PORT_ID, EDITOR_SAVE_ID, EDITOR_USERNAME_ID, SavedConnectionsUi,
};
use super::{EDITOR_GROUP_ID, EditorField};

const DIALOG_WIDTH: f32 = 560.0;
const DIALOG_PADDING: f32 = 24.0;
const DIALOG_RADIUS: f32 = 8.0;
const PORT_WIDTH: f32 = 104.0;

impl SavedConnectionsUi {
    /// The editor replaces the picker body inside the same modal layer.
    pub(in crate::terminal_demo::window::saved_connections) fn editor_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let Some(editor) = self.editor.as_ref() else {
            return div().id(EDITOR_DIALOG_ID);
        };
        let theme = self.theme;
        let colors = theme.colors;
        let title = if editor.id.is_some() {
            self.text(cx, keys::PROFILES_EDIT_TITLE)
        } else {
            self.text(cx, keys::CONNECTION_NEW)
        };
        let description = self.text(cx, keys::SAVED_CONNECTIONS_EDITOR_DESCRIPTION);
        let mut dialog = div()
            .id(EDITOR_DIALOG_ID)
            .debug_selector(|| EDITOR_DIALOG_ID.to_string())
            .role(Role::Dialog)
            .aria_label(title.clone())
            .aria_description(description.clone())
            .w(px(DIALOG_WIDTH))
            .max_h(px(640.0))
            .p(px(DIALOG_PADDING))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .overflow_y_scroll()
            .rounded(px(DIALOG_RADIUS))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .bg(hsla(colors.background, 1.0))
            .text_color(hsla(colors.foreground, 1.0))
            .occlude()
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation()
            })
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(FontWeight(600.0))
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(description),
            );
        if editor.loading {
            dialog = dialog.child(status_line(
                EDITOR_LOADING_ID,
                self.text(cx, keys::SAVED_CONNECTIONS_LOADING_EDITOR),
                colors.muted_foreground,
                Role::Status,
            ));
        } else if editor.load_failed {
            if let Some(error) = &editor.error {
                dialog = dialog.child(status_line(
                    EDITOR_ERROR_ID,
                    error.clone(),
                    colors.destructive,
                    Role::Status,
                ));
            }
        } else {
            if let Some(error) = &editor.error {
                dialog = dialog.child(status_line(
                    EDITOR_ERROR_ID,
                    error.clone(),
                    colors.destructive,
                    Role::Status,
                ));
            }
            dialog = dialog.child(self.editor_fields(cx, editor));
        }
        dialog.child(self.editor_footer(cx, editor))
    }

    fn editor_fields(&self, cx: &mut Context<Self>, editor: &super::Editor) -> Div {
        let theme = self.theme;
        let colors = theme.colors;
        let name_label = self.text(cx, keys::CONNECTION_TITLE);
        let group_label = self.text(cx, keys::CONNECTION_GROUP);
        let host_label = self.text(cx, keys::CONNECTION_HOST);
        let port_label = self.text(cx, keys::CONNECTION_PORT);
        let username_label = self.text(cx, keys::CONNECTION_USERNAME);
        let auth_label = self.text(cx, keys::CONNECTION_AUTH_METHOD);
        let password_label = self.text(cx, keys::CONNECTION_PASSWORD);
        let key_label = self.text(cx, keys::CONNECTION_SSH_KEY);
        let password_selected = !editor.is_key_auth();
        let auth_disabled = editor.saving;
        let password_button = button(
            EDITOR_AUTH_PASSWORD_ID,
            password_label.clone(),
            if password_selected {
                Variant::Default
            } else {
                Variant::Outline
            },
            ButtonSize::Default,
            ButtonState {
                disabled: auth_disabled,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| EDITOR_AUTH_PASSWORD_ID.to_string())
        .role(Role::Button)
        .aria_label(password_label)
        .aria_selected(password_selected)
        .when(!auth_disabled, |password_button| {
            password_button.on_click(
                cx.listener(|this, _, _, cx| this.set_editor_auth(AuthKind::Password, cx)),
            )
        });
        let key_button = button(
            EDITOR_AUTH_KEY_ID,
            key_label.clone(),
            if password_selected {
                Variant::Outline
            } else {
                Variant::Default
            },
            ButtonSize::Default,
            ButtonState {
                disabled: auth_disabled,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| EDITOR_AUTH_KEY_ID.to_string())
        .role(Role::Button)
        .aria_label(key_label)
        .aria_selected(!password_selected)
        .when(!auth_disabled, |key_button| {
            key_button
                .on_click(cx.listener(|this, _, _, cx| this.set_editor_auth(AuthKind::Key, cx)))
        });
        let mut fields = div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(labeled_input(
                EDITOR_NAME_ID,
                name_label,
                &editor.name,
                theme,
            ))
            .child(labeled_input(
                EDITOR_GROUP_ID,
                group_label,
                editor.field_input(EditorField::Group),
                theme,
            ))
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap(px(8.0))
                    .child(div().min_w_0().flex_1().child(labeled_input(
                        EDITOR_HOST_ID,
                        host_label,
                        &editor.host,
                        theme,
                    )))
                    .child(div().w(px(PORT_WIDTH)).child(labeled_input(
                        EDITOR_PORT_ID,
                        port_label,
                        &editor.port,
                        theme,
                    ))),
            )
            .child(labeled_input(
                EDITOR_USERNAME_ID,
                username_label,
                &editor.username,
                theme,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight(500.0))
                            .child(auth_label),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.0))
                            .child(password_button)
                            .child(key_button),
                    ),
            );
        if editor.is_key_auth() {
            fields = fields.child(labeled_input(
                EDITOR_KEY_PATH_ID,
                self.text(cx, keys::CONNECTION_PRIVATE_KEY_PATH),
                &editor.key_path,
                theme,
            ));
        } else {
            fields = fields.child(
                div()
                    .text_size(px(12.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::SAVED_CONNECTIONS_PASSWORD_HINT)),
            );
        }
        fields
    }

    fn editor_footer(&self, cx: &mut Context<Self>, editor: &super::Editor) -> Div {
        let theme = self.theme;
        let cancel = self.text(cx, keys::COMMON_CANCEL);
        let save = if editor.saving {
            self.text(cx, keys::SAVED_CONNECTIONS_SAVING)
        } else {
            self.text(cx, keys::CONNECTION_SAVE)
        };
        let save_disabled = editor.saving || editor.loading || editor.load_failed;
        let cancel_button = button(
            EDITOR_CANCEL_ID,
            cancel.clone(),
            Variant::Outline,
            ButtonSize::Default,
            ButtonState {
                disabled: editor.saving,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| EDITOR_CANCEL_ID.to_string())
        .role(Role::Button)
        .aria_label(cancel)
        .when(!editor.saving, |cancel_button| {
            cancel_button.on_click(cx.listener(|this, _, window, cx| {
                this.cancel_editor(window, cx);
            }))
        });
        let save_button = button(
            EDITOR_SAVE_ID,
            save.clone(),
            Variant::Default,
            ButtonSize::Default,
            ButtonState {
                disabled: save_disabled,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| EDITOR_SAVE_ID.to_string())
        .role(Role::Button)
        .aria_label(save)
        .when(!save_disabled, |save_button| {
            save_button.on_click(cx.listener(|this, _, window, cx| {
                this.submit_editor(window, cx);
            }))
        });
        div()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(8.0))
            .child(cancel_button)
            .child(save_button)
    }
}

fn labeled_input(id: &'static str, label: String, state: &Entity<InputState>, theme: Theme) -> Div {
    let aria_label = label.clone();
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight(500.0))
                .child(label),
        )
        .child(
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .aria_label(aria_label)
                .child(input(
                    state,
                    InputOptions {
                        theme: Some(theme),
                        ..InputOptions::default()
                    },
                )),
        )
}

fn status_line(id: &'static str, message: String, color: Hsl, role: Role) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .role(role)
        .aria_label(message.clone())
        .text_size(px(13.0))
        .text_color(hsla(color, 1.0))
        .child(message)
}
