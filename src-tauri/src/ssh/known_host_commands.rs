use super::store::{delete_known_host_entries, list_known_host_entries, KnownHostEntry};
use crate::core::blocking::run_blocking;

#[tauri::command]
pub async fn list_known_hosts() -> Result<Vec<KnownHostEntry>, String> {
    run_blocking(|| crate::db::read(list_known_host_entries)).await
}

/// Returns how many entries were removed.
#[tauri::command]
pub async fn delete_known_hosts(ids: Vec<i64>) -> Result<usize, String> {
    run_blocking(move || {
        crate::db::write(|transaction| delete_known_host_entries(transaction, &ids))
    })
    .await
}
