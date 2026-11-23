//! The title-bar scrollback setting and its lazily created editor.
//!
//! The window root is built from `&mut App` alone, so this entity cannot
//! create an [`InputState`] until the user opens the editor and a
//! [`Window`] is available. Closing restores focus to the terminal view that
//! had it before the editor opened.

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Div, Entity, FocusHandle, FontWeight, KeyDownEvent, MouseButton, ParentElement,
    Render, Stateful, Styled, Window, div, px,
};
use tterm_core::config::scrollback::MAX_EXPLICIT_SCROLLBACK_LINES;
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::dialog::presentation::OVERLAY_OPACITY;
use tterm_ui::components::input::{InputOptions, input};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use crate::config_state::ConfigState;
use crate::locale::Locale;

#[cfg(test)]
mod tests;

const DIALOG_ID: &str = "scrollback-setting-dialog";
const CANCEL_BUTTON_ID: &str = "scrollback-setting-cancel";
const SAVE_BUTTON_ID: &str = "scrollback-setting-save";
const PAGE_SECTION_ID: &str = "settings-scrollback-lines";
const PAGE_INPUT_ID: &str = "settings-scrollback-input";
const PAGE_SAVE_BUTTON_ID: &str = "settings-scrollback-save";
const PAGE_ERROR_ID: &str = "settings-scrollback-error";
const SCROLLBACK_PRESETS: [u32; 5] = [1_000, 5_000, 10_000, 50_000, 100_000];
const DIALOG_WIDTH: f32 = 448.0;
const DIALOG_PADDING: f32 = 24.0;
const DIALOG_RADIUS: f32 = 8.0;

/// The title-bar button and the editor it opens.
pub(crate) struct ScrollbackSetting {
    config: Entity<ConfigState>,
    locale: Entity<Locale>,
    theme: Theme,
    open: bool,
    page_mode: bool,
    input: Option<Entity<InputState>>,
    error: Option<String>,
    saving: bool,
    restore_focus: Option<FocusHandle>,
}

