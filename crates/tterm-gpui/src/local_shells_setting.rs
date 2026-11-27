//! The local-shells section of the settings page and its editor.
//!
//! The section lists Auto, the shells found on this machine and the shells the
//! shared config file holds, makes one of them the default, and edits the
//! user's own shells one at a time. Saving is explicit: while the write is in
//! flight the inputs stay editable and a failure keeps the draft and shows the
//! error, so the user can retry without typing the shell again.

use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Div, Entity, FontWeight, KeyDownEvent, Role, Stateful, Subscription, Task,
    Window, div, px,
};
use tterm_core::config::local_shells::LocalShell;
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{TranslationKey, TranslationRequest};
use tterm_core::terminal::pty::check_local_shell_service;
use tterm_core::workspace::local_shells::default_shell::default_shell_key;
use tterm_core::workspace::local_shells::entries::build_shell_entries;
use tterm_core::workspace::local_shells::program_name;
use tterm_core::workspace::local_shells::saved::new_local_shell_id;
use tterm_core::workspace::local_shells::types::{ShellConfig, ShellEntry, ShellEntryKind};
use tterm_terminal::js_text::js_trim;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use self::path_status::PathStatus;
use self::picker::ProgramPicker;
use crate::config_state::ConfigState;
use crate::locale::Locale;

mod list_actions;
mod path_status;
mod picker;
#[cfg(test)]
mod tests;
mod view;

pub(crate) const PAGE_SECTION_ID: &str = "settings-local-shells-section";
pub(crate) const PAGE_ADD_ID: &str = "settings-local-shells-add";
pub(crate) const PAGE_EMPTY_ID: &str = "settings-local-shells-empty";
pub(crate) const EDITOR_ID: &str = "settings-local-shells-editor";
pub(crate) const PATH_INPUT_ID: &str = "settings-local-shells-path";
pub(crate) const BROWSE_ID: &str = "settings-local-shells-browse";
pub(crate) const NAME_INPUT_ID: &str = "settings-local-shells-name";
pub(crate) const ARGS_INPUT_ID: &str = "settings-local-shells-args";
pub(crate) const SAVE_ID: &str = "settings-local-shells-save";
pub(crate) const CANCEL_ID: &str = "settings-local-shells-cancel";
pub(crate) const ERROR_ID: &str = "settings-local-shells-error";
pub(crate) const ROW_PREFIX: &str = "settings-local-shell-row-";
pub(crate) const EDIT_PREFIX: &str = "settings-local-shell-edit-";
pub(crate) const DEFAULT_PREFIX: &str = "settings-local-shell-default-";
pub(crate) const DETECTED_PREFIX: &str = "settings-local-shell-detected-";
pub(crate) const HIDDEN_PREFIX: &str = "settings-local-shell-hidden-";
pub(crate) const COPY_PREFIX: &str = "settings-local-shell-copy-";
pub(crate) const VISIBILITY_PREFIX: &str = "settings-local-shell-visibility-";
pub(crate) const MOVE_UP_PREFIX: &str = "settings-local-shell-move-up-";
pub(crate) const MOVE_DOWN_PREFIX: &str = "settings-local-shell-move-down-";
pub(crate) const REMOVE_PREFIX: &str = "settings-local-shell-remove-";
pub(crate) const SET_DEFAULT_PREFIX: &str = "settings-local-shell-set-default-";
pub(crate) const LIST_ERROR_ID: &str = "settings-local-shells-write-error";
const MISSING_PREFIX: &str = "settings-local-shell-row-missing-";
/// The missing badge's rendered selector, shared by every row that has one.
pub(crate) const MISSING_ID: &str = "settings-local-shell-missing-badge";

const SECTION_GAP: f32 = 8.0;
const ROW_PAD_Y: f32 = 8.0;
const EDITOR_PADDING: f32 = 12.0;
/// Long enough that typing a path does not check every letter of it.
const PATH_CHECK_DELAY: Duration = Duration::from_millis(400);

