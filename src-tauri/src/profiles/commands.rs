use super::parser::build_ssh_config_import_preview;
use super::storage::{
    delete_profile_secrets, load_all_profile_groups, load_configured_profile_groups,
    load_profiles_from_disk, normalize_group_name, normalize_profile, push_unique_group,
    sanitize_profile, write_profile_groups_to_disk, write_profiles_to_disk,
};
use super::types::{
    SavedProfile, SshConfigImportOptions, SshConfigImportPreview, SshConfigImportResult,
};
use crate::core::state::HostPromptMap;
use crate::ssh::SshConnectionProgressPayload;
use crate::ssh::{
    ConnectionStatusOptions, HostKeyVerificationMode, SecretLocation, SshClientHandler,
};
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[tauri::command]
pub fn list_profiles() -> Result<Vec<SavedProfile>, String> {
    let mut profiles = load_profiles_from_disk()?;
    for profile in &mut profiles {
        sanitize_profile(profile);
    }
    Ok(profiles)
}

#[tauri::command]
pub fn preview_ssh_config_import(
    source_path: Option<String>,
) -> Result<SshConfigImportPreview, String> {
    build_ssh_config_import_preview(source_path)
}

#[tauri::command]
pub fn import_ssh_config_profiles(
    options: SshConfigImportOptions,
) -> Result<SshConfigImportResult, String> {
    let preview = build_ssh_config_import_preview(options.source_path)?;
    let selected_hosts = options
        .selected_hosts
        .into_iter()
        .collect::<HashSet<String>>();
    let import_all = selected_hosts.is_empty();
    let group = normalize_group_name(
        options
            .group
            .as_deref()
            .unwrap_or("Imported from SSH config"),
    );
    let mut profiles = load_profiles_from_disk()?;
    let mut imported = 0;
    let mut updated = 0;
    let mut skipped = 0;

    for host in preview.hosts {
        if host.skipped || (!import_all && !selected_hosts.contains(&host.host_pattern)) {
            skipped += 1;
            continue;
        }

        let Some(hostname) = host.host else {
            skipped += 1;
            continue;
        };

        let existing_index = profiles
            .iter()
            .position(|profile| profile.name == host.name && profile.group == group);

        if existing_index.is_some() && !options.overwrite_existing {
            skipped += 1;
            continue;
        }

        let profile = SavedProfile {
            id: existing_index
                .map(|index| profiles[index].id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: host.name,
            group: group.clone(),
            connection_type: "ssh".to_string(),
            host: Some(hostname),
            port: Some(host.port),
            username: host.username,
            password: None,
            ignore_saved_password: false,
            remember_password: false,
            auth_method: Some(host.auth_method),
            private_key_path: host.private_key_path,
            private_key_passphrase: None,
            keepalive_interval_secs: host.keepalive_interval_secs,
            keepalive_count_max: host.keepalive_count_max,
            server_monitor_visible: false,
            use_jump_host: Some(!host.jump_hosts.is_empty()),
            legacy_jump_host: None,
            jump_hosts: host.jump_hosts,
        };

        if let Some(index) = existing_index {
            profiles[index] = profile;
            updated += 1;
        } else {
            profiles.push(profile);
            imported += 1;
        }
    }

    for profile in &mut profiles {
        sanitize_profile(profile);
    }
    write_profiles_to_disk(&profiles)?;

    if !group.is_empty() {
        let mut groups = load_configured_profile_groups()?;
        push_unique_group(&mut groups, group);
        write_profile_groups_to_disk(&groups)?;
    }

    let mut sanitized_profiles = profiles.clone();
    for profile in &mut sanitized_profiles {
        sanitize_profile(profile);
    }

    Ok(SshConfigImportResult {
        imported,
        updated,
        skipped,
        profiles: sanitized_profiles,
    })
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

    if !profile.remember_password || profile.auth_method.as_deref() == Some("key") {
        let _ = secret_state.delete_password(&app, profile.id.as_str());
        if !profile.name.trim().is_empty() {
            let _ = secret_state.delete_password(&app, profile.name.as_str());
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
        } else {
            let secret_key = crate::core::session::jump_host_identity_secret_key(
                Some(profile.id.as_str()),
                profile.name.as_str(),
                &jump.host,
                jump.port,
                &jump.username,
            );
            let _ = secret_state.delete_password(&app, &secret_key);
            if !profile.name.trim().is_empty() {
                let name_key = crate::core::session::jump_host_identity_secret_key(
                    None,
                    profile.name.as_str(),
                    &jump.host,
                    jump.port,
                    &jump.username,
                );
                let _ = secret_state.delete_password(&app, &name_key);
            }
        }
    }

    sanitize_profile(&mut profile);

    let mut profiles = load_profiles_from_disk()?;
    if let Some(pos) = profiles.iter().position(|p| p.id == profile.id) {
        let mut previous = profiles[pos].clone();
        normalize_profile(&mut previous);
        for summary in super::storage::profile_secret_summaries(&previous) {
            if !super::storage::profile_secret_summaries(&profile)
                .iter()
                .any(|current| current.key == summary.key)
            {
                let _ = secret_state.delete_password(&app, &summary.key);
            }
        }
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
pub fn list_profile_groups() -> Result<Vec<String>, String> {
    load_all_profile_groups()
}

#[tauri::command]
pub fn save_profile_group(name: String) -> Result<Vec<String>, String> {
    let name = normalize_group_name(&name);
    if name.is_empty() {
        return Err("Group name is required".to_string());
    }

    let mut groups = load_all_profile_groups()?;
    push_unique_group(&mut groups, name);
    write_profile_groups_to_disk(&groups)?;
    load_all_profile_groups()
}

#[tauri::command]
pub fn rename_profile_group(old_name: String, new_name: String) -> Result<Vec<String>, String> {
    let old_name = normalize_group_name(&old_name);
    let new_name = normalize_group_name(&new_name);
    if old_name.is_empty() || new_name.is_empty() {
        return Err("Group name is required".to_string());
    }

    let mut profiles = load_profiles_from_disk()?;
    let mut profiles_changed = false;
    for profile in &mut profiles {
        if normalize_group_name(&profile.group) == old_name {
            profile.group = new_name.clone();
            profiles_changed = true;
        }
    }
    if profiles_changed {
        write_profiles_to_disk(&profiles)?;
    }

    let mut groups = load_configured_profile_groups()?
        .into_iter()
        .filter(|group| normalize_group_name(group) != old_name)
        .collect::<Vec<_>>();
    push_unique_group(&mut groups, new_name);
    write_profile_groups_to_disk(&groups)?;
    load_all_profile_groups()
}

#[tauri::command]
pub fn delete_profile_group(name: String) -> Result<Vec<String>, String> {
    let name = normalize_group_name(&name);
    if name.is_empty() {
        return Err("Group name is required".to_string());
    }

    let mut profiles = load_profiles_from_disk()?;
    let mut profiles_changed = false;
    for profile in &mut profiles {
        if normalize_group_name(&profile.group) == name {
            profile.group = String::new();
            profiles_changed = true;
        }
    }
    if profiles_changed {
        write_profiles_to_disk(&profiles)?;
    }

    let groups = load_configured_profile_groups()?
        .into_iter()
        .filter(|group| normalize_group_name(group) != name)
        .collect::<Vec<_>>();
    write_profile_groups_to_disk(&groups)?;
    load_all_profile_groups()
}

#[tauri::command]
pub fn move_profile_to_group(
    id: String,
    group: String,
    target_id: Option<String>,
) -> Result<(), String> {
    let group = normalize_group_name(&group);
    let mut profiles = load_profiles_from_disk()?;
    let Some(source_index) = profiles.iter().position(|profile| profile.id == id) else {
        return Err("Profile not found".to_string());
    };

    let mut profile = profiles.remove(source_index);
    profile.group = group.clone();
    let insert_index = target_id
        .as_deref()
        .and_then(|target_id| profiles.iter().position(|profile| profile.id == target_id))
        .unwrap_or(profiles.len());
    profiles.insert(insert_index, profile);
    write_profiles_to_disk(&profiles)?;

    if !group.is_empty() {
        let mut groups = load_configured_profile_groups()?;
        push_unique_group(&mut groups, group);
        write_profile_groups_to_disk(&groups)?;
    }

    Ok(())
}

#[tauri::command]
pub fn delete_profile(
    app: tauri::AppHandle,
    secret_state: tauri::State<'_, crate::ssh::SecretStoreState>,
    id: String,
) -> Result<(), String> {
    let mut profiles = load_profiles_from_disk()?;
    if let Some(profile) = profiles.iter().find(|p| p.id == id).cloned() {
        let mut profile = profile;
        normalize_profile(&mut profile);
        delete_profile_secrets(&app, &secret_state, &profile)?;
    }
    profiles.retain(|p| p.id != id);
    write_profiles_to_disk(&profiles)
}

#[tauri::command]
pub fn set_profile_server_monitor_visible(id: String, visible: bool) -> Result<(), String> {
    let mut profiles = load_profiles_from_disk()?;
    if let Some(profile) = profiles.iter_mut().find(|profile| profile.id == id) {
        profile.server_monitor_visible = visible;
        write_profiles_to_disk(&profiles)?;
    }
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionResult {
    message: String,
    network_latency_ms: Option<u64>,
}

#[tauri::command]
pub async fn test_connection(
    app: tauri::AppHandle,
    profile: SavedProfile,
    prompt_state: tauri::State<'_, HostPromptMap>,
    secret_state: tauri::State<'_, crate::ssh::SecretStoreState>,
) -> Result<TestConnectionResult, String> {
    if profile.connection_type != "ssh" {
        return Err("Only SSH connections can be tested".to_string());
    }

    let host = profile.host.clone().ok_or("Host is required")?;
    let username = profile.username.clone().ok_or("Username is required")?;
    let port = profile.port.unwrap_or(22);

    let mut profile = profile;
    normalize_profile(&mut profile);

    let jump_hosts = if profile.uses_jump_host() {
        profile
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
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut plan = crate::core::session::SessionPlan {
        kind: crate::core::SessionKind::Ssh,
        profile_id: Some(profile.id.clone()),
        profile_name: profile.name.clone(),
        host: Some(host.clone()),
        port,
        username: Some(username.clone()),
        password: profile.password.clone(),
        ignore_saved_password: profile.ignore_saved_password,
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

    let test_tab_id = format!("test-{}", profile.id);
    crate::core::session::resolve_ssh_password(&app, &secret_state, &mut plan)?;

    crate::ssh::emit_connection_progress(
        &app,
        &test_tab_id,
        ConnectionStatusOptions::SILENT,
        SshConnectionProgressPayload::new(
            "resolving_credentials",
            "Resolved saved credentials for test connection",
        ),
    );

    use std::time::Duration;
    let network_latency_ms;

    if !plan.jump_hosts.is_empty() {
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
            test_connection_handler(
                &app,
                &test_tab_id,
                &plan,
                &host,
                port,
                prompt_state.inner().clone(),
                HostKeyVerificationMode::TrustUnknownForSession,
            ),
            target_config,
            prompt_state.inner().clone(),
            ConnectionStatusOptions::SILENT,
            HostKeyVerificationMode::TrustUnknownForSession,
        )
        .await?;

        crate::ssh::emit_connection_progress(
            &app,
            &test_tab_id,
            ConnectionStatusOptions::SILENT,
            SshConnectionProgressPayload::new(
                "target_authenticating",
                format!("Authenticating target as {}", username),
            )
            .host(host.clone(), port)
            .username(username.clone()),
        );

        let auth_result =
            authenticate_test_connection(&mut target_session, &username, &plan).await?;

        match auth_result {
            russh::client::AuthResult::Success => {
                network_latency_ms = crate::ssh::measure_ssh_latency(&target_session).await.ok();
                crate::ssh::emit_connection_progress(
                    &app,
                    &test_tab_id,
                    ConnectionStatusOptions::SILENT,
                    SshConnectionProgressPayload::new(
                        "ready",
                        format!("Successfully connected to {}@{}:{}", username, host, port),
                    )
                    .host(host.clone(), port)
                    .username(username.clone())
                    .network_latency(network_latency_ms),
                );
            }
            _ => return Err("Authentication failed".to_string()),
        }

        let _ = target_session
            .disconnect(russh::Disconnect::ByApplication, "", "")
            .await;
        drop(jump_chain);
    } else {
        use russh::client;

        let config = std::sync::Arc::new(crate::ssh::jump::compatibility_client_config(
            plan.keepalive_interval_secs as u64,
            plan.keepalive_count_max as usize,
        ));

        crate::ssh::emit_connection_progress(
            &app,
            &test_tab_id,
            ConnectionStatusOptions::SILENT,
            SshConnectionProgressPayload::new(
                "target_connecting",
                format!("Connecting to target {}:{}", host, port),
            )
            .host(host.clone(), port)
            .username(username.clone()),
        );

        let mut session = tokio::time::timeout(
            Duration::from_secs(10),
            client::connect(
                config,
                (host.as_str(), port),
                test_connection_handler(
                    &app,
                    &test_tab_id,
                    &plan,
                    &host,
                    port,
                    prompt_state.inner().clone(),
                    HostKeyVerificationMode::TrustUnknownForSession,
                ),
            ),
        )
        .await
        .map_err(|_| "Connection timeout".to_string())?
        .map_err(|e| format!("Connection failed: {}", e))?;

        crate::ssh::emit_connection_progress(
            &app,
            &test_tab_id,
            ConnectionStatusOptions::SILENT,
            SshConnectionProgressPayload::new(
                "target_authenticating",
                format!("Authenticating target as {}", username),
            )
            .host(host.clone(), port)
            .username(username.clone()),
        );

        let auth_result = authenticate_test_connection(&mut session, &username, &plan).await?;

        match auth_result {
            russh::client::AuthResult::Success => {
                network_latency_ms = crate::ssh::measure_ssh_latency(&session).await.ok();
                crate::ssh::emit_connection_progress(
                    &app,
                    &test_tab_id,
                    ConnectionStatusOptions::SILENT,
                    SshConnectionProgressPayload::new(
                        "ready",
                        format!("Successfully connected to {}@{}:{}", username, host, port),
                    )
                    .host(host.clone(), port)
                    .username(username.clone())
                    .network_latency(network_latency_ms),
                );
            }
            _ => return Err("Authentication failed".to_string()),
        }

        session
            .disconnect(russh::Disconnect::ByApplication, "", "")
            .await
            .ok();
    }

    Ok(TestConnectionResult {
        message: format!("Successfully connected to {}@{}:{}", username, host, port),
        network_latency_ms,
    })
}

fn test_connection_handler(
    app: &tauri::AppHandle,
    tab_id: &str,
    plan: &crate::core::session::SessionPlan,
    host: &str,
    port: u16,
    prompts: HostPromptMap,
    host_key_verification_mode: HostKeyVerificationMode,
) -> SshClientHandler {
    SshClientHandler {
        app: app.clone(),
        tab_id: tab_id.to_string(),
        profile_id: plan.profile_id.clone(),
        profile_name: plan.profile_name.clone(),
        host: host.to_string(),
        port,
        prompts,
        user_rejected_host_key: Arc::new(AtomicBool::new(false)),
        status_options: ConnectionStatusOptions::SILENT,
        host_key_verification_mode,
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
