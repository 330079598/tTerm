//! The theme choice and library on the Appearance settings page.
//!
//! This editor saves the shared `theme`, `theme_follow_system`, `theme_light`
//! and `theme_dark` fields. The window repaints from the accepted save, so the
//! choice does not wait for a restart. Choosing a preset keeps the follow-system
//! mode exactly as the user left it; the page changes that mode only through
//! its own switch.
//!
//! The library browser is transient: highlighting an entry previews it, but
//! only Apply writes the shared fields and Cancel or Escape restores the saved
//! theme.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Entity, EventEmitter, FocusHandle, KeyDownEvent, Subscription, Window,
    WindowAppearance,
};
use tterm_core::config::theme_file::ThemeSettings;
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_core::theme::definitions;
use tterm_core::theme::derivation;
use tterm_core::theme::types::PRESET_THEME_IDS;
use tterm_ui::tokens::theme::Theme;

use crate::config_state::ConfigState;
use crate::locale::Locale;

pub(crate) mod library;
mod library_view;
mod view;

#[cfg(test)]
mod tests;

use self::library::{ThemeFilter, ThemeLibrary};
use crate::app_window::theme::{self, ResolvedTheme};

/// A transient library preview the window should paint.
///
/// The Appearance page emits this while browsing and when it restores the
/// saved theme; no configuration write is involved.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ThemePreviewRequested {
    /// The palette to paint without saving it.
    pub(crate) resolved: ResolvedTheme,
}

/// The section and the controls the route tests look up.
pub(crate) const SECTION_ID: &str = "settings-appearance-theme";
pub(crate) const FOLLOW_ROW_ID: &str = "settings-theme-follow-system";
pub(crate) const FOLLOW_SWITCH_ID: &str = "settings-theme-follow-system-switch";
pub(crate) const ERROR_ID: &str = "settings-theme-error";
pub(crate) const LIGHT_GROUP_ID: &str = "settings-theme-light-group";
pub(crate) const DARK_GROUP_ID: &str = "settings-theme-dark-group";
const OPTION_PREFIX: &str = "settings-theme-option-";

/// The built-in themes in the order the reference theme library shows them.
pub(crate) const PRESETS: [&str; 6] = PRESET_THEME_IDS;

/// Which saved field a preset card changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThemeSlot {
    /// The single theme shown when the client does not follow the system.
    Single,
    /// The theme for a light system appearance.
    Light,
    /// The theme for a dark system appearance.
    Dark,
}

impl ThemeSlot {
    fn key(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

/// The debug selector of one preset card.
pub(crate) fn option_id(slot: ThemeSlot, id: &str) -> String {
    format!("{OPTION_PREFIX}{}-{id}", slot.key())
}

/// The localized name of a built-in theme.
fn name_key(id: &str) -> TranslationKey {
    match id {
        "light" => keys::THEME_LIGHT,
        "ocean" => keys::THEME_OCEAN,
        "forest" => keys::THEME_FOREST,
        "sunset" => keys::THEME_SUNSET,
        "ubuntu" => keys::THEME_UBUNTU,
        _ => keys::THEME_DEFAULT,
    }
}

impl EventEmitter<ThemePreviewRequested> for ThemeSetting {}

/// Whether a saved preset or library id is a dark theme.
fn theme_is_dark(id: &str) -> bool {
    theme::resolve_theme_id(id)
        .is_some_and(|resolved| derivation::is_dark_palette(&resolved.terminal))
}

/// Whether a saved preset belongs in the dark slot.
fn is_dark(id: &str) -> bool {
    definitions::get_preset_theme(id)
        .is_some_and(|theme| derivation::is_dark_palette(&theme.terminal))
}

/// The built-in theme editor on the Appearance page.
pub(crate) struct ThemeSetting {
    config: Entity<ConfigState>,
    locale: Entity<Locale>,
    theme: Theme,
    draft: ThemeSettings,
    /// The system appearance this window currently shows.
    system_dark: bool,
    saving: bool,
    error: Option<String>,
    focus: FocusHandle,
    follow_focus: FocusHandle,
    library: ThemeLibrary,
    open: bool,
    _config: Subscription,
    _locale: Subscription,
}

impl ThemeSetting {
    /// Creates the editor from the saved configuration.
    pub(crate) fn new(
        config: Entity<ConfigState>,
        locale: Entity<Locale>,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        let draft = ThemeSettings::from(config.read(cx).config());
        let config_observer = cx.observe(&config, |_, _, cx| cx.notify());
        let locale_observer = cx.observe(&locale, |_, _, cx| cx.notify());
        let library_focus = cx.focus_handle();
        Self {
            config,
            locale,
            theme,
            draft,
            system_dark: false,
            saving: false,
            error: None,
            focus: cx.focus_handle(),
            follow_focus: cx.focus_handle(),
            library: ThemeLibrary::new(library_focus),
            open: false,
            _config: config_observer,
            _locale: locale_observer,
        }
    }

    /// Replaces the interface palette without reloading the saved draft.
    ///
    /// While the library is open it owns the palette, because the entry under
    /// the pointer is a transient preview rather than the saved theme.
    pub(crate) fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        if self.library.open || self.theme == theme {
            return;
        }
        self.theme = theme;
        cx.notify();
    }

    /// Whether the library browser is open.
    pub(crate) fn library_open(&self) -> bool {
        self.library.open
    }

    /// Tracks the window appearance for the follow-system draft rules.
    pub(crate) fn set_system_dark(&mut self, system_dark: bool, cx: &mut Context<Self>) {
        if self.system_dark == system_dark {
            return;
        }
        self.system_dark = system_dark;
        cx.notify();
    }

    /// Shows the section for this settings visit, reloading every saved value.
    pub(crate) fn open_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        self.update_system_appearance(window);
        self.reload(cx);
        cx.notify();
    }