impl ScrollbackSetting {
    /// Creates the setting without a window; the editor state is created on
    /// the first [`ScrollbackSetting::open`].
    pub(crate) fn new(
        config: Entity<ConfigState>,
        locale: Entity<Locale>,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&locale, |_, _, cx| cx.notify()).detach();
        Self {
            config,
            locale,
            theme,
            open: false,
            page_mode: false,
            input: None,
            error: None,
            saving: false,
            restore_focus: None,
        }
    }

    #[cfg(test)]
    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_editor(window, cx, false);
    }

    /// Opens the editor as a section of the settings page.
    pub(crate) fn open_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_editor(window, cx, true);
    }

    fn open_editor(&mut self, window: &mut Window, cx: &mut Context<Self>, page_mode: bool) {
        if self.open {
            // A dialog editor can become the page's section when settings
            // opens above it; keep the Escape owner in sync with the route.
            self.page_mode = page_mode;
            cx.notify();
            return;
        }
        self.page_mode = page_mode;
        self.restore_focus = (!page_mode).then(|| window.focused(cx)).flatten();
        self.error = None;
        self.saving = false;
        let value = self.config.read(cx).scrollback_lines().to_string();
        let state = cx.new(|cx| InputState::new(window, cx));
        state.update(cx, |state, cx| {
            state.set_placeholder(
                self.text(cx, keys::SCROLLBACK_SETTING_PLACEHOLDER),
                window,
                cx,
            );
            state.set_value(value, window, cx);
            state.focus(window, cx);
        });
        self.input = Some(state);
        self.open = true;
        cx.notify();
    }

    /// Puts the page's draft field in focus when a search result opens.
    ///
    /// The whole draft is selected so the next typed digit replaces the old
    /// capacity instead of appending to it.
    pub(crate) fn focus_page_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.page_mode {
            return;
        }
        if let Some(input) = &self.input {
            input.update(cx, |state, cx| {
                state.focus(window, cx);
                state.select_all(window, cx);
            });
        }
    }

    /// Refreshes the field placeholder after a language change.
    pub(crate) fn refresh_placeholder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.input {
            let placeholder = self.text(cx, keys::SCROLLBACK_SETTING_PLACEHOLDER);
            input.update(cx, |state, cx| {
                state.set_placeholder(placeholder, window, cx);
            });
        }
    }

    /// Clears the page editor without taking focus back from the route.
    ///
    /// Returns `false` while a save is in flight so the route stays open
    /// rather than discarding the user's pending change.
    pub(crate) fn close_page(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.open {
            return true;
        }
        if self.saving {
            return false;
        }
        self.open = false;
        self.page_mode = false;
        self.error = None;
        self.input = None;
        self.restore_focus = None;
        cx.notify();
        true
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.open = false;
        self.error = None;
        self.input = None;
        if let Some(focus) = self.restore_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let value = self
            .input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let lines = match self.parse_lines(&value, cx) {
            Ok(lines) => lines,
            Err(message) => {
                self.error = Some(message);
                self.focus_input(window, cx);
                cx.notify();
                return;
            }
        };
        self.saving = true;
        self.error = None;
        cx.notify();
        let task = self
            .config
            .update(cx, |config, cx| config.save_scrollback_lines(lines, cx));
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|window, app| {
                let _ = this.update(app, |this, cx| this.finish_save(result, window, cx));
            });
        })
        .detach();
    }

    fn finish_save(
        &mut self,
        result: Result<u32, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.saving = false;
        match result {
            Ok(_) if self.page_mode => cx.notify(),
            Ok(_) => self.close(window, cx),
            Err(message) => {
                self.error = Some(message);
                self.focus_input(window, cx);
                cx.notify();
            }
        }
    }

    fn focus_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.input {
            input.update(cx, |state, cx| state.focus(window, cx));
        }
    }

    fn parse_lines(&self, value: &str, cx: &App) -> Result<u32, String> {
        let value = value.trim();
        if value.is_empty() {
            return Err(self.text(cx, keys::SCROLLBACK_SETTING_EMPTY));
        }
        let lines = value
            .parse::<u32>()
            .map_err(|_| self.text(cx, keys::SCROLLBACK_SETTING_INVALID))?;
        if lines > MAX_EXPLICIT_SCROLLBACK_LINES {
            return Err(self.text(cx, keys::SCROLLBACK_SETTING_TOO_LARGE));
        }
        Ok(lines)
    }

    fn text(&self, cx: &App, key: TranslationKey) -> String {
        self.locale.read(cx).text(key).unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate a scrollback setting label: {error}");
            key.as_str().to_string()
        })
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "tab" => {
                window.prevent_default();
                cx.stop_propagation();
                if self.page_mode {
                    // The page has a focus order of its own; trapping Tab in
                    // the number field would make the search unreachable.
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                } else {
                    // The dialog has nowhere else to go.
                    self.focus_input(window, cx);
                }
            }
            "escape" => {
                // The route owns Escape and returns to the terminal.
                if self.page_mode {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                self.close(window, cx);
            }
            "enter" => {
                window.prevent_default();
                cx.stop_propagation();
                self.submit(window, cx);
            }
            _ => {}
        }
    }

    /// The current draft, for a test that reopens the page.
    #[cfg(test)]
    pub(crate) fn value(&self, cx: &App) -> String {
        self.input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default()
    }

    /// The dialog field's focus handle, for the editor regression.
    #[cfg(test)]
    pub(crate) fn input_focus(&self, cx: &App) -> FocusHandle {
        self.input
            .as_ref()
            .map(|input| input.read(cx).presentation().focus_handle().clone())
            .unwrap_or_else(|| cx.focus_handle())
    }

    /// The focused draft field, for route-level focus assertions.
    #[cfg(test)]
    pub(crate) fn page_focus(&self, cx: &App) -> Option<FocusHandle> {
        self.input
            .as_ref()
            .map(|input| input.read(cx).presentation().focus_handle().clone())
    }

    /// The draft field's placeholder, for language-change assertions.
    #[cfg(test)]
    pub(crate) fn page_placeholder(&self, cx: &App) -> String {
        self.input
            .as_ref()
            .map(|input| input.read(cx).presentation().placeholder().to_string())
            .unwrap_or_default()
    }

    /// Renders the scrollback field as the body of the Terminal settings page.
    ///
    /// The page's save button is separate so the field keeps the reference
    /// layout, where saving the page is one action below every setting.
    pub(crate) fn section(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let title = self.text(cx, keys::FONT_SETTINGS_SCROLLBACK_LINES);
        let description = self.text(cx, keys::FONT_SETTINGS_SCROLLBACK_LINES_DESC);
        let unlimited = self.text(cx, keys::FONT_SETTINGS_UNLIMITED);
        let value = self
            .input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let current = value.trim().parse::<u32>().ok();
        let mut section = div()
            .id(PAGE_SECTION_ID)
            .debug_selector(|| PAGE_SECTION_ID.to_string())
            .flex()
            .flex_col()
            .capture_key_down(cx.listener(Self::handle_key))
            .child(
                div()
                    .text_size(px(14.0))
                    .line_height(px(14.0))
                    .font_weight(FontWeight(500.0))
                    .mb(px(8.0))
                    .child(title),
            );
        if let Some(state) = &self.input {
            section = section.child(
                input(
                    state,
                    InputOptions {
                        theme: Some(theme),
                        ..InputOptions::default()
                    },
                )
                .id(PAGE_INPUT_ID)
                .debug_selector(|| PAGE_INPUT_ID.to_string()),
            );
        }
        section = section.child(
            div()
                .mt(px(8.0))
                .text_size(px(12.0))
                .line_height(px(13.2))
                .text_color(hsla(theme.colors.muted_foreground, 1.0))
                .child(description),
        );
        if let Some(error) = &self.error {
            section = section.child(
                div()
                    .id(PAGE_ERROR_ID)
                    .debug_selector(|| PAGE_ERROR_ID.to_string())
                    .mt(px(8.0))
                    .text_size(px(12.0))
                    .text_color(hsla(theme.colors.destructive, 1.0))
                    .child(error.clone()),
            );
        }
        let mut presets = div()
            .mt(px(8.0))
            .flex()
            .flex_wrap()
            .gap(px(6.0))
            .child(preset_button(
                theme,
                unlimited,
                0,
                current == Some(0),
                cx.listener(|this, _, window, cx| this.set_preset(0, window, cx)),
            ));
        for lines in SCROLLBACK_PRESETS {
            let label = if lines >= 1_000 {
                format!("{}k", lines / 1_000)
            } else {
                lines.to_string()
            };
            presets = presets.child(preset_button(
                theme,
                label,
                lines,
                current == Some(lines),
                cx.listener(move |this, _, window, cx| this.set_preset(lines, window, cx)),
            ));
        }
        section.child(presets)
    }

    /// The full-width save action below the Terminal page.
    pub(crate) fn save_button(&mut self, cx: &mut Context<Self>) -> Div {
        let save = self.text(cx, keys::FONT_SETTINGS_SAVE);
        let state = ButtonState {
            disabled: self.saving,
            ..ButtonState::default()
        };
        div().w_full().child(
            button(
                PAGE_SAVE_BUTTON_ID,
                save.clone(),
                Variant::Default,
                ButtonSize::Default,
                state,
                self.theme,
            )
            .w_full()
            .debug_selector(|| PAGE_SAVE_BUTTON_ID.to_string())
            .aria_label(save)
            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
        )
    }

    fn set_preset(&mut self, lines: u32, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.input {
            input.update(cx, |state, cx| {
                state.set_value(lines.to_string(), window, cx);
                state.focus(window, cx);
            });
        }
        self.error = None;
        cx.notify();
    }

    fn dialog(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let title = self.text(cx, keys::SCROLLBACK_SETTING_TITLE);
        let description = self.text(cx, keys::SCROLLBACK_SETTING_DESCRIPTION);
        let cancel = self.text(cx, keys::COMMON_CANCEL);
        let save = self.text(cx, keys::SCROLLBACK_SETTING_SAVE);
        let mut field = div().mt(px(16.0)).flex().flex_col().gap(px(8.0)).child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight(500.0))
                .child(title.clone()),
        );
        if let Some(state) = &self.input {
            field = field.child(input(
                state,
                InputOptions {
                    theme: Some(theme),
                    ..InputOptions::default()
                },
            ));
        }
        let mut content = div()
            .id(DIALOG_ID)
            .w(px(DIALOG_WIDTH))
            .p(px(DIALOG_PADDING))
            .rounded(px(DIALOG_RADIUS))
            .border(px(1.0))
            .border_color(hsla(theme.colors.border, 1.0))
            .bg(hsla(theme.colors.background, 1.0))
            .text_color(hsla(theme.colors.foreground, 1.0))
            .occlude()
            .capture_key_down(cx.listener(Self::handle_key))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(FontWeight(600.0))
                    .child(title),
            )
            .child(
                div()
                    .mt(px(8.0))
                    .text_size(px(14.0))
                    .text_color(hsla(theme.colors.muted_foreground, 1.0))
                    .child(description),
            )
            .child(field);
        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .mt(px(8.0))
                    .text_size(px(12.0))
                    .text_color(hsla(theme.colors.destructive, 1.0))
                    .child(error.clone()),
            );
        }
        content.child(
            div()
                .mt(px(24.0))
                .flex()
                .justify_end()
                .gap(px(8.0))
                .child(
                    button(
                        CANCEL_BUTTON_ID,
                        cancel,
                        Variant::Outline,
                        ButtonSize::Default,
                        ButtonState::default(),
                        theme,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                )
                .child(
                    button(
                        SAVE_BUTTON_ID,
                        save,
                        Variant::Default,
                        ButtonSize::Default,
                        ButtonState::default(),
                        theme,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                ),
        )
    }
}

impl Render for ScrollbackSetting {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div();
        }
        let dialog = self.dialog(cx);
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(hsla(self.theme.colors.background, 1.0).opacity(OVERLAY_OPACITY))
            .on_any_mouse_down(cx.listener(|this, _, window, cx| this.close(window, cx)))
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(dialog),
            )
    }
}

/// One preset chip under the scrollback field.
///
/// A preset only fills the draft; it never saves, so a mistaken click cannot
/// shrink a running terminal's history before the page is saved.
fn preset_button(
    theme: Theme,
    label: String,
    lines: u32,
    selected: bool,
    on_click: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let variant = if selected {
        Variant::Default
    } else {
        Variant::Outline
    };
    button(
        format!("settings-scrollback-preset-{lines}"),
        label.clone(),
        variant,
        ButtonSize::Xs,
        ButtonState::default(),
        theme,
    )
    .debug_selector(move || format!("settings-scrollback-preset-{lines}"))
    .aria_label(label)
    .on_click(on_click)
}
