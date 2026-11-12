#[tauri::command]
pub fn has_typed_password(tab_id: String, host: String, port: u16, username: String) -> bool {
    super::typed_passwords::has_typed_password(tab_id, host, port, username)
}

#[tauri::command]
pub fn forget_typed_passwords(tab_id: String) {
    super::typed_passwords::forget_typed_passwords(tab_id);
}
