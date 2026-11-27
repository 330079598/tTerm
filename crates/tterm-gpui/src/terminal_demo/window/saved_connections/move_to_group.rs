//! Moving checked saved connections into one existing group.
//!
//! The picker only holds the checked ids and the target list. The write goes
//! through the same core group service the Tauri connections page uses, so
//! both clients file profiles the same way.

use std::collections::HashSet;

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, MouseButton, Role, Stateful, Window, div, px};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{ArgumentValue, TranslationRequest};
use tterm_core::profiles::group_service::{list_profile_groups, move_profiles_to_group};
use tterm_core::profiles::groups::group_paths;
use tterm_core::profiles::types::ProfileGroup;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::tokens::gpui::hsla;

use super::SavedConnectionsUi;

pub(super) const MOVE_BUTTON_ID: &str = "saved-connections-move";
pub(super) const MOVE_PANEL_ID: &str = "saved-connections-move-panel";
pub(super) const MOVE_GROUP_PREFIX: &str = "saved-connections-move-group-";
pub(super) const MOVE_UNGROUPED_ID: &str = "saved-connections-move-ungrouped";
pub(super) const MOVE_CANCEL_ID: &str = "saved-connections-move-cancel";
pub(super) const MOVE_LOADING_ID: &str = "saved-connections-move-loading";
pub(super) const MOVE_EMPTY_ID: &str = "saved-connections-move-empty";
pub(super) const MOVE_ERROR_ID: &str = "saved-connections-move-error";
pub(super) const ROW_TOGGLE_PREFIX: &str = "saved-connections-row-select-";

/// The checked rows and, while one is open, the target group list.
#[derive(Default)]
pub(super) struct MoveSelection {
    checked: HashSet<String>,
    picker: Option<MovePicker>,
    #[cfg(test)]
    injected_groups: Option<Result<Vec<ProfileGroup>, String>>,
    #[cfg(test)]
    injected_move: Option<Result<(), String>>,
    /// The last ids and group handed to the move operation, for tests.
    #[cfg(test)]
    last_move_request: Option<(Vec<String>, Option<String>)>,
}

/// One open target list. Dropping it drops the read groups with it.
struct MovePicker {
    groups: Vec<ProfileGroup>,
    loading: bool,
    moving: bool,
    error: Option<String>,
}

impl MoveSelection {
    fn is_checked(&self, id: &str) -> bool {
        self.checked.contains(id)
    }

    /// Adds or removes one row, reporting the new membership.
    fn toggle(&mut self, id: &str) -> bool {
        if self.checked.remove(id) {
            false
        } else {
            self.checked.insert(id.to_string());
            true
        }
    }

    /// Forgets ids that a reload no longer contains, so a move can never send
    /// a stale profile id.
    pub(super) fn retain_existing(&mut self, ids: &HashSet<&str>) {
        self.checked.retain(|id| ids.contains(id.as_str()));
    }

    /// Drops the checked set and any open target list between openings.
    pub(super) fn reset(&mut self) {
        self.checked.clear();
        self.picker = None;
    }

    /// Checked ids in a stable order, which keeps the move request testable.
    pub(super) fn checked_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.checked.iter().cloned().collect();
        ids.sort();
        ids
    }

    /// Forgets every checked row, used after a destructive action succeeds.
    pub(super) fn clear(&mut self) {
        self.checked.clear();
    }
}

impl SavedConnectionsUi {
    /// Adds or removes one row from the checked set.
    pub(super) fn toggle_checked(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.opening || !self.open || !self.listing.is_visible(id) {
            return;
        }
        if self.move_selection.toggle(id) {
            self.selected_id = Some(id.to_string());
        }
        if !self.has_checked() {
            self.delete_confirm.reset();
        }
        cx.notify();
    }

    pub(super) fn has_checked(&self) -> bool {
        !self.move_selection.checked.is_empty()
    }

