//! The picker's search field, order choice and visible-selection rules.
//!
//! The search field is a native text input with its own state subscription.
//! Every way of changing what is in sight settles the selection on a visible
//! stable id, never on a row the user can no longer see.

use std::collections::HashSet;
use std::sync::Arc;

#[cfg(test)]
use gpui_kit::Focusable as _;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Entity, Window};
use tterm_core::connections::profile_groups::GroupNode;
use tterm_core::i18n::keys;

use super::super::SavedConnectionsUi;
use super::{ListingOptions, ProfileSort, UNGROUPED_KEY};

#[cfg(not(test))]
use super::sort_store;

impl SavedConnectionsUi {
    /// Creates the search field once per window and subscribes to its edits.
    pub(super) fn ensure_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(search) = &self.search {
            return search.clone();
        }
        let placeholder = self.text(cx, keys::PROFILES_SEARCH_PLACEHOLDER);
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let events = cx.subscribe_in(
            &search,
            window,
            |this, field, event, window, cx| match event {
                InputEvent::Change => {
                    this.sync_search_value(field.read(cx).value().to_string(), cx);
                }
                InputEvent::PressEnter { .. } => this.confirm_selection(window, cx),
                InputEvent::Focus | InputEvent::Blur => {}
            },
        );
        let observer = cx.observe(&search, |this, field, cx| {
            let value = field.read(cx).value().to_string();
            this.sync_search_value(value, cx);
            cx.notify();
        });
        self._search_events = Some(events);
        self._search_observer = Some(observer);
        self.search = Some(search.clone());
        search
    }

    /// Keeps one tab stop per group heading while the profile set changes.
    pub(in super::super) fn sync_group_focus(&mut self, cx: &mut Context<Self>) {
        fn collect(node: &GroupNode, keys: &mut HashSet<String>) {
            keys.insert(node.group.id.clone());
            for child in &node.children {
                collect(child, keys);
            }
        }

        let mut keys = HashSet::new();
        for root in self.listing.roots() {
            if root.total > 0 {
                collect(root, &mut keys);
            }
        }
        if !self.listing.ungrouped().is_empty() {
            keys.insert(UNGROUPED_KEY.to_string());
        }
        self.group_focus.retain(|key, _| keys.contains(key));
        for key in keys {
            self.group_focus
                .entry(key)
                .or_insert_with(|| cx.focus_handle().tab_stop(true));
        }
    }

    /// Whether the keyboard is on a group heading rather than a profile row.
    pub(in super::super) fn group_header_focused(&self, window: &Window, cx: &App) -> bool {
        let Some(focused) = window.focused(cx) else {
            return false;
        };
        self.group_focus.values().any(|handle| *handle == focused)
    }

    /// Claims Home and End before the search field's own caret bindings see
    /// them, so both keys move through the visible rows as they do in Tauri.
    pub(in super::super) fn install_list_key_interceptor(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let handle = window.window_handle();
        let this = cx.weak_entity();
        self._key_interceptor = Some(cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle() != handle {
                return;
            }
            let home = match event.keystroke.key.as_str() {
                "home" => true,
                "end" => false,
                _ => return,
            };
            let _ = this.update(cx, |this, cx| {
                if !this.open || this.editor.is_some() || this.group_admin_is_open() {
                    return;
                }
                if home {
                    this.select_index(0, cx);
                } else {
                    let last = this.listing.visible_ids().len().saturating_sub(1);
                    this.select_index(last, cx);
                }
                cx.stop_propagation();
                window.prevent_default();
            });
        }));
    }

    /// Clears the previous query, creates the field and focuses it.
    pub(in super::super) fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search = self.ensure_search_input(window, cx);
        self.listing.set_query(String::new());
        search.update(cx, |state, cx| {
            state.set_value("", window, cx);
            state.focus(window, cx);
        });
    }

    fn sync_search_value(&mut self, query: String, cx: &mut Context<Self>) {
        if !self.open || self.editor.is_some() {
            return;
        }
        if self.listing.set_query(query) {
            self.reset_selection(cx);
            cx.notify();
        }
    }

    /// Reads the saved order and recent uses without blocking the window.
    pub(in super::super) fn load_listing_options(
        &mut self,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(test)]
        if let Some(options) = self.injected_listing_options.clone() {
            self.apply_listing_options(generation, &options, cx);
            return;
        }
        #[cfg(test)]
        if self.injected.is_some() {
            return;
        }
        let services = Arc::clone(&self.services);
        let load = async move {
            services
                .shared
                .runtime
                .spawn_blocking(super::load_options)
                .await
        };
        cx.spawn_in(window, async move |this, cx| {
            let options = match load.await {
                Ok(options) => options,
                Err(error) => {
                    eprintln!("tterm-gpui: failed to read the saved connection options: {error}");
                    return;
                }
            };
            let _ = this.update(cx, |this, cx| {
                this.apply_listing_options(generation, &options, cx);
            });
        })
        .detach();
    }

    fn apply_listing_options(
        &mut self,
        generation: u64,
        options: &ListingOptions,
        cx: &mut Context<Self>,
    ) {
        if !self.open || generation != self.generation {
            return;
        }
        if !self.sort_touched {
            self.listing.set_sort(options.sort);
        }
        self.listing.set_recents(&options.recents);
        self.settle_selection(cx);
        cx.notify();
    }

    /// Switches between the tree and the recently used list.
    pub(super) fn choose_sort(&mut self, sort: ProfileSort, cx: &mut Context<Self>) {
        if !self.open || self.opening {
            return;
        }
        if self.listing.set_sort(sort) {
            self.sort_touched = true;
            self.reset_selection(cx);
            cx.notify();
            self.save_sort(sort);
        }
    }

    /// Queues the choice while the click still orders it against the last one.
    fn save_sort(&self, sort: ProfileSort) {
        #[cfg(not(test))]
        {
            let runtime = self.services.shared.runtime.clone();
            match sort_store::request_save(sort) {
                Ok(ticket) => {
                    runtime.spawn_blocking(move || {
                        if let Err(error) = ticket.wait() {
                            eprintln!("tterm-gpui: failed to save the profile order: {error}");
                        }
                    });
                }
                Err(error) => eprintln!("tterm-gpui: failed to save the profile order: {error}"),
            }
        }
        #[cfg(test)]
        let _ = sort;
    }

    /// The stable id keyboard, Open and Edit may act on.
    pub(in super::super) fn active_profile_id(&self) -> Option<String> {
        self.listing.active_id(self.selected_id.as_deref())
    }

    /// Moves the selection to one row currently in sight.
    pub(in super::super) fn select_index(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.opening {
            return;
        }
        let Some(id) = self.listing.visible_ids().get(index).cloned() else {
            return;
        };
        if self.selected_id.as_deref() == Some(id.as_str()) {
            return;
        }
        self.navigation.reveal(&id, &self.scroll_handle);
        self.selected_id = Some(id);
        cx.notify();
    }

    /// Moves by one visible row, wrapping; the first key press always moves.
    pub(in super::super) fn step_selection(&mut self, step: isize, cx: &mut Context<Self>) {
        let count = self.listing.visible_ids().len();
        if count == 0 {
            return;
        }
        let current = self
            .selected_id
            .as_deref()
            .and_then(|id| self.listing.row_index(id));
        let next = match current {
            Some(index) => (index as isize + step).rem_euclid(count as isize) as usize,
            None if step >= 0 => 0,
            None => count - 1,
        };
        self.select_index(next, cx);
    }

    /// Selects the first row in the new order, or nothing when none is left.
    pub(super) fn reset_selection(&mut self, cx: &mut Context<Self>) {
        let next = self.listing.first_visible_id().map(str::to_string);
        if self.selected_id == next {
            return;
        }
        self.selected_id = next;
        if let Some(id) = self.selected_id.as_deref() {
            self.navigation.reveal(id, &self.scroll_handle);
        }
        cx.notify();
    }

    /// Keeps a still-visible selection, otherwise takes the first row in sight.
    pub(super) fn settle_selection(&mut self, cx: &mut Context<Self>) {
        let next = self.active_profile_id();
        if self.selected_id == next {
            return;
        }
        self.selected_id = next;
        if let Some(id) = self.selected_id.as_deref() {
            self.navigation.reveal(id, &self.scroll_handle);
        }
        cx.notify();
    }

    /// Uses the saved id from a finished save when it is still in sight.
    pub(in super::super) fn settle_after_load(&mut self, requested: Option<String>) {
        self.selected_id = self.listing.active_id(requested.as_deref());
        if let Some(id) = self.selected_id.as_deref() {
            self.navigation.reveal(id, &self.scroll_handle);
        }
    }

    /// Selects a visible row from a pointer action.
    pub(in crate::terminal_demo::window) fn select_profile(
        &mut self,
        id: &str,
        cx: &mut Context<Self>,
    ) {
        if self.opening
            || !self.open
            || !self.listing.is_visible(id)
            || self.selected_id.as_deref() == Some(id)
        {
            return;
        }
        self.selected_id = Some(id.to_string());
        cx.notify();
    }

    #[cfg(test)]
    pub(super) fn focus_group_for_test(
        &self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(focus) = self.group_focus.get(key) {
            focus.focus(window, cx);
        }
    }

    #[cfg(test)]
    pub(super) fn set_listing_options_for_test(
        &mut self,
        options: ListingOptions,
        cx: &mut Context<Self>,
    ) {
        self.injected_listing_options = Some(options.clone());
        self.listing.set_sort(options.sort);
        self.listing.set_recents(&options.recents);
        self.settle_selection(cx);
        cx.notify();
    }

    #[cfg(test)]
    pub(super) fn visible_ids_for_test(&self) -> Vec<String> {
        self.listing.visible_ids().to_vec()
    }

    #[cfg(test)]
    pub(super) fn sort_for_test(&self) -> ProfileSort {
        self.listing.sort()
    }

    #[cfg(test)]
    pub(super) fn search_focused_for_test(&self, window: &Window, cx: &App) -> bool {
        self.search
            .as_ref()
            .is_some_and(|search| search.read(cx).focus_handle(cx).is_focused(window))
    }

    #[cfg(test)]
    pub(super) fn search_value_for_test(&self, cx: &App) -> String {
        self.search
            .as_ref()
            .map(|search| search.read(cx).value().to_string())
            .unwrap_or_default()
    }
}
