use crate::config::{ensure_config_dir, get_config_path};
use crate::ssh::SecretLocation;
use serde::{Deserialize, Serialize};
use std::fs;

fn default_auth_method() -> String {
    "password".to_string()
}

/// Jump host configuration stored as part of a saved profile.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SavedJumpHost {
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default = "default_auth_method")]
    pub auth_method: String,
    #[serde(default)]
    pub private_key_path: Option<String>,
    #[serde(default, skip_serializing)]
    pub private_key_passphrase: Option<String>,
    #[serde(default, skip_serializing)]
    pub password: Option<String>,
}

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
    #[serde(default = "default_keepalive_interval")]
    pub keepalive_interval_secs: u32,
    #[serde(default = "default_keepalive_count")]
    pub keepalive_count_max: u32,
    /// Legacy single jump host field kept only for backward-compatible reads.
    #[serde(default, rename = "jump_host", skip_serializing)]
    legacy_jump_host: Option<SavedJumpHost>,
    /// Ordered jump host chain used throughout the app and for all new saves.
    #[serde(default)]
    pub jump_hosts: Vec<SavedJumpHost>,
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

fn normalize_profile(profile: &mut SavedProfile) {
    if profile.jump_hosts.is_empty() {
        if let Some(jump) = profile.legacy_jump_host.take() {
            profile.jump_hosts.push(jump);
        }
    } else {
        profile.legacy_jump_host = None;
    }

    if profile.group.trim().is_empty() {
        profile.group = String::new();
    }
}

fn sanitize_profile(profile: &mut SavedProfile) {
    normalize_profile(profile);
    profile.password = None;
    profile.private_key_passphrase = None;
    if let Some(jump) = &mut profile.legacy_jump_host {
        jump.password = None;
        jump.private_key_passphrase = None;
    }
    for jump in &mut profile.jump_hosts {
        jump.password = None;
        jump.private_key_passphrase = None;
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
pub fn save_profile(
    app: tauri::AppHandle,
    secret_state: tauri::State<'_, crate::ssh::SecretStoreState>,
    mut profile: SavedProfile,
) -> Result<(), String> {
    normalize_profile(&mut profile);

    if profile.auth_method.as_deref() != Some("key") && profile.remember_password {
        if let Some(password) = profile
            .password
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            let location = secret_state.save_password(&app, profile.id.as_str(), password)?;
            if matches!(location, SecretLocation::Memory) {
                return Err(
                    "Password persistence is unavailable. Enable the app vault or use a supported system credential store."
                        .to_string(),
                );
            }
        }
    }

    for jump in &profile.jump_hosts {
        if profile.remember_password && jump.auth_method != "key" {
            if let Some(password) = jump.password.as_deref().filter(|value| !value.is_empty()) {
                let secret_key = crate::core::session::jump_host_identity_secret_key(
                    Some(profile.id.as_str()),
                    profile.name.as_str(),
                    &jump.host,
                    jump.port,
                    &jump.username,
                );
                let location = secret_state.save_password(&app, &secret_key, password)?;
                if matches!(location, SecretLocation::Memory) {
                    return Err(
                        "Jump host password persistence is unavailable. Enable the app vault or use a supported system credential store."
                            .to_string(),
                    );
                }
            }
        }
    }

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
    prompt_state: tauri::State<'_, crate::core::state::HostPromptMap>,
    secret_state: tauri::State<'_, crate::ssh::SecretStoreState>,
) -> Result<String, String> {
    if profile.connection_type != "ssh" {
        return Err("Only SSH connections can be tested".to_string());
    }

    let host = profile.host.clone().ok_or("Host is required")?;
    let username = profile.username.clone().ok_or("Username is required")?;
    let port = profile.port.unwrap_or(22);

    let mut profile = profile;
    normalize_profile(&mut profile);

    let jump_hosts = profile
        .jump_hosts
        .into_iter()
        .map(|j| crate::core::session::JumpHostPlan {
            host: j.host,
            port: j.port,
            username: j.username,
            password: j.password,
            private_key_path: if j.auth_method == "key" {
                j.private_key_path
            } else {
                None
            },
            private_key_passphrase: if j.auth_method == "key" {
                j.private_key_passphrase
            } else {
                None
            },
        })
        .collect::<Vec<_>>();

    let mut plan = crate::core::session::SessionPlan {
        kind: crate::core::SessionKind::Ssh,
        profile_id: Some(profile.id.clone()),
        profile_name: profile.name.clone(),
        host: Some(host.clone()),
        port,
        username: Some(username.clone()),
        password: profile.password.clone(),
        remember_password: false,
        private_key_path: if profile.auth_method.as_deref() == Some("key") {
            profile.private_key_path.clone()
        } else {
            None
        },
        private_key_passphrase: profile.private_key_passphrase.clone(),
        terminal_shell: None,
        keepalive_interval_secs: profile.keepalive_interval_secs as u16,
        keepalive_count_max: profile.keepalive_count_max as u16,
        jump_hosts,
    };

    crate::core::session::resolve_ssh_password(&app, &secret_state, &mut plan)?;

    // Try to establish connection
    use std::time::Duration;

    if !plan.jump_hosts.is_empty() {
        let test_tab_id = format!("test-{}", profile.id);
        let target_config = std::sync::Arc::new(crate::ssh::jump::compatibility_client_config(
            plan.keepalive_interval_secs as u64,
            plan.keepalive_count_max as usize,
        ));

        let (jump_chain, mut target_session) = crate::ssh::jump::connect_via_jump_chain(
            &app,
            &test_tab_id,
            &plan.jump_hosts,
            &host,
            port,
            TestConnectionHandler,
            target_config,
            prompt_state.inner().clone(),
        )
        .await?;

        // Authenticate on target through tunnel
        let auth_result =
            authenticate_test_connection(&mut target_session, &username, &plan).await?;

        match auth_result {
            russh::client::AuthResult::Success => {}
            _ => return Err("Authentication failed".to_string()),
        }

        let _ = target_session
            .disconnect(russh::Disconnect::ByApplication, "", "")
            .await;
        drop(jump_chain);
    } else {
        // Direct connection
        use russh::client;

        let config = std::sync::Arc::new(crate::ssh::jump::compatibility_client_config(
            plan.keepalive_interval_secs as u64,
            plan.keepalive_count_max as usize,
        ));

        let mut session = tokio::time::timeout(
            Duration::from_secs(10),
            client::connect(config, (host.as_str(), port), TestConnectionHandler),
        )
        .await
        .map_err(|_| "Connection timeout".to_string())?
        .map_err(|e| format!("Connection failed: {}", e))?;

        let auth_result = authenticate_test_connection(&mut session, &username, &plan).await?;

        match auth_result {
            russh::client::AuthResult::Success => {}
            _ => return Err("Authentication failed".to_string()),
        }

        session
            .disconnect(russh::Disconnect::ByApplication, "", "")
            .await
            .ok();
    }

    Ok(format!(
        "Successfully connected to {}@{}:{}",
        username, host, port
    ))
}

// Simple handler for test connections that auto-accepts host keys
struct TestConnectionHandler;

impl russh::client::Handler for TestConnectionHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // Auto-accept for test connections
        Ok(true)
    }
}

