//! The local-shells section of the settings page and its editor.
//!
//! The section lists the shells the shared config file holds and edits one at
//! a time. Saving is explicit: while the write is in flight the inputs stay
//! editable and a failure keeps the draft and shows the error, so the user can
//! retry without typing the shell again.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Div, Entity, FontWeight, KeyDownEvent, Role, Stateful, Subscription, Window, div,
    px,
};
use tterm_core::config::local_shells::LocalShell;
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_core::workspace::local_shells::entries::build_shell_entries;
use tterm_core::workspace::local_shells::program_name;
use tterm_core::workspace::local_shells::saved::new_local_shell_id;
use tterm_core::workspace::local_shells::types::{ShellConfig, ShellEntry, ShellEntryKind};
use tterm_terminal::js_text::js_trim;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::input::{InputOptions, input};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use crate::config_state::ConfigState;
use crate::locale::Locale;

#[cfg(test)]
mod tests;

pub(crate) const PAGE_SECTION_ID: &str = "settings-local-shells-section";
pub(crate) const PAGE_ADD_ID: &str = "settings-local-shells-add";
pub(crate) const PAGE_EMPTY_ID: &str = "settings-local-shells-empty";
pub(crate) const EDITOR_ID: &str = "settings-local-shells-editor";
pub(crate) const PATH_INPUT_ID: &str = "settings-local-shells-path";
pub(crate) const NAME_INPUT_ID: &str = "settings-local-shells-name";
pub(crate) const ARGS_INPUT_ID: &str = "settings-local-shells-args";
pub(crate) const SAVE_ID: &str = "settings-local-shells-save";
pub(crate) const CANCEL_ID: &str = "settings-local-shells-cancel";
pub(crate) const ERROR_ID: &str = "settings-local-shells-error";
pub(crate) const ROW_PREFIX: &str = "settings-local-shell-row-";
pub(crate) const EDIT_PREFIX: &str = "settings-local-shell-edit-";

const SECTION_GAP: f32 = 8.0;
const ROW_PAD_Y: f32 = 8.0;
const EDITOR_PADDING: f32 = 12.0;

/// The local-shells list and the editor for the shell being added or changed.
pub(crate) struct LocalShellsSetting {
    config: Entity<ConfigState>,
    locale: Entity<Locale>,
    theme: Theme,
    editor: Option<ShellDraft>,
    error: Option<String>,
    saving: bool,
    open: bool,
}

/// One shell's draft fields, kept together so closing the editor drops them.
struct ShellDraft {
    id: String,
    path: Entity<InputState>,
    name: Entity<InputState>,
    args: Entity<InputState>,
    _path_events: Subscription,
}

