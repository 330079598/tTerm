use super::types::{SavedProfile, SavedSecretSummary};
use crate::config::{ensure_config_dir, get_config_path};
use std::fs;

pub(crate) fn load_profiles_from_disk() -> Result<Vec<SavedProfile>, String> {
    let config_dir = get_config_path()?;
    let profiles_file = config_dir.join("profiles.json");
    if !profiles_file.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(&profiles_file)
        .map_err(|e| format!("Failed to read profiles file: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse profiles: {}", e))
}

pub(crate) fn normalize_profile(profile: &mut SavedProfile) {
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

pub(crate) fn sanitize_profile(profile: &mut SavedProfile) {
    normalize_profile(profile);
    profile.password = None;
    profile.ignore_saved_password = false;
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

pub(crate) fn write_profiles_to_disk(profiles: &[SavedProfile]) -> Result<(), String> {
    let config_dir = ensure_config_dir()?;
    let profiles_file = config_dir.join("profiles.json");
    let content = serde_json::to_string_pretty(profiles)
        .map_err(|e| format!("Failed to serialize profiles: {}", e))?;
    fs::write(&profiles_file, content).map_err(|e| format!("Failed to write profiles file: {}", e))
}

pub(crate) fn profile_secret_summaries(profile: &SavedProfile) -> Vec<SavedSecretSummary> {
    let mut summaries = Vec::new();

    if profile.connection_type == "ssh" && profile.auth_method.as_deref() != Some("key") {
        summaries.push(SavedSecretSummary {
            key: profile.id.clone(),
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
            label: profile.name.clone(),
            kind: "ssh".to_string(),
        });
        if !profile.name.trim().is_empty() {
            summaries.push(SavedSecretSummary {
                key: profile.name.clone(),
                profile_id: profile.id.clone(),
                profile_name: profile.name.clone(),
                label: profile.name.clone(),
                kind: "ssh-legacy-name".to_string(),
            });
        }
    }

    for jump in &profile.jump_hosts {
        if jump.auth_method == "key" {
            continue;
        }

        let label = format!(
            "{} via {}@{}:{}",
            profile.name, jump.username, jump.host, jump.port
        );
        summaries.push(SavedSecretSummary {
            key: crate::core::session::jump_host_identity_secret_key(
                Some(profile.id.as_str()),
                profile.name.as_str(),
                &jump.host,
                jump.port,
                &jump.username,
            ),
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
            label: label.clone(),
            kind: "jump-host".to_string(),
        });
        if !profile.name.trim().is_empty() {
            summaries.push(SavedSecretSummary {
                key: crate::core::session::jump_host_identity_secret_key(
                    None,
                    profile.name.as_str(),
                    &jump.host,
                    jump.port,
                    &jump.username,
                ),
                profile_id: profile.id.clone(),
                profile_name: profile.name.clone(),
                label,
                kind: "jump-host-legacy-name".to_string(),
            });
        }
    }

    summaries
}

pub fn saved_secret_summaries() -> Result<Vec<SavedSecretSummary>, String> {
    let profiles = load_profiles_from_disk()?;
    let mut summaries = Vec::new();

    for mut profile in profiles {
        normalize_profile(&mut profile);
        summaries.extend(profile_secret_summaries(&profile));
    }

    Ok(summaries)
}

pub fn saved_secret_keys() -> Result<Vec<String>, String> {
    Ok(saved_secret_summaries()?
        .into_iter()
        .map(|summary| summary.key)
        .collect())
}

pub(crate) fn delete_profile_secrets(
    app: &tauri::AppHandle,
    secret_state: &crate::ssh::SecretStoreState,
    profile: &SavedProfile,
) -> Result<usize, String> {
    let mut deleted = 0;
    for summary in profile_secret_summaries(profile) {
        if secret_state.delete_password(app, &summary.key)? {
            deleted += 1;
        }
    }
    Ok(deleted)
}

pub(crate) fn profile_groups_file_path() -> Result<std::path::PathBuf, String> {
    Ok(get_config_path()?.join("profile_groups.json"))
}

pub(crate) fn normalize_group_name(value: &str) -> String {
    value.trim().to_string()
}

pub(crate) fn push_unique_group(groups: &mut Vec<String>, group: String) {
    if group.is_empty() || groups.iter().any(|existing| existing == &group) {
        return;
    }

    groups.push(group);
}

pub(crate) fn load_configured_profile_groups() -> Result<Vec<String>, String> {
    let groups_file = profile_groups_file_path()?;
    if !groups_file.exists() {
        return Ok(vec![]);
    }

    let content = fs::read_to_string(&groups_file)
        .map_err(|e| format!("Failed to read profile groups file: {}", e))?;
    let raw_groups: Vec<String> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse profile groups: {}", e))?;

    let mut groups = Vec::new();
    for group in raw_groups {
        push_unique_group(&mut groups, normalize_group_name(&group));
    }

    Ok(groups)
}

pub(crate) fn write_profile_groups_to_disk(groups: &[String]) -> Result<(), String> {
    let config_dir = ensure_config_dir()?;
    let groups_file = config_dir.join("profile_groups.json");
    let mut normalized_groups = Vec::new();
    for group in groups {
        push_unique_group(&mut normalized_groups, normalize_group_name(group));
    }
    normalized_groups.sort_by(|left, right| left.to_lowercase().cmp(&right.to_lowercase()));

    let content = serde_json::to_string_pretty(&normalized_groups)
        .map_err(|e| format!("Failed to serialize profile groups: {}", e))?;
    fs::write(&groups_file, content)
        .map_err(|e| format!("Failed to write profile groups file: {}", e))
}

pub(crate) fn load_all_profile_groups() -> Result<Vec<String>, String> {
    let mut groups = load_configured_profile_groups()?;
    for profile in load_profiles_from_disk()? {
        push_unique_group(&mut groups, normalize_group_name(&profile.group));
    }
    groups.sort_by(|left, right| left.to_lowercase().cmp(&right.to_lowercase()));
    Ok(groups)
}
