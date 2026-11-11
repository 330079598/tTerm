//! Remote file drag protocol: discovers the target before SFTP downloads.
use crate::PlatformFuture;
use std::path::PathBuf;
/// Metadata displayed during a native drag.
#[derive(Clone, Debug)]
pub struct DragOutItem {
    pub name: String,
    pub is_dir: bool,
}
/// The native session's target, or why no target directory was obtained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragOutResult {
    Dropped { directory: String },
    Cancelled,
    UnknownTarget,
}
/// The client's bound native source window starts the drag while the mouse remains pressed.
pub trait FileDrag: Send + Sync {
    /// Starts a native drag while the initiating mouse button is still pressed.
    fn start(&self, items: Vec<DragOutItem>) -> PlatformFuture<'_, DragOutResult>;
    /// Notifies the file manager after the deferred download completes.
    fn refresh_folder(&self, directory: PathBuf) -> PlatformFuture<'_, ()>;
}
