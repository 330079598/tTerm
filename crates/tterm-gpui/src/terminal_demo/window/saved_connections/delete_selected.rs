//! Deleting the checked saved connections.
//!
//! The confirmation is inline so the destructive action stays inside the
//! picker. The write goes through the shared profile service, which also
//! removes each profile's stored secrets and stops its tunnels.

use std::sync::Arc;

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, MouseButton, Role, Stateful, Window, div, px};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{ArgumentValue, TranslationRequest};
use tterm_core::profiles::service::delete_profiles;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::tokens::gpui::hsla;

use super::SavedConnectionsUi;

pub(super) const DELETE_BUTTON_ID: &str = "saved-connections-delete-selected";
pub(super) const DELETE_PANEL_ID: &str = "saved-connections-delete-panel";
pub(super) const DELETE_CONFIRM_ID: &str = "saved-connections-delete-confirm";
pub(super) const DELETE_CANCEL_ID: &str = "saved-connections-delete-cancel";
pub(super) const DELETE_ERROR_ID: &str = "saved-connections-delete-error";

/// The pending delete confirmation and its last result.
#[derive(Default)]
pub(super) struct DeleteConfirm {
    open: bool,
    deleting: bool,
    error: Option<String>,
    #[cfg(test)]
    injected: Option<Result<(), String>>,
    /// The ids last handed to the delete operation, for tests.
    #[cfg(test)]
    last_request: Option<Vec<String>>,
}

impl DeleteConfirm {
    pub(super) fn reset(&mut self) {
        self.open = false;
        self.deleting = false;
        self.error = None;
    }
}

impl SavedConnectionsUi {
    /// Whether the confirmation is waiting for a yes or a no.
    pub(super) fn delete_confirm_is_open(&self) -> bool {
        self.delete_confirm.open
    }

