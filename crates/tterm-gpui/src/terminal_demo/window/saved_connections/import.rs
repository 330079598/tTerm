//! Importing saved SSH connections from an OpenSSH config file.
//!
//! The dialog previews the config with the same core parser the Tauri client
//! uses, lets the user choose hosts, and files them in one group. Reading and
//! writing stay in core; this module only holds the dialog's state.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::input::InputState;
use gpui_kit::{App, AppContext as _, Context, Entity, PathPromptOptions, ScrollHandle, Window};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationRequest;
use tterm_core::profiles::import::import_ssh_config_profiles_blocking;
use tterm_core::profiles::parser::build_ssh_config_import_preview;
use tterm_core::profiles::types::{
    SshConfigImportHost, SshConfigImportOptions, SshConfigImportPreview, SshConfigImportResult,
};

use super::SavedConnectionsUi;

pub(super) const IMPORT_BUTTON_ID: &str = "saved-connections-import";
pub(super) const IMPORT_DIALOG_ID: &str = "saved-connections-import-dialog";
pub(super) const IMPORT_SOURCE_ID: &str = "saved-connections-import-source";
pub(super) const IMPORT_CHOOSE_FILE_ID: &str = "saved-connections-import-choose-file";
pub(super) const IMPORT_CHOOSE_DIRECTORY_ID: &str = "saved-connections-import-choose-directory";
pub(super) const IMPORT_GROUP_ID: &str = "saved-connections-import-group";
pub(super) const IMPORT_PREVIEW_ID: &str = "saved-connections-import-preview";
pub(super) const IMPORT_LIST_ID: &str = "saved-connections-import-list";
pub(super) const IMPORT_SELECT_ALL_ID: &str = "saved-connections-import-select-all";
pub(super) const IMPORT_FORWARDS_ID: &str = "saved-connections-import-forwards";
pub(super) const IMPORT_OVERWRITE_ID: &str = "saved-connections-import-overwrite";
pub(super) const IMPORT_CONFIRM_ID: &str = "saved-connections-import-confirm";
pub(super) const IMPORT_CANCEL_ID: &str = "saved-connections-import-cancel";
pub(super) const IMPORT_LOADING_ID: &str = "saved-connections-import-loading";
pub(super) const IMPORT_ERROR_ID: &str = "saved-connections-import-error";
pub(super) const IMPORT_EMPTY_ID: &str = "saved-connections-import-empty";
pub(super) const IMPORT_WARNINGS_ID: &str = "saved-connections-import-warnings";
pub(super) const IMPORT_COUNT_ID: &str = "saved-connections-import-count";
pub(super) const IMPORT_ROW_PREFIX: &str = "saved-connections-import-row-";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ImportSourceKind {
    File,
    Directory,
}

/// One open import dialog. Dropping it drops both text fields.
pub(super) struct ImportDialog {
    pub(super) source: Entity<InputState>,
    pub(super) group: Entity<InputState>,
    pub(super) preview: Option<SshConfigImportPreview>,
    /// Host patterns to import. The core treats an empty list as "all", so the
    /// dialog only sends the explicit choices.
    pub(super) selected: HashSet<String>,
    pub(super) loading: bool,
    pub(super) importing: bool,
    pub(super) error: Option<String>,
    pub(super) import_forwards: bool,
    pub(super) overwrite: bool,
    pub(super) scroll: ScrollHandle,
    /// Identifies this dialog across preview generation changes.
    pub(super) instance: u64,
    /// Invalidates a preview or import that finishes after the dialog closed.
    pub(super) generation: u64,
    /// Prevents a second native picker while one is already open.
    pub(super) picker_open: bool,
    /// The path the picker wrote, until the user edits the source field.
    pub(super) picked_source: Option<String>,
}

