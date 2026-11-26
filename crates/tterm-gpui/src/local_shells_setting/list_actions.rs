//! The writes the local-shells list starts from its rows.
//!
//! Every row action goes through the same pending flag and error slot as the
//! default-shell write. A failed action keeps the list and any open draft
//! untouched; a successful removal drops the draft for that shell only.

use gpui_kit::{Context, Window};
use tterm_core::config::local_shells::LocalShell;
use tterm_core::config::local_shells_file::ShellListEdit;
use tterm_core::workspace::local_shells::default_shell::default_shell_key;
use tterm_core::workspace::local_shells::entries::copied_shell_command;
use tterm_core::workspace::local_shells::saved::new_local_shell_id;
use tterm_core::workspace::local_shells::types::{LocalShellChoice, ShellConfig, ShellEntry};

use super::LocalShellsSetting;
use crate::config_state::LocalShellsChanged;

/// One list write, kept until its result decides what the rows show.
#[derive(Clone)]
enum ListWrite {
    Default(LocalShellChoice),
    Visibility { key: String, hidden: bool },
    Move { id: String, offset: i32 },
    Remove { id: String },
}

/// Whether a detected shell may be hidden: a shell that is already hidden can
/// always be shown again, and the visible default must stay in the list.
pub(super) fn can_hide(is_default: bool, hidden: bool) -> bool {
    hidden || !is_default
}

/// Whether a custom shell may move by `offset`, counting only custom shells.
pub(super) fn can_move_custom(index: usize, count: usize, offset: i32) -> bool {
    match offset {
        -1 => index > 0,
        1 => index + 1 < count,
        _ => false,
    }
}

impl LocalShellsSetting {
    /// Starts a draft copied from a detected shell, with a fresh id so saving
    /// it adds a shell instead of replacing the detected one.
    pub(super) fn copy_shell(
        &mut self,
        entry: &ShellEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.saving || self.list_saving {
            return;
        }
        let command = copied_shell_command(entry);
        self.start_editor(
            LocalShell {
                id: new_local_shell_id(),
                name: entry.label.clone(),
                path: command.path,
                args: command.args,
            },
            window,
            cx,
        );
    }

    /// Makes `choice` the shell a new tab starts.
    pub(super) fn set_default(&mut self, choice: LocalShellChoice, cx: &mut Context<Self>) {
        self.start_list_write(ListWrite::Default(choice), cx);
    }

    /// Hides or shows a detected shell. The visible default stays listed so a
    /// new tab always has a default row it can open.
    pub(super) fn set_hidden(&mut self, key: String, hidden: bool, cx: &mut Context<Self>) {
        let (is_default, currently_hidden) = {
            let config = self.config.read(cx).config();
            (
                default_shell_key(&ShellConfig::from(config)) == key,
                config.hidden_shells.iter().any(|other| other == &key),
            )
        };
        if hidden && !can_hide(is_default, currently_hidden) {
            return;
        }
        self.start_list_write(ListWrite::Visibility { key, hidden }, cx);
    }

    /// Moves one of the user's own shells by one place among the custom ones.
    pub(super) fn move_shell(&mut self, id: String, offset: i32, cx: &mut Context<Self>) {
        let (index, count) = {
            let shells = &self.config.read(cx).config().local_shells;
            (shells.iter().position(|shell| shell.id == id), shells.len())
        };
        let Some(index) = index else {
            return;
        };
        if !can_move_custom(index, count, offset) {
            return;
        }
        self.start_list_write(ListWrite::Move { id, offset }, cx);
    }

    /// Removes one of the user's own shells by its stable id.
    pub(super) fn remove_shell(&mut self, id: String, cx: &mut Context<Self>) {
        if !self
            .config
            .read(cx)
            .config()
            .local_shells
            .iter()
            .any(|shell| shell.id == id)
        {
            return;
        }
        self.start_list_write(ListWrite::Remove { id }, cx);
    }

    fn start_list_write(&mut self, write: ListWrite, cx: &mut Context<Self>) {
        if self.saving || self.list_saving {
            return;
        }
        self.list_saving = true;
        self.list_error = None;
        cx.notify();
        let task = match &write {
            ListWrite::Default(choice) => self.config.update(cx, |config, cx| {
                config.set_default_shell(choice.clone(), cx)
            }),
            ListWrite::Visibility { key, hidden } => {
                let edit = ShellListEdit::Visibility {
                    key: key.clone(),
                    hidden: *hidden,
                };
                self.config
                    .update(cx, |config, cx| config.edit_shell_list(edit, cx))
            }
            ListWrite::Move { id, offset } => {
                let edit = ShellListEdit::Move {
                    id: id.clone(),
                    offset: *offset,
                };
                self.config
                    .update(cx, |config, cx| config.edit_shell_list(edit, cx))
            }
            ListWrite::Remove { id } => {
                let edit = ShellListEdit::Remove { id: id.clone() };
                self.config
                    .update(cx, |config, cx| config.edit_shell_list(edit, cx))
            }
        };
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| this.finish_list_write(write, result, cx));
        })
        .detach();
    }

    /// Publishes a row action's result. The error stays visible without a
    /// draft, a failure preserves every draft, and a successful removal closes
    /// only the draft for the removed shell.
    fn finish_list_write(
        &mut self,
        write: ListWrite,
        result: Result<LocalShellsChanged, String>,
        cx: &mut Context<Self>,
    ) {
        self.list_saving = false;
        match result {
            Ok(_) => {
                self.list_error = None;
                if let ListWrite::Remove { id } = write
                    && self.editor.as_ref().is_some_and(|draft| draft.id == id)
                {
                    self.clear_editor();
                }
            }
            Err(message) => self.list_error = Some(message),
        }
        cx.notify();
    }
}
