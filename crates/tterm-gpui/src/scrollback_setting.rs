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
const OPEN_BUTTON_ID: &str = "scrollback-setting-open";
const CANCEL_BUTTON_ID: &str = "scrollback-setting-cancel";
const SAVE_BUTTON_ID: &str = "scrollback-setting-save";
const DIALOG_WIDTH: f32 = 448.0;
const DIALOG_PADDING: f32 = 24.0;
const DIALOG_RADIUS: f32 = 8.0;

/// The title-bar button and the editor it opens.
pub(crate) struct ScrollbackSetting {
    config: Entity<ConfigState>,
    locale: Entity<Locale>,
    theme: Theme,
    open: bool,
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
            input: None,
            error: None,
            saving: false,
            restore_focus: None,
        }
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.restore_focus = window.focused(cx);
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

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                self.focus_input(window, cx);
            }
            "escape" => {
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

    pub(crate) fn button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        button(
            OPEN_BUTTON_ID,
            self.text(cx, keys::SCROLLBACK_SETTING_OPEN),
            Variant::Ghost,
            ButtonSize::Sm,
            ButtonState::default(),
            self.theme,
        )
        .on_click(cx.listener(|this, _, window, cx| this.open(window, cx)))
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
