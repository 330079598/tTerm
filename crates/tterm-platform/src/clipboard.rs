//! Text and local file clipboard access.
use crate::PlatformFuture;
use std::path::PathBuf;

/// Access to the system clipboard, without granting terminal protocols read access.
pub trait Clipboard: Send + Sync {
    /// Reads text using the existing clipboard failure semantics.
    fn read_text(&self) -> PlatformFuture<'_, String>;
    /// Replaces the system clipboard text.
    fn write_text(&self, text: String) -> PlatformFuture<'_, ()>;
    /// Reads existing local file paths; no file format yields an empty list.
    fn read_files(&self) -> PlatformFuture<'_, Vec<PathBuf>>;
}
