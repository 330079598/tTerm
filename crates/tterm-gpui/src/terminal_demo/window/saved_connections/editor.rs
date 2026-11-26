//! The saved SSH connection editor's fields and draft rules.
//!
//! The editor changes only the address, group path and sign-in method. It
//! never collects a password: the connection runtime asks for one when a
//! session starts. Saving goes through [`super::profile`], which keeps
//! everything this form does not show untouched on an existing profile.

use std::sync::Arc;

use gpui_kit::component::input::InputState;
use gpui_kit::{App, AppContext as _, Context, Entity, Window};
use tterm_core::connections::profile_groups::normalize_group_path;
use tterm_core::i18n::keys;

use super::SavedConnectionsUi;
use super::profile::{AuthKind, ProfileDraft};

pub(super) const EDITOR_GROUP_ID: &str = "saved-connections-editor-group";

#[cfg(test)]
mod tests;
mod view;

/// The placeholders the picker translates before creating the fields.
pub(super) struct EditorPlaceholders {
    pub(super) name: String,
    pub(super) host: String,
    pub(super) username: String,
    pub(super) group_path: String,
}

/// A field the editor can focus after a validation error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EditorField {
    Name,
    Group,
    Host,
    Port,
    Username,
    KeyPath,
}

/// The editable text and sign-in method at the moment Save is pressed.
pub(super) struct EditorValues {
    pub(super) name: String,
    pub(super) host: String,
    pub(super) port: String,
    pub(super) username: String,
    pub(super) key_auth: bool,
    pub(super) key_path: String,
    pub(super) group_path: String,
    pub(super) group_changed: bool,
}

/// One open editor. Dropping it drops every field's input state.
pub(super) struct Editor {
    /// `None` creates a profile; `Some` edits that exact saved id.
    pub(super) id: Option<String>,
    pub(super) name: Entity<InputState>,
    pub(super) group_path: Entity<InputState>,
    pub(super) host: Entity<InputState>,
    pub(super) port: Entity<InputState>,
    pub(super) username: Entity<InputState>,
    pub(super) key_path: Entity<InputState>,
    /// The path the field held when editing loaded, for telling an untouched
    /// group from one the user typed or cleared.
    pub(super) loaded_group_path: String,
    pub(super) auth: AuthKind,
    pub(super) loading: bool,
    pub(super) load_failed: bool,
    pub(super) saving: bool,
    pub(super) error: Option<String>,
}

impl Editor {
    /// Builds a blank editor and focuses the name field.
    pub(super) fn new(
        id: Option<String>,
        placeholders: EditorPlaceholders,
        window: &mut Window,
        cx: &mut Context<super::SavedConnectionsUi>,
    ) -> Self {
        let name = new_input(window, cx);
        let group_path = new_input(window, cx);
        let host = new_input(window, cx);
        let port = new_input(window, cx);
        let username = new_input(window, cx);
        let key_path = new_input(window, cx);
        name.update(cx, |state, cx| {
            state.set_placeholder(placeholders.name, window, cx);
            state.focus(window, cx);
        });
        group_path.update(cx, |state, cx| {
            state.set_placeholder(placeholders.group_path, window, cx);
        });
        host.update(cx, |state, cx| {
            state.set_placeholder(placeholders.host, window, cx);
        });
        port.update(cx, |state, cx| {
            state.set_value("22", window, cx);
        });
        username.update(cx, |state, cx| {
            state.set_placeholder(placeholders.username, window, cx);
        });
        key_path.update(cx, |state, cx| {
            state.set_placeholder("~/.ssh/id_rsa", window, cx);
        });
        Self {
            id,
            name,
            group_path,
            host,
            port,
            username,
            key_path,
            loaded_group_path: String::new(),
            auth: AuthKind::Password,
            loading: false,
            load_failed: false,
            saving: false,
            error: None,
        }
    }

