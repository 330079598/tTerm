//! The clipboard preferences of the terminal settings page.
//!
//! Each row is a labelled switch that changes only a draft. The page's one
//! Save writes all three values with the rest of the Terminal page, so
//! turning a switch never writes the configuration by itself. A failed save
//! keeps the draft so the user can retry without choosing the switches again.

use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Div, Entity, FontWeight, Stateful, div, px};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_ui::components::switch::{SwitchState, switch};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use crate::config_state::ConfigState;
use crate::locale::Locale;

/// The section, its rows and their switches, shared with the route tests.
pub(crate) const SECTION_ID: &str = "settings-clipboard";
pub(crate) const TITLE_ID: &str = "settings-clipboard-title";
pub(crate) const DESCRIPTION_ID: &str = "settings-clipboard-description";
pub(crate) const ERROR_ID: &str = "settings-clipboard-error";
pub(crate) const CONFIRM_MULTILINE_PASTE_ROW_ID: &str =
    "settings-clipboard-confirm-multiline-paste";
pub(crate) const CONFIRM_MULTILINE_PASTE_SWITCH_ID: &str =
    "settings-clipboard-confirm-multiline-paste-switch";
pub(crate) const COPY_ON_SELECT_ROW_ID: &str = "settings-clipboard-copy-on-select";
pub(crate) const COPY_ON_SELECT_SWITCH_ID: &str = "settings-clipboard-copy-on-select-switch";
pub(crate) const RIGHT_CLICK_PASTE_ROW_ID: &str = "settings-clipboard-right-click-paste";
pub(crate) const RIGHT_CLICK_PASTE_SWITCH_ID: &str = "settings-clipboard-right-click-paste-switch";

/// The three clipboard preferences, in the order the reference page shows
/// them inside its "Terminal behavior" section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClipboardPreference {
    ConfirmMultilinePaste,
    CopyOnSelect,
    RightClickPaste,
}

impl ClipboardPreference {
    const ALL: [Self; 3] = [
        Self::ConfirmMultilinePaste,
        Self::CopyOnSelect,
        Self::RightClickPaste,
    ];

    fn label_key(self) -> TranslationKey {
        match self {
            Self::ConfirmMultilinePaste => keys::FONT_SETTINGS_CONFIRM_MULTILINE_PASTE,
            Self::CopyOnSelect => keys::FONT_SETTINGS_COPY_ON_SELECT,
            Self::RightClickPaste => keys::FONT_SETTINGS_RIGHT_CLICK_PASTE,
        }
    }

    fn description_key(self) -> TranslationKey {
        match self {
            Self::ConfirmMultilinePaste => keys::FONT_SETTINGS_CONFIRM_MULTILINE_PASTE_DESC,
            Self::CopyOnSelect => keys::FONT_SETTINGS_COPY_ON_SELECT_DESC,
            Self::RightClickPaste => keys::FONT_SETTINGS_RIGHT_CLICK_PASTE_DESC,
        }
    }

    fn row_id(self) -> &'static str {
        match self {
            Self::ConfirmMultilinePaste => CONFIRM_MULTILINE_PASTE_ROW_ID,
            Self::CopyOnSelect => COPY_ON_SELECT_ROW_ID,
            Self::RightClickPaste => RIGHT_CLICK_PASTE_ROW_ID,
        }
    }

    fn switch_id(self) -> &'static str {
        match self {
            Self::ConfirmMultilinePaste => CONFIRM_MULTILINE_PASTE_SWITCH_ID,
            Self::CopyOnSelect => COPY_ON_SELECT_SWITCH_ID,
            Self::RightClickPaste => RIGHT_CLICK_PASTE_SWITCH_ID,
        }
    }
}

/// The clipboard switches shown on the Terminal settings page.
pub(crate) struct ClipboardSetting {
    focus: [gpui_kit::FocusHandle; 3],
    config: Entity<ConfigState>,
    locale: Entity<Locale>,
    theme: Theme,
    confirm_multiline_paste: bool,
    copy_on_select: bool,
    right_click_paste: bool,
    error: Option<String>,
    open: bool,
}