/// The labels a row needs, translated once per render.
struct RowLabels<'a> {
    edit: &'a str,
    default_badge: &'a str,
    detected_badge: &'a str,
    hidden_badge: &'a str,
    make_default: &'a str,
    copy: &'a str,
    show: &'a str,
    hide: &'a str,
    move_up: &'a str,
    move_down: &'a str,
    remove: &'a str,
}

/// Where a row sits among all entries and among the user's own shells.
#[derive(Clone, Copy)]
struct RowSlot {
    entry: usize,
    custom: Option<usize>,
    custom_count: usize,
}

/// Every shell the local-shells list shows, labelled in `locale`.
///
/// The launcher shares this so both lists order, label and key their rows the
/// same way the core builds them.
pub(crate) fn shell_entries(config: &ConfigState, locale: &Locale) -> Vec<ShellEntry> {
    build_shell_entries(
        config.detected_shells(),
        &ShellConfig::from(config.config()),
        |key, fallback| translate_shell_label(locale, key, fallback),
    )
}

/// Translates one of the shell catalogue keys the core names while it builds
/// the entries. A key outside the registry, such as the placeholder for an
/// unknown shell id, keeps the core's fallback label.
fn translate_shell_label(locale: &Locale, key: &str, fallback: &str) -> String {
    let Some(key) = [
        keys::SHELL_LAUNCH_AUTO,
        keys::SHELL_LAUNCH_CMD,
        keys::SHELL_LAUNCH_POWERSHELL,
        keys::SHELL_LAUNCH_PWSH,
        keys::SHELL_LAUNCH_WSL,
        keys::SHELL_LAUNCH_GIT_BASH,
        keys::SHELL_LAUNCH_CUSTOM,
    ]
    .into_iter()
    .find(|candidate| candidate.as_str() == key) else {
        return fallback.to_string();
    };
    locale.text(key).unwrap_or_else(|error| {
        eprintln!("tterm-gpui: failed to translate a local shell label: {error}");
        fallback.to_string()
    })
}

