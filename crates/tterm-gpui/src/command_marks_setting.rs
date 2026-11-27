//! The command-mark and local shell-integration switches on the Terminal page.
//!
//! Both rows continue the "Terminal behavior" section the clipboard switches
//! start, so they carry no heading of their own. They only change a draft;
//! the page's one Save writes every value together. The local integration row
//! exists only on macOS and Linux, where tTerm can start bash, zsh and
//! fish through its scripts, and is disabled while Mark commands is off, as
//! the reference page does. The command-mark row also copies the same remote
//! scripts the reference page offers; copying writes the clipboard and never
//! runs the script or changes the draft.

use gpui_kit::prelude::*;
use gpui_kit::{
    App, ClipboardItem, Context, Div, Entity, FocusHandle, FontWeight, Role, Stateful, div, px,
};
use tterm_core::config::AppConfig;
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_core::remote_shell_integration::{FISH_SHELL_INTEGRATION, POSIX_SHELL_INTEGRATION};
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::switch::{SwitchState, switch};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use crate::config_state::ConfigState;
use crate::locale::Locale;

/// The section that continues the behavior block, and its rows.
pub(crate) const SECTION_ID: &str = "settings-command-marks";
pub(crate) const COMMAND_MARKS_ROW_ID: &str = "settings-command-marks-row";
pub(crate) const COMMAND_MARKS_SWITCH_ID: &str = "settings-command-marks-switch";
pub(crate) const LOCAL_INTEGRATION_ROW_ID: &str = "settings-local-shell-integration";
pub(crate) const LOCAL_INTEGRATION_SWITCH_ID: &str = "settings-local-shell-integration-switch";
pub(crate) const COPY_POSIX_ID: &str = "settings-command-marks-copy-posix";
pub(crate) const COPY_FISH_ID: &str = "settings-command-marks-copy-fish";
pub(crate) const COPIED_ID: &str = "settings-command-marks-copied";

/// Whether this platform has a local shell integration row.
///
/// Windows shells always mark their commands, so the reference page hides
/// the row there instead of showing a switch that cannot do anything.
pub(crate) const fn local_integration_visible() -> bool {
    !cfg!(target_os = "windows")
}

/// The two switches the page submits as part of the Terminal page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CommandMarksDraft {
    /// Whether shells mark each command's prompt and output.
    pub(crate) command_marks: bool,
    /// Whether local bash, zsh and fish start through the integration.
    pub(crate) local_unix_shell_integration: bool,
}

/// Everything one switch row shows and does.
struct RowSpec<'a> {
    row_id: &'static str,
    switch_id: &'static str,
    label: String,
    description: String,
    extra: Option<String>,
    checked: bool,
    disabled: bool,
    copy_scripts: bool,
    focus: &'a FocusHandle,
    on_change: fn(&mut CommandMarksSetting, bool, &mut Context<CommandMarksSetting>),
}

impl From<&AppConfig> for CommandMarksDraft {
    fn from(config: &AppConfig) -> Self {
        Self {
            command_marks: config.command_marks,
            local_unix_shell_integration: config.local_unix_shell_integration,
        }
    }
}

/// The command-mark and local integration switches.
pub(crate) struct CommandMarksSetting {
    config: Entity<ConfigState>,
    locale: Entity<Locale>,
    theme: Theme,
    draft: CommandMarksDraft,
    command_marks_focus: FocusHandle,
    local_integration_focus: FocusHandle,
    copied: bool,
    error: Option<String>,
    open: bool,
}

