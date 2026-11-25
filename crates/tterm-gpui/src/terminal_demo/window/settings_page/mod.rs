//! The singleton settings route.
//!
//! Settings is a route above the workspace rather than a tab. Opening it
//! hides the visible panes but keeps every tab, split and session alive, so
//! closing it returns to the exact pane that had focus. The page only lists
//! settings the native client actually implements; unimplemented sections
//! stay out rather than appearing as dead entries.

mod search;
mod shell;
#[cfg(test)]
mod test_access;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, Div, Entity, KeyDownEvent, Role, Stateful, Subscription, Window,
};
use tterm_core::config::terminal_font_file::TerminalPageSettings;
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_core::settings::search::SettingsSearchResult;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::tokens::theme::Theme;

use self::search::{LOCAL_SHELLS_PAGE_VALUE, TERMINAL_PAGE_VALUE};
use super::Prototype;
use crate::appearance_setting::{AppearancePageSubmit, AppearanceSetting};
use crate::clipboard_setting::ClipboardSetting;
use crate::command_marks_setting::CommandMarksSetting;
use crate::font_setting::{FontPageSubmit, FontSetting};
use crate::local_shells_setting::LocalShellsSetting;
use crate::locale::Locale;
use crate::scrollback_setting::{ScrollbackPageSubmit, ScrollbackSetting};

pub(super) const SETTINGS_PAGE_ID: &str = "settings-page";
pub(super) const SETTINGS_OPEN_BUTTON_ID: &str = "settings-page-open";
pub(super) const SETTINGS_HEADER_ID: &str = "settings-header";
pub(super) const SETTINGS_TITLE_ID: &str = "settings-title";
pub(super) const SETTINGS_SIDEBAR_ID: &str = "settings-sidebar";
pub(super) const SETTINGS_TAB_TERMINAL_ID: &str = "settings-tab-terminal";
pub(super) const SETTINGS_TAB_LOCAL_SHELLS_ID: &str = "settings-tab-local-shells";
pub(super) const SETTINGS_PANEL_ID: &str = "settings-panel";
pub(super) const SETTINGS_SEARCH_FIELD_ID: &str = "settings-search-field";
pub(super) const SETTINGS_SEARCH_CLEAR_ID: &str = "settings-search-clear";
pub(super) const SETTINGS_SEARCH_RESULTS_ID: &str = "settings-search-results";
pub(super) const SETTINGS_SEARCH_COUNT_ID: &str = "settings-search-count";
pub(super) const SETTINGS_SEARCH_EMPTY_ID: &str = "settings-search-empty";
pub(super) const SETTINGS_SEARCH_RESULT_0_ID: &str = "settings-search-result-0";
pub(super) const SETTINGS_SAVE_ID: &str = "settings-save";
pub(super) const SETTINGS_SAVE_ERROR_ID: &str = "settings-save-error";
pub(super) const SETTINGS_SAVE_CONFIRMATION_ID: &str = "settings-save-confirmation";
/// The scrollback section and its field, shared with the route tests.
#[cfg(test)]
pub(super) const SETTINGS_SCROLLBACK_SECTION_ID: &str = "settings-scrollback-lines";
#[cfg(test)]
pub(super) const SETTINGS_SCROLLBACK_INPUT_ID: &str = "settings-scrollback-input";
#[cfg(test)]
pub(super) const SETTINGS_SCROLLBACK_ERROR_ID: &str = "settings-scrollback-error";

/// The page the settings route shows below its header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsTab {
    Terminal,
    LocalShells,
}

/// The sections of the Terminal page, in the order the reference page shows them.
///
/// The content loop and the search scroll use this one order, so adding a
/// section cannot leave a search result pointing at the wrong child index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SettingsSection {
    Font,
    Cursor,
    Clipboard,
    CommandMarks,
    Scrollback,
    Padding,
    Preview,
    Save,
}

pub(super) const SETTINGS_SECTIONS: [SettingsSection; 8] = [
    SettingsSection::Font,
    SettingsSection::Cursor,
    SettingsSection::Clipboard,
    SettingsSection::CommandMarks,
    SettingsSection::Scrollback,
    SettingsSection::Padding,
    SettingsSection::Preview,
    SettingsSection::Save,
];

impl SettingsSection {
    fn index(self) -> usize {
        SETTINGS_SECTIONS
            .iter()
            .position(|section| *section == self)
            .unwrap_or_default()
    }

    /// Whether this section continues the block above without a section gap.
    pub(super) fn continues_previous(self) -> bool {
        self == Self::CommandMarks
    }
}