    /// Hides the section and drops its draft.
    pub(crate) fn close_page(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.open {
            return true;
        }
        if self.library.open {
            self.library.close();
            self.restore_effective_theme(cx);
        }
        self.open = false;
        self.error = None;
        cx.notify();
        true
    }

    /// Whether the route may close without losing a pending write.
    pub(crate) fn can_close(&self) -> bool {
        !self.saving
    }

    /// Puts the preset cards in focus when their search result opens.
    pub(crate) fn focus_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.library.open && !self.close_library(window, cx) {
            return;
        }
        self.focus.focus(window, cx);
    }

    /// Puts the follow-system switch in focus when its search result opens.
    pub(crate) fn focus_follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.follow_focus.focus(window, cx);
    }

    /// Opens the bundled library over the preset list.
    pub(crate) fn open_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || self.library.open {
            return;
        }
        self.update_system_appearance(window);
        let effective = self.effective_theme(cx);
        self.library.open(Some(effective.id));
        self.ensure_library_search(window, cx);
        if let Some(field) = &self.library.search {
            field.update(cx, |state, cx| state.focus(window, cx));
        }
        self.error = None;
        cx.notify();
    }

    /// Closes the library and restores the saved theme. Returns whether the
    /// library was open and could be closed.
    pub(crate) fn close_library(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.saving || !self.library.open {
            return false;
        }
        let previewing = self.library.selected.is_some();
        self.library.close();
        self.focus.focus(window, cx);
        if previewing {
            self.restore_effective_theme(cx);
        }
        self.error = None;
        cx.notify();
        true
    }

    /// Closes the library from the route's Escape handling. The same path is
    /// used for the footer's Cancel button.
    pub(crate) fn cancel_library(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.close_library(window, cx)
    }

    /// Re-reads the bundled catalog after a load failure.
    fn retry_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.library.open {
            return;
        }
        self.library.close();
        self.open_library(window, cx);
    }

    /// Applies the highlighted library entry through the shared save path.
    fn apply_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(selected) = self.library.selected_theme() else {
            return;
        };
        self.update_system_appearance(window);
        let id = selected.id.clone();
        let dark = selected.is_dark();
        if self.draft.follow_system {
            if dark {
                self.draft.theme_dark = id;
            } else {
                self.draft.theme_light = id;
            }
        } else {
            self.draft.theme = id;
        }
        if self.draft == ThemeSettings::from(self.config.read(cx).config()) {
            self.library.close();
            self.restore_effective_theme(cx);
            cx.notify();
            return;
        }
        self.persist(window, cx);
    }

    /// Keyboard handling for the library: arrows browse, Enter applies, and
    /// Escape cancels the transient preview.
    fn library_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.library.open {
            return;
        }
        let handled = match event.keystroke.key.as_str() {
            "escape" if !event.prefer_character_input => self.close_library(window, cx),
            "enter" => {
                self.apply_library(window, cx);
                true
            }
            "down" => self.step_library(1, cx),
            "up" => self.step_library(-1, cx),
            "pagedown" => self.step_library(10, cx),
            "pageup" => self.step_library(-10, cx),
            "home" => self.jump_library(false, cx),
            "end" => self.jump_library(true, cx),
            _ => false,
        };
        if handled {
            window.prevent_default();
            cx.stop_propagation();
        }
    }

    fn step_library(&mut self, delta: isize, cx: &mut Context<Self>) -> bool {
        if !self.library.open || self.saving {
            return false;
        }
        self.update_system_appearance_from_app(cx);
        if !self.library.step(delta) {
            return false;
        }
        if let Some(id) = self.library.selected.clone() {
            self.preview_theme_id(&id, cx);
        }
        cx.notify();
        true
    }

    fn jump_library(&mut self, to_last: bool, cx: &mut Context<Self>) -> bool {
        if !self.library.open || self.saving {
            return false;
        }
        if !self.library.jump(to_last) {
            return false;
        }
        if let Some(id) = self.library.selected.clone() {
            self.preview_theme_id(&id, cx);
        }
        cx.notify();
        true
    }

    fn select_library(&mut self, theme_id: &str, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.library.select(theme_id);
        if let Some(id) = self.library.selected.clone() {
            self.preview_theme_id(&id, cx);
        }
        cx.notify();
    }

    fn set_library_filter(&mut self, filter: ThemeFilter, cx: &mut Context<Self>) {
        if self.saving || self.library.filter == filter {
            return;
        }
        self.library.filter = filter;
        if self.library.settle_selection() {
            self.restore_effective_theme(cx);
        }
        cx.notify();
    }

    fn library_query_changed(&mut self, cx: &mut Context<Self>) {
        if self.library.settle_selection() {
            self.restore_effective_theme(cx);
        }
        cx.notify();
    }

    fn clear_library_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.library.query.clear();
        if let Some(field) = &self.library.search {
            field.update(cx, |state, cx| {
                state.set_value("", window, cx);
                state.focus(window, cx);
            });
        }
        if self.library.settle_selection() {
            self.restore_effective_theme(cx);
        }
        cx.notify();
    }

    fn ensure_library_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = self.text(cx, keys::THEME_GALLERY_SEARCH);
        let field = cx.new(|cx| InputState::new(window, cx));
        field.update(cx, |state, cx| {
            state.set_placeholder(placeholder, window, cx);
        });
        let events = cx.subscribe_in(&field, window, |this, field, event, _window, cx| {
            if matches!(event, InputEvent::Change) {
                this.library.query = field.read(cx).value().to_string();
                this.library_query_changed(cx);
            }
        });
        let observer = cx.observe(&field, |_, _, cx| cx.notify());
        self.library._search_events = Some(events);
        self.library._search_observer = Some(observer);
        self.library.search = Some(field);
    }

    /// Paints one library entry without writing the configuration.
    fn preview_theme_id(&mut self, theme_id: &str, cx: &mut Context<Self>) {
        let Some(resolved) = theme::resolve_theme_id(theme_id) else {
            return;
        };
        self.theme = resolved.ui;
        cx.emit(ThemePreviewRequested { resolved });
        cx.notify();
    }

    /// Restores the palette the saved configuration selects.
    fn restore_effective_theme(&mut self, cx: &mut Context<Self>) {
        let resolved = self.effective_theme(cx);
        self.theme = resolved.ui;
        cx.emit(ThemePreviewRequested { resolved });
        cx.notify();
    }

    fn effective_theme(&self, cx: &App) -> ResolvedTheme {
        theme::resolve(self.config.read(cx).config(), self.system_dark)
    }

    fn update_system_appearance_from_app(&mut self, cx: &App) {
        self.system_dark = matches!(
            cx.window_appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
    }

    fn reload(&mut self, cx: &App) {
        self.draft = ThemeSettings::from(self.config.read(cx).config());
        self.error = None;
    }

    fn update_system_appearance(&mut self, window: &Window) {
        self.system_dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
    }

    fn select_preset(
        &mut self,
        slot: ThemeSlot,
        id: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving {
            return;
        }
        match slot {
            ThemeSlot::Single => self.draft.theme = id.to_string(),
            ThemeSlot::Light => self.draft.theme_light = id.to_string(),
            ThemeSlot::Dark => self.draft.theme_dark = id.to_string(),
        }
        self.persist(window, cx);
    }

    fn set_follow_system(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || next == self.draft.follow_system {
            return;
        }
        self.update_system_appearance(window);
        self.draft.follow_system = next;
        if next {
            // The theme on screen becomes the theme for its own appearance,
            // like the reference page does when Following is turned on.
            let current = self.draft.theme.clone();
            if theme::resolve_theme_id(&current).is_some() {
                if theme_is_dark(&current) {
                    self.draft.theme_dark = current;
                } else {
                    self.draft.theme_light = current;
                }
            }
        } else {
            // Turning Following off freezes the slot this window would show.
            let slot = if self.system_dark {
                self.draft.theme_dark.clone()
            } else {
                self.draft.theme_light.clone()
            };
            self.draft.theme = if theme::resolve_theme_id(&slot).is_some() {
                slot
            } else {
                "default".to_string()
            };
        }
        self.persist(window, cx);
    }

    fn persist(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.update_system_appearance(window);
        if self.draft == ThemeSettings::from(self.config.read(cx).config()) {
            self.error = None;
            cx.notify();
            return;
        }
        self.saving = true;
        self.error = None;
        cx.notify();
        let settings = self.draft.clone();
        let config = self.config.clone();
        let task = config.update(cx, |config, cx| config.save_theme_settings(settings, cx));
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|window, app| {
                let _ = this.update(app, |this, cx| this.finish(result, window, cx));
            });
        })
        .detach();
    }

    fn finish(&mut self, result: Result<(), String>, _window: &mut Window, cx: &mut Context<Self>) {
        self.saving = false;
        let was_browsing = self.library.open;
        match result {
            Ok(()) => {
                self.reload(cx);
                if was_browsing {
                    self.library.close();
                    self.restore_effective_theme(cx);
                }
            }
            Err(error) => self.error = Some(error),
        }
        cx.notify();
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "tab" {
            // The section is one focus stop; Tab leaves it instead of being
            // trapped by the cards.
            window.prevent_default();
            cx.stop_propagation();
            if event.keystroke.modifiers.shift {
                window.focus_prev(cx);
            } else {
                window.focus_next(cx);
            }
        }
    }

    fn text(&self, cx: &App, key: TranslationKey) -> String {
        self.locale.read(cx).text(key).unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate a theme setting label: {error}");
            key.as_str().to_string()
        })
    }

    fn selected_in(&self, slot: ThemeSlot) -> &str {
        match slot {
            ThemeSlot::Single => &self.draft.theme,
            ThemeSlot::Light => &self.draft.theme_light,
            ThemeSlot::Dark => &self.draft.theme_dark,
        }
    }
}

/// Test access to the staged theme and its save state.
#[cfg(test)]
impl ThemeSetting {
    #[cfg(test)]
    pub(crate) fn theme(&self) -> Theme {
        self.theme
    }

    pub(crate) fn draft(&self) -> &ThemeSettings {
        &self.draft
    }

    pub(crate) fn is_saving(&self) -> bool {
        self.saving
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn system_dark(&self) -> bool {
        self.system_dark
    }

    pub(crate) fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    pub(crate) fn follow_focus_handle(&self) -> FocusHandle {
        self.follow_focus.clone()
    }

    pub(crate) fn library_query(&self) -> &str {
        &self.library.query
    }

    pub(crate) fn library_filter(&self) -> ThemeFilter {
        self.library.filter
    }

    pub(crate) fn library_ids(&self) -> Vec<String> {
        self.library
            .visible()
            .iter()
            .map(|theme| theme.id.clone())
            .collect()
    }

    pub(crate) fn library_selected(&self) -> Option<String> {
        self.library.selected.clone()
    }
}