impl CommandMarksSetting {
    /// Creates the section from the saved values.
    pub(crate) fn new(
        config: Entity<ConfigState>,
        locale: Entity<Locale>,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&config, |_, _, cx| cx.notify()).detach();
        cx.observe(&locale, |_, _, cx| cx.notify()).detach();
        let draft = CommandMarksDraft::from(config.read(cx).config());
        Self {
            config,
            locale,
            theme,
            draft,
            command_marks_focus: cx.focus_handle(),
            local_integration_focus: cx.focus_handle(),
            copied: false,
            error: None,
            open: false,
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

    /// Shows the rows for this settings visit with the saved values.
    pub(crate) fn open_page(&mut self, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        self.copied = false;
        self.reload_from_saved(cx);
        cx.notify();
    }

    /// Hides the rows. They hold no write of their own, so closing never
    /// waits and a draft is discarded with the route.
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

    /// The draft the page writes in one configuration edit.
    pub(crate) fn page_values(&mut self, cx: &mut Context<Self>) -> CommandMarksDraft {
        self.error = None;
        cx.notify();
        self.draft
    }

    /// Applies the unified page save result.
    ///
    /// Success reloads what the file reported so an external edit wins over
    /// the draft; failure keeps the draft and shows the writer's message.
    pub(crate) fn page_save_result(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => self.reload_from_saved(cx),
            Err(message) => self.error = Some(message),
        }
        cx.notify();
    }

    /// Renders the rows that continue the behavior section.
    pub(crate) fn section(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
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
            .gap(px(12.0))
            .child(self.command_marks_row(cx));
        if local_integration_visible() {
            section = section.child(self.local_integration_row(cx));
        }
        if let Some(error) = &self.error {
            section = section.child(
                div()
                    .mt(px(4.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(colors.destructive, 1.0))
                    .child(error.clone()),
            );
        }
        section
    }

    fn command_marks_row(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = self.text(cx, keys::FONT_SETTINGS_COMMAND_MARKS);
        let description = self.text(cx, keys::FONT_SETTINGS_COMMAND_MARKS_DESC);
        let remote = self.text(cx, keys::FONT_SETTINGS_COMMAND_MARKS_REMOTE);
        self.row(
            RowSpec {
                row_id: COMMAND_MARKS_ROW_ID,
                switch_id: COMMAND_MARKS_SWITCH_ID,
                label,
                description,
                extra: Some(remote),
                checked: self.draft.command_marks,
                disabled: false,
                copy_scripts: true,
                focus: &self.command_marks_focus,
                on_change: CommandMarksSetting::set_command_marks,
            },
            cx,
        )
    }

    fn local_integration_row(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = self.text(cx, keys::FONT_SETTINGS_LOCAL_SHELL_INTEGRATION);
        let description = self.text(cx, keys::FONT_SETTINGS_LOCAL_SHELL_INTEGRATION_DESC);
        self.row(
            RowSpec {
                row_id: LOCAL_INTEGRATION_ROW_ID,
                switch_id: LOCAL_INTEGRATION_SWITCH_ID,
                label,
                description,
                extra: None,
                checked: self.draft.local_unix_shell_integration,
                disabled: !self.draft.command_marks,
                copy_scripts: false,
                focus: &self.local_integration_focus,
                on_change: CommandMarksSetting::set_local_integration,
            },
            cx,
        )
    }

    fn row(&self, spec: RowSpec<'_>, cx: &mut Context<Self>) -> Stateful<Div> {
        let RowSpec {
            row_id,
            switch_id,
            label,
            description,
            extra,
            checked,
            disabled,
            copy_scripts,
            focus,
            on_change,
        } = spec;
        let colors = self.theme.colors;
        let entity = cx.entity().downgrade();
        let switch_label = label.clone();
        let control = switch(
            switch_id,
            SwitchState {
                checked,
                disabled,
                ..SwitchState::default()
            },
            self.theme,
            move |next, _, _, app| {
                let _ = entity.update(app, |this, cx| on_change(this, next, cx));
            },
        )
        .track_focus(focus)
        .flex_shrink_0()
        .debug_selector(move || switch_id.to_string())
        .accessibility_label(switch_label);
        let mut text = div()
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
            );
        if let Some(extra) = extra {
            text = text.child(
                div()
                    .mt(px(8.0))
                    .text_size(px(12.0))
                    .line_height(px(20.0))
                    .whitespace_normal()
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(extra),
            );
        }
        if copy_scripts {
            text = text.child(self.copy_actions(cx));
        }
        div()
            .id(row_id)
            .debug_selector(move || row_id.to_string())
            .flex()
            .items_start()
            .justify_between()
            .gap(px(16.0))
            .child(text)
            .child(control)
    }

