use super::store::{delete_known_host_entries, list_known_host_entries, KnownHostEntry};

#[tauri::command]
pub fn list_known_hosts() -> Result<Vec<KnownHostEntry>, String> {
    crate::db::read(list_known_host_entries)
}

/// Returns how many entries were removed.
#[tauri::command]
pub fn delete_known_hosts(ids: Vec<i64>) -> Result<usize, String> {
    crate::db::write(|transaction| delete_known_host_entries(transaction, &ids))
}