    /// Whether the password or the key-file sign-in method is selected.
    pub(super) fn is_key_auth(&self) -> bool {
        matches!(&self.auth, AuthKind::Key)
    }

    /// Reads trimmed field text, leaving the port default to [`build_draft`].
    pub(super) fn values(&self, cx: &App) -> EditorValues {
        let text = |state: &Entity<InputState>| state.read(cx).value().trim().to_string();
        let group_path = text(&self.group_path);
        EditorValues {
            name: text(&self.name),
            host: text(&self.host),
            port: text(&self.port),
            username: text(&self.username),
            key_auth: self.is_key_auth(),
            key_path: text(&self.key_path),
            group_changed: group_path_changed(&group_path, &self.loaded_group_path),
            group_path,
        }
    }

    /// Fills the fields from a loaded profile and moves focus to the name.
    pub(super) fn apply_draft(
        &mut self,
        draft: ProfileDraft,
        window: &mut Window,
        cx: &mut Context<super::SavedConnectionsUi>,
    ) {
        self.name
            .update(cx, |state, cx| state.set_value(draft.name, window, cx));
        self.loaded_group_path = draft.group_path.clone();
        self.group_path.update(cx, |state, cx| {
            state.set_value(draft.group_path, window, cx)
        });
        self.host
            .update(cx, |state, cx| state.set_value(draft.host, window, cx));
        let port = if draft.port.trim().is_empty() {
            "22".to_string()
        } else {
            draft.port
        };
        self.port
            .update(cx, |state, cx| state.set_value(port, window, cx));
        self.username
            .update(cx, |state, cx| state.set_value(draft.username, window, cx));
        self.key_path
            .update(cx, |state, cx| state.set_value(draft.key_path, window, cx));
        self.auth = draft.auth;
        self.loading = false;
        self.load_failed = false;
        self.focus(EditorField::Name, window, cx);
    }

    /// Returns the input that holds one field's text.
    pub(super) fn field_input(&self, field: EditorField) -> &Entity<InputState> {
        match field {
            EditorField::Name => &self.name,
            EditorField::Group => &self.group_path,
            EditorField::Host => &self.host,
            EditorField::Port => &self.port,
            EditorField::Username => &self.username,
            EditorField::KeyPath => &self.key_path,
        }
    }

    /// Moves keyboard focus to one field.
    pub(super) fn focus(
        &self,
        field: EditorField,
        window: &mut Window,
        cx: &mut Context<super::SavedConnectionsUi>,
    ) {
        self.field_input(field)
            .update(cx, |state, cx| state.focus(window, cx));
    }
}

/// The first field that stops a save, in the order the model validates them.
pub(super) fn invalid_field(values: &EditorValues) -> Option<EditorField> {
    if values.host.is_empty() {
        return Some(EditorField::Host);
    }
    if values.username.is_empty() {
        return Some(EditorField::Username);
    }
    let port = normalized_port(values);
    if port.parse::<u16>().ok().filter(|port| *port > 0).is_none() {
        return Some(EditorField::Port);
    }
    if values.key_auth && values.key_path.is_empty() {
        return Some(EditorField::KeyPath);
    }
    None
}

/// Builds the model draft, keeping an existing id and a hidden key path so an
/// edit does not rewrite fields the user did not change.
pub(super) fn build_draft(id: Option<String>, values: EditorValues) -> ProfileDraft {
    let port = normalized_port(&values);
    ProfileDraft {
        id,
        name: values.name,
        host: values.host,
        port,
        username: values.username,
        auth: if values.key_auth {
            AuthKind::Key
        } else {
            AuthKind::Password
        },
        key_path: values.key_path,
        group_path: values.group_path,
        group_changed: values.group_changed,
    }
}

/// Whether a typed path names a different group than the one editing loaded.
/// The shared rules make ` Prod /Web ` and `Prod / Web` the same group.
fn group_path_changed(path: &str, loaded: &str) -> bool {
    normalize_group_path(path) != normalize_group_path(loaded)
}

