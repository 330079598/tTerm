//! The Terminal font family, size and spacing section of the settings page.
//!
//! The draft lives in the real inputs until the route saves the whole page; the
//! preview follows the draft so a family, size, line height or letter spacing
//! can be checked before it reaches the saved configuration. The system font
//! list is read on the background executor, so a large font directory cannot
//! hold the settings frame.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Div, Entity, EventEmitter, FontWeight, KeyDownEvent, Role, Stateful,
    Subscription, Task, Window, div, px,
};
use tterm_core::config::terminal_font::first_installed_font;
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::icons::{EYE, SEARCH, SQUARE_TERMINAL, icon};
use tterm_ui::components::input::{InputOptions, input};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use crate::config_state::ConfigState;
use crate::locale::Locale;

#[cfg(test)]
use gpui_kit::FocusHandle;

mod parsers;
#[cfg(test)]
mod tests;
mod view;

use self::parsers::{is_nerd_font, parse_letter_spacing, parse_line_height, parse_size};

/// The section, its inputs and its actions, shared with the route tests.
pub(crate) const SECTION_ID: &str = "settings-font";
pub(crate) const TITLE_ID: &str = "settings-font-title";
pub(crate) const FAMILY_INPUT_ID: &str = "settings-font-family-input";
pub(crate) const SIZE_INPUT_ID: &str = "settings-font-size-input";
pub(crate) const LINE_HEIGHT_INPUT_ID: &str = "settings-font-line-height-input";
pub(crate) const LETTER_SPACING_INPUT_ID: &str = "settings-font-letter-spacing-input";
pub(crate) const SIZE_PRESET_PREFIX: &str = "settings-font-size-";
pub(crate) const SEARCH_INPUT_ID: &str = "settings-font-search-input";
pub(crate) const NERD_TOGGLE_ID: &str = "settings-font-nerd-toggle";
pub(crate) const FONT_LIST_ID: &str = "settings-font-list";
pub(crate) const FONT_ROW_PREFIX: &str = "settings-font-row-";
pub(crate) const LOADING_ID: &str = "settings-font-loading";
pub(crate) const LOAD_ERROR_ID: &str = "settings-font-load-error";
pub(crate) const EMPTY_ID: &str = "settings-font-empty";
pub(crate) const NO_MATCH_ID: &str = "settings-font-no-match";
pub(crate) const WARNING_ID: &str = "settings-font-nerd-warning";
pub(crate) const PREVIEW_ID: &str = "settings-font-preview";
pub(crate) const NERD_PREVIEW_ID: &str = "settings-font-preview-nerd";
#[cfg(test)]
pub(crate) const PREVIEW_TEXT_ID: &str = "settings-font-preview-text";
#[cfg(test)]
pub(crate) const PREVIEW_SPACED_ID: &str = "settings-font-preview-spaced";
#[cfg(test)]
pub(crate) const SAVE_ID: &str = "settings-font-save";
#[cfg(test)]
pub(crate) const CANCEL_ID: &str = "settings-font-cancel";
pub(crate) const ERROR_ID: &str = "settings-font-error";
pub(crate) const SAVED_ID: &str = "settings-font-saved";

/// The size buttons the React page shows before the free-form field.
pub(crate) const FONT_SIZE_OPTIONS: [u16; 11] = [10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 24];

const FONT_LIST_HEIGHT: f32 = 192.0;
const PREVIEW_TEXT: &str = "The quick brown fox jumps over the lazy dog 0123456789";
const NERD_PREVIEW_LINES: [&str; 2] = [" 󰊢 main 󰁕 ~ ", " 12:34:56 user@host ~/projects"];

/// Emitted when Enter in a page field asks the page to save every draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FontPageSubmit;

impl EventEmitter<FontPageSubmit> for FontSetting {}

/// The Terminal font editor on the Terminal settings page.
pub(crate) struct FontSetting {
    config: Entity<ConfigState>,
    locale: Entity<Locale>,
    theme: Theme,
    family_input: Option<Entity<InputState>>,
    size_input: Option<Entity<InputState>>,
    line_height_input: Option<Entity<InputState>>,
    letter_spacing_input: Option<Entity<InputState>>,
    font_search_input: Option<Entity<InputState>>,
    system_fonts: Option<Vec<String>>,
    fonts_loading: bool,
    fonts_error: Option<String>,
    /// A test or capture seeded the list; a late loader result must not replace it.
    fonts_seeded: bool,
    show_nerd_fonts_only: bool,
    error: Option<String>,
    saving: bool,
    saved: bool,
    open: bool,
    _family_events: Option<Subscription>,
    _size_events: Option<Subscription>,
    _line_height_events: Option<Subscription>,
    _letter_spacing_events: Option<Subscription>,
    _search_events: Option<Subscription>,
    _font_task: Option<Task<()>>,
}