    /// The reference page's two copy actions for the remote scripts.
    fn copy_actions(&self, cx: &mut Context<Self>) -> Div {
        let posix = self.text(cx, keys::FONT_SETTINGS_COPY_POSIX_INTEGRATION);
        let fish = self.text(cx, keys::FONT_SETTINGS_COPY_FISH_INTEGRATION);
        let mut actions = div()
            .mt(px(8.0))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.0))
            .child(
                button(
                    COPY_POSIX_ID,
                    posix.clone(),
                    Variant::Outline,
                    ButtonSize::Xs,
                    ButtonState::default(),
                    self.theme,
                )
                .debug_selector(|| COPY_POSIX_ID.to_string())
                .role(Role::Button)
                .aria_label(posix)
                .on_click(
                    cx.listener(|this, _, _, cx| this.copy_script(POSIX_SHELL_INTEGRATION, cx)),
                ),
            )
            .child(
                button(
                    COPY_FISH_ID,
                    fish.clone(),
                    Variant::Outline,
                    ButtonSize::Xs,
                    ButtonState::default(),
                    self.theme,
                )
                .debug_selector(|| COPY_FISH_ID.to_string())
                .role(Role::Button)
                .aria_label(fish)
                .on_click(
                    cx.listener(|this, _, _, cx| this.copy_script(FISH_SHELL_INTEGRATION, cx)),
                ),
            );
        if self.copied {
            actions = actions.child(
                div()
                    .debug_selector(|| COPIED_ID.to_string())
                    .text_size(px(12.0))
                    .line_height(px(24.0))
                    .text_color(hsla(self.theme.colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::FONT_SETTINGS_INTEGRATION_COPIED)),
            );
        }
        actions
    }

    fn copy_script(&mut self, script: &'static str, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(script.to_string()));
        self.copied = true;
        cx.notify();
    }

    fn set_command_marks(&mut self, value: bool, cx: &mut Context<Self>) {
        self.draft.command_marks = value;
        self.error = None;
        cx.notify();
    }

    fn set_local_integration(&mut self, value: bool, cx: &mut Context<Self>) {
        if !self.draft.command_marks {
            return;
        }
        self.draft.local_unix_shell_integration = value;
        self.error = None;
        cx.notify();
    }

    fn reload_from_saved(&mut self, cx: &App) {
        self.draft = CommandMarksDraft::from(self.config.read(cx).config());
        self.error = None;
    }

    /// Focuses the switch a search result names, when this section owns it.
    pub(crate) fn focus_search_result(
        &self,
        title: &str,
        window: &mut gpui_kit::Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let command_marks = keys::FONT_SETTINGS_COMMAND_MARKS;
        if self.text(cx, command_marks) == title {
            self.command_marks_focus.focus(window, cx);
            return true;
        }
        if local_integration_visible()
            && self.text(cx, keys::FONT_SETTINGS_LOCAL_SHELL_INTEGRATION) == title
        {
            self.local_integration_focus.focus(window, cx);
            return true;
        }
        false
    }

    fn text(&self, cx: &App, key: TranslationKey) -> String {
        self.locale.read(cx).text(key).unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate a command mark label: {error}");
            key.as_str().to_string()
        })
    }
}

/// Test access to the drafts and their error.
#[cfg(test)]
impl CommandMarksSetting {
    pub(crate) fn values(&self) -> CommandMarksDraft {
        self.draft
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}
