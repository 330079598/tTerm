//! The cursor-style and terminal-padding controls on the Terminal settings page.
//!
//! The page owns one Save for every Terminal draft, so these controls only keep
//! a draft and validate it. The selected cursor is one of the three samples the
//! reference page shows; the padding fields accept whole pixels from 0 to 80.
//! A save that fails keeps the draft so the user can retry.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Div, Entity, EventEmitter, FocusHandle, KeyDownEvent, Subscription, Window,
};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_ui::tokens::theme::Theme;

use crate::config_state::ConfigState;
use crate::locale::Locale;

mod view;

#[cfg(test)]
mod tests;

/// The cursor section and its sample controls.
pub(crate) const CURSOR_SECTION_ID: &str = "settings-cursor-style";
pub(crate) const CURSOR_CHOICES_ID: &str = "settings-cursor-choices";
pub(crate) const CURSOR_OPTION_PREFIX: &str = "settings-cursor-option-";
/// The padding section, its fields and its validation message.
pub(crate) const PADDING_SECTION_ID: &str = "settings-terminal-padding";
pub(crate) const PADDING_LEFT_INPUT_ID: &str = "settings-padding-left-input";
pub(crate) const PADDING_RIGHT_INPUT_ID: &str = "settings-padding-right-input";
pub(crate) const PADDING_BOTTOM_INPUT_ID: &str = "settings-padding-bottom-input";
pub(crate) const PADDING_ERROR_ID: &str = "settings-padding-error";

/// The shapes the shared config accepts, in the order the page shows them.
pub(crate) const CURSOR_STYLES: [(&str, TranslationKey); 3] = [
    ("bar", keys::FONT_SETTINGS_CURSOR_STYLE_BAR),
    ("block", keys::FONT_SETTINGS_CURSOR_STYLE_BLOCK),
    ("underline", keys::FONT_SETTINGS_CURSOR_STYLE_UNDERLINE),
];

/// The sharing bound the shared config applies to each padding field.
const MAX_PADDING: u16 = 80;

/// Emitted when Enter in a page field asks the page to save every draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AppearancePageSubmit;

impl EventEmitter<AppearancePageSubmit> for AppearanceSetting {}

/// The three padding fields, in the order the page lays them out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaddingSide {
    Left,
    Right,
    Bottom,
}

impl PaddingSide {
    fn label_key(self) -> TranslationKey {
        match self {
            Self::Left => keys::FONT_SETTINGS_TERMINAL_PADDING_LEFT,
            Self::Right => keys::FONT_SETTINGS_TERMINAL_PADDING_RIGHT,
            Self::Bottom => keys::FONT_SETTINGS_TERMINAL_PADDING_BOTTOM,
        }
    }

    fn input_id(self) -> &'static str {
        match self {
            Self::Left => PADDING_LEFT_INPUT_ID,
            Self::Right => PADDING_RIGHT_INPUT_ID,
            Self::Bottom => PADDING_BOTTOM_INPUT_ID,
        }
    }
}

/// Why a padding draft cannot be saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaddingError {
    Required,
    Invalid,
    Range,
}

impl PaddingError {
    fn key(self) -> TranslationKey {
        match self {
            Self::Required => keys::FONT_SETTINGS_PADDING_REQUIRED,
            Self::Invalid => keys::FONT_SETTINGS_PADDING_INVALID,
            Self::Range => keys::FONT_SETTINGS_PADDING_RANGE,
        }
    }
}

/// Parses one padding draft, refusing empty, fractional and out-of-range text.
fn parse_padding(value: &str) -> Result<u16, PaddingError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(PaddingError::Required);
    }
    let padding = value.parse::<u16>().map_err(|_| PaddingError::Invalid)?;
    if padding > MAX_PADDING {
        return Err(PaddingError::Range);
    }
    Ok(padding)
}

/// Keeps a saved cursor style only when the page can show it selected.
fn cursor_style(value: &str) -> String {
    CURSOR_STYLES
        .iter()
        .find(|(style, _)| *style == value)
        .map(|(style, _)| (*style).to_string())
        .unwrap_or_else(|| "block".to_string())
}

/// The cursor and padding editor shown on the Terminal page.
pub(crate) struct AppearanceSetting {
    config: Entity<ConfigState>,
    locale: Entity<Locale>,
    theme: Theme,
    cursor_style: String,
    cursor_focus: FocusHandle,
    padding_left: Option<Entity<InputState>>,
    padding_right: Option<Entity<InputState>>,
    padding_bottom: Option<Entity<InputState>>,
    error: Option<String>,
    open: bool,
    _left_events: Option<Subscription>,
    _right_events: Option<Subscription>,
    _bottom_events: Option<Subscription>,
}

