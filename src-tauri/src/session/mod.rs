use crate::config::get_config_path;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionData {
    pub tabs: serde_json::Value,
    pub active_tab_id: Option<String>,
    pub last_saved: i64,
}

impl Default for SessionData {
    fn default() -> Self {
        Self {
            tabs: serde_json::json!([]),
            active_tab_id: None,
            last_saved: 0,
        }
    }
}

// Re-sanitize session payloads on the backend so no caller can persist raw secrets.
fn sanitize_session_tabs(tabs: serde_json::Value) -> serde_json::Value {
    match tabs {
        serde_json::Value::Array(entries) => serde_json::Value::Array(
            entries
                .into_iter()
                .map(sanitize_session_tab)
                .filter(|tab| !tab.is_null())
                .collect(),
        ),
        _ => serde_json::json!([]),
    }
}

fn sanitize_session_tab(tab: serde_json::Value) -> serde_json::Value {
    let mut tab = match tab {
        serde_json::Value::Object(map) => map,
        _ => return serde_json::Value::Null,
    };

    if let Some(serde_json::Value::Object(connection)) = tab.get_mut("connection") {
        connection.remove("password");
        connection.remove("privateKeyPassphrase");

        if let Some(legacy_jump_host) = connection.remove("jumpHost") {
            let should_adopt_legacy = !matches!(
                connection.get("jumpHosts"),
                Some(serde_json::Value::Array(entries)) if !entries.is_empty()
            );

            if should_adopt_legacy {
                connection.insert(
                    "jumpHosts".to_string(),
                    serde_json::Value::Array(vec![sanitize_jump_host_value(legacy_jump_host)]),
                );
            }
        }

        if let Some(serde_json::Value::Array(jump_hosts)) = connection.get_mut("jumpHosts") {
            for jump_host in jump_hosts.iter_mut() {
                let sanitized = sanitize_jump_host_value(std::mem::take(jump_host));
                *jump_host = sanitized;
            }
        }
    }

    serde_json::Value::Object(tab)
}

fn sanitize_jump_host_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(mut jump_host) => {
            jump_host.remove("password");
            jump_host.remove("privateKeyPassphrase");
            serde_json::Value::Object(jump_host)
        }
        other => other,
    }
}

#[tauri::command]
pub fn load_session() -> Result<SessionData, String> {
    let config_dir = get_config_path()?;
    let session_file = config_dir.join("session.json");
    if !session_file.exists() {
        return Ok(SessionData::default());
    }
    let content = fs::read_to_string(&session_file)
        .map_err(|e| format!("Failed to read session file: {}", e))?;
    let mut session = serde_json::from_str::<SessionData>(&content)
        .map_err(|e| format!("Failed to parse session: {}", e))?;
    // Clean older session files as they are loaded so stale plaintext secrets are dropped immediately.
    session.tabs = sanitize_session_tabs(session.tabs);
    Ok(session)
}

#[tauri::command]
pub fn save_session(mut session: SessionData) -> Result<(), String> {
    let config_dir = crate::config::ensure_config_dir()?;
    let session_file = config_dir.join("session.json");
    // Keep a backend-side guard even if the frontend payload changes in the future.
    session.tabs = sanitize_session_tabs(session.tabs);
    let content = serde_json::to_string_pretty(&session)
        .map_err(|e| format!("Failed to serialize session: {}", e))?;
    fs::write(&session_file, content).map_err(|e| format!("Failed to write session file: {}", e))
}

#[tauri::command]
pub fn clear_session() -> Result<(), String> {
    let config_dir = get_config_path()?;
    let session_file = config_dir.join("session.json");
    if session_file.exists() {
        fs::remove_file(&session_file)
            .map_err(|e| format!("Failed to remove session file: {}", e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::sanitize_session_tabs;

    #[test]
    fn sanitize_session_tabs_migrates_legacy_jump_host_to_jump_hosts() {
        let tabs = serde_json::json!([
            {
                "id": "1",
                "connection": {
                    "jumpHost": {
                        "host": "bastion",
                        "port": 22,
                        "username": "stone",
                        "password": "secret"
                    }
                }
            }
        ]);

        let sanitized = sanitize_session_tabs(tabs);
        let connection = sanitized[0]["connection"]
            .as_object()
            .expect("connection object");
        assert!(connection.get("jumpHost").is_none());
        let jump_hosts = connection
            .get("jumpHosts")
            .and_then(|value| value.as_array())
            .expect("jumpHosts array");
        assert_eq!(jump_hosts.len(), 1);
        assert_eq!(jump_hosts[0]["host"], "bastion");
        assert!(jump_hosts[0].get("password").is_none());
    }
}
