//! The singleton settings route.
//!
//! Settings is a route above the workspace rather than a tab. Opening it
//! hides the visible panes but keeps every tab, split and session alive, so
//! closing it returns to the exact pane that had focus. The page only lists
//! settings the native client actually implements; unimplemented sections
//! stay out rather than appearing as dead entries.

mod search;
mod shell;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, Div, Entity, KeyDownEvent, Role, Stateful, Subscription, Window,
};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_core::settings::search::SettingsSearchResult;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::tokens::theme::Theme;

use self::search::PAGE_VALUE;
use super::Prototype;
use crate::locale::Locale;
use crate::scrollback_setting::ScrollbackSetting;

pub(super) const SETTINGS_PAGE_ID: &str = "settings-page";
pub(super) const SETTINGS_OPEN_BUTTON_ID: &str = "settings-page-open";
pub(super) const SETTINGS_HEADER_ID: &str = "settings-header";
pub(super) const SETTINGS_TITLE_ID: &str = "settings-title";
pub(super) const SETTINGS_SIDEBAR_ID: &str = "settings-sidebar";
pub(super) const SETTINGS_TAB_TERMINAL_ID: &str = "settings-tab-terminal";
pub(super) const SETTINGS_PANEL_ID: &str = "settings-panel";
pub(super) const SETTINGS_SEARCH_FIELD_ID: &str = "settings-search-field";
pub(super) const SETTINGS_SEARCH_CLEAR_ID: &str = "settings-search-clear";
pub(super) const SETTINGS_SEARCH_RESULTS_ID: &str = "settings-search-results";
pub(super) const SETTINGS_SEARCH_COUNT_ID: &str = "settings-search-count";
pub(super) const SETTINGS_SEARCH_EMPTY_ID: &str = "settings-search-empty";
pub(super) const SETTINGS_SEARCH_RESULT_0_ID: &str = "settings-search-result-0";
/// The scrollback section and its field, shared with the route tests.
#[cfg(test)]
pub(super) const SETTINGS_SCROLLBACK_SECTION_ID: &str = "settings-scrollback-lines";
#[cfg(test)]
pub(super) const SETTINGS_SCROLLBACK_INPUT_ID: &str = "settings-scrollback-input";
#[cfg(test)]
pub(super) const SETTINGS_SCROLLBACK_SAVE_ID: &str = "settings-scrollback-save";
#[cfg(test)]
pub(super) const SETTINGS_SCROLLBACK_ERROR_ID: &str = "settings-scrollback-error";

/// The settings page entity. It owns the live search and delegates the one
/// implemented setting to the same editor the title-bar shortcut used.
pub(super) struct SettingsPage {
    locale: Entity<Locale>,
    theme: Theme,
    scrollback: Entity<ScrollbackSetting>,
    search_input: Option<Entity<InputState>>,
    _search_events: Option<Subscription>,
    _search_observer: Option<Subscription>,
    query: String,
    active_index: usize,
    open: bool,
    _locale_observer: Option<Subscription>,
}

