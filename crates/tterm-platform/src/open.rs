//! Opening files and their associated applications.
use crate::PlatformFuture;
use std::{collections::HashMap, path::PathBuf};

/// An application associated with a file type. Icons contain PNG bytes.
#[derive(Debug, Clone)]
pub struct ApplicationChoice {
    pub name: String,
    pub path: String,
    pub is_default: bool,
    pub icon: Option<Vec<u8>>,
}

/// System opening operations; successful dispatch does not guarantee another app rendered.
pub trait OpenTargets: Send + Sync {
    /// Hands a URL to the system URL handler.
    fn open_url(&self, url: String) -> PlatformFuture<'_, ()>;
    /// Opens a local path in its default application.
    fn open_file(&self, path: PathBuf) -> PlatformFuture<'_, ()>;
    /// Selects a path in the system file manager.
    fn reveal(&self, path: PathBuf) -> PlatformFuture<'_, ()>;
    /// Opens one path as one argument to an explicitly chosen program.
    fn open_with(&self, program: String, path: PathBuf) -> PlatformFuture<'_, ()>;
    /// Opens the system application chooser; false means the user cancelled.
    fn choose_application(&self, path: PathBuf) -> PlatformFuture<'_, bool>;
    /// Lists associated applications without exposing toolkit image objects.
    fn applications_for(&self, file_name: String) -> PlatformFuture<'_, Vec<ApplicationChoice>>;
    /// Returns cached or newly rendered PNG icons keyed by application path.
    fn application_icons(
        &self,
        programs: Vec<String>,
    ) -> PlatformFuture<'_, HashMap<String, Option<Vec<u8>>>>;
}
