//! Creating, renaming and deleting saved-connection groups.
//!
//! Every change goes through the shared profile group service, so the GPUI
//! picker and the Tauri settings page file groups the same way. The name field
//! doubles as the create and rename input: selecting a group fills it with that
//! group's name.

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Div, Entity, FontWeight, Role, Stateful, Window, div, px};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{ArgumentValue, TranslationKey, TranslationRequest};
use tterm_core::profiles::group_service::{
    create_profile_group, delete_profile_group, list_profile_groups, rename_profile_group,
};
use tterm_core::profiles::groups::group_paths;
use tterm_core::profiles::types::ProfileGroup;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::input::{InputOptions, input};
use tterm_ui::tokens::gpui::hsla;

use super::SavedConnectionsUi;
use super::view::status_line;

pub(super) const GROUP_ADMIN_BUTTON_ID: &str = "saved-connections-groups";
pub(super) const GROUP_ADMIN_PANEL_ID: &str = "saved-connections-groups-panel";
pub(super) const GROUP_ADMIN_NAME_ID: &str = "saved-connections-groups-name";
pub(super) const GROUP_ADMIN_CREATE_ID: &str = "saved-connections-groups-create";
pub(super) const GROUP_ADMIN_ROW_PREFIX: &str = "saved-connections-groups-row-";
pub(super) const GROUP_ADMIN_RENAME_ID: &str = "saved-connections-groups-rename";
pub(super) const GROUP_ADMIN_DELETE_ID: &str = "saved-connections-groups-delete";
pub(super) const GROUP_ADMIN_CANCEL_ID: &str = "saved-connections-groups-cancel";
pub(super) const GROUP_ADMIN_LOADING_ID: &str = "saved-connections-groups-loading";
pub(super) const GROUP_ADMIN_EMPTY_ID: &str = "saved-connections-groups-empty";
pub(super) const GROUP_ADMIN_ERROR_ID: &str = "saved-connections-groups-error";
pub(super) const GROUP_ADMIN_NOTICE_ID: &str = "saved-connections-groups-notice";
pub(super) const GROUP_ADMIN_HINT_ID: &str = "saved-connections-groups-hint";

/// One write the group panel can send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum GroupChange {
    Create { name: String },
    Rename { id: String, name: String },
    Delete { id: String },
}

/// The group panel's state; dropping it drops its name field.
#[derive(Default)]
pub(super) struct GroupAdmin {
    /// Outlives one panel so a late read cannot write into a reopened panel.
    instance: u64,
    panel: Option<GroupPanel>,
    #[cfg(test)]
    injected_groups: Option<Result<Vec<ProfileGroup>, String>>,
    #[cfg(test)]
    injected_change: Option<Result<Vec<ProfileGroup>, String>>,
    #[cfg(test)]
    last_change: Option<GroupChange>,
}

struct GroupPanel {
    groups: Vec<ProfileGroup>,
    selected: Option<String>,
    name: Option<Entity<InputState>>,
    loading: bool,
    busy: bool,
    confirming_delete: bool,
    error: Option<String>,
    notice: Option<String>,
}

impl GroupAdmin {
    /// Closes the panel and invalidates any request it already sent.
    pub(super) fn reset(&mut self) {
        self.instance = self.instance.wrapping_add(1);
        self.panel = None;
    }

    fn next_instance(&mut self) -> u64 {
        self.instance = self.instance.wrapping_add(1);
        self.instance
    }
}

impl SavedConnectionsUi {
    pub(super) fn group_admin_is_open(&self) -> bool {
        self.group_admin.panel.is_some()
    }

