#[tauri::command]
pub fn has_typed_password(tab_id: String, host: String, port: u16, username: String) -> bool {
    tterm_core::ssh::typed_passwords::has_typed_password(tab_id, host, port, username)
}

#[tauri::command]
pub fn forget_typed_passwords(tab_id: String) {
    tterm_core::ssh::typed_passwords::forget_typed_passwords(tab_id);
}