impl SettingsTab {
    pub(super) fn value(self) -> &'static str {
        match self {
            Self::Terminal => TERMINAL_PAGE_VALUE,
            Self::LocalShells => LOCAL_SHELLS_PAGE_VALUE,
        }
    }

    fn from_value(value: &str) -> Option<Self> {
        [Self::Terminal, Self::LocalShells]
            .into_iter()
            .find(|tab| tab.value() == value)
    }
}

/// The settings page entity. It owns the live search and delegates each
/// implemented page to its own setting editor.
pub(super) struct SettingsPage {
    locale: Entity<Locale>,
    theme: Theme,
    scrollback: Entity<ScrollbackSetting>,
    font: Entity<FontSetting>,
    appearance: Entity<AppearanceSetting>,
    clipboard: Entity<ClipboardSetting>,
    command_marks: Entity<CommandMarksSetting>,
    local_shells: Entity<LocalShellsSetting>,
    search_input: Option<Entity<InputState>>,
    _search_events: Option<Subscription>,
    _search_observer: Option<Subscription>,
    query: String,
    active_index: usize,
    active_tab: SettingsTab,
    content_scroll: gpui_kit::ScrollHandle,
    open: bool,
    page_saving: bool,
    page_saved: bool,
    page_error: Option<String>,
    _locale_observer: Option<Subscription>,
    _font_submit: Option<Subscription>,
    _scrollback_submit: Option<Subscription>,
    _appearance_submit: Option<Subscription>,
}

impl SettingsPage {
    #[cfg(any(test, all(debug_assertions, feature = "automation")))]
    pub(super) fn set_font_system_fonts(&self, fonts: Vec<String>, cx: &mut App) {
        self.font
            .update(cx, |setting, _| setting.set_system_fonts(fonts));
    }