impl ClipboardSetting {
    /// Creates the section from the saved values; opening reloads them so a
    /// draft from the previous settings visit cannot reappear.
    pub(crate) fn new(
        config: Entity<ConfigState>,
        locale: Entity<Locale>,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&config, |_, _, cx| cx.notify()).detach();
        cx.observe(&locale, |_, _, cx| cx.notify()).detach();
        let (confirm_multiline_paste, copy_on_select, right_click_paste) = {
            let config = config.read(cx).config();
            (
                config.confirm_multiline_paste,
                config.copy_on_select,
                config.right_click_paste,
            )
        };
        Self {
            focus: std::array::from_fn(|_| cx.focus_handle()),
            config,
            locale,
            theme,
            confirm_multiline_paste,
            copy_on_select,
            right_click_paste,
            error: None,
            open: false,
        }
    }

    /// Shows the section for this settings visit with the saved values.
    pub(crate) fn open_page(&mut self, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        self.reload_from_saved(cx);
        cx.notify();
    }

    /// Hides the section. The switches hold no write of their own, so closing
    /// never has to wait and a draft is discarded with the route.
    pub(crate) fn close_page(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.open {
            return true;
        }
        self.open = false;
        self.error = None;
        cx.notify();
        true
    }

    /// Whether the route may close without dropping a pending save.
    pub(crate) fn can_close(&self) -> bool {
        true
    }

    /// The three draft values the page writes in one configuration edit.
    pub(crate) fn page_values(&mut self, cx: &mut Context<Self>) -> (bool, bool, bool) {
        self.error = None;
        cx.notify();
        (
            self.confirm_multiline_paste,
            self.copy_on_select,
            self.right_click_paste,
        )
    }

    /// Applies the unified page save result.
    ///
    /// Success reloads what the file reported so an external edit wins over
    /// the draft; failure keeps every draft and shows the writer's message.
    pub(crate) fn page_save_result(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => self.reload_from_saved(cx),
            Err(message) => self.error = Some(message),
        }
        cx.notify();
    }

    /// Renders the three switches below the cursor choices.
    pub(crate) fn section(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let title = self.text(cx, keys::FONT_SETTINGS_BEHAVIOR);
        let description = self.text(cx, keys::FONT_SETTINGS_BEHAVIOR_DESC);
        let mut rows = div().mt(px(12.0)).flex().flex_col().gap(px(8.0));
        for preference in ClipboardPreference::ALL {
            rows = rows.child(self.row(preference, cx));
        }
        let mut section = div()
            .id(SECTION_ID)
            .debug_selector(|| SECTION_ID.to_string())
            .capture_key_down(|event, window, cx| {
                if event.keystroke.key == "tab" {
                    window.prevent_default();
                    cx.stop_propagation();
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                }
            })
            .flex()
            .flex_col()
            .child(
                div()
                    .id(TITLE_ID)
                    .debug_selector(|| TITLE_ID.to_string())
                    .text_size(px(14.0))
                    .line_height(px(14.0))
                    .font_weight(FontWeight(500.0))
                    .text_color(hsla(colors.foreground, 1.0))
                    .child(title),
            )
            .child(
                div()
                    .id(DESCRIPTION_ID)
                    .debug_selector(|| DESCRIPTION_ID.to_string())
                    .mt(px(4.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(description),
            )
            .child(rows);
        if let Some(error) = &self.error {
            section = section.child(
                div()
                    .id(ERROR_ID)
                    .debug_selector(|| ERROR_ID.to_string())
                    .mt(px(8.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(colors.destructive, 1.0))
                    .child(error.clone()),
            );
        }
        section
    }

    fn row(&self, preference: ClipboardPreference, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let label = self.text(cx, preference.label_key());
        let description = self.text(cx, preference.description_key());
        let checked = self.value(preference);
        let row_id = preference.row_id();
        let switch_id = preference.switch_id();
        let entity = cx.entity().downgrade();
        let control = switch(
            switch_id,
            SwitchState {
                checked,
                ..SwitchState::default()
            },
            self.theme,
            move |next, _, _, app| {
                let _ = entity.update(app, |this, cx| this.set(preference, next, cx));
            },
        )
        .track_focus(&self.focus[preference as usize])
        .flex_shrink_0()
        .debug_selector(move || switch_id.to_string())
        .accessibility_label(label.clone());
        div()
            .id(row_id)
            .debug_selector(move || row_id.to_string())
            .flex()
            .items_start()
            .justify_between()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .line_height(px(14.0))
                            .font_weight(FontWeight(500.0))
                            .text_color(hsla(colors.foreground, 1.0))
                            .child(label),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(px(12.0))
                            .line_height(px(20.0))
                            .whitespace_normal()
                            .text_color(hsla(colors.muted_foreground, 1.0))
                            .child(description),
                    ),
            )
            .child(control)
    }

    fn value(&self, preference: ClipboardPreference) -> bool {
        match preference {
            ClipboardPreference::ConfirmMultilinePaste => self.confirm_multiline_paste,
            ClipboardPreference::CopyOnSelect => self.copy_on_select,
            ClipboardPreference::RightClickPaste => self.right_click_paste,
        }
    }

    fn set(&mut self, preference: ClipboardPreference, value: bool, cx: &mut Context<Self>) {
        match preference {
            ClipboardPreference::ConfirmMultilinePaste => self.confirm_multiline_paste = value,
            ClipboardPreference::CopyOnSelect => self.copy_on_select = value,
            ClipboardPreference::RightClickPaste => self.right_click_paste = value,
        }
        self.error = None;
        cx.notify();
    }

    fn reload_from_saved(&mut self, cx: &App) {
        let config = self.config.read(cx).config();
        self.confirm_multiline_paste = config.confirm_multiline_paste;
        self.copy_on_select = config.copy_on_select;
        self.right_click_paste = config.right_click_paste;
        self.error = None;
    }

    pub(crate) fn focus_search_result(
        &self,
        title: &str,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(preference) = ClipboardPreference::ALL
            .into_iter()
            .find(|p| self.text(cx, p.label_key()) == title)
        {
            self.focus[preference as usize].focus(window, cx);
            true
        } else {
            false
        }
    }

    fn text(&self, cx: &App, key: TranslationKey) -> String {
        self.locale.read(cx).text(key).unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate a clipboard setting label: {error}");
            key.as_str().to_string()
        })
    }
}

/// Test access to the clipboard drafts and their error.
#[cfg(test)]
impl ClipboardSetting {
    pub(crate) fn values(&self) -> (bool, bool, bool) {
        (
            self.confirm_multiline_paste,
            self.copy_on_select,
            self.right_click_paste,
        )
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}