impl ImportDialog {
    fn new(
        default_group: String,
        source_placeholder: String,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<SavedConnectionsUi>,
    ) -> Self {
        let source = cx.new(|cx| InputState::new(window, cx));
        let group = cx.new(|cx| InputState::new(window, cx));
        source.update(cx, |state, cx| {
            state.set_placeholder(source_placeholder, window, cx);
            state.focus(window, cx);
        });
        group.update(cx, |state, cx| {
            state.set_value(default_group, window, cx);
        });
        Self {
            source,
            group,
            preview: None,
            selected: HashSet::new(),
            loading: false,
            importing: false,
            error: None,
            import_forwards: true,
            overwrite: false,
            scroll: ScrollHandle::default(),
            instance: generation,
            generation,
            picker_open: false,
            picked_source: None,
        }
    }

    /// The host entries that can be selected, in preview order.
    pub(super) fn selectable(&self) -> impl Iterator<Item = &SshConfigImportHost> {
        self.preview
            .iter()
            .flat_map(|preview| preview.hosts.iter())
            .filter(|host| !host.skipped)
    }

    pub(super) fn selectable_count(&self) -> usize {
        self.selectable().count()
    }

    pub(super) fn forward_count(&self) -> usize {
        self.selectable()
            .filter(|host| self.selected.contains(&host.host_pattern))
            .map(|host| host.forwards.len())
            .sum()
    }

    pub(super) fn all_selected(&self) -> bool {
        let count = self.selectable_count();
        count > 0 && self.selected.len() == count
    }

    /// What one host is, for the row's second line.
    pub(super) fn endpoint(&self, host: &SshConfigImportHost) -> String {
        let address = match (&host.username, &host.host) {
            (Some(username), Some(name)) => format!("{username}@{name}"),
            (None, Some(name)) => name.clone(),
            _ => host.name.clone(),
        };
        if host.port == 22 {
            address
        } else {
            format!("{address}:{}", host.port)
        }
    }
}