    /// The footer entry to the confirmation. It needs a checked row and no
    /// other dialog or panel to be open.
    pub(super) fn delete_selected_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = self.text(cx, keys::SAVED_CONNECTIONS_DELETE_SELECTED);
        let disabled = self.loading
            || self.opening
            || self.editor.is_some()
            || self.import.is_some()
            || self.move_picker_is_open()
            || !self.has_checked();
        button(
            DELETE_BUTTON_ID,
            label.clone(),
            Variant::Outline,
            ButtonSize::Default,
            ButtonState {
                disabled,
                ..ButtonState::default()
            },
            self.theme,
        )
        .debug_selector(|| DELETE_BUTTON_ID.to_string())
        .role(Role::Button)
        .aria_label(label)
        .when(!disabled, |delete_button| {
            delete_button.on_click(cx.listener(|this, _, _, cx| this.open_delete_confirm(cx)))
        })
    }

    /// Asks for one confirmation; the selection itself is unchanged.
    pub(super) fn open_delete_confirm(&mut self, cx: &mut Context<Self>) {
        if !self.open
            || self.loading
            || self.opening
            || !self.has_checked()
            || self.move_picker_is_open()
        {
            return;
        }
        self.group_admin.reset();
        self.delete_confirm.open = true;
        self.delete_confirm.deleting = false;
        self.delete_confirm.error = None;
        cx.notify();
    }

    pub(super) fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        if !self.delete_confirm.open || self.delete_confirm.deleting {
            return;
        }
        self.delete_confirm.reset();
        cx.notify();
    }

    /// Deletes the checked rows once, after the user confirms.
    pub(super) fn confirm_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open || !self.delete_confirm.open || self.delete_confirm.deleting {
            return;
        }
        if !self.has_checked() {
            self.delete_confirm.reset();
            cx.notify();
            return;
        }
        self.delete_confirm.deleting = true;
        self.delete_confirm.error = None;
        cx.notify();

        let generation = self.generation;
        let ids = self.move_selection.checked_ids();
        #[cfg(test)]
        {
            self.delete_confirm.last_request = Some(ids.clone());
            if let Some(result) = self.delete_confirm.injected.clone() {
                self.finish_delete(generation, result, window, cx);
                return;
            }
        }

        let runtime = self.services.shared.runtime.clone();
        let secrets = self.services.shared.secrets.clone();
        let tunnels = Arc::clone(&self.services.app.tunnels);
        let task = runtime.spawn(async move { delete_profiles(&secrets, &tunnels, ids).await });
        cx.spawn_in(window, async move |this, cx| {
            let result = match task.await {
                Ok(result) => result,
                Err(error) => Err(format!("Failed to delete the saved connections: {error}")),
            };
            let _ = cx.update(|window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.finish_delete(generation, result, window, cx);
                });
            });
        })
        .detach();
    }

    fn finish_delete(
        &mut self,
        generation: u64,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.open || generation != self.generation {
            return;
        }
        let deleted = self.text(cx, keys::SAVED_CONNECTIONS_DELETED);
        let delete_failed = self.text(cx, keys::SAVED_CONNECTIONS_DELETE_FAILED);
        self.delete_confirm.deleting = false;
        match result {
            Ok(()) => {
                self.move_selection.clear();
                self.delete_confirm.reset();
                self.notice = Some(deleted);
                self.error = None;
                self.selected_id = None;
                self.select_after_load = None;
                self.loading = true;
                cx.notify();
                self.load(window, cx);
            }
            Err(error) => {
                eprintln!("tterm-gpui: failed to delete saved SSH connections: {error}");
                self.delete_confirm.error = Some(delete_failed);
                cx.notify();
            }
        }
    }

    /// The inline confirmation shown under the search controls.
    pub(super) fn delete_confirm_panel(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        if !self.delete_confirm.open {
            return None;
        }
        let colors = self.theme.colors;
        let count = self.move_selection.checked_ids().len();
        let question = self.delete_question(count, cx);
        let mut panel = div()
            .id(DELETE_PANEL_ID)
            .debug_selector(|| DELETE_PANEL_ID.to_string())
            .role(Role::Group)
            .aria_label(question.clone())
            .flex()
            .flex_col()
            .gap(px(6.0))
            .p(px(10.0))
            .rounded(px(6.0))
            .border(px(1.0))
            .border_color(hsla(colors.destructive, 0.8))
            .bg(hsla(colors.muted, 0.35))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(gpui_kit::FontWeight(600.0))
                    .child(question),
            );
        if let Some(error) = &self.delete_confirm.error {
            panel = panel.child(
                div()
                    .id(DELETE_ERROR_ID)
                    .debug_selector(|| DELETE_ERROR_ID.to_string())
                    .role(Role::Status)
                    .text_size(px(12.0))
                    .text_color(hsla(colors.destructive, 1.0))
                    .child(error.clone()),
            );
        }
        let cancel_label = self.text(cx, keys::COMMON_CANCEL);
        let delete_label = self.text(cx, keys::SAVED_CONNECTIONS_DELETE_SELECTED);
        let busy = self.delete_confirm.deleting;
        let cancel = button(
            DELETE_CANCEL_ID,
            cancel_label.clone(),
            Variant::Ghost,
            ButtonSize::Sm,
            ButtonState {
                disabled: busy,
                ..ButtonState::default()
            },
            self.theme,
        )
        .debug_selector(|| DELETE_CANCEL_ID.to_string())
        .role(Role::Button)
        .aria_label(cancel_label)
        .when(!busy, |cancel| {
            cancel.on_click(cx.listener(|this, _, _, cx| this.cancel_delete(cx)))
        });
        let confirm = button(
            DELETE_CONFIRM_ID,
            delete_label.clone(),
            Variant::Destructive,
            ButtonSize::Sm,
            ButtonState {
                disabled: busy,
                ..ButtonState::default()
            },
            self.theme,
        )
        .debug_selector(|| DELETE_CONFIRM_ID.to_string())
        .role(Role::Button)
        .aria_label(delete_label)
        .when(!busy, |confirm| {
            confirm.on_click(cx.listener(|this, _, window, cx| {
                this.confirm_delete(window, cx);
            }))
        });
        Some(
            panel.child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(6.0))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(cancel)
                    .child(confirm),
            ),
        )
    }

    fn delete_question(&self, count: usize, cx: &Context<Self>) -> String {
        let arguments = [("count", ArgumentValue::Number(count as f64))];
        self.locale
            .read(cx)
            .translate(
                TranslationRequest::new(keys::SAVED_CONNECTIONS_DELETE_CONFIRM)
                    .arguments(&arguments),
            )
            .unwrap_or_else(|_| self.text(cx, keys::SAVED_CONNECTIONS_DELETE_SELECTED))
    }

    #[cfg(test)]
    pub(super) fn set_delete_result_for_test(&mut self, result: Result<(), String>) {
        self.delete_confirm.injected = Some(result);
    }

    #[cfg(test)]
    pub(super) fn last_delete_request_for_test(&self) -> Option<Vec<String>> {
        self.delete_confirm.last_request.clone()
    }
}