fn normalized_port(values: &EditorValues) -> String {
    if values.port.is_empty() {
        "22".to_string()
    } else {
        values.port.clone()
    }
}

fn new_input(
    window: &mut Window,
    cx: &mut Context<super::SavedConnectionsUi>,
) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx))
}

impl SavedConnectionsUi {
    /// Opens the blank form from the picker, keeping the picker beneath it.
    pub(super) fn open_new_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_editor(None, window, cx);
    }

    /// Edits the row the picker currently has selected.
    pub(super) fn open_edit_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        self.open_edit_editor_for(id, window, cx);
    }

    /// Edits one exact saved id; stale row ids are ignored.
    pub(super) fn open_edit_editor_for(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.profiles.iter().any(|profile| profile.id == id) {
            return;
        }
        self.start_editor(Some(id), window, cx);
    }

    fn start_editor(&mut self, id: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open || self.opening || self.editor_saving() {
            return;
        }
        let placeholders = EditorPlaceholders {
            name: self.text(cx, keys::CONNECTION_TITLE),
            host: self.text(cx, keys::CONNECTION_HOST_PLACEHOLDER),
            username: self.text(cx, keys::CONNECTION_USERNAME_PLACEHOLDER),
            group_path: self.text(cx, keys::CONNECTION_GROUP_PLACEHOLDER),
        };
        let editing = id.is_some();
        let mut editor = Editor::new(id.clone(), placeholders, window, cx);
        editor.loading = editing;
        self.editor = Some(editor);
        self.editor_generation = self.editor_generation.wrapping_add(1);
        let generation = self.editor_generation;
        // A load must not leave the keyboard in an empty field that is about
        // to be replaced. The newly loaded form focuses the name field.
        if editing {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
        let Some(id) = id else {
            return;
        };
        #[cfg(test)]
        if let Some(result) = self.injected_editor_load.clone() {
            self.finish_editor_load(generation, &id, result, window, cx);
            return;
        }
        let services = Arc::clone(&self.services);
        cx.spawn_in(window, async move |this, cx| {
            let result = super::profile::load(services.as_ref(), id.clone()).await;
            let _ = cx.update(|window, app| {
                let _ = this.update(app, |this, cx| {
                    this.finish_editor_load(generation, &id, result, window, cx);
                });
            });
        })
        .detach();
    }

    fn finish_editor_load(
        &mut self,
        generation: u64,
        id: &str,
        result: Result<ProfileDraft, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.open || generation != self.editor_generation {
            return;
        }
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        if editor.id.as_deref() != Some(id) {
            return;
        }
        match result {
            Ok(draft) => {
                editor.error = None;
                editor.apply_draft(draft, window, cx);
            }
            Err(message) => {
                eprintln!(
                    "tterm-gpui: failed to load a saved SSH connection for editing: {message}"
                );
                editor.loading = false;
                editor.load_failed = true;
                editor.error = Some(message);
                self.focus_handle.focus(window, cx);
            }
        }
        cx.notify();
    }

    /// Returns to the list without writing anything, keeping its selection.
    pub(super) fn cancel_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor_saving() || self.editor.is_none() {
            return;
        }
        self.editor = None;
        self.editor_generation = self.editor_generation.wrapping_add(1);
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    /// Switches between password and key-file sign-in without collecting a
    /// secret; the hidden key path is kept so an edit does not erase it.
    pub(super) fn set_editor_auth(&mut self, auth: AuthKind, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        if editor.loading || editor.load_failed || editor.saving {
            return;
        }
        editor.auth = auth;
        editor.error = None;
        cx.notify();
    }

    /// Validates the visible fields once and saves through the profile model.
    pub(super) fn submit_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        if editor.loading || editor.load_failed || editor.saving {
            return;
        }
        let values = editor.values(cx);
        if let Some(field) = invalid_field(&values) {
            let message = match field {
                // A group path needs no validation; the shared resolver trims
                // it and treats an empty one as ungrouped.
                EditorField::Group | EditorField::Host | EditorField::Name => {
                    self.text(cx, keys::SAVED_CONNECTIONS_HOST_REQUIRED)
                }
                EditorField::Username => self.text(cx, keys::SAVED_CONNECTIONS_USERNAME_REQUIRED),
                EditorField::Port => self.text(cx, keys::SAVED_CONNECTIONS_PORT_INVALID),
                EditorField::KeyPath => self.text(cx, keys::SAVED_CONNECTIONS_KEY_PATH_REQUIRED),
            };
            if let Some(editor) = self.editor.as_mut() {
                editor.error = Some(message);
                editor.focus(field, window, cx);
            }
            cx.notify();
            return;
        }
        let id = self.editor.as_ref().and_then(|editor| editor.id.clone());
        let draft = build_draft(id, values);
        let generation = self.editor_generation;
        if let Some(editor) = self.editor.as_mut() {
            editor.saving = true;
            editor.error = None;
        }
        cx.notify();
        #[cfg(test)]
        if let Some(result) = self.injected_editor_save.clone() {
            self.finish_editor_save(generation, result, window, cx);
            return;
        }
        let services = Arc::clone(&self.services);
        cx.spawn_in(window, async move |this, cx| {
            let result = super::profile::save(services.as_ref(), draft).await;
            let _ = cx.update(|window, app| {
                let _ = this.update(app, |this, cx| {
                    this.finish_editor_save(generation, result, window, cx);
                });
            });
        })
        .detach();
    }

    fn finish_editor_save(
        &mut self,
        generation: u64,
        result: Result<String, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.open || generation != self.editor_generation {
            return;
        }
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        editor.saving = false;
        match result {
            Ok(id) => {
                self.editor = None;
                self.editor_generation = self.editor_generation.wrapping_add(1);
                self.error = None;
                self.loading = true;
                self.select_after_load = Some(id);
                self.focus_handle.focus(window, cx);
                cx.notify();
                self.load(window, cx);
            }
            Err(message) => {
                eprintln!("tterm-gpui: failed to save a saved SSH connection: {message}");
                editor.error = Some(message);
                editor.focus(EditorField::Name, window, cx);
                cx.notify();
            }
        }
    }

    pub(super) fn editor_saving(&self) -> bool {
        self.editor.as_ref().is_some_and(|editor| editor.saving)
    }

    #[cfg(test)]
    pub(super) fn set_editor_load_for_test(&mut self, result: Result<ProfileDraft, String>) {
        self.injected_editor_load = Some(result);
    }

    #[cfg(test)]
    pub(super) fn set_editor_save_for_test(&mut self, result: Result<String, String>) {
        self.injected_editor_save = Some(result);
    }

    #[cfg(test)]
    pub(super) fn editor_is_open(&self) -> bool {
        self.editor.is_some()
    }

    #[cfg(test)]
    pub(super) fn editor_id_for_test(&self) -> Option<&str> {
        self.editor.as_ref().and_then(|editor| editor.id.as_deref())
    }

    #[cfg(test)]
    pub(super) fn editor_values_for_test(&self, cx: &App) -> Option<EditorValues> {
        self.editor.as_ref().map(|editor| editor.values(cx))
    }

    #[cfg(test)]
    pub(super) fn focus_editor_field_for_test(
        &mut self,
        field: EditorField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = self.editor.as_ref() {
            editor.focus(field, window, cx);
        }
    }

    #[cfg(test)]
    pub(super) fn editor_name_focused_for_test(&self, window: &Window, cx: &App) -> bool {
        self.editor.as_ref().is_some_and(|editor| {
            editor
                .name
                .read(cx)
                .presentation()
                .focus_handle()
                .is_focused(window)
        })
    }

    #[cfg(test)]
    pub(super) fn editor_error_for_test(&self) -> Option<&str> {
        self.editor
            .as_ref()
            .and_then(|editor| editor.error.as_deref())
    }
}