async fn authenticate_test_connection<H: russh::client::Handler>(
    session: &mut russh::client::Handle<H>,
    username: &str,
    plan: &crate::core::session::SessionPlan,
) -> Result<russh::client::AuthResult, String> {
    if let Some(key_path) = &plan.private_key_path {
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
                username,
                russh::keys::PrivateKeyWithHashAlg::new(std::sync::Arc::new(key), None),
            )
            .await
            .map_err(|e| format!("Authentication failed: {}", e))
    } else {
        let password = plan.password.as_deref().ok_or("Password is required")?;
        session
            .authenticate_password(username, password)
            .await
            .map_err(|e| format!("Authentication failed: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::{sanitize_profile, SavedProfile};

    #[test]
    fn sanitize_profile_migrates_legacy_jump_host_to_chain() {
        let mut profile: SavedProfile = serde_json::from_value(serde_json::json!({
            "id": "profile-1",
            "name": "demo",
            "group": "",
            "connection_type": "ssh",
            "remember_password": false,
            "keepalive_interval_secs": 30,
            "keepalive_count_max": 3,
            "jump_host": {
                "host": "bastion",
                "port": 22,
                "username": "stone",
                "auth_method": "password",
                "password": "secret"
            }
        }))
        .expect("profile should deserialize");

        sanitize_profile(&mut profile);

        assert_eq!(profile.jump_hosts.len(), 1);
        assert_eq!(profile.jump_hosts[0].host, "bastion");
        assert!(profile.jump_hosts[0].password.is_none());
        let serialized = serde_json::to_value(&profile).expect("profile should serialize");
        assert!(serialized.get("jump_host").is_none());
    }
}
