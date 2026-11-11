//! Native dragging bound to a client's main-thread dispatcher.
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
use crate::{
    PlatformFuture,
    drag::{DragOutItem, DragOutResult, FileDrag},
    window::WindowHost,
};
use std::{path::PathBuf, sync::Arc};

/// Holds the client's dispatcher without owning or exposing native window pointers.
pub struct NativeFileDrag(pub Arc<dyn WindowHost>);
impl FileDrag for NativeFileDrag {
    fn start(&self, items: Vec<DragOutItem>) -> PlatformFuture<'_, DragOutResult> {
        Box::pin(async move {
            if items.is_empty() {
                return Ok(DragOutResult::Cancelled);
            }
            #[cfg(target_os = "macos")]
            return macos::drag_out(self.0.clone(), items).await;
            #[cfg(windows)]
            return windows::drag_out(self.0.clone(), items).await;
            #[cfg(not(any(target_os = "macos", windows)))]
            Err("Native dragging is not supported on this platform.".into())
        })
    }
    fn refresh_folder(&self, directory: PathBuf) -> PlatformFuture<'_, ()> {
        Box::pin(async move {
            #[cfg(windows)]
            super::run_blocking(move || {
                windows::refresh_folder(&directory);
                Ok(())
            })
            .await?;
            #[cfg(not(windows))]
            let _ = directory;
            Ok(())
        })
    }
}
