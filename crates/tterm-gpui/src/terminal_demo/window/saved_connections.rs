//! The singleton saved-SSH profile picker.
//!
//! Reading and selecting only change this overlay. A profile connects when the
//! user presses Open or Enter, and then it opens in a new tab without replacing
//! or reflowing the current pane tree.

use std::sync::Arc;

use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Entity, FocusHandle, KeyDownEvent, ScrollHandle, WeakEntity, Window};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_core::profiles::launch::SavedSshProfileSummary;
use tterm_core::services::AppServices;
use tterm_ui::tokens::theme::Theme;

use super::profile_launch;
use super::{Locale, Prototype};

mod view;
mod wiring;

pub(super) const OPEN_BUTTON_ID: &str = "saved-connections-open";
pub(super) const DIALOG_ID: &str = "saved-connections-dialog";
pub(super) const LIST_ID: &str = "saved-connections-list";
pub(super) const CONFIRM_BUTTON_ID: &str = "saved-connections-confirm";
pub(super) const CANCEL_BUTTON_ID: &str = "saved-connections-cancel";
pub(super) const ERROR_ID: &str = "saved-connections-error";
pub(super) const EMPTY_ID: &str = "saved-connections-empty";
pub(super) const LOADING_ID: &str = "saved-connections-loading";
pub(super) const ROW_ID_PREFIX: &str = "saved-connections-row-";

const DIALOG_WIDTH: f32 = 520.0;
const DIALOG_PADDING: f32 = 24.0;
const DIALOG_RADIUS: f32 = 8.0;
const LIST_MAX_HEIGHT: f32 = 300.0;

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
    restore_focus: Option<FocusHandle>,
    generation: u64,
    scroll_handle: ScrollHandle,
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
            restore_focus: None,
            generation: 0,
            scroll_handle: ScrollHandle::default(),
        })
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

    #[cfg(test)]
    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    #[cfg(test)]
    pub(super) fn is_opening(&self) -> bool {
        self.opening
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
        self.selected_id = None;
        self.error = None;
        self.generation = self.generation.wrapping_add(1);
        self.focus_handle.focus(window, cx);
        cx.notify();
        self.load(window, cx);
    }

    /// Hides the overlay without restoring focus, for another route taking over.
    pub(super) fn dismiss(&mut self, cx: &mut Context<Self>) {
        if !self.open || self.opening {
            return;
        }
        self.open = false;
        self.loading = false;
        self.opening = false;
        self.profiles.clear();
        self.selected_id = None;
        self.error = None;
        self.restore_focus = None;
        self.generation = self.generation.wrapping_add(1);
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open || self.opening {
            return;
        }
        self.open = false;
        self.loading = false;
        self.opening = false;
        self.profiles.clear();
        self.selected_id = None;
        self.error = None;
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
        match result {
            Ok(profiles) => {
                self.selected_id = profiles.first().map(|profile| profile.id.clone());
                self.profiles = profiles;
            }
            Err(error) => {
                eprintln!("tterm-gpui: failed to read saved SSH profiles: {error}");
                self.profiles.clear();
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
                self.select_index(self.profiles.len().saturating_sub(1), cx);
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

    fn step_selection(&mut self, step: isize, cx: &mut Context<Self>) {
        let count = self.profiles.len();
        if count == 0 {
            return;
        }
        let current = self
            .selected_id
            .as_deref()
            .and_then(|id| self.profiles.iter().position(|profile| profile.id == id))
            .unwrap_or(0) as isize;
        let next = (current + step).rem_euclid(count as isize) as usize;
        self.select_index(next, cx);
    }

    fn select_index(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.opening {
            return;
        }
        let Some(profile) = self.profiles.get(index) else {
            return;
        };
        if self.selected_id.as_deref() == Some(profile.id.as_str()) {
            return;
        }
        self.selected_id = Some(profile.id.clone());
        self.scroll_handle.scroll_to_item(index);
        cx.notify();
    }

    pub(super) fn select_profile(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.opening
            || !self.open
            || !self.profiles.iter().any(|profile| profile.id == id)
            || self.selected_id.as_deref() == Some(id)
        {
            return;
        }
        self.selected_id = Some(id.to_string());
        cx.notify();
    }

    /// Confirms the selected profile once and keeps the dialog open until the
    /// lookup succeeds, so a failure is visible and can be retried.
    fn confirm_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open || self.loading || self.opening {
            return;
        }
        let Some(id) = self.selected_id.clone() else {
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
    pub(super) fn set_profiles_for_test(
        &mut self,
        result: Result<Vec<SavedSshProfileSummary>, String>,
        cx: &mut Context<Self>,
    ) {
        self.injected = Some(result);
        self.profiles.clear();
        self.selected_id = None;
        self.error = None;
        self.loading = false;
        self.opening = false;
        cx.notify();
    }
}
