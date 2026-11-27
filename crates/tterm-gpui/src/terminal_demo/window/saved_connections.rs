//! The singleton saved-SSH profile picker and its connection editor.
//!
//! Reading and selecting only change this overlay. A profile connects when the
//! user presses Open or Enter, and then it opens in a new tab without replacing
//! or reflowing the current pane tree. New and Edit stay inside the overlay:
//! saving returns to the refreshed list and selects the saved id instead of
//! opening a session.

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Entity, FocusHandle, KeyDownEvent, ScrollHandle, Subscription, WeakEntity, Window,
};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_core::profiles::launch::SavedSshProfileSummary;
use tterm_core::services::AppServices;
use tterm_ui::tokens::theme::Theme;

use super::profile_launch;
use super::{Locale, Prototype};

mod delete_selected;
mod editor;
mod group_admin;
mod import;
mod listing;
mod move_to_group;
mod profile;
mod view;
mod wiring;

pub(super) const OPEN_BUTTON_ID: &str = "saved-connections-open";
pub(super) const DIALOG_ID: &str = "saved-connections-dialog";
pub(super) const HEADER_ID: &str = "saved-connections-header";
pub(super) const TITLE_ID: &str = "saved-connections-title";
pub(super) const DESCRIPTION_ID: &str = "saved-connections-description";
pub(super) const CLOSE_BUTTON_ID: &str = "saved-connections-close";
pub(super) const FOOTER_ID: &str = "saved-connections-footer";
pub(super) const LIST_ID: &str = "saved-connections-list";
pub(super) const CONFIRM_BUTTON_ID: &str = "saved-connections-confirm";
pub(super) const CANCEL_BUTTON_ID: &str = "saved-connections-cancel";
pub(super) const ERROR_ID: &str = "saved-connections-error";
pub(super) const EMPTY_ID: &str = "saved-connections-empty";
pub(super) const LOADING_ID: &str = "saved-connections-loading";
pub(super) const NOTICE_ID: &str = "saved-connections-notice";
pub(super) const ROW_ID_PREFIX: &str = "saved-connections-row-";
pub(super) const NEW_BUTTON_ID: &str = "saved-connections-new";
pub(super) const EMPTY_NEW_BUTTON_ID: &str = "saved-connections-empty-new";
pub(super) const EDIT_BUTTON_ID: &str = "saved-connections-edit";
pub(super) const ROW_EDIT_PREFIX: &str = "saved-connections-row-edit-";
pub(super) const EDITOR_DIALOG_ID: &str = "saved-connections-editor-dialog";
pub(super) const EDITOR_NAME_ID: &str = "saved-connections-editor-name";
pub(super) const EDITOR_HOST_ID: &str = "saved-connections-editor-host";
pub(super) const EDITOR_PORT_ID: &str = "saved-connections-editor-port";
pub(super) const EDITOR_USERNAME_ID: &str = "saved-connections-editor-username";
pub(super) const EDITOR_AUTH_PASSWORD_ID: &str = "saved-connections-editor-auth-password";
pub(super) const EDITOR_AUTH_KEY_ID: &str = "saved-connections-editor-auth-key";
pub(super) const EDITOR_KEY_PATH_ID: &str = "saved-connections-editor-key-path";
pub(super) const EDITOR_SAVE_ID: &str = "saved-connections-editor-save";
pub(super) const EDITOR_CANCEL_ID: &str = "saved-connections-editor-cancel";
pub(super) const EDITOR_ERROR_ID: &str = "saved-connections-editor-error";
pub(super) const EDITOR_LOADING_ID: &str = "saved-connections-editor-loading";

const DIALOG_MAX_WIDTH: f32 = 768.0;
const DIALOG_MAX_HEIGHT: f32 = 720.0;
const DIALOG_VIEWPORT_MARGIN: f32 = 32.0;
const DIALOG_HEIGHT_RATIO: f32 = 0.85;
const DIALOG_RADIUS: f32 = 8.0;

