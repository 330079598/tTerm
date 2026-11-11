//! Native implementations shared by the two clients.
pub mod clipboard_files;
pub mod drag;
pub mod integration;
pub mod notification;
pub mod open;
pub mod verification;

/// Runs blocking native operations without holding up a client UI thread.
pub async fn run_blocking<T: Send + 'static>(
    task: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|e| e.to_string())?
}