/// The local-shells list and the editor for the shell being added or changed.
pub(crate) struct LocalShellsSetting {
    config: Entity<ConfigState>,
    locale: Entity<Locale>,
    theme: Theme,
    editor: Option<ShellDraft>,
    error: Option<String>,
    /// A write error from an action outside the editor, such as changing the
    /// default shell, so it stays visible when no draft is open.
    list_error: Option<String>,
    saving: bool,
    /// A row action's write, such as hiding or removing a shell.
    list_saving: bool,
    open: bool,
    /// What the saved paths and the editor's path were last found to be.
    paths: PathStatus,
    /// The editor's open program dialog, if any.
    picker: ProgramPicker,
    /// The editor's pending debounce and path check; dropping it cancels both.
    editor_check: Option<Task<()>>,
    /// The saved paths' pending checks for the latest batch.
    saved_checks: Vec<Task<()>>,
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
        cx.observe(&config, |this, _, cx| {
            this.refresh_saved_paths(cx);
            cx.notify();
        })
        .detach();
        cx.observe(&locale, |_, _, cx| cx.notify()).detach();
        Self {
            config,
            locale,
            theme,
            editor: None,
            error: None,
            list_error: None,
            saving: false,
            list_saving: false,
            open: false,
            paths: PathStatus::new(),
            picker: ProgramPicker::new(),
            editor_check: None,
            saved_checks: Vec::new(),
        }
    }

    /// Replaces the interface palette without touching a draft or focus.
    pub(crate) fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        cx.notify();
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
        self.list_error = None;
        self.saving = false;
        self.list_saving = false;
        self.picker.reset();
        self.editor_check = None;
        self.saved_checks.clear();
        self.paths.open();
        self.config
            .update(cx, |config, cx| config.discover_shells(cx));
        self.refresh_saved_paths(cx);
        cx.notify();
    }

    /// Hides the section. Returns `false` while a save is in flight so the
    /// route does not discard the pending write.
    pub(crate) fn close_page(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.open {
            return true;
        }
        if self.saving || self.list_saving {
            return false;
        }
        self.open = false;
        self.editor = None;
        self.error = None;
        self.list_error = None;
        self.picker.reset();
        self.editor_check = None;
        self.saved_checks.clear();
        self.paths.close();
        cx.notify();
        true
    }

    /// Whether the route may close without dropping a pending save.
    pub(crate) fn can_close(&self) -> bool {
        !self.saving && !self.list_saving
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
        self.picker.reset();
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
                let value = path.read(cx).value().to_string();
                this.begin_editor_check(&value, cx);
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
        self.begin_editor_check(&shell.path, cx);
        cx.notify();
    }

    /// Records the program field's text and checks it once typing pauses, so
    /// the editor says whether the path leads to a program before saving.
    fn begin_editor_check(&mut self, raw: &str, cx: &mut Context<Self>) {
        // Replacing the task cancels the previous timer and its unanswered
        // request, so only the newest text can publish.
        self.editor_check = None;
        let Some((generation, path)) = self.paths.edit(raw) else {
            return;
        };
        let runtime = self.config.read(cx).runtime().clone();
        self.editor_check = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PATH_CHECK_DELAY).await;
            let waits = this
                .update(cx, |this, _| this.paths.editor_waits(generation, &path))
                .unwrap_or(false);
            if !waits {
                return;
            }
            let result = runtime
                .spawn(check_local_shell_service(path.clone()))
                .await
                .unwrap_or_else(|error| Err(format!("the path check did not finish: {error}")));
            let _ = this.update(cx, |this, cx| {
                if this.paths.apply_editor(generation, &path, result) {
                    cx.notify();
                }
            });
        }));
    }

    /// Checks the saved shells' paths when the page opens or the list changes,
    /// and keeps their last answers for the rows to show as badges.
    fn refresh_saved_paths(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<String> = self
            .config
            .read(cx)
            .config()
            .local_shells
            .iter()
            .map(|shell| shell.path.clone())
            .collect();
        let wanted = self.paths.request_saved(&paths);
        if wanted.is_empty() {
            return;
        }
        self.saved_checks.clear();
        let generation = self.paths.saved_generation();
        let runtime = self.config.read(cx).runtime().clone();
        for path in wanted {
            let runtime = runtime.clone();
            self.saved_checks.push(cx.spawn(async move |this, cx| {
                let found = matches!(
                    runtime.spawn(check_local_shell_service(path.clone())).await,
                    Ok(Ok(_))
                );
                let _ = this.update(cx, |this, cx| {
                    if this.paths.apply_saved(generation, &path, found) {
                        cx.notify();
                    }
                });
            }));
        }
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
        self.clear_editor();
        cx.notify();
    }

    /// Drops the open draft without writing anything, so a copied shell that
    /// the user cancels never reaches the config file.
    fn clear_editor(&mut self) {
        self.editor = None;
        self.error = None;
        self.picker.reset();
        self.editor_check = None;
        self.paths.clear_editor();
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
            Ok(_) => self.clear_editor(),
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

    fn text(&self, cx: &App, key: TranslationKey) -> String {
        self.translated(cx, TranslationRequest::new(key))
    }

    fn translated(&self, cx: &App, request: TranslationRequest<'_>) -> String {
        self.locale
            .read(cx)
            .translate(request)
            .unwrap_or_else(|error| {
                eprintln!("tterm-gpui: failed to translate a local shell label: {error}");
                request.key.as_str().to_string()
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
        let edit = self.text(cx, keys::LOCAL_SHELLS_EDIT);
        let default_badge = self.text(cx, keys::LOCAL_SHELLS_DEFAULT);
        let detected_badge = self.text(cx, keys::LOCAL_SHELLS_DETECTED);
        let hidden_badge = self.text(cx, keys::LOCAL_SHELLS_HIDDEN);
        let make_default = self.text(cx, keys::LOCAL_SHELLS_MAKE_DEFAULT);
        let copy = self.text(cx, keys::LOCAL_SHELLS_COPY);
        let show = self.text(cx, keys::LOCAL_SHELLS_SHOW);
        let hide = self.text(cx, keys::LOCAL_SHELLS_HIDE);
        let move_up = self.text(cx, keys::LOCAL_SHELLS_MOVE_UP);
        let move_down = self.text(cx, keys::LOCAL_SHELLS_MOVE_DOWN);
        let remove = self.text(cx, keys::LOCAL_SHELLS_REMOVE);
        let labels = RowLabels {
            edit: &edit,
            default_badge: &default_badge,
            detected_badge: &detected_badge,
            hidden_badge: &hidden_badge,
            make_default: &make_default,
            copy: &copy,
            show: &show,
            hide: &hide,
            move_up: &move_up,
            move_down: &move_down,
            remove: &remove,
        };
        let (entries, default_key, no_custom_shells) = {
            let config = self.config.read(cx);
            let snapshot = config.config();
            (
                shell_entries(config, self.locale.read(cx)),
                default_shell_key(&ShellConfig::from(snapshot)),
                snapshot.local_shells.is_empty(),
            )
        };
        let editing = self.editor.as_ref().map(|draft| draft.id.clone());
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
        if let Some(error) = self.list_error.clone() {
            section = section.child(
                div()
                    .id(LIST_ERROR_ID)
                    .debug_selector(|| LIST_ERROR_ID.to_string())
                    .mb(px(SECTION_GAP))
                    .text_size(px(12.0))
                    .text_color(hsla(theme.colors.destructive, 1.0))
                    .child(error),
            );
        }
        let section = self.append_rows(
            section,
            cx,
            &entries,
            &default_key,
            editing.as_deref(),
            &labels,
        );
        self.append_trailing_editor(section, cx, &empty, no_custom_shells, editing.as_deref())
    }

    /// Appends one row per shell, opening the editor under the one being edited.
    fn append_rows(
        &mut self,
        mut section: Stateful<Div>,
        cx: &mut Context<Self>,
        entries: &[ShellEntry],
        default_key: &str,
        editing: Option<&str>,
        labels: &RowLabels<'_>,
    ) -> Stateful<Div> {
        let custom_count = entries
            .iter()
            .filter(|entry| entry.kind == ShellEntryKind::Custom)
            .count();
        let mut custom_index = 0;
        for (index, entry) in entries.iter().enumerate() {
            let slot = if entry.kind == ShellEntryKind::Custom {
                let slot = RowSlot {
                    entry: index,
                    custom: Some(custom_index),
                    custom_count,
                };
                custom_index += 1;
                slot
            } else {
                RowSlot {
                    entry: index,
                    custom: None,
                    custom_count,
                }
            };
            let is_default = entry.key == default_key;
            let is_editing =
                entry.kind == ShellEntryKind::Custom && entry.choice.shell_id.as_deref() == editing;
            let mut row = self.row(cx, slot, entry, is_default, is_editing, labels);
            if is_editing {
                row = row.child(self.editor_panel(cx));
            }
            section = section.child(row);
        }
        section
    }

    /// Shows the empty placeholder, and the editor for a draft the list lacks.
    fn append_trailing_editor(
        &mut self,
        mut section: Stateful<Div>,
        cx: &mut Context<Self>,
        empty: &str,
        no_custom_shells: bool,
        editing: Option<&str>,
    ) -> Stateful<Div> {
        if no_custom_shells && editing.is_none() {
            section = section.child(
                div()
                    .id(PAGE_EMPTY_ID)
                    .debug_selector(|| PAGE_EMPTY_ID.to_string())
                    .py(px(16.0))
                    .text_size(px(12.0))
                    .text_color(hsla(self.theme.colors.muted_foreground, 1.0))
                    .child(empty.to_string()),
            );
        }
        let editing_unsaved = editing.is_some_and(|id| {
            !self
                .config
                .read(cx)
                .config()
                .local_shells
                .iter()
                .any(|shell| shell.id == id)
        });
        if editing_unsaved {
            section = section.child(self.editor_panel(cx));
        }
        section
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