impl SettingsPage {
    pub(super) fn new(
        locale: Entity<Locale>,
        theme: Theme,
        scrollback: Entity<ScrollbackSetting>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&scrollback, |_, _, cx| cx.notify()).detach();
        Self {
            locale,
            theme,
            scrollback,
            search_input: None,
            _search_events: None,
            _search_observer: None,
            query: String::new(),
            active_index: 0,
            open: false,
            _locale_observer: None,
        }
    }

    /// Opens the route, rebuilds the search field for this window and focuses
    /// it the way the reference page does.
    pub(super) fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            // Opening the same route again is a no-op: it must not discard a
            // draft or steal focus from the field the user is editing.
            return;
        }
        self.query.clear();
        self.active_index = 0;
        self.scrollback
            .update(cx, |setting, cx| setting.open_page(window, cx));
        let field = self.ensure_search_input(window, cx);
        self.ensure_locale_observer(window, cx);
        field.update(cx, |state, cx| state.focus(window, cx));
        self.open = true;
        cx.notify();
    }

    /// Closes the route. Returns `false` while the scrollback write is in
    /// flight so the page is not discarded mid-save.
    pub(super) fn close(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.open {
            return true;
        }
        if !self
            .scrollback
            .update(cx, |setting, cx| setting.close_page(cx))
        {
            return false;
        }
        self.open = false;
        self.query.clear();
        self.active_index = 0;
        self._search_events = None;
        self._search_observer = None;
        self._locale_observer = None;
        self.search_input = None;
        cx.notify();
        true
    }

    /// Clears the search before the route handles Escape. Returns whether it
    /// changed anything, so a second Escape can close the route.
    pub(super) fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.query.is_empty() {
            return false;
        }
        self.query.clear();
        self.active_index = 0;
        if let Some(field) = &self.search_input {
            field.update(cx, |state, cx| {
                state.set_value("", window, cx);
                state.focus(window, cx);
            });
        }
        cx.notify();
        true
    }

    /// Renders the page body. The title bar stays mounted above it.
    pub(super) fn render_page(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        shell::page(self, window, cx)
    }

    fn ensure_locale_observer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self._locale_observer.is_some() {
            return;
        }
        let observer = cx.observe_in(&self.locale, window, |this, _, window, cx| {
            this.refresh_placeholders(window, cx);
            cx.notify();
        });
        self._locale_observer = Some(observer);
    }

    fn refresh_placeholders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(field) = &self.search_input {
            let placeholder = search::text(self.locale.read(cx), keys::SETTINGS_SEARCH_PLACEHOLDER);
            field.update(cx, |state, cx| {
                state.set_placeholder(placeholder, window, cx);
            });
        }
        self.scrollback.update(cx, |setting, cx| {
            setting.refresh_placeholder(window, cx);
        });
    }

    fn ensure_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(field) = &self.search_input {
            return field.clone();
        }
        let placeholder = search::text(self.locale.read(cx), keys::SETTINGS_SEARCH_PLACEHOLDER);
        let field = cx.new(|cx| InputState::new(window, cx));
        field.update(cx, |state, cx| {
            state.set_placeholder(placeholder, window, cx);
        });
        let events = cx.subscribe_in(&field, window, |this, field, event, _window, cx| {
            if matches!(event, InputEvent::Change) {
                this.query = field.read(cx).value().to_string();
                this.active_index = 0;
                cx.notify();
            }
        });
        let observer = cx.observe(&field, |_, _, cx| cx.notify());
        self._search_events = Some(events);
        self._search_observer = Some(observer);
        self.search_input = Some(field.clone());
        field
    }

    fn results(&self, cx: &App) -> Vec<SettingsSearchResult> {
        search::results(self.locale.read(cx), &self.query)
    }

    fn has_query(&self) -> bool {
        !self.query.trim().is_empty()
    }

    fn move_selection(&mut self, forward: bool, cx: &App) {
        let count = self.results(cx).len();
        if count == 0 {
            return;
        }
        self.active_index = if forward {
            (self.active_index + 1) % count
        } else {
            (self.active_index + count - 1) % count
        };
    }

    fn open_active_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(result) = self.results(cx).get(self.active_index).cloned() else {
            return;
        };
        self.open_result(result, window, cx);
    }

    /// Opens one search result: the query ends and the one implemented page
    /// is shown. `result.is_page` is accepted so the page row behaves like the
    /// setting row when it is the only page.
    fn open_result(
        &mut self,
        result: SettingsSearchResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if result.tab != PAGE_VALUE {
            return;
        }
        self.query.clear();
        self.active_index = 0;
        if let Some(field) = &self.search_input {
            field.update(cx, |state, cx| state.set_value("", window, cx));
            if result.is_page {
                field.update(cx, |state, cx| state.focus(window, cx));
            }
        }
        if !result.is_page {
            self.scrollback.update(cx, |setting, cx| {
                setting.focus_page_input(window, cx);
            });
        }
        cx.notify();
    }

    fn search_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" if self.has_query() => {
                window.prevent_default();
                cx.stop_propagation();
                self.clear_search(window, cx);
            }
            "down" => {
                window.prevent_default();
                cx.stop_propagation();
                self.move_selection(true, cx);
                cx.notify();
            }
            "up" => {
                window.prevent_default();
                cx.stop_propagation();
                self.move_selection(false, cx);
                cx.notify();
            }
            "enter" if self.has_query() => {
                window.prevent_default();
                cx.stop_propagation();
                self.open_active_result(window, cx);
            }
            "tab" => {
                // A single-line input shadows the root Tab binding, so the
                // page advances its own order here. It still has to leave the
                // field and eventually cycle back.
                window.prevent_default();
                cx.stop_propagation();
                if event.keystroke.modifiers.shift {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
            }
            _ => {}
        }
    }

    #[cfg(test)]
    pub(super) fn search_query(&self) -> &str {
        &self.query
    }

    #[cfg(test)]
    pub(super) fn search_value(&self, cx: &App) -> String {
        self.search_input
            .as_ref()
            .map(|field| field.read(cx).value().to_string())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(super) fn search_placeholder(&self, cx: &App) -> String {
        self.search_input
            .as_ref()
            .map(|field| field.read(cx).presentation().placeholder().to_string())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(super) fn search_focus(&self, cx: &App) -> Option<gpui_kit::FocusHandle> {
        self.search_input
            .as_ref()
            .map(|field| field.read(cx).presentation().focus_handle().clone())
    }

    #[cfg(test)]
    pub(super) fn scrollback_focus(&self, cx: &App) -> Option<gpui_kit::FocusHandle> {
        self.scrollback.read(cx).page_focus(cx)
    }

    #[cfg(test)]
    pub(super) fn scrollback_placeholder(&self, cx: &App) -> String {
        self.scrollback.read(cx).page_placeholder(cx)
    }

    #[cfg(test)]
    pub(super) fn search_result_count(&self, cx: &App) -> usize {
        self.results(cx).len()
    }

    #[cfg(test)]
    pub(super) fn active_index(&self) -> usize {
        self.active_index
    }

    #[cfg(test)]
    pub(super) fn scrollback_value(&self, cx: &App) -> String {
        self.scrollback.read(cx).value(cx)
    }

    #[cfg(test)]
    pub(super) fn empty_message(&self, cx: &App) -> String {
        search::empty_text(self.locale.read(cx), &self.query)
    }

    #[cfg(test)]
    pub(super) fn result_titles(&self, cx: &App) -> Vec<String> {
        self.results(cx)
            .into_iter()
            .map(|result| result.title)
            .collect()
    }
}

impl Prototype {
    /// Opens the settings route, or does nothing when it is already showing.
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open {
            self.ui
                .settings_page
                .update(cx, |page, cx| page.open(window, cx));
            return;
        }
        if self.tunnel_page_is_open() {
            self.close_tunnel_page(window, cx);
        }
        self.picker = None;
        self.ui
            .template_ui
            .update(cx, |template_ui, cx| template_ui.dismiss(cx));
        let previous = self.visible_ids();
        for id in previous {
            if let Some(tab) = self.tabs.get(id) {
                tab.leave(cx);
            }
        }
        self.ui
            .settings_page
            .update(cx, |page, cx| page.open(window, cx));
        self.settings_open = true;
        cx.notify();
    }

    /// Closes the settings route and restores the workspace's visible panes.
    ///
    /// Returns `false` while the scrollback setting is being saved so the
    /// route does not discard a pending write.
    pub(super) fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.settings_open {
            return true;
        }
        let closed = self.ui.settings_page.update(cx, |page, cx| page.close(cx));
        if !closed {
            return false;
        }
        self.settings_open = false;
        self.refresh_panes(Vec::new(), window, cx);
        cx.notify();
        true
    }

    /// Whether the settings route owns the window content.
    pub(super) fn settings_is_open(&self) -> bool {
        self.settings_open
    }

    /// The title-bar entry that opens settings.
    pub(super) fn settings_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = self.settings_text(cx, keys::SETTINGS_OPEN);
        button(
            SETTINGS_OPEN_BUTTON_ID,
            "⚙",
            Variant::Ghost,
            ButtonSize::Sm,
            ButtonState::default(),
            self.ui.theme,
        )
        .debug_selector(|| SETTINGS_OPEN_BUTTON_ID.to_string())
        .role(Role::Button)
        .aria_label(label)
        .on_click(cx.listener(|this, _, window, cx| this.toggle_settings(window, cx)))
    }

    /// The title-bar entry both opens and closes the singleton route, so a
    /// pointer user has the same way back as Escape.
    fn toggle_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open {
            self.close_settings(window, cx);
        } else {
            self.open_settings(window, cx);
        }
    }

    /// Renders the route content; the title bar stays mounted above it.
    pub(super) fn settings_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        self.ui
            .settings_page
            .update(cx, |page, cx| page.render_page(window, cx))
    }

    fn settings_text(&self, cx: &App, key: TranslationKey) -> String {
        self.ui.locale.read(cx).text(key).unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate a settings label: {error}");
            key.as_str().to_string()
        })
    }
}