impl LocalShellsSetting {
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
            editor: None,
            error: None,
            saving: false,
            open: false,
        }
    }

    /// Shows the list for this settings visit and drops a draft left from the
    /// previous one.
    pub(crate) fn open_page(&mut self, cx: &mut Context<Self>) {
        if self.open {
            return;
        }
        self.open = true;
        self.editor = None;
        self.error = None;
        self.saving = false;
        cx.notify();
    }

    /// Hides the section. Returns `false` while a save is in flight so the
    /// route does not discard the pending write.
    pub(crate) fn close_page(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.open {
            return true;
        }
        if self.saving {
            return false;
        }
        self.open = false;
        self.editor = None;
        self.error = None;
        cx.notify();
        true
    }

    /// Whether the route may close without dropping a pending save.
    pub(crate) fn can_close(&self) -> bool {
        !self.saving
    }

    /// Puts the editor's program field in focus when a search result opens.
    pub(crate) fn focus_page_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(draft) = &self.editor {
            draft.path.update(cx, |state, cx| state.focus(window, cx));
        }
    }

    /// Refreshes the name placeholder after a language change.
    pub(crate) fn refresh_placeholders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = &self.editor else {
            return;
        };
        let path = draft.path.read(cx).value().to_string();
        let placeholder = self.name_placeholder(&path, cx);
        draft.name.update(cx, |state, cx| {
            state.set_placeholder(placeholder, window, cx);
        });
    }

    fn start_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_editor(
            LocalShell {
                id: new_local_shell_id(),
                name: String::new(),
                path: String::new(),
                args: String::new(),
            },
            window,
            cx,
        );
    }

    fn start_edit(&mut self, shell: LocalShell, window: &mut Window, cx: &mut Context<Self>) {
        self.start_editor(shell, window, cx);
    }

    fn start_editor(&mut self, shell: LocalShell, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        let path = cx.new(|cx| InputState::new(window, cx));
        let name = cx.new(|cx| InputState::new(window, cx));
        let args = cx.new(|cx| InputState::new(window, cx));
        path.update(cx, |state, cx| {
            state.set_value(shell.path.clone(), window, cx);
            state.focus(window, cx);
        });
        let placeholder = self.name_placeholder(&shell.path, cx);
        name.update(cx, |state, cx| {
            state.set_value(shell.name.clone(), window, cx);
            state.set_placeholder(placeholder, window, cx);
        });
        args.update(cx, |state, cx| {
            state.set_value(shell.args.clone(), window, cx)
        });
        let path_events = cx.subscribe_in(&path, window, |this, path, event, window, cx| {
            if matches!(event, InputEvent::Change) {
                this.refresh_name_placeholder(path, window, cx);
                cx.notify();
            }
        });
        self.editor = Some(ShellDraft {
            id: shell.id,
            path,
            name,
            args,
            _path_events: path_events,
        });
        cx.notify();
    }

    fn refresh_name_placeholder(
        &mut self,
        path: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(draft) = &self.editor else {
            return;
        };
        let value = path.read(cx).value().to_string();
        let placeholder = self.name_placeholder(&value, cx);
        draft.name.update(cx, |state, cx| {
            state.set_placeholder(placeholder, window, cx);
        });
    }

    fn name_placeholder(&self, path: &str, cx: &App) -> String {
        let name = program_name(path);
        if name.is_empty() {
            self.text(cx, keys::LOCAL_SHELLS_NAME_PLACEHOLDER)
        } else {
            name
        }
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.editor = None;
        self.error = None;
        cx.notify();
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(draft) = &self.editor else {
            return;
        };
        let path = js_trim(draft.path.read(cx).value().as_ref()).to_string();
        if path.is_empty() {
            self.error = Some(self.text(cx, keys::LOCAL_SHELLS_PROGRAM_REQUIRED));
            draft.path.update(cx, |state, cx| state.focus(window, cx));
            cx.notify();
            return;
        }
        let shell = LocalShell {
            id: draft.id.clone(),
            name: js_trim(draft.name.read(cx).value().as_ref()).to_string(),
            path,
            args: js_trim(draft.args.read(cx).value().as_ref()).to_string(),
        };
        self.saving = true;
        self.error = None;
        cx.notify();
        let task = self
            .config
            .update(cx, |config, cx| config.save_local_shell(shell, cx));
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
        result: Result<crate::config_state::LocalShellsChanged, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.saving = false;
        match result {
            Ok(_) => {
                self.editor = None;
                self.error = None;
            }
            Err(message) => {
                self.error = Some(message);
                self.focus_page_input(window, cx);
            }
        }
        cx.notify();
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.is_none() {
            return;
        }
        match event.keystroke.key.as_str() {
            "escape" => {
                window.prevent_default();
                cx.stop_propagation();
                self.cancel(cx);
            }
            "enter" if !self.saving => {
                window.prevent_default();
                cx.stop_propagation();
                self.submit(window, cx);
            }
            _ => {}
        }
    }

    fn entries(&self, cx: &App) -> (Vec<LocalShell>, Vec<ShellEntry>) {
        let config = self.config.read(cx).config();
        let shells = config.local_shells.clone();
        let entries = build_shell_entries(&[], &ShellConfig::from(config), |_, fallback| {
            fallback.to_string()
        })
        .into_iter()
        .filter(|entry| entry.kind == ShellEntryKind::Custom)
        .collect();
        (shells, entries)
    }

    fn text(&self, cx: &App, key: TranslationKey) -> String {
        self.locale.read(cx).text(key).unwrap_or_else(|error| {
            eprintln!("tterm-gpui: failed to translate a local shell label: {error}");
            key.as_str().to_string()
        })
    }

    /// Renders the section inside the settings page's scroll area.
    pub(crate) fn section(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let title = self.text(cx, keys::LOCAL_SHELLS_TITLE);
        let description = self.text(cx, keys::LOCAL_SHELLS_DESCRIPTION);
        let list_title = self.text(cx, keys::LOCAL_SHELLS_LIST_TITLE);
        let list_description = self.text(cx, keys::LOCAL_SHELLS_LIST_DESCRIPTION);
        let add = self.text(cx, keys::LOCAL_SHELLS_ADD);
        let empty = self.text(cx, keys::LOCAL_SHELLS_EMPTY);
        let (shells, entries) = self.entries(cx);
        let editing = self.editor.as_ref().map(|draft| draft.id.clone());
        let edit = self.text(cx, keys::LOCAL_SHELLS_EDIT);
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
                    .mb(px(4.0))
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(theme.colors.muted_foreground, 1.0))
                    .mb(px(16.0))
                    .child(description),
            )
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap(px(12.0))
                    .mb(px(SECTION_GAP))
                    .child(
                        div()
                            .min_w(px(0.0))
                            .flex_1()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(500.0))
                                    .child(list_title),
                            )
                            .child(
                                div()
                                    .mt(px(2.0))
                                    .text_size(px(12.0))
                                    .line_height(px(16.0))
                                    .text_color(hsla(theme.colors.muted_foreground, 1.0))
                                    .child(list_description),
                            ),
                    )
                    .child(
                        button(
                            PAGE_ADD_ID,
                            add.clone(),
                            Variant::Outline,
                            ButtonSize::Sm,
                            ButtonState::default(),
                            theme,
                        )
                        .debug_selector(|| PAGE_ADD_ID.to_string())
                        .role(Role::Button)
                        .aria_label(add)
                        .on_click(cx.listener(|this, _, window, cx| this.start_add(window, cx))),
                    ),
            );
        if shells.is_empty() && editing.is_none() {
            section = section.child(
                div()
                    .id(PAGE_EMPTY_ID)
                    .debug_selector(|| PAGE_EMPTY_ID.to_string())
                    .py(px(16.0))
                    .text_size(px(12.0))
                    .text_color(hsla(theme.colors.muted_foreground, 1.0))
                    .child(empty),
            );
        }
        for (index, shell) in shells.iter().enumerate() {
            let entry = entries.iter().find(|entry| {
                entry.key == tterm_core::workspace::local_shells::custom_shell_key(&shell.id)
            });
            let is_editing = editing.as_deref() == Some(&shell.id);
            let mut row = self.row(cx, index, shell, entry, is_editing, &edit);
            if is_editing {
                row = row.child(self.editor_panel(cx));
            }
            section = section.child(row);
        }
        let editing_unsaved = editing.is_some()
            && shells.iter().all(|shell| {
                self.editor
                    .as_ref()
                    .is_some_and(|draft| draft.id != shell.id)
            });
        if editing_unsaved {
            section = section.child(self.editor_panel(cx));
        }
        section
    }

    fn row(
        &self,
        cx: &mut Context<Self>,
        index: usize,
        shell: &LocalShell,
        entry: Option<&ShellEntry>,
        editing: bool,
        edit: &str,
    ) -> Stateful<Div> {
        let colors = self.theme.colors;
        let label = entry.map_or_else(
            || {
                let name = js_trim(&shell.name);
                if name.is_empty() {
                    program_name(&shell.path)
                } else {
                    name.to_string()
                }
            },
            |entry| entry.label.clone(),
        );
        let detail = entry.map_or_else(
            || js_trim(&shell.path).to_string(),
            |entry| entry.detail.clone(),
        );
        let id = shell.id.clone();
        let config = self.config.clone();
        let row_selector = format!("{ROW_PREFIX}{index}");
        let edit_selector = format!("{EDIT_PREFIX}{index}");
        div()
            .id(row_selector.clone())
            .debug_selector(move || row_selector.clone())
            .flex()
            .flex_col()
            .py(px(ROW_PAD_Y))
            .border_b(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .when(editing, |row| row.bg(hsla(colors.primary, 0.05)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .min_w(px(0.0))
                            .flex_1()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(500.0))
                                    .child(label),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(12.0))
                                    .text_color(hsla(colors.muted_foreground, 1.0))
                                    .child(detail),
                            ),
                    )
                    .child(
                        button(
                            edit_selector.clone(),
                            edit.to_string(),
                            Variant::Ghost,
                            ButtonSize::Xs,
                            ButtonState::default(),
                            self.theme,
                        )
                        .debug_selector(move || edit_selector.clone())
                        .role(Role::Button)
                        .aria_label(edit.to_string())
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                // Resolve the shell again so the editor opens the
                                // latest values rather than a row captured at render.
                                let shell = config
                                    .read(cx)
                                    .config()
                                    .local_shells
                                    .iter()
                                    .find(|shell| shell.id == id)
                                    .cloned();
                                if let Some(shell) = shell {
                                    this.start_edit(shell, window, cx);
                                }
                            },
                        )),
                    ),
            )
    }

    fn editor_panel(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(draft) = &self.editor else {
            return div().id("settings-local-shells-no-editor");
        };
        let theme = self.theme;
        let program = self.text(cx, keys::LOCAL_SHELLS_PROGRAM);
        let name = self.text(cx, keys::LOCAL_SHELLS_NAME);
        let args = self.text(cx, keys::LOCAL_SHELLS_ARGS);
        let save = self.text(cx, keys::LOCAL_SHELLS_SAVE);
        let cancel = self.text(cx, keys::LOCAL_SHELLS_CANCEL);
        let path_input = draft.path.clone();
        let name_input = draft.name.clone();
        let args_input = draft.args.clone();
        let error = self.error.clone();
        let state = ButtonState {
            disabled: self.saving,
            ..ButtonState::default()
        };
        let mut panel = div()
            .id(EDITOR_ID)
            .debug_selector(|| EDITOR_ID.to_string())
            .mt(px(SECTION_GAP))
            .mb(px(SECTION_GAP))
            .p(px(EDITOR_PADDING))
            .rounded(px(6.0))
            .border(px(1.0))
            .border_color(hsla(theme.colors.border, 1.0))
            .bg(hsla(theme.colors.muted, 0.4))
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            .child(field(theme, program, PATH_INPUT_ID, &path_input))
            .child(
                div()
                    .flex()
                    .gap(px(SECTION_GAP))
                    .child(div().min_w(px(0.0)).flex_1().child(field(
                        theme,
                        name,
                        NAME_INPUT_ID,
                        &name_input,
                    )))
                    .child(div().min_w(px(0.0)).flex_1().child(field(
                        theme,
                        args,
                        ARGS_INPUT_ID,
                        &args_input,
                    ))),
            );
        if let Some(error) = error {
            panel = panel.child(
                div()
                    .id(ERROR_ID)
                    .debug_selector(|| ERROR_ID.to_string())
                    .text_size(px(12.0))
                    .text_color(hsla(theme.colors.destructive, 1.0))
                    .child(error),
            );
        }
        panel.child(
            div()
                .flex()
                .justify_end()
                .gap(px(SECTION_GAP))
                .child(
                    button(
                        CANCEL_ID,
                        cancel,
                        Variant::Ghost,
                        ButtonSize::Sm,
                        ButtonState::default(),
                        theme,
                    )
                    .debug_selector(|| CANCEL_ID.to_string())
                    .role(Role::Button)
                    .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                )
                .child(
                    button(
                        SAVE_ID,
                        save.clone(),
                        Variant::Default,
                        ButtonSize::Sm,
                        state,
                        theme,
                    )
                    .debug_selector(|| SAVE_ID.to_string())
                    .role(Role::Button)
                    .aria_label(save)
                    .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                ),
        )
    }

    /// The draft's current field values, for the route tests.
    #[cfg(test)]
    pub(crate) fn editor_values(&self, cx: &App) -> Option<(String, String, String)> {
        self.editor.as_ref().map(|draft| {
            (
                draft.path.read(cx).value().to_string(),
                draft.name.read(cx).value().to_string(),
                draft.args.read(cx).value().to_string(),
            )
        })
    }

    /// The save error, for the route tests.
    #[cfg(test)]
    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn is_saving(&self) -> bool {
        self.saving
    }
}

fn field(theme: Theme, label: String, id: &'static str, state: &Entity<InputState>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight(500.0))
                .child(label),
        )
        .child(
            input(
                state,
                InputOptions {
                    theme: Some(theme),
                    ..InputOptions::default()
                },
            )
            .id(id)
            .debug_selector(move || id.to_string()),
        )
}