    pub(super) fn move_picker_is_open(&self) -> bool {
        self.move_selection.picker.is_some()
    }

    /// The footer action that opens the target list.
    pub(super) fn move_selected_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = self.text(cx, keys::SAVED_CONNECTIONS_MOVE_SELECTED);
        let disabled = self.loading
            || self.opening
            || self.editor.is_some()
            || self.import.is_some()
            || self.delete_confirm_is_open()
            || !self.has_checked();
        button(
            MOVE_BUTTON_ID,
            label.clone(),
            Variant::Outline,
            ButtonSize::Default,
            ButtonState {
                disabled,
                ..ButtonState::default()
            },
            self.theme,
        )
        .debug_selector(|| MOVE_BUTTON_ID.to_string())
        .role(Role::Button)
        .aria_label(label)
        .when(!disabled, |move_button| {
            move_button.on_click(cx.listener(|this, _, window, cx| {
                this.open_move_picker(window, cx);
            }))
        })
    }

    /// The checkbox in front of one row. It stops the click so the row's
    /// single-selection handler does not replace the checked set.
    pub(super) fn row_toggle(
        &self,
        selector: String,
        profile_id: &str,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let colors = self.theme.colors;
        let checked = self.move_selection.is_checked(profile_id);
        let debug_selector = selector.clone();
        let label = self.include_label(name, cx);
        let id = profile_id.to_string();
        div()
            .id(selector)
            .debug_selector(move || debug_selector.clone())
            .role(Role::Button)
            .aria_label(label)
            .aria_selected(checked)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(18.0))
            .h(px(18.0))
            .rounded(px(4.0))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .text_size(px(12.0))
            .cursor_pointer()
            .when(checked, |toggle| {
                toggle
                    .bg(hsla(colors.accent, 1.0))
                    .text_color(hsla(colors.foreground, 1.0))
                    .child("✓")
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.toggle_checked(&id, cx);
            }))
    }

    fn include_label(&self, name: &str, cx: &Context<Self>) -> String {
        let arguments = [("name", ArgumentValue::Text(name))];
        self.locale
            .read(cx)
            .translate(
                TranslationRequest::new(keys::SAVED_CONNECTIONS_INCLUDE_NAMED)
                    .arguments(&arguments),
            )
            .unwrap_or_else(|_| name.to_string())
    }

    /// Opens the target list and reads the groups that exist now.
    pub(super) fn open_move_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open || self.loading || self.opening || !self.has_checked() {
            return;
        }
        if self.delete_confirm_is_open() {
            return;
        }
        self.group_admin.reset();
        self.move_selection.picker = Some(MovePicker {
            groups: Vec::new(),
            loading: true,
            moving: false,
            error: None,
        });
        cx.notify();
        self.load_move_groups(window, cx);
    }

    pub(super) fn cancel_move(&mut self, cx: &mut Context<Self>) {
        if self.move_selection.picker.take().is_some() {
            cx.notify();
        }
    }

    fn load_move_groups(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let generation = self.generation;
        #[cfg(test)]
        if let Some(result) = self.move_selection.injected_groups.clone() {
            self.finish_move_groups(generation, result, cx);
            return;
        }
        let runtime = self.services.shared.runtime.clone();
        let task = runtime.spawn(async move { list_profile_groups().await });
        cx.spawn_in(window, async move |this, cx| {
            let result = match task.await {
                Ok(result) => result,
                Err(error) => Err(format!("Failed to read the profile groups: {error}")),
            };
            let _ = this.update(cx, |this, cx| {
                this.finish_move_groups(generation, result, cx);
            });
        })
        .detach();
    }

    fn finish_move_groups(
        &mut self,
        generation: u64,
        result: Result<Vec<ProfileGroup>, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.open || generation != self.generation {
            return;
        }
        let groups_failed = self.text(cx, keys::SAVED_CONNECTIONS_MOVE_GROUPS_FAILED);
        let Some(picker) = self.move_selection.picker.as_mut() else {
            return;
        };
        picker.loading = false;
        match result {
            Ok(groups) => picker.groups = groups,
            Err(error) => {
                eprintln!("tterm-gpui: failed to read the profile groups: {error}");
                picker.error = Some(groups_failed);
            }
        }
        cx.notify();
    }

    /// Files the checked rows in `group_id`, or at the top for `None`.
    pub(super) fn move_checked_to_group(
        &mut self,
        group_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.open {
            return;
        }
        if !self.has_checked() {
            return;
        }
        let Some(picker) = self.move_selection.picker.as_mut() else {
            return;
        };
        if picker.loading || picker.moving {
            return;
        }
        picker.moving = true;
        picker.error = None;
        cx.notify();
        let ids = self.move_selection.checked_ids();
        let generation = self.generation;
        #[cfg(test)]
        {
            self.move_selection.last_move_request = Some((ids.clone(), group_id.clone()));
            if let Some(result) = self.move_selection.injected_move.clone() {
                self.finish_move(generation, result, window, cx);
                return;
            }
        }
        let runtime = self.services.shared.runtime.clone();
        let target = group_id.clone();
        let task = runtime.spawn(async move { move_profiles_to_group(ids, target, None).await });
        cx.spawn_in(window, async move |this, cx| {
            let result = match task.await {
                Ok(result) => result,
                Err(error) => Err(format!("Failed to move the saved connections: {error}")),
            };
            let _ = cx.update(|window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.finish_move(generation, result, window, cx);
                });
            });
        })
        .detach();
    }

    fn finish_move(
        &mut self,
        generation: u64,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.open || generation != self.generation {
            return;
        }
        let moved = self.text(cx, keys::SAVED_CONNECTIONS_MOVED);
        let move_failed = self.text(cx, keys::SAVED_CONNECTIONS_MOVE_FAILED);
        {
            let Some(picker) = self.move_selection.picker.as_mut() else {
                return;
            };
            picker.moving = false;
            if result.is_err() {
                picker.error = Some(move_failed);
            }
        }
        match result {
            Ok(()) => {
                self.move_selection.checked.clear();
                self.move_selection.picker = None;
                self.notice = Some(moved);
                self.error = None;
                self.loading = true;
                cx.notify();
                self.load(window, cx);
            }
            Err(error) => {
                eprintln!("tterm-gpui: failed to move saved SSH connections: {error}");
                cx.notify();
            }
        }
    }

    /// The target list shown under the search controls while one is open.
    pub(super) fn move_picker_panel(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let picker = self.move_selection.picker.as_ref()?;
        let colors = self.theme.colors;
        let title = self.text(cx, keys::SAVED_CONNECTIONS_MOVE_PICKER_TITLE);
        let cancel_label = self.text(cx, keys::COMMON_CANCEL);
        let mut panel = div()
            .id(MOVE_PANEL_ID)
            .debug_selector(|| MOVE_PANEL_ID.to_string())
            .role(Role::Group)
            .aria_label(title.clone())
            .flex()
            .flex_col()
            .gap(px(6.0))
            .p(px(10.0))
            .rounded(px(6.0))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .bg(hsla(colors.muted, 0.35))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(gpui_kit::FontWeight(600.0))
                    .child(title),
            );
        if picker.loading {
            panel = panel.child(
                div()
                    .id(MOVE_LOADING_ID)
                    .debug_selector(|| MOVE_LOADING_ID.to_string())
                    .role(Role::Status)
                    .text_size(px(12.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::SAVED_CONNECTIONS_MOVE_GROUPS_LOADING)),
            );
        } else {
            if let Some(error) = &picker.error {
                panel = panel.child(
                    div()
                        .id(MOVE_ERROR_ID)
                        .debug_selector(|| MOVE_ERROR_ID.to_string())
                        .role(Role::Status)
                        .text_size(px(12.0))
                        .text_color(hsla(colors.destructive, 1.0))
                        .child(error.clone()),
                );
            }
            if picker.groups.is_empty() {
                panel = panel.child(
                    div()
                        .id(MOVE_EMPTY_ID)
                        .debug_selector(|| MOVE_EMPTY_ID.to_string())
                        .role(Role::Status)
                        .text_size(px(12.0))
                        .text_color(hsla(colors.muted_foreground, 1.0))
                        .child(self.text(cx, keys::SAVED_CONNECTIONS_MOVE_GROUPS_EMPTY)),
                );
            }
            let paths = group_paths(&picker.groups);
            let mut targets: Vec<(&ProfileGroup, String)> = picker
                .groups
                .iter()
                .map(|group| {
                    let path = paths
                        .get(&group.id)
                        .filter(|path| !path.is_empty())
                        .cloned()
                        .unwrap_or_else(|| group.name.clone());
                    (group, path)
                })
                .collect();
            targets.sort_by(|left, right| left.1.cmp(&right.1));
            let mut list = div()
                .id("saved-connections-move-groups")
                .flex()
                .flex_col()
                .gap(px(2.0))
                .max_h(px(140.0))
                .overflow_y_scroll();
            for (group, path) in targets {
                list = list.child(self.move_target_row(
                    format!("{MOVE_GROUP_PREFIX}{}", group.id),
                    path,
                    Some(group.id.clone()),
                    picker.moving,
                    cx,
                ));
            }
            let ungrouped_label = self.text(cx, keys::PROFILES_UNGROUPED);
            panel = panel.child(list).child(self.move_target_row(
                MOVE_UNGROUPED_ID.to_string(),
                ungrouped_label,
                None,
                picker.moving,
                cx,
            ));
        }
        let cancel = button(
            MOVE_CANCEL_ID,
            cancel_label.clone(),
            Variant::Ghost,
            ButtonSize::Sm,
            ButtonState::default(),
            self.theme,
        )
        .debug_selector(|| MOVE_CANCEL_ID.to_string())
        .role(Role::Button)
        .aria_label(cancel_label)
        .on_click(cx.listener(|this, _, _, cx| this.cancel_move(cx)));
        Some(panel.child(div().flex().justify_end().child(cancel)))
    }

    fn move_target_row(
        &self,
        selector: String,
        label: String,
        group_id: Option<String>,
        moving: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let colors = self.theme.colors;
        let debug_selector = selector.clone();
        let aria_label = label.clone();
        div()
            .id(selector)
            .debug_selector(move || debug_selector.clone())
            .role(Role::Button)
            .aria_label(aria_label)
            .flex()
            .items_center()
            .w_full()
            .px(px(8.0))
            .py(px(5.0))
            .rounded(px(4.0))
            .text_size(px(13.0))
            .when(!moving, |row| {
                row.cursor_pointer()
                    .hover(|style| style.bg(hsla(colors.accent, 0.65)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.move_checked_to_group(group_id.clone(), window, cx);
                    }))
            })
            .child(label)
    }

    #[cfg(test)]
    pub(super) fn set_move_groups_for_test(
        &mut self,
        result: Result<Vec<ProfileGroup>, String>,
        cx: &mut Context<Self>,
    ) {
        self.move_selection.injected_groups = Some(result);
        cx.notify();
    }

    #[cfg(test)]
    pub(super) fn set_move_result_for_test(&mut self, result: Result<(), String>) {
        self.move_selection.injected_move = Some(result);
    }

    #[cfg(test)]
    pub(super) fn checked_ids_for_test(&self) -> Vec<String> {
        self.move_selection.checked_ids()
    }

    #[cfg(test)]
    pub(super) fn last_move_request_for_test(&self) -> Option<(Vec<String>, Option<String>)> {
        self.move_selection.last_move_request.clone()
    }
}

#[cfg(test)]
mod tests;
