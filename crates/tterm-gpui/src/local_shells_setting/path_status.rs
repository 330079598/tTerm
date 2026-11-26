//! Whether the local shells' program paths lead to a program on this machine.
//!
//! A check reads the file system, so it runs off the UI thread and its answer
//! can arrive after the editor moved on, the saved list changed, or the page
//! closed. Every request carries the generation it was made in, and a reply
//! publishes only while that generation is still the current one.

use std::collections::HashMap;

use tterm_terminal::js_text::js_trim;

#[cfg(test)]
mod tests;

/// What the editor shows below its program field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum EditorStatus {
    /// The field is blank, so there is nothing to check.
    #[default]
    Idle,
    /// The path's answer has not arrived yet.
    Checking,
    /// The path starts this program here.
    Found(String),
    /// The path leads nowhere on this machine.
    NotFound,
}

/// The path-check answers and the generations that keep them current.
pub(crate) struct PathStatus {
    active: bool,
    saved: HashMap<String, bool>,
    /// The trimmed paths the current saved batch asks about.
    wanted: Vec<String>,
    saved_generation: u64,
    editor_path: Option<String>,
    editor_status: EditorStatus,
    editor_generation: u64,
}

impl PathStatus {
    pub(crate) fn new() -> Self {
        Self {
            active: false,
            saved: HashMap::new(),
            wanted: Vec::new(),
            saved_generation: 0,
            editor_path: None,
            editor_status: EditorStatus::Idle,
            editor_generation: 0,
        }
    }

    /// Starts a settings visit. The cached badges stay until the new answers
    /// arrive, so opening the page does not flash every shell as unchecked.
    pub(crate) fn open(&mut self) {
        self.active = true;
        // Clear the last batch so this visit asks again even when nothing
        // changed while the page was closed.
        self.wanted.clear();
        self.clear_editor();
    }

    /// Stops publishing answers. Work already running no longer reaches state.
    pub(crate) fn close(&mut self) {
        self.active = false;
        self.wanted.clear();
        self.saved_generation = self.saved_generation.wrapping_add(1);
        self.clear_editor();
    }

    /// The trimmed paths to check for this visit, or none when the saved list
    /// did not change since the last request. Checks repeat when the page
    /// opens and when the saved paths change, not on every render.
    pub(crate) fn request_saved(&mut self, paths: &[String]) -> Vec<String> {
        if !self.active {
            return Vec::new();
        }
        let mut wanted: Vec<String> = paths
            .iter()
            .map(|path| js_trim(path).to_string())
            .filter(|path| !path.is_empty())
            .collect();
        wanted.sort();
        wanted.dedup();
        if wanted == self.wanted {
            return Vec::new();
        }
        self.wanted = wanted.clone();
        self.saved_generation = self.saved_generation.wrapping_add(1);
        wanted
    }

    /// The generation the saved paths' current answers belong to.
    pub(crate) fn saved_generation(&self) -> u64 {
        self.saved_generation
    }

    /// Whether a saved shell's path is known to lead nowhere.
    pub(crate) fn saved_missing(&self, path: &str) -> bool {
        self.saved.get(js_trim(path)) == Some(&false)
    }

    /// Publishes one saved path's answer unless its batch is no longer current.
    pub(crate) fn apply_saved(&mut self, generation: u64, path: &str, found: bool) -> bool {
        if !self.active || generation != self.saved_generation {
            return false;
        }
        self.saved.insert(path.to_string(), found);
        true
    }

    /// Records the editor's new program text. Returns the request to run once
    /// typing pauses, or none when the field is blank.
    pub(crate) fn edit(&mut self, raw: &str) -> Option<(u64, String)> {
        if !self.active {
            return None;
        }
        // A new generation drops every answer to an older text in one place.
        self.editor_generation = self.editor_generation.wrapping_add(1);
        let path = js_trim(raw);
        if path.is_empty() {
            self.editor_path = None;
            self.editor_status = EditorStatus::Idle;
            return None;
        }
        self.editor_path = Some(path.to_string());
        self.editor_status = EditorStatus::Checking;
        Some((self.editor_generation, path.to_string()))
    }

    /// What the editor shows for the path it currently holds.
    pub(crate) fn editor_status(&self) -> &EditorStatus {
        &self.editor_status
    }

    /// Whether the editor still waits for this request's answer.
    pub(crate) fn editor_waits(&self, generation: u64, path: &str) -> bool {
        self.active
            && generation == self.editor_generation
            && self.editor_path.as_deref() == Some(path)
    }

    /// Publishes an editor answer unless the text or the visit moved on.
    pub(crate) fn apply_editor(
        &mut self,
        generation: u64,
        path: &str,
        result: Result<String, String>,
    ) -> bool {
        if !self.editor_waits(generation, path) {
            return false;
        }
        self.editor_status = match result {
            Ok(program) => EditorStatus::Found(program),
            Err(_) => EditorStatus::NotFound,
        };
        true
    }

    /// Drops the editor's status and its pending request.
    pub(crate) fn clear_editor(&mut self) {
        self.editor_generation = self.editor_generation.wrapping_add(1);
        self.editor_path = None;
        self.editor_status = EditorStatus::Idle;
    }
}
