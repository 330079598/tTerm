/// Runs blocking work (key derivation, file and database I/O) on a worker
/// thread. A synchronous `#[tauri::command]` runs on the main thread and
/// freezes the window until it returns.
pub async fn run_blocking<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|err| format!("Task join error: {err}"))?
}
