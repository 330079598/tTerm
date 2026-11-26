//! Choosing a local shell's program with the platform's file dialog.
//!
//! The dialog answers after an arbitrary delay, and the user can open another
//! editor, save, or close the section while it is open. Every editor change
//! bumps a generation, so a late answer for a draft that is gone is ignored
//! instead of overwriting the editor the user is looking at.

use std::path::PathBuf;

use gpui_kit::component::input::InputState;
use gpui_kit::{Context, Entity, PathPromptOptions, Window};
use tterm_core::i18n::keys;

use super::LocalShellsSetting;

/// Which editor a pending program dialog belongs to.
pub(crate) struct ProgramPicker {
    generation: u64,
    open: bool,
    /// Answers handled so far, so a test can wait for a stale one to return.
    #[cfg(test)]
    finished: u32,
}

impl ProgramPicker {
    pub(crate) fn new() -> Self {
        Self {
            generation: 0,
            open: false,
            #[cfg(test)]
            finished: 0,
        }
    }

    /// Whether a dialog is open, so a second click cannot open another.
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    /// Invalidates any answer still in flight because the draft changed.
    pub(crate) fn reset(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.open = false;
    }

    /// Starts accepting the answer for one dialog.
    fn begin(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.open = true;
        self.generation
    }

    /// Records that an answer arrived. A stale answer leaves the newer
    /// dialog's open state alone.
    fn finish(&mut self, generation: u64) -> bool {
        #[cfg(test)]
        {
            self.finished += 1;
        }
        if generation != self.generation {
            return false;
        }
        self.open = false;
        true
    }

    /// How many answers arrived, including ones for a closed editor.
    #[cfg(test)]
    pub(crate) fn finished(&self) -> u32 {
        self.finished
    }
}

impl LocalShellsSetting {
    /// Opens the platform's file dialog and puts the chosen program in the
    /// draft. Nothing is written until the user saves the editor.
    pub(super) fn pick_program(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving || self.picker.is_open() {
            return;
        }
        let Some(draft) = &self.editor else {
            return;
        };
        let target = draft.path.clone();
        let title = self.text(cx, keys::LOCAL_SHELLS_PICK_PROGRAM);
        let generation = self.picker.begin();
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(title.into()),
        });
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let selected = match receiver.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            let _ = cx.update(|window, app| {
                let _ = this.update(app, |this, cx| {
                    this.apply_picked_program(generation, target, selected, window, cx);
                });
            });
        })
        .detach();
    }

    /// Puts a dialog answer into the draft it belongs to.
    fn apply_picked_program(
        &mut self,
        generation: u64,
        target: Entity<InputState>,
        selected: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.picker.finish(generation) {
            return;
        }
        // Cancelling, an error, and a save started while the dialog was open
        // all leave the draft untouched.
        let editing = !self.saving
            && self
                .editor
                .as_ref()
                .is_some_and(|draft| draft.path == target);
        if !editing {
            cx.notify();
            return;
        }
        let Some(path) = selected else {
            cx.notify();
            return;
        };
        let value = path.to_string_lossy().into_owned();
        if value.is_empty() {
            cx.notify();
            return;
        }
        // `set_value` does not emit a change event, so refresh the name
        // placeholder and the path check here, as typing would.
        target.update(cx, |state, cx| state.set_value(value.clone(), window, cx));
        self.refresh_name_placeholder(&target, window, cx);
        self.begin_editor_check(&value, cx);
        cx.notify();
    }
}
