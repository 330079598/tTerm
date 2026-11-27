//! The saved-profile dialog and its focus-trapping overlay.

use gpui_kit::base::FocusTrapElement;
use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, MouseButton, Role, Stateful, Window, div, px};
use tterm_core::i18n::keys;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::dialog::presentation::OVERLAY_OPACITY;
use tterm_ui::components::icons::{PLUS, X, icon};
use tterm_ui::tokens::color::Hsl;
use tterm_ui::tokens::gpui::hsla;

use super::{
    CANCEL_BUTTON_ID, CLOSE_BUTTON_ID, CONFIRM_BUTTON_ID, DESCRIPTION_ID, DIALOG_HEIGHT_RATIO,
    DIALOG_ID, DIALOG_MAX_HEIGHT, DIALOG_MAX_WIDTH, DIALOG_RADIUS, DIALOG_VIEWPORT_MARGIN,
    EDIT_BUTTON_ID, ERROR_ID, FOOTER_ID, HEADER_ID, LIST_ID, LOADING_ID, NEW_BUTTON_ID, NOTICE_ID,
    SavedConnectionsUi, TITLE_ID,
};

/// The dialog width Tauri's `sm:max-w-3xl` resolves to, still leaving a margin
/// on viewports narrower than that.
fn dialog_width(viewport_width: f32) -> f32 {
    (viewport_width - DIALOG_VIEWPORT_MARGIN).clamp(0.0, DIALOG_MAX_WIDTH)
}

/// Tauri's `h-[min(720px,85vh)]`, so the list grows with the window rather than
/// keeping a fixed row count.
fn dialog_height(viewport_height: f32) -> f32 {
    (viewport_height * DIALOG_HEIGHT_RATIO).min(DIALOG_MAX_HEIGHT)
}