    /// The footer entry to the group panel.
    pub(super) fn group_admin_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = self.text(cx, keys::SAVED_CONNECTIONS_GROUPS);
        let disabled = self.loading
            || self.opening
            || self.editor.is_some()
            || self.import.is_some()
            || self.delete_confirm_is_open();
        button(
            GROUP_ADMIN_BUTTON_ID,
            label.clone(),
            Variant::Outline,
            ButtonSize::Default,
            ButtonState {
                disabled,
                ..ButtonState::default()
            },
            self.theme,
        )
        .debug_selector(|| GROUP_ADMIN_BUTTON_ID.to_string())
        .role(Role::Button)
        .aria_label(label)
        .when(!disabled, |groups| {
            groups.on_click(cx.listener(|this, _, window, cx| {
                this.open_group_admin(window, cx);
            }))
        })
    }

    /// Opens the panel with an empty name field and reads the groups.
    pub(super) fn open_group_admin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open
            || self.loading
            || self.opening
            || self.editor.is_some()
            || self.import.is_some()
            || self.delete_confirm_is_open()
        {
            return;
        }
        self.move_selection.reset();
        let placeholder = self.text(cx, keys::SAVED_CONNECTIONS_GROUP_NAME_PLACEHOLDER);
        let name = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        name.update(cx, |state, cx| state.focus(window, cx));
        let instance = self.group_admin.next_instance();
        self.group_admin.panel = Some(GroupPanel {
            groups: Vec::new(),
            selected: None,
            name: Some(name),
            loading: true,
            busy: false,
            confirming_delete: false,
            error: None,
            notice: None,
        });
        cx.notify();
        self.load_group_admin(instance, window, cx);
    }

    pub(super) fn close_group_admin(&mut self, cx: &mut Context<Self>) {
        if self.group_admin_is_open() {
            self.group_admin.reset();
            cx.notify();
        }
    }

    fn load_group_admin(&mut self, instance: u64, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(test)]
        if let Some(result) = self.group_admin.injected_groups.clone() {
            self.finish_group_admin_load(instance, result, cx);
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
                this.finish_group_admin_load(instance, result, cx);
            });
        })
        .detach();
    }

    fn finish_group_admin_load(
        &mut self,
        instance: u64,
        result: Result<Vec<ProfileGroup>, String>,
        cx: &mut Context<Self>,
    ) {
        if self.group_admin.instance != instance {
            return;
        }
        let load_failed = self.text(cx, keys::SAVED_CONNECTIONS_GROUP_LOAD_FAILED);
        let Some(panel) = self.group_admin.panel.as_mut() else {
            return;
        };
        panel.loading = false;
        match result {
            Ok(groups) => {
                panel.groups = groups;
                if panel
                    .selected
                    .as_deref()
                    .is_some_and(|id| !panel.groups.iter().any(|group| group.id == id))
                {
                    panel.selected = None;
                }
            }
            Err(error) => {
                eprintln!("tterm-gpui: failed to read the profile groups: {error}");
                panel.error = Some(load_failed);
            }
        }
        cx.notify();
    }

    /// Selects one group and puts its name in the field, ready to rename.
    pub(super) fn select_group_for_admin(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = self.group_admin.panel.as_mut() else {
            return;
        };
        let Some(group) = panel.groups.iter().find(|group| group.id == id) else {
            return;
        };
        let name = group.name.clone();
        let input = panel.name.clone();
        panel.selected = Some(id.to_string());
        panel.confirming_delete = false;
        panel.error = None;
        panel.notice = None;
        if let Some(input) = input {
            input.update(cx, |state, cx| state.set_value(name, window, cx));
        }
        cx.notify();
    }

    /// Creates a top-level group from the name field.
    pub(super) fn create_group_from_admin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.group_admin_name(cx);
        if name.is_empty() {
            self.group_admin_error(keys::SAVED_CONNECTIONS_GROUP_NAME_REQUIRED, cx);
            return;
        }
        self.start_group_change(GroupChange::Create { name }, window, cx);
    }

    /// Renames the selected group to the name field's value.
    pub(super) fn rename_group_from_admin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self
            .group_admin
            .panel
            .as_ref()
            .and_then(|panel| panel.selected.clone())
        else {
            return;
        };
        let name = self.group_admin_name(cx);
        if name.is_empty() {
            self.group_admin_error(keys::SAVED_CONNECTIONS_GROUP_NAME_REQUIRED, cx);
            return;
        }
        self.start_group_change(GroupChange::Rename { id, name }, window, cx);
    }

    /// Deletes the selected group after a second click confirms it.
    pub(super) fn delete_group_from_admin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(panel) = self.group_admin.panel.as_mut() else {
            return;
        };
        let Some(id) = panel.selected.clone() else {
            return;
        };
        if !panel.confirming_delete {
            panel.confirming_delete = true;
            panel.notice = None;
            cx.notify();
            return;
        }
        self.start_group_change(GroupChange::Delete { id }, window, cx);
    }

    fn group_admin_name(&self, cx: &App) -> String {
        self.group_admin
            .panel
            .as_ref()
            .and_then(|panel| panel.name.as_ref())
            .map(|name| name.read(cx).value().trim().to_string())
            .unwrap_or_default()
    }

    fn group_admin_error(&mut self, key: TranslationKey, cx: &mut Context<Self>) {
        let message = self.text(cx, key);
        if let Some(panel) = self.group_admin.panel.as_mut() {
            panel.error = Some(message);
            panel.notice = None;
            cx.notify();
        }
    }

    fn start_group_change(
        &mut self,
        change: GroupChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = self.group_admin.panel.as_mut() else {
            return;
        };
        if panel.loading || panel.busy {
            return;
        }
        panel.busy = true;
        panel.confirming_delete = false;
        panel.error = None;
        cx.notify();
        let instance = self.group_admin.instance;
        #[cfg(test)]
        {
            self.group_admin.last_change = Some(change.clone());
            if let Some(result) = self.group_admin.injected_change.clone() {
                self.finish_group_change(instance, change, result, window, cx);
                return;
            }
        }
        let runtime = self.services.shared.runtime.clone();
        let request = change.clone();
        let task = runtime.spawn(async move {
            match request {
                GroupChange::Create { name } => create_profile_group(name, None).await,
                GroupChange::Rename { id, name } => rename_profile_group(id, name).await,
                GroupChange::Delete { id } => delete_profile_group(id).await,
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = match task.await {
                Ok(result) => result,
                Err(error) => Err(format!("Failed to change the profile groups: {error}")),
            };
            let _ = cx.update(|window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.finish_group_change(instance, change, result, window, cx);
                });
            });
        })
        .detach();
    }

    fn finish_group_change(
        &mut self,
        instance: u64,
        change: GroupChange,
        result: Result<Vec<ProfileGroup>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.group_admin.instance != instance || self.group_admin.panel.is_none() {
            return;
        }
        let succeeded = result.is_ok();
        let notice = succeeded.then(|| {
            self.text(
                cx,
                match change {
                    GroupChange::Create { .. } => keys::SAVED_CONNECTIONS_GROUP_CREATED,
                    GroupChange::Rename { .. } => keys::SAVED_CONNECTIONS_GROUP_RENAMED,
                    GroupChange::Delete { .. } => keys::SAVED_CONNECTIONS_GROUP_DELETED,
                },
            )
        });
        let failure = (!succeeded).then(|| {
            self.text(
                cx,
                match change {
                    GroupChange::Create { .. } => keys::SAVED_CONNECTIONS_GROUP_CREATE_FAILED,
                    GroupChange::Rename { .. } => keys::SAVED_CONNECTIONS_GROUP_RENAME_FAILED,
                    GroupChange::Delete { .. } => keys::SAVED_CONNECTIONS_GROUP_DELETE_FAILED,
                },
            )
        });
        let Some(panel) = self.group_admin.panel.as_mut() else {
            return;
        };
        panel.busy = false;
        panel.confirming_delete = false;
        match result {
            Ok(groups) => {
                panel.groups = groups;
                panel.error = None;
                panel.notice = notice;
                match &change {
                    GroupChange::Create { .. } => {
                        panel.selected = None;
                        if let Some(name) = panel.name.clone() {
                            name.update(cx, |state, cx| state.set_value("", window, cx));
                        }
                    }
                    GroupChange::Rename { id, .. } => {
                        panel.selected = Some(id.clone());
                    }
                    GroupChange::Delete { id } => {
                        if panel.selected.as_deref() == Some(id) {
                            panel.selected = None;
                        }
                    }
                }
            }
            Err(error) => {
                eprintln!("tterm-gpui: failed to change saved SSH groups: {error}");
                panel.error = failure;
                panel.notice = None;
            }
        }
        cx.notify();
        self.loading = true;
        self.load(window, cx);
    }

    /// The panel shown under the search controls while it is open.
    pub(super) fn group_admin_panel(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let panel = self.group_admin.panel.as_ref()?;
        let colors = self.theme.colors;
        let title = self.text(cx, keys::SAVED_CONNECTIONS_GROUP_PANEL_TITLE);
        let hint = self.text(cx, keys::SAVED_CONNECTIONS_GROUP_HINT);
        let mut body = div()
            .id(GROUP_ADMIN_PANEL_ID)
            .debug_selector(|| GROUP_ADMIN_PANEL_ID.to_string())
            .role(Role::Group)
            .aria_label(title.clone())
            .flex()
            .flex_col()
            .gap(px(8.0))
            .p(px(10.0))
            .rounded(px(6.0))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .bg(hsla(colors.muted, 0.35))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight(600.0))
                    .child(title),
            )
            .child(
                div()
                    .id(GROUP_ADMIN_HINT_ID)
                    .text_size(px(11.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(hint),
            );
        if panel.loading {
            body = body.child(status_line(
                GROUP_ADMIN_LOADING_ID,
                self.text(cx, keys::SAVED_CONNECTIONS_GROUP_LOADING),
                colors.muted_foreground,
                Role::Status,
            ));
        } else {
            if let Some(error) = &panel.error {
                body = body.child(status_line(
                    GROUP_ADMIN_ERROR_ID,
                    error.clone(),
                    colors.destructive,
                    Role::Status,
                ));
            }
            if let Some(notice) = &panel.notice {
                body = body.child(status_line(
                    GROUP_ADMIN_NOTICE_ID,
                    notice.clone(),
                    colors.success,
                    Role::Status,
                ));
            }
            body = body.child(self.group_admin_rows(panel, cx));
            body = body.child(self.group_admin_name_field(panel, cx));
            body = body.child(self.group_admin_actions(panel, cx));
        }
        Some(body)
    }

    fn group_admin_rows(&self, panel: &GroupPanel, cx: &mut Context<Self>) -> Stateful<Div> {
        if panel.groups.is_empty() {
            return div()
                .id("saved-connections-groups-empty-wrap")
                .child(status_line(
                    GROUP_ADMIN_EMPTY_ID,
                    self.text(cx, keys::SAVED_CONNECTIONS_GROUP_EMPTY),
                    self.theme.colors.muted_foreground,
                    Role::Status,
                ));
        }
        let colors = self.theme.colors;
        let paths = group_paths(&panel.groups);
        let mut targets: Vec<(&ProfileGroup, String)> = panel
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
            .id("saved-connections-groups-list")
            .flex()
            .flex_col()
            .gap(px(2.0))
            .max_h(px(150.0))
            .overflow_y_scroll()
            .role(Role::ListBox)
            .aria_label(self.text(cx, keys::SAVED_CONNECTIONS_GROUPS));
        for (group, path) in targets {
            let selected = panel.selected.as_deref() == Some(group.id.as_str());
            let selector = format!("{GROUP_ADMIN_ROW_PREFIX}{}", group.id);
            let debug_selector = selector.clone();
            let id = group.id.clone();
            let aria_label = self.group_select_label(&path, cx);
            list = list.child(
                div()
                    .id(selector)
                    .debug_selector(move || debug_selector.clone())
                    .role(Role::Button)
                    .aria_label(aria_label)
                    .aria_selected(selected)
                    .w_full()
                    .px(px(8.0))
                    .py(px(5.0))
                    .rounded(px(4.0))
                    .text_size(px(13.0))
                    .cursor_pointer()
                    .hover(|style| style.bg(hsla(colors.accent, 0.65)))
                    .when(selected, |row| row.bg(hsla(colors.accent, 0.9)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_group_for_admin(&id, window, cx);
                    }))
                    .child(path),
            );
        }
        list
    }

    fn group_select_label(&self, name: &str, cx: &Context<Self>) -> String {
        let arguments = [("name", ArgumentValue::Text(name))];
        self.locale
            .read(cx)
            .translate(
                TranslationRequest::new(keys::SAVED_CONNECTIONS_GROUP_SELECT).arguments(&arguments),
            )
            .unwrap_or_else(|_| name.to_string())
    }

    fn group_admin_name_field(&self, panel: &GroupPanel, cx: &mut Context<Self>) -> Div {
        let label = self.text(cx, keys::SAVED_CONNECTIONS_GROUP_NAME);
        let aria_label = label.clone();
        let Some(name) = panel.name.as_ref() else {
            return div();
        };
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
                    .id(GROUP_ADMIN_NAME_ID)
                    .debug_selector(|| GROUP_ADMIN_NAME_ID.to_string())
                    .aria_label(aria_label)
                    .child(input(
                        name,
                        InputOptions {
                            theme: Some(self.theme),
                            ..InputOptions::default()
                        },
                    )),
            )
    }

    fn group_admin_actions(&self, panel: &GroupPanel, cx: &mut Context<Self>) -> Div {
        let disabled = panel.loading || panel.busy;
        let has_selection = panel.selected.is_some();
        let create = self.group_admin_action_button(
            GROUP_ADMIN_CREATE_ID,
            keys::SAVED_CONNECTIONS_GROUP_CREATE,
            disabled,
            cx,
            |this, window, cx| this.create_group_from_admin(window, cx),
        );
        let rename = self.group_admin_action_button(
            GROUP_ADMIN_RENAME_ID,
            keys::SAVED_CONNECTIONS_GROUP_RENAME,
            disabled || !has_selection,
            cx,
            |this, window, cx| this.rename_group_from_admin(window, cx),
        );
        let delete_key = if panel.confirming_delete {
            keys::SAVED_CONNECTIONS_GROUP_DELETE_CONFIRM
        } else {
            keys::SAVED_CONNECTIONS_GROUP_DELETE
        };
        let delete = self.group_admin_action_button(
            GROUP_ADMIN_DELETE_ID,
            delete_key,
            disabled || !has_selection,
            cx,
            |this, window, cx| this.delete_group_from_admin(window, cx),
        );
        let cancel_label = self.text(cx, keys::COMMON_CANCEL);
        let cancel = button(
            GROUP_ADMIN_CANCEL_ID,
            cancel_label.clone(),
            Variant::Ghost,
            ButtonSize::Sm,
            ButtonState::default(),
            self.theme,
        )
        .debug_selector(|| GROUP_ADMIN_CANCEL_ID.to_string())
        .role(Role::Button)
        .aria_label(cancel_label)
        .on_click(cx.listener(|this, _, _, cx| this.close_group_admin(cx)));
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.0))
            .child(create)
            .child(rename)
            .child(delete)
            .child(div().flex_1())
            .child(cancel)
    }

    fn group_admin_action_button(
        &self,
        id: &'static str,
        label_key: TranslationKey,
        disabled: bool,
        cx: &mut Context<Self>,
        on_click: fn(&mut Self, &mut Window, &mut Context<Self>),
    ) -> Stateful<Div> {
        let label = self.text(cx, label_key);
        button(
            id,
            label.clone(),
            Variant::Outline,
            ButtonSize::Sm,
            ButtonState {
                disabled,
                ..ButtonState::default()
            },
            self.theme,
        )
        .debug_selector(move || id.to_string())
        .role(Role::Button)
        .aria_label(label)
        .when(!disabled, |action| {
            action.on_click(cx.listener(move |this, _, window, cx| {
                on_click(this, window, cx);
            }))
        })
    }

    #[cfg(test)]
    pub(super) fn set_group_admin_groups_for_test(
        &mut self,
        result: Result<Vec<ProfileGroup>, String>,
        cx: &mut Context<Self>,
    ) {
        self.group_admin.injected_groups = Some(result.clone());
        let instance = self.group_admin.instance;
        if self.group_admin.panel.is_some() {
            self.finish_group_admin_load(instance, result, cx);
        }
    }

    #[cfg(test)]
    pub(super) fn set_group_change_for_test(&mut self, result: Result<Vec<ProfileGroup>, String>) {
        self.group_admin.injected_change = Some(result);
    }

    #[cfg(test)]
    pub(super) fn group_admin_selected_for_test(&self) -> Option<&str> {
        self.group_admin
            .panel
            .as_ref()
            .and_then(|panel| panel.selected.as_deref())
    }

    #[cfg(test)]
    pub(super) fn group_admin_confirming_delete_for_test(&self) -> bool {
        self.group_admin
            .panel
            .as_ref()
            .is_some_and(|panel| panel.confirming_delete)
    }

    #[cfg(test)]
    pub(super) fn group_admin_error_for_test(&self) -> Option<&str> {
        self.group_admin
            .panel
            .as_ref()
            .and_then(|panel| panel.error.as_deref())
    }

    #[cfg(test)]
    pub(super) fn last_group_change_for_test(&self) -> Option<GroupChange> {
        self.group_admin.last_change.clone()
    }

    #[cfg(test)]
    pub(super) fn group_admin_names_for_test(&self) -> Vec<String> {
        self.group_admin
            .panel
            .as_ref()
            .map(|panel| {
                panel
                    .groups
                    .iter()
                    .map(|group| group.name.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(super) fn group_admin_name_value_for_test(&self, cx: &App) -> String {
        self.group_admin_name(cx)
    }

    #[cfg(test)]
    pub(super) fn set_group_admin_name_for_test(
        &mut self,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = self
            .group_admin
            .panel
            .as_ref()
            .and_then(|panel| panel.name.clone());
        if let Some(name) = name {
            name.update(cx, |state, cx| state.set_value(value, window, cx));
        }
    }
}

#[cfg(test)]
mod tests;