/// The saved-profile picker owned by one workspace window.
pub(super) struct SavedConnectionsUi {
    workspace: WeakEntity<Prototype>,
    services: Arc<AppServices>,
    #[cfg(test)]
    injected: Option<Result<Vec<SavedSshProfileSummary>, String>>,
    locale: Entity<Locale>,
    theme: Theme,
    focus_handle: FocusHandle,
    profiles: Vec<SavedSshProfileSummary>,
    selected_id: Option<String>,
    open: bool,
    loading: bool,
    opening: bool,
    error: Option<String>,
    /// The result of the last SSH config import, shown until the picker closes.
    notice: Option<String>,
    restore_focus: Option<FocusHandle>,
    generation: u64,
    scroll_handle: ScrollHandle,
    navigation: listing::navigation::ListingNavigation,
    listing: listing::ProfileListing,
    search: Option<Entity<InputState>>,
    _search_events: Option<Subscription>,
    _search_observer: Option<Subscription>,
    _key_interceptor: Option<Subscription>,
    /// One tab stop per group heading, keyed by its canonical path.
    group_focus: HashMap<String, FocusHandle>,
    /// A sort chosen in this opening must not be overwritten by the stored one
    /// when its asynchronous read finishes.
    sort_touched: bool,
    #[cfg(test)]
    injected_listing_options: Option<listing::ListingOptions>,
    editor: Option<editor::Editor>,
    /// Invalidates an editor load or save that finishes after it was replaced.
    editor_generation: u64,
    /// The id a finished save should select once the list is read again.
    select_after_load: Option<String>,
    #[cfg(test)]
    injected_editor_load: Option<Result<profile::ProfileDraft, String>>,
    #[cfg(test)]
    injected_editor_save: Option<Result<String, String>>,
    import: Option<import::ImportDialog>,
    /// A counter that outlives each dialog, so a request from a closed dialog
    /// cannot be mistaken for one from the dialog opened after it.
    import_generation: u64,
    #[cfg(test)]
    injected_import_preview:
        Option<Result<tterm_core::profiles::types::SshConfigImportPreview, String>>,
    #[cfg(test)]
    injected_import_result:
        Option<Result<tterm_core::profiles::types::SshConfigImportResult, String>>,
    #[cfg(test)]
    import_options_for_test: Option<tterm_core::profiles::types::SshConfigImportOptions>,
    /// The rows checked for a group move, and the open target list.
    move_selection: move_to_group::MoveSelection,
    /// The group management panel and its name field.
    group_admin: group_admin::GroupAdmin,
    /// The pending confirmation for deleting the checked rows.
    delete_confirm: delete_selected::DeleteConfirm,
}

impl SavedConnectionsUi {
    pub(super) fn for_workspace(
        services: Arc<AppServices>,
        locale: Entity<Locale>,
        theme: Theme,
        cx: &mut Context<Prototype>,
    ) -> Entity<Self> {
        let workspace = cx.weak_entity();
        cx.new(|cx| Self {
            workspace,
            services,
            #[cfg(test)]
            injected: None,
            locale,
            theme,
            focus_handle: cx.focus_handle().tab_stop(true),
            profiles: Vec::new(),
            selected_id: None,
            open: false,
            loading: false,
            opening: false,
            error: None,
            notice: None,
            restore_focus: None,
            generation: 0,
            scroll_handle: ScrollHandle::default(),
            navigation: listing::navigation::ListingNavigation::default(),
            listing: listing::ProfileListing::default(),
            search: None,
            _search_events: None,
            _search_observer: None,
            _key_interceptor: None,
            group_focus: HashMap::new(),
            sort_touched: false,
            #[cfg(test)]
            injected_listing_options: None,
            editor: None,
            editor_generation: 0,
            select_after_load: None,
            #[cfg(test)]
            injected_editor_load: None,
            #[cfg(test)]
            injected_editor_save: None,
            import: None,
            import_generation: 0,
            #[cfg(test)]
            injected_import_preview: None,
            #[cfg(test)]
            injected_import_result: None,
            #[cfg(test)]
            import_options_for_test: None,
            move_selection: move_to_group::MoveSelection::default(),
            group_admin: group_admin::GroupAdmin::default(),
            delete_confirm: delete_selected::DeleteConfirm::default(),
        })
    }