impl SavedConnectionsUi {
    /// Opens the import dialog above the picker and starts reading the config.
    pub(super) fn open_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open
            || self.opening
            || self.editor_saving()
            || self.editor.is_some()
            || self.import.is_some()
        {
            return;
        }
        self.group_admin.reset();
        let default_group = self.text(cx, keys::SSH_CONFIG_IMPORT_DEFAULT_GROUP);
        let source_placeholder = self.text(cx, keys::SSH_CONFIG_IMPORT_SOURCE_PLACEHOLDER);
        self.import_generation = self.import_generation.wrapping_add(1);
        let generation = self.import_generation;
        self.notice = None;
        self.import = Some(ImportDialog::new(
            default_group,
            source_placeholder,
            generation,
            window,
            cx,
        ));
        cx.notify();
        self.start_import_preview(window, cx);
    }

    /// Reads the config at the field's path; an empty path means the default.
    pub(super) fn start_import_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.import_generation = self.import_generation.wrapping_add(1);
        let generation = self.import_generation;
        let Some(dialog) = self.import.as_mut() else {
            return;
        };
        dialog.generation = generation;
        dialog.loading = true;
        dialog.error = None;
        dialog.preview = None;
        dialog.selected.clear();
        let source = dialog.source.read(cx).value().trim().to_string();
        let source = (!source.is_empty()).then_some(source);
        cx.notify();
        #[cfg(test)]
        if let Some(result) = self.injected_import_preview.clone() {
            self.finish_import_preview(generation, result, window, cx);
            return;
        }
        let services = Arc::clone(&self.services);
        cx.spawn_in(window, async move |this, cx| {
            let result = services
                .shared
                .runtime
                .spawn_blocking(move || build_ssh_config_import_preview(source))
                .await
                .map_err(|error| format!("Failed to read the SSH config: {error}"))
                .and_then(|result| result);
            let _ = cx.update(|window, app| {
                let _ = this.update(app, |this, cx| {
                    this.finish_import_preview(generation, result, window, cx);
                });
            });
        })
        .detach();
    }

    /// Opens the native picker for a config file or a directory of configs.
    pub(super) fn choose_import_source(
        &mut self,
        kind: ImportSourceKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .import
            .as_ref()
            .is_none_or(|dialog| dialog.loading || dialog.importing || dialog.picker_open)
        {
            return;
        }
        let title = match kind {
            ImportSourceKind::File => self.text(cx, keys::SSH_CONFIG_IMPORT_CHOOSE_SOURCE_FILE),
            ImportSourceKind::Directory => {
                self.text(cx, keys::SSH_CONFIG_IMPORT_CHOOSE_SOURCE_DIRECTORY)
            }
        };
        let Some(dialog) = self.import.as_mut() else {
            return;
        };
        let instance = dialog.instance;
        let generation = dialog.generation;
        dialog.picker_open = true;
        dialog.picked_source = None;
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: kind == ImportSourceKind::File,
            directories: kind == ImportSourceKind::Directory,
            multiple: false,
            prompt: Some(title.into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = paths
                .await
                .map_err(|_| "The native picker closed unexpectedly".to_string())
                .and_then(|result| result.map_err(|error| error.to_string()))
                .map(|paths| {
                    paths.and_then(|mut paths| (!paths.is_empty()).then(|| paths.remove(0)))
                });
            let _ = cx.update(|window, app| {
                let _ = this.update(app, |this, cx| {
                    this.finish_import_source_choice(instance, generation, result, window, cx);
                });
            });
        })
        .detach();
    }

    fn finish_import_source_choice(
        &mut self,
        instance: u64,
        generation: u64,
        result: Result<Option<PathBuf>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self.import.as_ref() else {
            return;
        };
        if dialog.instance != instance {
            return;
        }
        let (current_generation, importing) = (dialog.generation, dialog.importing);
        if let Some(dialog) = self.import.as_mut() {
            dialog.picker_open = false;
        }
        if current_generation != generation || importing {
            cx.notify();
            return;
        }
        let path = {
            match result {
                Ok(Some(path)) => path,
                Ok(None) => {
                    cx.notify();
                    return;
                }
                Err(message) => {
                    eprintln!("tterm-gpui: failed to choose an SSH config source: {message}");
                    let text = import_error_text(self, &message, cx);
                    if let Some(dialog) = self.import.as_mut() {
                        dialog.error = Some(text);
                    }
                    cx.notify();
                    return;
                }
            }
        };
        let path = path.to_string_lossy().into_owned();
        if path.is_empty() {
            cx.notify();
            return;
        }
        if let Some(dialog) = self.import.as_mut() {
            dialog.picked_source = Some(path.clone());
            dialog.source.update(cx, |source, cx| {
                source.set_value(path, window, cx);
            });
        }
        self.start_import_preview(window, cx);
    }

    fn finish_import_preview(
        &mut self,
        generation: u64,
        result: Result<SshConfigImportPreview, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .import
            .as_ref()
            .is_none_or(|dialog| dialog.generation != generation)
        {
            return;
        }
        match result {
            Ok(preview) => {
                if let Some(dialog) = self.import.as_mut() {
                    let source_matches_picker = dialog
                        .picked_source
                        .as_deref()
                        .is_some_and(|picked| dialog.source.read(cx).value() == picked);
                    if source_matches_picker {
                        let source_path = preview.source_path.clone();
                        dialog.source.update(cx, |source, cx| {
                            source.set_value(source_path, window, cx);
                        });
                    }
                    dialog.picked_source = None;
                    dialog.loading = false;
                    dialog.selected = preview
                        .hosts
                        .iter()
                        .filter(|host| !host.skipped)
                        .map(|host| host.host_pattern.clone())
                        .collect();
                    dialog.preview = Some(preview);
                }
            }
            Err(message) => {
                eprintln!("tterm-gpui: failed to preview an SSH config import: {message}");
                let text = import_error_text(self, &message, cx);
                if let Some(dialog) = self.import.as_mut() {
                    dialog.loading = false;
                    dialog.error = Some(text);
                }
            }
        }
        cx.notify();
    }

    /// Selects or clears one importable host.
    pub(super) fn toggle_import_host(&mut self, pattern: &str, cx: &mut Context<Self>) {
        let Some(dialog) = self.import.as_mut() else {
            return;
        };
        if dialog.loading || dialog.importing {
            return;
        }
        if !dialog.selectable().any(|host| host.host_pattern == pattern) {
            return;
        }
        if !dialog.selected.remove(pattern) {
            dialog.selected.insert(pattern.to_string());
        }
        dialog.error = None;
        cx.notify();
    }

    /// Selects every importable host, or clears them all when all are chosen.
    pub(super) fn toggle_import_all(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.import.as_mut() else {
            return;
        };
        if dialog.loading || dialog.importing {
            return;
        }
        if dialog.all_selected() {
            dialog.selected.clear();
        } else {
            dialog.selected = dialog
                .selectable()
                .map(|host| host.host_pattern.clone())
                .collect();
        }
        dialog.error = None;
        cx.notify();
    }

    pub(super) fn toggle_import_forwards(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.import.as_mut() else {
            return;
        };
        if dialog.loading || dialog.importing || dialog.forward_count() == 0 {
            return;
        }
        dialog.import_forwards = !dialog.import_forwards;
        cx.notify();
    }

    pub(super) fn toggle_import_overwrite(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.import.as_mut() else {
            return;
        };
        if dialog.loading || dialog.importing {
            return;
        }
        dialog.overwrite = !dialog.overwrite;
        cx.notify();
    }

    /// Returns to the picker without writing anything.
    pub(super) fn cancel_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.import.as_ref() else {
            return;
        };
        if dialog.importing {
            return;
        }
        self.import = None;
        self.import_generation = self.import_generation.wrapping_add(1);
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    /// Writes the chosen hosts through the core importer, then reloads the list.
    pub(super) fn submit_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.import.as_ref() else {
            return;
        };
        if dialog.loading || dialog.importing || dialog.selected.is_empty() {
            return;
        }
        let options = SshConfigImportOptions {
            // Import from the file that was previewed, even if the field was
            // edited afterwards without pressing Preview again.
            source_path: dialog
                .preview
                .as_ref()
                .map(|preview| preview.source_path.clone()),
            group: Some(dialog.group.read(cx).value().trim().to_string()),
            overwrite_existing: dialog.overwrite,
            selected_hosts: dialog.selected.iter().cloned().collect(),
            import_forwards: dialog.import_forwards,
        };
        #[cfg(test)]
        {
            self.import_options_for_test = Some(SshConfigImportOptions {
                source_path: options.source_path.clone(),
                group: options.group.clone(),
                overwrite_existing: options.overwrite_existing,
                selected_hosts: options.selected_hosts.clone(),
                import_forwards: options.import_forwards,
            });
        }
        let generation = dialog.generation;
        let Some(dialog) = self.import.as_mut() else {
            return;
        };
        dialog.importing = true;
        dialog.error = None;
        cx.notify();
        #[cfg(test)]
        if let Some(result) = self.injected_import_result.clone() {
            self.finish_import(generation, result, window, cx);
            return;
        }
        let services = Arc::clone(&self.services);
        cx.spawn_in(window, async move |this, cx| {
            let result = services
                .shared
                .runtime
                .spawn_blocking(move || import_ssh_config_profiles_blocking(options))
                .await
                .map_err(|error| format!("Failed to import the SSH config: {error}"))
                .and_then(|result| result);
            let _ = cx.update(|window, app| {
                let _ = this.update(app, |this, cx| {
                    this.finish_import(generation, result, window, cx);
                });
            });
        })
        .detach();
    }

    fn finish_import(
        &mut self,
        generation: u64,
        result: Result<SshConfigImportResult, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .import
            .as_ref()
            .is_none_or(|dialog| dialog.generation != generation)
        {
            return;
        }
        match result {
            Ok(result) => {
                self.notice = Some(self.import_summary(&result, cx));
                self.import = None;
                self.error = None;
                self.loading = true;
                self.select_after_load = None;
                self.focus_handle.focus(window, cx);
                cx.notify();
                self.load(window, cx);
            }
            Err(message) => {
                eprintln!("tterm-gpui: failed to import an SSH config: {message}");
                let text = import_error_text(self, &message, cx);
                if let Some(dialog) = self.import.as_mut() {
                    dialog.importing = false;
                    dialog.error = Some(text);
                }
                cx.notify();
            }
        }
    }

    /// Translates a request with arguments, falling back to its key.
    pub(super) fn import_label(&self, request: TranslationRequest<'_>, cx: &App) -> String {
        let key = request.key;
        self.locale
            .read(cx)
            .translate(request)
            .unwrap_or_else(|error| {
                eprintln!("tterm-gpui: failed to translate an SSH config import label: {error}");
                key.as_str().to_string()
            })
    }

    /// What the picker reports after an import finished.
    fn import_summary(&self, result: &SshConfigImportResult, cx: &App) -> String {
        let arguments = [
            (
                "imported",
                tterm_core::i18n::translate::ArgumentValue::Number(result.imported as f64),
            ),
            (
                "updated",
                tterm_core::i18n::translate::ArgumentValue::Number(result.updated as f64),
            ),
            (
                "skipped",
                tterm_core::i18n::translate::ArgumentValue::Number(result.skipped as f64),
            ),
        ];
        let mut summary = self.import_label(
            TranslationRequest::new(keys::SSH_CONFIG_IMPORT_COMPLETE_DESC).arguments(&arguments),
            cx,
        );
        if result.tunnels_imported > 0 {
            let tunnels = self.import_label(
                TranslationRequest::new(keys::SSH_CONFIG_IMPORT_COMPLETE_TUNNELS)
                    .count(result.tunnels_imported as f64),
                cx,
            );
            summary.push(' ');
            summary.push_str(&tunnels);
        }
        summary
    }

    #[cfg(test)]
    pub(super) fn set_import_preview_for_test(
        &mut self,
        result: Result<SshConfigImportPreview, String>,
    ) {
        self.injected_import_preview = Some(result);
    }

    #[cfg(test)]
    pub(super) fn set_import_result_for_test(
        &mut self,
        result: Result<SshConfigImportResult, String>,
    ) {
        self.injected_import_result = Some(result);
    }

    #[cfg(test)]
    pub(super) fn import_is_open(&self) -> bool {
        self.import.is_some()
    }

    #[cfg(test)]
    pub(super) fn import_state_for_test(&self) -> Option<&ImportDialog> {
        self.import.as_ref()
    }

    #[cfg(test)]
    pub(super) fn import_error_for_test(&self) -> Option<&str> {
        self.import
            .as_ref()
            .and_then(|dialog| dialog.error.as_deref())
    }

    #[cfg(test)]
    pub(super) fn import_options_for_test(&self) -> Option<&SshConfigImportOptions> {
        self.import_options_for_test.as_ref()
    }
}

/// Turns a core path or parse failure into the message the dialog shows.
fn import_error_text(ui: &SavedConnectionsUi, message: &str, cx: &App) -> String {
    let lower = message.to_lowercase();
    if lower.contains("not found") {
        return ui.text(cx, keys::SSH_CONFIG_IMPORT_ERROR_NOT_FOUND);
    }
    if lower.contains("permission denied") || lower.contains("access is denied") {
        return ui.text(cx, keys::SSH_CONFIG_IMPORT_ERROR_PERMISSION_DENIED);
    }
    if lower.contains("parse") {
        return ui.text(cx, keys::SSH_CONFIG_IMPORT_ERROR_PARSE);
    }
    if message.starts_with("SSH config path is not a file") {
        return ui.text(cx, keys::SSH_CONFIG_IMPORT_ERROR_NOT_FILE);
    }
    message.to_string()
}

mod view;

#[cfg(test)]
mod tests;