impl SavedConnectionsUi {
    pub(super) fn dialog(&self, window: &Window, cx: &mut Context<Self>) -> Stateful<Div> {
        if self.editor.is_some() {
            return self.editor_dialog(cx);
        }
        if self.import.is_some() {
            return self.import_dialog(cx);
        }
        let colors = self.theme.colors;
        let title = self.text(cx, keys::PROFILES_TITLE);
        let description = self.text(cx, keys::PROFILES_DESCRIPTION);
        let viewport = window.viewport_size();
        div()
            .id(DIALOG_ID)
            .debug_selector(|| DIALOG_ID.to_string())
            .role(Role::Dialog)
            .aria_label(title.clone())
            .aria_description(description.clone())
            .w(px(dialog_width(f32::from(viewport.width))))
            .h(px(dialog_height(f32::from(viewport.height))))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(DIALOG_RADIUS))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .bg(hsla(colors.card, 1.0))
            .text_color(hsla(colors.foreground, 1.0))
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.header(cx))
            .child(self.search_section(cx))
            .children(self.move_picker_panel(cx))
            .children(self.delete_confirm_panel(cx))
            .children(self.group_admin_panel(cx))
            .child(self.list_body(cx))
            .child(self.footer(cx))
    }

    /// The production ProfilesPanel header: title and description on the left,
    /// import, new and close on the right.
    fn header(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let title = self.text(cx, keys::PROFILES_TITLE);
        let description = self.text(cx, keys::PROFILES_DESCRIPTION);
        let close_label = self.text(cx, keys::COMMON_CLOSE);
        let import_button = self.import_config_button(cx);
        let new_button = self.new_connection_button(NEW_BUTTON_ID, true, cx);
        let close_button = button(
            CLOSE_BUTTON_ID,
            "",
            Variant::Ghost,
            ButtonSize::IconSm,
            ButtonState {
                disabled: self.opening,
                ..ButtonState::default()
            },
            self.theme,
        )
        .debug_selector(|| CLOSE_BUTTON_ID.to_string())
        .role(Role::Button)
        .aria_label(close_label)
        .child(icon(X, hsla(colors.foreground, 1.0), 16.0))
        .when(!self.opening, |close| {
            close.on_click(cx.listener(|this, _, window, cx| this.close(window, cx)))
        });
        div()
            .id(HEADER_ID)
            .debug_selector(|| HEADER_ID.to_string())
            .flex_none()
            .flex()
            .flex_wrap()
            .items_start()
            .justify_between()
            .gap(px(16.0))
            .px(px(20.0))
            .py(px(14.0))
            .border_b(px(1.0))
            .border_color(hsla(colors.border, 0.8))
            .child(
                div()
                    .min_w(px(180.0))
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .id(TITLE_ID)
                            .debug_selector(|| TITLE_ID.to_string())
                            .truncate()
                            .text_size(px(16.0))
                            .line_height(px(20.0))
                            .font_weight(gpui_kit::FontWeight(600.0))
                            .child(title),
                    )
                    .child(
                        div()
                            .id(DESCRIPTION_ID)
                            .debug_selector(|| DESCRIPTION_ID.to_string())
                            .truncate()
                            .text_size(px(14.0))
                            .line_height(px(20.0))
                            .text_color(hsla(colors.muted_foreground, 1.0))
                            .child(description),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_end()
                    .gap(px(8.0))
                    .child(import_button)
                    .child(new_button)
                    .child(close_button),
            )
    }

    /// The search row keeps the production panel's horizontal inset separate
    /// from the scrolling list below it.
    fn search_section(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_none()
            .px(px(20.0))
            .pt(px(16.0))
            .pb(px(12.0))
            .child(self.search_controls(cx))
    }

    fn list_body(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let mut body = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(8.0))
            .px(px(12.0))
            .pt(px(4.0))
            .pb(px(12.0))
            .id(LIST_ID)
            .debug_selector(|| LIST_ID.to_string())
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
            if let Some(notice) = &self.notice {
                body = body.child(status_line(
                    NOTICE_ID,
                    notice.clone(),
                    colors.success,
                    Role::Status,
                ));
            }
            if let Some(error) = &self.error {
                body = body.child(status_line(
                    ERROR_ID,
                    error.clone(),
                    colors.destructive,
                    Role::Status,
                ));
            }
            if self.error.is_none() || !self.profiles.is_empty() {
                body = body.child(self.listing_content(cx));
            }
        }
        body
    }

    fn footer(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let colors = theme.colors;
        let cancel = self.text(cx, keys::COMMON_CANCEL);
        let open_label = if self.opening {
            self.text(cx, keys::SAVED_CONNECTIONS_OPENING)
        } else {
            self.text(cx, keys::SAVED_CONNECTIONS_OPEN_ACTION)
        };
        let open_disabled = self.loading || self.opening || self.active_profile_id().is_none();
        let edit_button = self.edit_selected_button(cx);
        let move_button = self.move_selected_button(cx);
        let delete_button = self.delete_selected_button(cx);
        let groups_button = self.group_admin_button(cx);
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
        // The bulk and keyboard actions stay in a compact secondary row below
        // the list instead of competing with the production header.
        div()
            .id(FOOTER_ID)
            .debug_selector(|| FOOTER_ID.to_string())
            .flex_none()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .px(px(20.0))
            .py(px(4.0))
            .border_t(px(1.0))
            .border_color(hsla(colors.border, 0.8))
            .child(
                div()
                    .min_w(px(120.0))
                    .flex_1()
                    .truncate()
                    .text_size(px(11.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::SAVED_CONNECTIONS_HINT)),
            )
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_wrap()
                    .justify_end()
                    .items_center()
                    .gap(px(8.0))
                    .child(delete_button)
                    .child(move_button)
                    .child(groups_button)
                    .child(cancel_button)
                    .child(edit_button)
                    .child(open_button),
            )
    }

    /// The always-available way into the blank form. The header shows it as a
    /// primary action, and the empty list repeats it as the only next step.
    pub(super) fn new_connection_button(
        &self,
        id: &'static str,
        prominent: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let label = self.text(cx, keys::CONNECTION_NEW);
        let colors = self.theme.colors;
        let icon_color = if prominent {
            colors.primary_foreground
        } else {
            colors.foreground
        };
        button(
            id,
            "",
            if prominent {
                Variant::Default
            } else {
                Variant::Outline
            },
            ButtonSize::Sm,
            ButtonState {
                disabled: self.opening,
                ..ButtonState::default()
            },
            self.theme,
        )
        .debug_selector(move || id.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .child(icon(PLUS, hsla(icon_color, 1.0), 14.0))
        .child(label)
        .when(!self.opening, |new_button| {
            new_button.on_click(cx.listener(|this, _, window, cx| this.open_new_editor(window, cx)))
        })
    }

    /// The entry to the SSH config importer, beside the new-connection action.
    fn import_config_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = self.text(cx, keys::SSH_CONFIG_IMPORT_TITLE);
        let disabled = self.loading || self.opening;
        button(
            super::import::IMPORT_BUTTON_ID,
            label.clone(),
            Variant::Outline,
            ButtonSize::Sm,
            ButtonState {
                disabled,
                ..ButtonState::default()
            },
            self.theme,
        )
        .debug_selector(|| super::import::IMPORT_BUTTON_ID.to_string())
        .role(Role::Button)
        .aria_label(label)
        .when(!disabled, |import| {
            import.on_click(cx.listener(|this, _, window, cx| this.open_import(window, cx)))
        })
    }

    /// Edits the selected row from the footer, so the action is reachable
    /// without opening the row's own button.
    fn edit_selected_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = self.text(cx, keys::PROFILES_EDIT);
        let active_id = self.active_profile_id();
        let disabled = self.loading || self.opening || active_id.is_none();
        button(
            EDIT_BUTTON_ID,
            label.clone(),
            Variant::Outline,
            ButtonSize::Default,
            ButtonState {
                disabled,
                ..ButtonState::default()
            },
            self.theme,
        )
        .debug_selector(|| EDIT_BUTTON_ID.to_string())
        .role(Role::Button)
        .aria_label(label)
        .when(!disabled, |edit| {
            edit.on_click(cx.listener(move |this, _, window, cx| {
                if let Some(id) = active_id.clone() {
                    this.select_profile(&id, cx);
                    this.open_edit_editor(window, cx);
                }
            }))
        })
    }

    /// The selected row's edit action. It sits inside the row's selectable
    /// area, so it stops the click before the row handles it.
    pub(super) fn row_edit_button(
        &self,
        selector: String,
        profile_id: &str,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let debug_selector = selector.clone();
        let label = self.text(cx, keys::PROFILES_EDIT);
        let aria_label = self.edit_label(name, cx);
        let id = profile_id.to_string();
        button(
            selector,
            label.clone(),
            Variant::Ghost,
            ButtonSize::Sm,
            ButtonState::default(),
            self.theme,
        )
        .debug_selector(move || debug_selector.clone())
        .role(Role::Button)
        .aria_label(aria_label)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |this, _, window, cx| {
            cx.stop_propagation();
            this.open_edit_editor_for(id.clone(), window, cx);
        }))
    }

    fn edit_label(&self, name: &str, cx: &Context<Self>) -> String {
        let arguments = [(
            "name",
            tterm_core::i18n::translate::ArgumentValue::Text(name),
        )];
        self.locale
            .read(cx)
            .translate(
                tterm_core::i18n::translate::TranslationRequest::new(
                    keys::SAVED_CONNECTIONS_EDIT_NAMED,
                )
                .arguments(&arguments),
            )
            .unwrap_or_else(|_| name.to_string())
    }

    pub(super) fn select_label(&self, name: &str, cx: &Context<Self>) -> String {
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

pub(super) fn status_line(
    id: &'static str,
    message: String,
    color: Hsl,
    role: Role,
) -> Stateful<Div> {
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div();
        }
        let dialog = self.dialog(window, cx);
        let trap_id = if self.import.is_some() {
            "saved-connections-import-focus-trap"
        } else if self.editor.is_some() {
            "saved-connections-editor-focus-trap"
        } else {
            "saved-connections-focus-trap"
        };
        let trapped = dialog.focus_trap(trap_id, &self.focus_handle);
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
            .on_any_mouse_down(cx.listener(|this, _, window, cx| {
                if this.editor.is_none() && this.import.is_none() {
                    this.close(window, cx);
                }
            }))
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