    /// Replaces the picker palette without closing it or discarding its draft.
    pub(super) fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        cx.notify();
    }

    #[cfg(test)]
    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    #[cfg(test)]
    pub(super) fn selected_profile_id(&self) -> Option<&str> {
        self.selected_id.as_deref()
    }

    #[cfg(test)]
    pub(super) fn profiles(&self) -> &[SavedSshProfileSummary] {
        &self.profiles
    }

    /// The picker's visibility and row count for the visual-acceptance capture.
    #[cfg(all(feature = "automation", debug_assertions))]
    pub(super) fn capture_state(&self, _cx: &App) -> (bool, bool, usize) {
        (self.open, self.loading, self.profiles.len())
    }

    #[cfg(test)]
    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    #[cfg(test)]
    pub(super) fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    #[cfg(test)]
    pub(super) fn is_opening(&self) -> bool {
        self.opening
    }

    #[cfg(test)]
    pub(super) fn list_focus_for_test(&self) -> &FocusHandle {
        &self.focus_handle
    }

    pub(super) fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.restore_focus = window.focused(cx);
        self.open = true;
        self.loading = true;
        self.opening = false;
        self.profiles.clear();
        self.listing.clear_profiles();
        self.listing.set_query(String::new());
        self.navigation.clear_rows();
        self.selected_id = None;
        self.error = None;
        self.notice = None;
        self.editor = None;
        self.editor_generation = self.editor_generation.wrapping_add(1);
        self.import = None;
        self.import_generation = self.import_generation.wrapping_add(1);
        self.move_selection.reset();
        self.group_admin.reset();
        self.delete_confirm.reset();
        self.select_after_load = None;
        self.sort_touched = false;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.open_search(window, cx);
        self.install_list_key_interceptor(window, cx);
        cx.notify();
        self.load(window, cx);
        self.load_listing_options(generation, window, cx);
    }

    /// Hides the overlay without restoring focus, for another route taking over.
    pub(super) fn dismiss(&mut self, cx: &mut Context<Self>) {
        if !self.open || self.opening {
            return;
        }
        self.open = false;
        self.loading = false;
        self.opening = false;
        self._key_interceptor = None;
        self.group_focus.clear();
        self.profiles.clear();
        self.listing.clear_profiles();
        self.listing.set_query(String::new());
        self.selected_id = None;
        self.error = None;
        self.notice = None;
        self.restore_focus = None;
        self.editor = None;
        self.editor_generation = self.editor_generation.wrapping_add(1);
        self.import = None;
        self.import_generation = self.import_generation.wrapping_add(1);
        self.move_selection.reset();
        self.group_admin.reset();
        self.delete_confirm.reset();
        self.select_after_load = None;
        self.generation = self.generation.wrapping_add(1);
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open || self.opening || self.editor_saving() {
            return;
        }
        self.open = false;
        self.loading = false;
        self.opening = false;
        self._key_interceptor = None;
        self.group_focus.clear();
        self.profiles.clear();
        self.listing.clear_profiles();
        self.listing.set_query(String::new());
        self.selected_id = None;
        self.error = None;
        self.notice = None;
        self.editor = None;
        self.editor_generation = self.editor_generation.wrapping_add(1);
        self.import = None;
        self.import_generation = self.import_generation.wrapping_add(1);
        self.move_selection.reset();
        self.group_admin.reset();
        self.delete_confirm.reset();
        self.select_after_load = None;
        self.generation = self.generation.wrapping_add(1);
        let active_focus = self.workspace.upgrade().map(|workspace| {
            workspace
                .read(cx)
                .tabs
                .active()
                .view
                .read(cx)
                .focus_handle()
        });
        let previous_focus = self.restore_focus.take();
        if let Some(focus) = active_focus.or(previous_focus) {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let generation = self.generation;
        #[cfg(test)]
        if let Some(result) = self.injected.clone() {
            self.finish_load(generation, result, cx);
            return;
        }
        let services = Arc::clone(&self.services);
        let load = async move { profile_launch::load_saved_ssh_profiles(services.as_ref()).await };
        cx.spawn_in(window, async move |this, cx| {
            let result = load.await;
            let _ = this.update(cx, |this, cx| {
                this.finish_load(generation, result, cx);
            });
        })
        .detach();
    }

    fn finish_load(
        &mut self,
        generation: u64,
        result: Result<Vec<SavedSshProfileSummary>, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.open || generation != self.generation {
            return;
        }
        self.loading = false;
        let requested = self.select_after_load.take();
        match result {
            Ok(profiles) => {
                self.profiles = profiles;
                self.listing.replace_profiles(&self.profiles);
                let existing: std::collections::HashSet<&str> = self
                    .profiles
                    .iter()
                    .map(|profile| profile.id.as_str())
                    .collect();
                self.move_selection.retain_existing(&existing);
                if !self.has_checked() {
                    self.delete_confirm.reset();
                }
                self.settle_after_load(requested);
                self.sync_group_focus(cx);
            }
            Err(error) => {
                eprintln!("tterm-gpui: failed to read saved SSH profiles: {error}");
                self.profiles.clear();
                self.listing.clear_profiles();
                self.group_focus.clear();
                self.selected_id = None;
                self.error = Some(self.text(cx, keys::SAVED_CONNECTIONS_LOAD_FAILED));
            }
        }
        cx.notify();
    }

    pub(super) fn handle_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.open {
            return;
        }
        if self.import.is_some() {
            if event.keystroke.key == "escape" {
                self.cancel_import(window, cx);
                window.prevent_default();
                cx.stop_propagation();
            }
            return;
        }
        if self.move_picker_is_open() {
            if event.keystroke.key == "escape" {
                self.cancel_move(cx);
                window.prevent_default();
                cx.stop_propagation();
            }
            return;
        }
        if self.group_admin_is_open() {
            if event.keystroke.key == "escape" {
                self.close_group_admin(cx);
                window.prevent_default();
                cx.stop_propagation();
            }
            return;
        }
        if self.delete_confirm_is_open() {
            if event.keystroke.key == "escape" {
                self.cancel_delete(cx);
                window.prevent_default();
                cx.stop_propagation();
            }
            return;
        }
        if self.editor.is_some() {
            let handled = match event.keystroke.key.as_str() {
                "escape" => {
                    self.cancel_editor(window, cx);
                    true
                }
                "enter" => {
                    self.submit_editor(window, cx);
                    true
                }
                // Up, Down, Home and End belong to the focused text field
                // while the editor is open, so the list must not act on them.
                _ => false,
            };
            if handled {
                window.prevent_default();
                cx.stop_propagation();
            }
            return;
        }
        if event.keystroke.key == "enter"
            && (self.group_header_focused(window, cx)
                || self.navigation.sort_control_focused(window))
        {
            return;
        }
        let handled = match event.keystroke.key.as_str() {
            "escape" => {
                self.close(window, cx);
                true
            }
            "up" => {
                self.step_selection(-1, cx);
                true
            }
            "down" => {
                self.step_selection(1, cx);
                true
            }
            "home" => {
                self.select_index(0, cx);
                true
            }
            "end" => {
                self.select_index(self.listing.visible_ids().len().saturating_sub(1), cx);
                true
            }
            "tab" => {
                listing::navigation::step_focus(window, cx, !event.keystroke.modifiers.shift);
                true
            }
            "enter" => {
                self.confirm_selection(window, cx);
                true
            }
            _ => false,
        };
        if handled {
            window.prevent_default();
            cx.stop_propagation();
        }
    }

    /// Confirms the selected profile once and keeps the dialog open until the
    /// lookup succeeds, so a failure is visible and can be retried.
    fn confirm_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open
            || self.loading
            || self.opening
            || self.import.is_some()
            || self.move_picker_is_open()
            || self.group_admin_is_open()
            || self.delete_confirm_is_open()
        {
            return;
        }
        let Some(id) = self.active_profile_id() else {
            return;
        };
        let Some(workspace) = self.workspace.upgrade() else {
            self.error = Some(self.text(cx, keys::SAVED_CONNECTIONS_OPEN_FAILED));
            cx.notify();
            return;
        };
        let generation = self.generation;
        let task = workspace.update(cx, |workspace, cx| {
            workspace.open_saved_profile(id, window, cx)
        });
        self.opening = true;
        self.error = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if !this.open || this.generation != generation {
                    return;
                }
                this.opening = false;
                if let Err(error) = result {
                    eprintln!("tterm-gpui: failed to open the saved SSH profile: {error}");
                    this.error = Some(this.text(cx, keys::SAVED_CONNECTIONS_OPEN_FAILED));
                    cx.notify();
                } else {
                    this.dismiss(cx);
                }
            });
        })
        .detach();
    }

    pub(super) fn text(&self, cx: &App, key: TranslationKey) -> String {
        self.locale.read(cx).text(key).unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate a saved-connections label: {error}");
            key.as_str().to_string()
        })
    }

    #[cfg(test)]
    pub(super) fn set_next_load_for_test(
        &mut self,
        result: Result<Vec<SavedSshProfileSummary>, String>,
    ) {
        self.injected = Some(result);
    }

    #[cfg(test)]
    pub(super) fn set_profiles_for_test(
        &mut self,
        result: Result<Vec<SavedSshProfileSummary>, String>,
        cx: &mut Context<Self>,
    ) {
        self.injected = Some(result);
        self.profiles.clear();
        self.listing.clear_profiles();
        self.selected_id = None;
        self.error = None;
        self.loading = false;
        self.opening = false;
        cx.notify();
    }
}
