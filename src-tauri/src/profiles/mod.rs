use crate::config::{ensure_config_dir, get_config_path};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavedProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub group: String,
    pub connection_type: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    #[serde(default, skip_serializing)]
    pub password: Option<String>,
    #[serde(default)]
    pub remember_password: bool,
    pub auth_method: Option<String>,
    pub private_key_path: Option<String>,
    #[serde(default, skip_serializing)]
    pub private_key_passphrase: Option<String>,
    #[serde(default)]
    pub reconnect: bool,
    #[serde(default = "default_reconnect_delay")]
    pub reconnect_delay_secs: u32,
    #[serde(default = "default_reconnect_max_delay")]
    pub reconnect_max_delay_secs: u32,
    #[serde(default = "default_reconnect_max_retries")]
    pub reconnect_max_retries: u32,
    #[serde(default = "default_keepalive_interval")]
    pub keepalive_interval_secs: u32,
    #[serde(default = "default_keepalive_count")]
    pub keepalive_count_max: u32,
}

fn default_reconnect_delay() -> u32 {
    5
}
fn default_reconnect_max_delay() -> u32 {
    60
}
fn default_reconnect_max_retries() -> u32 {
    10
}
fn default_keepalive_interval() -> u32 {
    30
}
fn default_keepalive_count() -> u32 {
    3
}

fn load_profiles_from_disk() -> Result<Vec<SavedProfile>, String> {
    let config_dir = get_config_path()?;
    let profiles_file = config_dir.join("profiles.json");
    if !profiles_file.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(&profiles_file)
        .map_err(|e| format!("Failed to read profiles file: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse profiles: {}", e))
}

fn sanitize_profile(profile: &mut SavedProfile) {
    profile.password = None;
    profile.private_key_passphrase = None;
    if profile.group.trim().is_empty() {
        profile.group = String::new();
    }
}

fn write_profiles_to_disk(profiles: &Vec<SavedProfile>) -> Result<(), String> {
    let config_dir = ensure_config_dir()?;
    let profiles_file = config_dir.join("profiles.json");
    let content = serde_json::to_string_pretty(profiles)
        .map_err(|e| format!("Failed to serialize profiles: {}", e))?;
    fs::write(&profiles_file, content).map_err(|e| format!("Failed to write profiles file: {}", e))
}

#[tauri::command]
pub fn list_profiles() -> Result<Vec<SavedProfile>, String> {
    let mut profiles = load_profiles_from_disk()?;
    for profile in &mut profiles {
        sanitize_profile(profile);
    }
    Ok(profiles)
}

#[tauri::command]
pub fn save_profile(mut profile: SavedProfile) -> Result<(), String> {
    sanitize_profile(&mut profile);

    let mut profiles = load_profiles_from_disk()?;
    if let Some(pos) = profiles.iter().position(|p| p.id == profile.id) {
        profiles[pos] = profile;
    } else {
        profiles.push(profile);
    }
    for existing in &mut profiles {
        sanitize_profile(existing);
    }
    write_profiles_to_disk(&profiles)
}

#[tauri::command]
pub fn delete_profile(id: String) -> Result<(), String> {
    let mut profiles = load_profiles_from_disk()?;
    profiles.retain(|p| p.id != id);
    write_profiles_to_disk(&profiles)
}

#[tauri::command]
pub async fn test_connection(
    app: tauri::AppHandle,
    profile: SavedProfile,
    secret_state: tauri::State<'_, crate::ssh::SecretStoreState>,
) -> Result<String, String> {
    if profile.connection_type != "ssh" {
        return Err("Only SSH connections can be tested".to_string());
    }

    let host = profile.host.ok_or("Host is required")?;
    let username = profile.username.ok_or("Username is required")?;
    let port = profile.port.unwrap_or(22);

    // Build connection plan
    let mut plan = crate::core::session::SessionPlan {
        kind: crate::core::SessionKind::Ssh,
        profile_name: profile.name.clone(),
        host: Some(host.clone()),
        port,
        username: Some(username.clone()),
        password: None,
        remember_password: false,
        private_key_path: if profile.auth_method.as_deref() == Some("key") {
            profile.private_key_path.clone()
        } else {
            None
        },
        private_key_passphrase: None,
        terminal_shell: None,
        reconnect: false,
        reconnect_initial_delay: std::time::Duration::from_secs(5),
        reconnect_max_delay: std::time::Duration::from_secs(60),
        reconnect_max_retries: Some(0),
        keepalive_interval_secs: profile.keepalive_interval_secs as u16,
        keepalive_count_max: profile.keepalive_count_max as u16,
    };

    // Resolve password/passphrase from secret store
    crate::core::session::resolve_ssh_password(&app, &secret_state, &mut plan)?;

    // Try to establish connection
    use russh::client;
    use std::time::Duration;

    let mut config = client::Config::default();
    config.keepalive_interval = Some(Duration::from_secs(plan.keepalive_interval_secs as u64));
    config.keepalive_max = plan.keepalive_count_max as usize;

    let handler = crate::ssh::SshClientHandler {
        app: app.clone(),
        tab_id: "test-connection".to_string(),
        profile_name: plan.profile_name.clone(),
        host: host.clone(),
        port,
        prompts: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        user_rejected_host_key: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };

    let mut session = tokio::time::timeout(
        Duration::from_secs(10),
        client::connect(
            std::sync::Arc::new(config),
            (host.as_str(), port),
            handler,
        )
    )
    .await
    .map_err(|_| "Connection timeout".to_string())?
    .map_err(|e| format!("Connection failed: {}", e))?;

    // Authenticate
    let auth_result = if let Some(key_path) = &plan.private_key_path {
        let key_data = std::fs::read_to_string(key_path)
            .map_err(|e| format!("Failed to read private key: {}", e))?;
        
        let key = if let Some(passphrase) = &plan.private_key_passphrase {
            russh::keys::decode_secret_key(&key_data, Some(passphrase))
                .map_err(|e| format!("Failed to decode private key: {}", e))?
        } else {
            russh::keys::decode_secret_key(&key_data, None)
                .map_err(|e| format!("Failed to decode private key: {}", e))?
        };

        session
            .authenticate_publickey(
                &username,
                russh::keys::PrivateKeyWithHashAlg::new(std::sync::Arc::new(key), None),
            )
            .await
            .map_err(|e| format!("Authentication failed: {}", e))?
    } else {
        let password = plan.password.ok_or("Password is required")?;
        session
            .authenticate_password(&username, &password)
            .await
            .map_err(|e| format!("Authentication failed: {}", e))?
    };

    match auth_result {
        russh::client::AuthResult::Success => {},
        _ => return Err("Authentication failed".to_string()),
    }

    // Close connection
    session
        .disconnect(russh::Disconnect::ByApplication, "", "")
        .await
        .ok();

    Ok(format!("Successfully connected to {}@{}:{}", username, host, port))
}
