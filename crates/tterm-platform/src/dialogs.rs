//! File dialogs are implemented by the native client when it has a Rust caller.
use crate::PlatformFuture;
use std::path::PathBuf;

/// A named list of permitted extensions.
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}
/// Options shared by file and directory selection.
pub struct OpenDialogOptions {
    pub title: String,
    pub default_path: Option<PathBuf>,
    pub filters: Vec<FileFilter>,
    pub directories: bool,
    pub multiple: bool,
    pub can_create_directories: bool,
}
/// Options for choosing a file to save.
pub struct SaveDialogOptions {
    pub title: String,
    pub default_path: Option<PathBuf>,
    pub filters: Vec<FileFilter>,
    pub file_name: Option<String>,
    pub can_create_directories: bool,
}
/// Cancellation returns None; native failures remain errors.
pub trait FileDialogs: Send + Sync {
    /// Waits for file or directory selection; cancellation returns None.
    fn open(&self, options: OpenDialogOptions) -> PlatformFuture<'_, Option<Vec<PathBuf>>>;
    /// Waits for a save destination; cancellation returns None.
    fn save(&self, options: SaveDialogOptions) -> PlatformFuture<'_, Option<PathBuf>>;
}