impl AppearanceSetting {
    /// Creates the section without inputs; the route builds them when it opens.
    pub(crate) fn new(
        config: Entity<ConfigState>,
        locale: Entity<Locale>,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&config, |_, _, cx| cx.notify()).detach();
        cx.observe(&locale, |_, _, cx| cx.notify()).detach();
        let cursor_style = cursor_style(&config.read(cx).config().cursor_style);
        Self {
            config,
            locale,
            theme,
            cursor_style,
            cursor_focus: cx.focus_handle(),
            padding_left: None,
            padding_right: None,
            padding_bottom: None,
            error: None,
            open: false,
            _left_events: None,
            _right_events: None,
            _bottom_events: None,
        }
    }

    /// Shows the section for this settings visit, reloading every saved value.
    pub(crate) fn open_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        self.ensure_inputs(window, cx);
        self.reload_from_saved(window, cx);
        cx.notify();
    }

    /// Hides the section and drops its draft.
    pub(crate) fn close_page(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.open {
            return true;
        }
        self.open = false;
        self.error = None;
        self.padding_left = None;
        self.padding_right = None;
        self.padding_bottom = None;
        self._left_events = None;
        self._right_events = None;
        self._bottom_events = None;
        cx.notify();
        true
    }

    /// Whether the route may close without dropping a pending save.
    pub(crate) fn can_close(&self) -> bool {
        true
    }

    fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.padding_left.is_some() {
            return;
        }
        let left = cx.new(|cx| InputState::new(window, cx));
        let right = cx.new(|cx| InputState::new(window, cx));
        let bottom = cx.new(|cx| InputState::new(window, cx));
        let left_events = Self::subscribe_input(&left, window, cx);
        let right_events = Self::subscribe_input(&right, window, cx);
        let bottom_events = Self::subscribe_input(&bottom, window, cx);
        self.padding_left = Some(left);
        self.padding_right = Some(right);
        self.padding_bottom = Some(bottom);
        self._left_events = Some(left_events);
        self._right_events = Some(right_events);
        self._bottom_events = Some(bottom_events);
    }

    fn subscribe_input(
        input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(input, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.error = None;
                cx.notify();
            }
        })
    }

    fn reload_from_saved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (cursor_style, left, right, bottom) = {
            let config = self.config.read(cx).config();
            (
                cursor_style(&config.cursor_style),
                config.terminal_padding_left_px,
                config.terminal_padding_right_px,
                config.terminal_padding_bottom_px,
            )
        };
        self.cursor_style = cursor_style;
        if let Some(input) = &self.padding_left {
            input.update(cx, |state, cx| {
                state.set_value(left.to_string(), window, cx)
            });
        }
        if let Some(input) = &self.padding_right {
            input.update(cx, |state, cx| {
                state.set_value(right.to_string(), window, cx)
            });
        }
        if let Some(input) = &self.padding_bottom {
            input.update(cx, |state, cx| {
                state.set_value(bottom.to_string(), window, cx)
            });
        }
        self.error = None;
    }

    fn padding_value(&self, side: PaddingSide, cx: &App) -> String {
        let input = match side {
            PaddingSide::Left => &self.padding_left,
            PaddingSide::Right => &self.padding_right,
            PaddingSide::Bottom => &self.padding_bottom,
        };
        input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    fn padding_input(&self, side: PaddingSide) -> Option<&Entity<InputState>> {
        match side {
            PaddingSide::Left => self.padding_left.as_ref(),
            PaddingSide::Right => self.padding_right.as_ref(),
            PaddingSide::Bottom => self.padding_bottom.as_ref(),
        }
    }

    fn focus_padding(&mut self, side: PaddingSide, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = self.padding_input(side) {
            input.update(cx, |state, cx| {
                state.focus(window, cx);
                state.select_all(window, cx);
            });
        }
    }

    /// Puts the cursor choices in focus when their search result opens.
    pub(crate) fn focus_cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cursor_focus.focus(window, cx);
    }

    /// Puts the left padding field in focus when its search result opens.
    pub(crate) fn focus_padding_left(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_padding(PaddingSide::Left, window, cx);
    }

    /// Validates the page draft, returning the values or showing the field error.
    pub(crate) fn page_values(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<(String, u16, u16, u16)> {
        let mut values = Vec::with_capacity(3);
        for side in [PaddingSide::Left, PaddingSide::Right, PaddingSide::Bottom] {
            match parse_padding(&self.padding_value(side, cx)) {
                Ok(value) => values.push(value),
                Err(error) => {
                    self.error = Some(self.text(cx, error.key()));
                    self.focus_padding(side, window, cx);
                    cx.notify();
                    return None;
                }
            }
        }
        self.error = None;
        cx.notify();
        Some((self.cursor_style.clone(), values[0], values[1], values[2]))
    }

    /// Applies the unified page save result to this draft.
    pub(crate) fn page_save_result(
        &mut self,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(()) => self.reload_from_saved(window, cx),
            Err(message) => self.error = Some(message),
        }
        cx.notify();
    }

    fn select_cursor(&mut self, style: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        if CURSOR_STYLES
            .iter()
            .any(|(candidate, _)| *candidate == style)
        {
            self.cursor_style = style.to_string();
        }
        self.error = None;
        self.cursor_focus.focus(window, cx);
        cx.notify();
    }

    fn cursor_label(&self, cx: &App, key: TranslationKey) -> String {
        self.text(cx, key)
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "tab" => {
                // The page owns the focus order, so the section must not trap
                // Tab in one field.
                window.prevent_default();
                cx.stop_propagation();
                if event.keystroke.modifiers.shift {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
            }
            "enter" => {
                window.prevent_default();
                cx.stop_propagation();
                // One Save below the page writes every draft; a field only
                // asks the page to run it.
                cx.emit(AppearancePageSubmit);
            }
            _ => {}
        }
    }

    fn text(&self, cx: &App, key: TranslationKey) -> String {
        self.locale.read(cx).text(key).unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate an appearance setting label: {error}");
            key.as_str().to_string()
        })
    }
}

/// Test access to the cursor choice and padding drafts.
#[cfg(test)]
impl AppearanceSetting {
    pub(crate) fn cursor_style(&self) -> &str {
        &self.cursor_style
    }

    pub(crate) fn padding_values(&self, cx: &App) -> (String, String, String) {
        (
            self.padding_value(PaddingSide::Left, cx),
            self.padding_value(PaddingSide::Right, cx),
            self.padding_value(PaddingSide::Bottom, cx),
        )
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn cursor_focus(&self) -> FocusHandle {
        self.cursor_focus.clone()
    }

    pub(crate) fn padding_left_focus(&self, cx: &App) -> Option<FocusHandle> {
        self.padding_left
            .as_ref()
            .map(|input| input.read(cx).presentation().focus_handle().clone())
    }
}