    pub(super) fn new(
        locale: Entity<Locale>,
        theme: Theme,
        scrollback: Entity<ScrollbackSetting>,
        local_shells: Entity<LocalShellsSetting>,
        cx: &mut Context<Self>,
    ) -> Self {
        let config = scrollback.read(cx).config_entity();
        let font = cx.new(|cx| FontSetting::new(config.clone(), locale.clone(), theme, cx));
        let appearance =
            cx.new(|cx| AppearanceSetting::new(config.clone(), locale.clone(), theme, cx));
        let clipboard =
            cx.new(|cx| ClipboardSetting::new(config.clone(), locale.clone(), theme, cx));
        let command_marks =
            cx.new(|cx| CommandMarksSetting::new(config.clone(), locale.clone(), theme, cx));
        cx.observe(&scrollback, |_, _, cx| cx.notify()).detach();
        cx.observe(&font, |_, _, cx| cx.notify()).detach();
        cx.observe(&appearance, |_, _, cx| cx.notify()).detach();
        cx.observe(&clipboard, |_, _, cx| cx.notify()).detach();
        cx.observe(&command_marks, |_, _, cx| cx.notify()).detach();
        let content_scroll = gpui_kit::ScrollHandle::new();
        cx.observe(&local_shells, |_, _, cx| cx.notify()).detach();
        Self {
            locale,
            theme,
            scrollback,
            font,
            appearance,
            clipboard,
            command_marks,
            local_shells,
            search_input: None,
            _search_events: None,
            _search_observer: None,
            query: String::new(),
            active_index: 0,
            active_tab: SettingsTab::Terminal,
            content_scroll,
            open: false,
            page_saving: false,
            page_saved: false,
            page_error: None,
            _locale_observer: None,
            _font_submit: None,
            _scrollback_submit: None,
            _appearance_submit: None,
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
        self.font
            .update(cx, |setting, cx| setting.open_page(window, cx));
        self.appearance
            .update(cx, |setting, cx| setting.open_page(window, cx));
        self.clipboard
            .update(cx, |setting, cx| setting.open_page(cx));
        self.command_marks
            .update(cx, |setting, cx| setting.open_page(cx));
        self.local_shells
            .update(cx, |setting, cx| setting.open_page(cx));
        self.page_saving = false;
        self.page_saved = false;
        self.page_error = None;
        let field = self.ensure_search_input(window, cx);
        self.ensure_locale_observer(window, cx);
        self.ensure_page_submit_events(window, cx);
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
        if self.page_saving
            || !self.scrollback.read(cx).can_close()
            || !self.font.read(cx).can_close()
            || !self.appearance.read(cx).can_close()
            || !self.clipboard.read(cx).can_close()
            || !self.command_marks.read(cx).can_close()
            || !self.local_shells.read(cx).can_close()
        {
            return false;
        }
        let scrollback_closed = self
            .scrollback
            .update(cx, |setting, cx| setting.close_page(cx));
        let font_closed = self.font.update(cx, |setting, cx| setting.close_page(cx));
        let appearance_closed = self
            .appearance
            .update(cx, |setting, cx| setting.close_page(cx));
        let clipboard_closed = self
            .clipboard
            .update(cx, |setting, cx| setting.close_page(cx));
        let command_marks_closed = self
            .command_marks
            .update(cx, |setting, cx| setting.close_page(cx));
        let local_shells_closed = self
            .local_shells
            .update(cx, |setting, cx| setting.close_page(cx));
        if !scrollback_closed
            || !font_closed
            || !appearance_closed
            || !clipboard_closed
            || !command_marks_closed
            || !local_shells_closed
        {
            return false;
        }
        self.open = false;
        self.query.clear();
        self.active_index = 0;
        self._search_events = None;
        self._search_observer = None;
        self._locale_observer = None;
        self._font_submit = None;
        self._scrollback_submit = None;
        self._appearance_submit = None;
        self.search_input = None;
        self.page_saving = false;
        self.page_saved = false;
        self.page_error = None;
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

    fn ensure_page_submit_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self._font_submit.is_none() {
            let font = self.font.clone();
            self._font_submit = Some(cx.subscribe_in(
                &font,
                window,
                |this, _, _: &FontPageSubmit, window, cx| this.submit_page(window, cx),
            ));
        }
        if self._scrollback_submit.is_none() {
            let scrollback = self.scrollback.clone();
            self._scrollback_submit = Some(cx.subscribe_in(
                &scrollback,
                window,
                |this, _, _: &ScrollbackPageSubmit, window, cx| this.submit_page(window, cx),
            ));
        }
        if self._appearance_submit.is_none() {
            let appearance = self.appearance.clone();
            self._appearance_submit = Some(cx.subscribe_in(
                &appearance,
                window,
                |this, _, _: &AppearancePageSubmit, window, cx| this.submit_page(window, cx),
            ));
        }
    }

    /// Saves every Terminal draft as one configuration edit.
    ///
    /// Both editors validate before the write starts, so an invalid font or
    /// scrollback field keeps every draft and no field is written on its own.
    /// While the write is in flight the Save button is disabled and the route
    /// refuses to close.
    pub(super) fn submit_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open || self.page_saving {
            return;
        }
        // A new attempt replaces the previous outcome before validation, so a
        // stale write error cannot sit beside a draft the user has changed.
        self.page_saved = false;
        self.page_error = None;
        let Some((family, size, line_height, letter_spacing)) = self
            .font
            .update(cx, |font, cx| font.page_values(window, cx))
        else {
            return;
        };
        let Some(scrollback_lines) = self
            .scrollback
            .update(cx, |scrollback, cx| scrollback.page_value(window, cx))
        else {
            return;
        };
        let Some((cursor_style, padding_left, padding_right, padding_bottom)) = self
            .appearance
            .update(cx, |appearance, cx| appearance.page_values(window, cx))
        else {
            return;
        };
        let clipboard = self
            .clipboard
            .update(cx, |clipboard, cx| clipboard.page_values(cx));
        let command_marks = self
            .command_marks
            .update(cx, |setting, cx| setting.page_values(cx));
        self.page_saving = true;
        self.page_saved = false;
        self.page_error = None;
        cx.notify();
        let settings = TerminalPageSettings {
            family,
            size,
            line_height,
            letter_spacing,
            scrollback_lines,
            cursor_style,
            padding_left,
            padding_right,
            padding_bottom,
            confirm_multiline_paste: clipboard.confirm_multiline_paste,
            copy_on_select: clipboard.copy_on_select,
            right_click_paste: clipboard.right_click_paste,
            osc52_clipboard_write: clipboard.osc52_clipboard_write,
            mac_option_is_meta: clipboard.mac_option_is_meta,
            command_marks: command_marks.command_marks,
            local_unix_shell_integration: command_marks.local_unix_shell_integration,
        };
        let config = self.scrollback.read(cx).config_entity();
        let task = config.update(cx, |config, cx| config.save_terminal_page(settings, cx));
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|window, app| {
                let _ = this.update(app, |this, cx| this.finish_page_save(result, window, cx));
            });
        })
        .detach();
    }

    fn finish_page_save(
        &mut self,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.page_saving = false;
        self.page_saved = result.is_ok();
        self.page_error = result.as_ref().err().cloned();
        self.font.update(cx, |font, cx| {
            font.page_save_result(result.clone(), window, cx)
        });
        self.appearance.update(cx, |appearance, cx| {
            appearance.page_save_result(result.clone(), window, cx)
        });
        self.clipboard.update(cx, |clipboard, cx| {
            clipboard.page_save_result(result.clone(), cx)
        });
        self.command_marks.update(cx, |setting, cx| {
            setting.page_save_result(result.clone(), cx)
        });
        self.scrollback.update(cx, |scrollback, cx| {
            scrollback.page_save_result(result, window, cx)
        });
        cx.notify();
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
        self.font.update(cx, |setting, cx| {
            setting.refresh_placeholders(window, cx);
        });
        self.local_shells.update(cx, |setting, cx| {
            setting.refresh_placeholders(window, cx);
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

    /// Opens one search result: the query ends, its page is shown, and a
    /// setting row gives the page's editor focus while a page row keeps focus
    /// in the search field.
    fn open_result(
        &mut self,
        result: SettingsSearchResult,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = SettingsTab::from_value(&result.tab) else {
            return;
        };
        self.active_tab = tab;
        self.content_scroll.set_offset(Default::default());
        self.query.clear();
        self.active_index = 0;
        if let Some(field) = &self.search_input {
            field.update(cx, |state, cx| state.set_value("", window, cx));
            if result.is_page {
                field.update(cx, |state, cx| state.focus(window, cx));
            }
        }
        if !result.is_page {
            match tab {
                SettingsTab::Terminal => {
                    if result.title
                        == search::text(self.locale.read(cx), keys::FONT_SETTINGS_FONT_SIZE)
                    {
                        self.font.update(cx, |setting, cx| {
                            setting.focus_size(window, cx);
                        });
                    } else if result.title
                        == search::text(self.locale.read(cx), keys::FONT_SETTINGS_LINE_HEIGHT)
                    {
                        self.font.update(cx, |setting, cx| {
                            setting.focus_line_height(window, cx);
                        });
                    } else if result.title
                        == search::text(self.locale.read(cx), keys::FONT_SETTINGS_LETTER_SPACING)
                    {
                        self.font.update(cx, |setting, cx| {
                            setting.focus_letter_spacing(window, cx);
                        });
                    } else if result.title
                        == search::text(self.locale.read(cx), keys::FONT_SETTINGS_FONT_FAMILY)
                        || result.title
                            == search::text(
                                self.locale.read(cx),
                                keys::FONT_SETTINGS_TEXT_RENDERING,
                            )
                    {
                        self.font.update(cx, |setting, cx| {
                            setting.focus_family(window, cx);
                        });
                    } else if result.title
                        == search::text(self.locale.read(cx), keys::FONT_SETTINGS_CURSOR_STYLE)
                    {
                        self.appearance.update(cx, |setting, cx| {
                            setting.focus_cursor(window, cx);
                        });
                        self.scroll_to_section(SettingsSection::Cursor);
                    } else if self.command_marks.update(cx, |setting, cx| {
                        setting.focus_search_result(&result.title, window, cx)
                    }) {
                        self.scroll_to_section(SettingsSection::CommandMarks);
                    } else if self.clipboard.update(cx, |editor, cx| {
                        editor.focus_search_result(&result.title, window, cx)
                    }) {
                        self.scroll_to_section(SettingsSection::Clipboard);
                    } else if result.title
                        == search::text(self.locale.read(cx), keys::FONT_SETTINGS_TERMINAL_PADDING)
                    {
                        self.appearance.update(cx, |setting, cx| {
                            setting.focus_padding_left(window, cx);
                        });
                        self.scroll_to_section(SettingsSection::Padding);
                    } else if result.title
                        == search::text(self.locale.read(cx), keys::FONT_SETTINGS_PREVIEW)
                    {
                        self.scroll_to_section(SettingsSection::Preview);
                    } else {
                        self.scroll_to_section(SettingsSection::Scrollback);
                        self.scrollback.update(cx, |setting, cx| {
                            setting.focus_page_input(window, cx);
                        });
                    }
                }
                SettingsTab::LocalShells => {
                    self.local_shells.update(cx, |setting, cx| {
                        setting.focus_page_input(window, cx);
                    });
                }
            }
        }
        cx.notify();
    }

    /// Scrolls a section to the top using the shared section order, never a
    /// literal child index.
    fn scroll_to_section(&self, section: SettingsSection) {
        self.content_scroll.scroll_to_top_of_item(section.index());
    }

    fn select_tab(&mut self, tab: SettingsTab, window: &mut Window, cx: &mut Context<Self>) {
        self.active_tab = tab;
        self.clear_search(window, cx);
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
}

impl Prototype {
    /// Opens the settings route, or does nothing when it is already showing.
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_active_search(window, cx);
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