impl FontSetting {
    /// Creates the section without inputs; they are built when the route opens.
    pub(crate) fn new(
        config: Entity<ConfigState>,
        locale: Entity<Locale>,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&config, |_, _, cx| cx.notify()).detach();
        cx.observe(&locale, |_, _, cx| cx.notify()).detach();
        Self {
            config,
            locale,
            theme,
            family_input: None,
            size_input: None,
            line_height_input: None,
            letter_spacing_input: None,
            font_search_input: None,
            system_fonts: None,
            fonts_loading: false,
            fonts_error: None,
            fonts_seeded: false,
            show_nerd_fonts_only: false,
            error: None,
            saving: false,
            saved: false,
            open: false,
            _family_events: None,
            _size_events: None,
            _line_height_events: None,
            _letter_spacing_events: None,
            _search_events: None,
            _font_task: None,
        }
    }

    /// Replaces the interface palette without touching the draft or focus.
    pub(crate) fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        cx.notify();
    }

    /// Shows the section for this settings visit, reloading the saved values so
    /// a draft from the previous visit cannot reappear.
    pub(crate) fn open_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        self.error = None;
        self.saved = false;
        self.saving = false;
        self.show_nerd_fonts_only = false;
        self.ensure_inputs(window, cx);
        self.reload_from_saved(window, cx);
        self.start_font_load(cx);
        cx.notify();
    }

    /// Hides the section and drops the draft. Returns `false` while a save is in
    /// flight so the route does not discard the pending write.
    pub(crate) fn close_page(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.open {
            return true;
        }
        if self.saving {
            return false;
        }
        self.open = false;
        self.error = None;
        self.saved = false;
        self.family_input = None;
        self.size_input = None;
        self.line_height_input = None;
        self.letter_spacing_input = None;
        self.font_search_input = None;
        self._family_events = None;
        self._size_events = None;
        self._line_height_events = None;
        self._letter_spacing_events = None;
        self._search_events = None;
        self._font_task = None;
        cx.notify();
        true
    }

    /// Whether the route may close without dropping a pending save.
    pub(crate) fn can_close(&self) -> bool {
        !self.saving
    }

    /// Refreshes the translated placeholders after a language change.
    pub(crate) fn refresh_placeholders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let family = self.text(cx, keys::FONT_SETTINGS_CUSTOM_FONT);
        let search = self.text(cx, keys::FONT_SETTINGS_SEARCH_FONTS);
        if let Some(input) = &self.family_input {
            input.update(cx, |state, cx| {
                state.set_placeholder(family, window, cx);
            });
        }
        if let Some(input) = &self.font_search_input {
            input.update(cx, |state, cx| {
                state.set_placeholder(search, window, cx);
            });
        }
    }

    /// Puts the family field in focus when its search result opens.
    pub(crate) fn focus_family(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.family_input {
            input.update(cx, |state, cx| {
                state.focus(window, cx);
                state.select_all(window, cx);
            });
        }
    }

    /// Puts the custom size field in focus when its search result opens.
    pub(crate) fn focus_size(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.size_input {
            input.update(cx, |state, cx| {
                state.focus(window, cx);
                state.select_all(window, cx);
            });
        }
    }

    fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.family_input.is_some() {
            return;
        }
        let family = cx.new(|cx| InputState::new(window, cx));
        let size = cx.new(|cx| InputState::new(window, cx));
        let line_height = cx.new(|cx| InputState::new(window, cx));
        let letter_spacing = cx.new(|cx| InputState::new(window, cx));
        let search = cx.new(|cx| InputState::new(window, cx));
        family.update(cx, |state, cx| {
            state.set_placeholder(self.text(cx, keys::FONT_SETTINGS_CUSTOM_FONT), window, cx);
        });
        search.update(cx, |state, cx| {
            state.set_placeholder(self.text(cx, keys::FONT_SETTINGS_SEARCH_FONTS), window, cx);
        });
        let family_events = cx.subscribe_in(&family, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.error = None;
                this.saved = false;
                cx.notify();
            }
        });
        let size_events = cx.subscribe_in(&size, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.error = None;
                this.saved = false;
                cx.notify();
            }
        });
        let line_height_events = cx.subscribe_in(&line_height, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.error = None;
                this.saved = false;
                cx.notify();
            }
        });
        let letter_spacing_events =
            cx.subscribe_in(&letter_spacing, window, |this, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.error = None;
                    this.saved = false;
                    cx.notify();
                }
            });
        let search_events = cx.subscribe_in(&search, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.show_nerd_fonts_only = false;
                cx.notify();
            }
        });
        self.family_input = Some(family);
        self.size_input = Some(size);
        self.line_height_input = Some(line_height);
        self.letter_spacing_input = Some(letter_spacing);
        self.font_search_input = Some(search);
        self._family_events = Some(family_events);
        self._size_events = Some(size_events);
        self._line_height_events = Some(line_height_events);
        self._letter_spacing_events = Some(letter_spacing_events);
        self._search_events = Some(search_events);
    }

    fn reload_from_saved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (family, size, line_height, letter_spacing) = {
            let config = self.config.read(cx).config();
            (
                config.font_family.clone(),
                config.font_size,
                config.terminal_line_height,
                config.terminal_letter_spacing,
            )
        };
        if let Some(input) = &self.family_input {
            input.update(cx, |state, cx| state.set_value(family, window, cx));
        }
        if let Some(input) = &self.size_input {
            input.update(cx, |state, cx| {
                state.set_value(size.to_string(), window, cx)
            });
        }
        if let Some(input) = &self.line_height_input {
            input.update(cx, |state, cx| {
                state.set_value(line_height.to_string(), window, cx)
            });
        }
        if let Some(input) = &self.letter_spacing_input {
            input.update(cx, |state, cx| {
                state.set_value(letter_spacing.to_string(), window, cx)
            });
        }
        if let Some(input) = &self.font_search_input {
            input.update(cx, |state, cx| state.set_value("", window, cx));
        }
        self.error = None;
        self.saved = false;
    }

    #[cfg(test)]
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.reload_from_saved(window, cx);
        cx.notify();
    }

    #[cfg(test)]
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let family = self
            .family_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let family = family.trim().to_string();
        if family.is_empty() {
            self.error = Some(self.text(cx, keys::FONT_SETTINGS_FAMILY_REQUIRED));
            self.focus_family(window, cx);
            cx.notify();
            return;
        }
        let size = self
            .size_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let size = match parse_size(&size) {
            Ok(size) => size,
            Err(key) => {
                self.error = Some(self.text(cx, key));
                self.focus_size(window, cx);
                cx.notify();
                return;
            }
        };
        self.saving = true;
        self.error = None;
        self.saved = false;
        cx.notify();
        let task = self
            .config
            .update(cx, |config, cx| config.save_terminal_font(family, size, cx));
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|window, app| {
                let _ = this.update(app, |this, cx| this.finish_save(result, window, cx));
            });
        })
        .detach();
    }

    #[cfg(test)]
    fn finish_save(
        &mut self,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.saving = false;
        match result {
            Ok(()) => {
                self.reload_from_saved(window, cx);
                self.saved = true;
            }
            Err(message) => {
                self.error = Some(message);
            }
        }
        cx.notify();
    }

    /// The validated page draft, or the focused field with its validation error.
    ///
    /// The unified Save writes all five values together, so an invalid line
    /// height, letter spacing, size or empty family keeps every draft intact
    /// and no writer sees a partial page.
    pub(crate) fn page_values(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<(String, u16, f64, i8)> {
        let family = self.family_value(cx).trim().to_string();
        if family.is_empty() {
            self.error = Some(self.text(cx, keys::FONT_SETTINGS_FAMILY_REQUIRED));
            self.focus_family(window, cx);
            cx.notify();
            return None;
        }
        let size = match parse_size(&self.size_value(cx)) {
            Ok(size) => size,
            Err(key) => {
                self.error = Some(self.text(cx, key));
                self.focus_size(window, cx);
                cx.notify();
                return None;
            }
        };
        let line_height = match parse_line_height(&self.line_height_value(cx)) {
            Ok(line_height) => line_height,
            Err(key) => {
                self.error = Some(self.text(cx, key));
                self.focus_line_height(window, cx);
                cx.notify();
                return None;
            }
        };
        let letter_spacing = match parse_letter_spacing(&self.letter_spacing_value(cx)) {
            Ok(letter_spacing) => letter_spacing,
            Err(key) => {
                self.error = Some(self.text(cx, key));
                self.focus_letter_spacing(window, cx);
                cx.notify();
                return None;
            }
        };
        self.error = None;
        self.saved = false;
        cx.notify();
        Some((family, size, line_height, letter_spacing))
    }

    /// Applies the unified page save result to the font draft.
    pub(crate) fn page_save_result(
        &mut self,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.saving = false;
        match result {
            Ok(()) => {
                self.reload_from_saved(window, cx);
                self.saved = true;
            }
            Err(message) => {
                self.saved = false;
                self.error = Some(message);
            }
        }
        cx.notify();
    }

    /// Puts the line-height field in focus when its search result opens.
    pub(crate) fn focus_line_height(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.line_height_input {
            input.update(cx, |state, cx| {
                state.focus(window, cx);
                state.select_all(window, cx);
            });
        }
    }

    /// Puts the letter-spacing field in focus when its search result opens.
    pub(crate) fn focus_letter_spacing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.letter_spacing_input {
            input.update(cx, |state, cx| {
                state.focus(window, cx);
                state.select_all(window, cx);
            });
        }
    }

    fn line_height_value(&self, cx: &App) -> String {
        self.line_height_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    fn letter_spacing_value(&self, cx: &App) -> String {
        self.letter_spacing_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "tab" => {
                // The section has several inputs and a font list, so leave
                // traversal to the page instead of trapping Tab in one field.
                window.prevent_default();
                cx.stop_propagation();
                if event.keystroke.modifiers.shift {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
            }
            "enter" if !self.saving => {
                window.prevent_default();
                cx.stop_propagation();
                // The page owns the single save action; each field only asks
                // it to write the whole draft.
                cx.emit(FontPageSubmit);
            }
            _ => {}
        }
    }

    fn start_font_load(&mut self, cx: &mut Context<Self>) {
        if self.system_fonts.is_some() || self.fonts_loading || self.fonts_seeded {
            return;
        }
        self.fonts_loading = true;
        self.fonts_error = None;
        let load = cx
            .background_executor()
            .spawn(async { tterm_core::fonts::list_fonts_sync() });
        self._font_task = Some(cx.spawn(async move |this, cx| {
            let fonts = load.await;
            let _ = this.update(cx, |this, cx| {
                if this.fonts_seeded {
                    return;
                }
                this.fonts_loading = false;
                this.system_fonts = Some(fonts);
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Whether the font picker finished its system-font load, for the capture.
    #[cfg(all(feature = "automation", debug_assertions))]
    pub(crate) fn capture_fonts_loaded(&self) -> bool {
        !self.fonts_loading && self.system_fonts.is_some()
    }

    /// Replaces the system list without starting the loader, for tests and the
    /// headless capture.
    #[cfg(any(test, all(debug_assertions, feature = "automation")))]
    pub(crate) fn set_system_fonts(&mut self, fonts: Vec<String>) {
        self.system_fonts = Some(fonts);
        self.fonts_loading = false;
        self.fonts_error = None;
        self.fonts_seeded = true;
        self._font_task = None;
    }

    fn family_value(&self, cx: &App) -> String {
        self.family_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    fn size_value(&self, cx: &App) -> String {
        self.size_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    fn search_value(&self, cx: &App) -> String {
        self.font_search_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    fn set_size(&mut self, size: u16, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.size_input {
            input.update(cx, |state, cx| {
                state.set_value(size.to_string(), window, cx);
                state.focus(window, cx);
            });
        }
        self.error = None;
        self.saved = false;
        cx.notify();
    }

    fn set_family_from_picker(
        &mut self,
        family: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let chain = format!("\"{family}\", monospace");
        if let Some(input) = &self.family_input {
            input.update(cx, |state, cx| {
                state.set_value(chain, window, cx);
                state.focus(window, cx);
            });
        }
        self.error = None;
        self.saved = false;
        cx.notify();
    }

    fn text(&self, cx: &App, key: TranslationKey) -> String {
        self.locale.read(cx).text(key).unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate a font setting label: {error}");
            key.as_str().to_string()
        })
    }

    fn visible_fonts(&self, cx: &App) -> Vec<String> {
        let Some(fonts) = &self.system_fonts else {
            return Vec::new();
        };
        let query = self.search_value(cx).trim().to_lowercase();
        fonts
            .iter()
            .filter(|font| query.is_empty() || font.to_lowercase().contains(&query))
            .filter(|font| !self.show_nerd_fonts_only || is_nerd_font(font))
            .cloned()
            .collect()
    }

    pub(crate) fn preview_size(&self, cx: &App) -> u16 {
        parse_size(&self.size_value(cx))
            .ok()
            .unwrap_or_else(|| self.config.read(cx).config().font_size)
    }

    pub(crate) fn preview_family(&self, cx: &App) -> String {
        let family = self.family_value(cx);
        let family = family.trim();
        if family.is_empty() {
            return "monospace".to_string();
        }
        let installed = self.system_fonts.as_deref().unwrap_or(&[]);
        let resolved = first_installed_font(family, installed);
        resolved
            .split(',')
            .map(|name| name.trim().trim_matches(['"', '\'']))
            .find(|name| !name.is_empty())
            .unwrap_or("monospace")
            .to_string()
    }

    pub(crate) fn preview_line_height(&self, cx: &App) -> f32 {
        parse_line_height(&self.line_height_value(cx))
            .map(|value| value as f32)
            .unwrap_or_else(|_| self.config.read(cx).config().terminal_line_height as f32)
    }

    pub(crate) fn preview_letter_spacing(&self, cx: &App) -> f32 {
        parse_letter_spacing(&self.letter_spacing_value(cx))
            .map(f32::from)
            .unwrap_or_else(|_| f32::from(self.config.read(cx).config().terminal_letter_spacing))
    }
}

/// Test access to the draft and the picker state.
#[cfg(test)]
impl FontSetting {
    pub(crate) fn family(&self, cx: &App) -> String {
        self.family_value(cx)
    }

    pub(crate) fn size(&self, cx: &App) -> String {
        self.size_value(cx)
    }

    pub(crate) fn line_height(&self, cx: &App) -> String {
        self.line_height_value(cx)
    }

    pub(crate) fn letter_spacing(&self, cx: &App) -> String {
        self.letter_spacing_value(cx)
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn is_saving(&self) -> bool {
        self.saving
    }

    pub(crate) fn saved(&self) -> bool {
        self.saved
    }

    pub(crate) fn system_fonts(&self) -> &[String] {
        self.system_fonts.as_deref().unwrap_or(&[])
    }

    pub(crate) fn family_focus(&self, cx: &App) -> Option<FocusHandle> {
        self.family_input
            .as_ref()
            .map(|input| input.read(cx).presentation().focus_handle().clone())
    }

    pub(crate) fn size_focus(&self, cx: &App) -> Option<FocusHandle> {
        self.size_input
            .as_ref()
            .map(|input| input.read(cx).presentation().focus_handle().clone())
    }

    pub(crate) fn line_height_focus(&self, cx: &App) -> Option<FocusHandle> {
        self.line_height_input
            .as_ref()
            .map(|input| input.read(cx).presentation().focus_handle().clone())
    }

    pub(crate) fn letter_spacing_focus(&self, cx: &App) -> Option<FocusHandle> {
        self.letter_spacing_input
            .as_ref()
            .map(|input| input.read(cx).presentation().focus_handle().clone())
    }

    pub(crate) fn family_placeholder(&self, cx: &App) -> String {
        self.family_input
            .as_ref()
            .map(|input| input.read(cx).presentation().placeholder().to_string())
            .unwrap_or_default()
    }

    pub(crate) fn set_draft(
        &mut self,
        family: &str,
        size: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(input) = &self.family_input {
            input.update(cx, |state, cx| state.set_value(family, window, cx));
        }
        if let Some(input) = &self.size_input {
            input.update(cx, |state, cx| state.set_value(size, window, cx));
        }
        self.error = None;
        self.saved = false;
        cx.notify();
    }

    pub(crate) fn set_spacing_draft(
        &mut self,
        line_height: &str,
        letter_spacing: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(input) = &self.line_height_input {
            input.update(cx, |state, cx| state.set_value(line_height, window, cx));
        }
        if let Some(input) = &self.letter_spacing_input {
            input.update(cx, |state, cx| state.set_value(letter_spacing, window, cx));
        }
        self.error = None;
        self.saved = false;
        cx.notify();
    }

    pub(crate) fn set_search(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.font_search_input {
            input.update(cx, |state, cx| state.set_value(text, window, cx));
        }
        cx.notify();
    }
}
