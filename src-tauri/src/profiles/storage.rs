use super::types::{SavedProfile, SavedSecretSummary};
use crate::db::sql_error;
use rusqlite::{params, Connection, OptionalExtension};

/// Profiles in display order.
pub(crate) fn load_profiles() -> Result<Vec<SavedProfile>, String> {
    crate::db::read(list_saved_profiles)
}

pub(crate) fn find_profile(id: &str) -> Result<Option<SavedProfile>, String> {
    crate::db::read(|connection| get_profile(connection, id))
}

fn encode_profile(profile: &SavedProfile) -> Result<String, String> {
    let mut profile = profile.clone();
    sanitize_profile(&mut profile);
    serde_json::to_string(&profile).map_err(|e| format!("Failed to serialize profile: {e}"))
}

fn decode_profile(data: &str) -> Result<SavedProfile, String> {
    serde_json::from_str(data).map_err(|e| format!("Failed to parse profile: {e}"))
}

pub(crate) fn list_saved_profiles(connection: &Connection) -> Result<Vec<SavedProfile>, String> {
    let mut statement = connection
        .prepare("SELECT data FROM profiles ORDER BY position, rowid")
        .map_err(sql_error("Failed to read profiles"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql_error("Failed to read profiles"))?;
    rows.map(|data| decode_profile(&data.map_err(sql_error("Failed to read profiles"))?))
        .collect()
}

pub(crate) fn get_profile(
    connection: &Connection,
    id: &str,
) -> Result<Option<SavedProfile>, String> {
    connection
        .query_row("SELECT data FROM profiles WHERE id = ?1", [id], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(sql_error("Failed to read profile"))?
        .map(|data| decode_profile(&data))
        .transpose()
}

/// Whether a saved profile offers sudo password autofill; `None` when no
/// saved profile has this id (a connection that was never saved).
pub fn profile_sudo_autofill(profile_id: &str) -> Result<Option<bool>, String> {
    crate::db::read(|connection| get_profile(connection, profile_id))
        .map(|profile| profile.map(|profile| profile.sudo_autofill))
}

/// Updates the profile in place, or appends it after the last profile.
pub(crate) fn upsert_profile(
    connection: &Connection,
    profile: &SavedProfile,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO profiles (id, position, data) \
             VALUES (?1, (SELECT COALESCE(MAX(position), -1) + 1 FROM profiles), ?2) \
             ON CONFLICT(id) DO UPDATE SET data = excluded.data",
            params![profile.id, encode_profile(profile)?],
        )
        .map_err(sql_error("Failed to save profile"))?;
    Ok(())
}

pub(crate) fn delete_profiles(connection: &Connection, ids: &[String]) -> Result<usize, String> {
    let mut statement = connection
        .prepare("DELETE FROM profiles WHERE id = ?1")
        .map_err(sql_error("Failed to delete profile"))?;
    let mut deleted = 0;
    for id in ids {
        deleted += statement
            .execute([id])
            .map_err(sql_error("Failed to delete profile"))?;
    }
    Ok(deleted)
}

/// Replaces every profile, keeping the order given. A later duplicate id
/// replaces the earlier one, as merging JSON arrays by id used to. Profiles
/// that stay are updated in place so their saved passwords are kept; only
/// removed ones are deleted (with their passwords).
pub(crate) fn replace_profiles(
    connection: &Connection,
    profiles: &[SavedProfile],
) -> Result<(), String> {
    let kept = profiles
        .iter()
        .map(|profile| profile.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let removed = list_saved_profiles(connection)?
        .into_iter()
        .map(|profile| profile.id)
        .filter(|id| !kept.contains(id.as_str()))
        .collect::<Vec<_>>();
    delete_profiles(connection, &removed)?;
    let mut statement = connection
        .prepare(
            "INSERT INTO profiles (id, position, data) VALUES (?1, ?2, ?3) \
             ON CONFLICT(id) DO UPDATE SET position = excluded.position, data = excluded.data",
        )
        .map_err(sql_error("Failed to save profiles"))?;
    for (position, profile) in profiles.iter().enumerate() {
        statement
            .execute(params![
                profile.id,
                position as i64,
                encode_profile(profile)?
            ])
            .map_err(sql_error("Failed to save profiles"))?;
    }
    Ok(())
}

/// Moves a profile into `group`, placing it before `target_id` or last.
pub(crate) fn move_profile(
    connection: &Connection,
    id: &str,
    group: &str,
    target_id: Option<&str>,
) -> Result<(), String> {
    let mut ids = ordered_profile_ids(connection)?;
    let Some(source_index) = ids.iter().position(|existing| existing == id) else {
        return Err("Profile not found".to_string());
    };
    let mut profile = get_profile(connection, id)?.ok_or("Profile not found")?;
    profile.group = group.to_string();
    upsert_profile(connection, &profile)?;

    let moved = ids.remove(source_index);
    let insert_index = target_id
        .and_then(|target_id| ids.iter().position(|existing| existing == target_id))
        .unwrap_or(ids.len());
    ids.insert(insert_index, moved);

    let mut statement = connection
        .prepare("UPDATE profiles SET position = ?1 WHERE id = ?2 AND position != ?1")
        .map_err(sql_error("Failed to reorder profiles"))?;
    for (position, id) in ids.iter().enumerate() {
        statement
            .execute(params![position as i64, id])
            .map_err(sql_error("Failed to reorder profiles"))?;
    }
    Ok(())
}

fn ordered_profile_ids(connection: &Connection) -> Result<Vec<String>, String> {
    let mut statement = connection
        .prepare("SELECT id FROM profiles ORDER BY position, rowid")
        .map_err(sql_error("Failed to read profiles"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql_error("Failed to read profiles"))?;
    rows.collect::<Result<_, _>>()
        .map_err(sql_error("Failed to read profiles"))
}

/// Moves every profile in group `from` to group `to`. Returns how many moved.
pub(crate) fn regroup_profiles(
    connection: &Connection,
    from: &str,
    to: &str,
) -> Result<usize, String> {
    let mut moved = 0;
    for mut profile in list_saved_profiles(connection)? {
        if normalize_group_name(&profile.group) == from {
            profile.group = to.to_string();
            upsert_profile(connection, &profile)?;
            moved += 1;
        }
    }
    Ok(moved)
}

pub(crate) fn normalize_profile(profile: &mut SavedProfile) {
    if profile.jump_hosts.is_empty() {
        if let Some(jump) = profile.legacy_jump_host.take() {
            profile.jump_hosts.push(jump);
        }
    } else {
        profile.legacy_jump_host = None;
    }

    if profile.use_jump_host.is_none() {
        profile.use_jump_host = Some(!profile.jump_hosts.is_empty());
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
    profile.sudo_password = None;
    profile.clear_sudo_password = false;
    if let Some(jump) = &mut profile.legacy_jump_host {
        jump.password = None;
        jump.private_key_passphrase = None;
    }
    for jump in &mut profile.jump_hosts {
        jump.password = None;
        jump.private_key_passphrase = None;
    }
}

pub(crate) fn profile_secret_summaries(profile: &SavedProfile) -> Vec<SavedSecretSummary> {
    let mut summaries = Vec::new();

    if profile.connection_type == "ssh" {
        summaries.push(SavedSecretSummary {
            key: crate::core::session::sudo_secret_key(&profile.id),
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
            label: profile.name.clone(),
            kind: "sudo".to_string(),
        });
    }

    if profile.connection_type == "ssh"
        && !matches!(profile.auth_method.as_deref(), Some("key") | Some("agent"))
    {
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
        if matches!(jump.auth_method.as_str(), "key" | "agent") {
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
    let profiles = load_profiles()?;
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

pub(crate) fn normalize_group_name(value: &str) -> String {
    value.trim().to_string()
}

pub(crate) fn push_unique_group(groups: &mut Vec<String>, group: String) {
    if group.is_empty() || groups.iter().any(|existing| existing == &group) {
        return;
    }

    groups.push(group);
}

/// Groups created explicitly, including ones no profile uses.
pub(crate) fn configured_profile_groups(connection: &Connection) -> Result<Vec<String>, String> {
    let mut statement = connection
        .prepare("SELECT name FROM profile_groups ORDER BY lower(name), name")
        .map_err(sql_error("Failed to read profile groups"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql_error("Failed to read profile groups"))?;
    rows.collect::<Result<_, _>>()
        .map_err(sql_error("Failed to read profile groups"))
}

pub(crate) fn add_profile_group(connection: &Connection, name: &str) -> Result<(), String> {
    let name = normalize_group_name(name);
    if name.is_empty() {
        return Ok(());
    }
    connection
        .execute(
            "INSERT OR IGNORE INTO profile_groups (name) VALUES (?1)",
            [name],
        )
        .map_err(sql_error("Failed to save profile group"))?;
    Ok(())
}

pub(crate) fn remove_profile_group(connection: &Connection, name: &str) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM profile_groups WHERE name = ?1",
            [normalize_group_name(name)],
        )
        .map_err(sql_error("Failed to delete profile group"))?;
    Ok(())
}

pub(crate) fn replace_profile_groups(
    connection: &Connection,
    groups: &[String],
) -> Result<(), String> {
    connection
        .execute("DELETE FROM profile_groups", [])
        .map_err(sql_error("Failed to clear profile groups"))?;
    for group in groups {
        add_profile_group(connection, group)?;
    }
    Ok(())
}

/// Configured groups plus every group a profile is in, sorted by name.
pub(crate) fn all_profile_groups(connection: &Connection) -> Result<Vec<String>, String> {
    let mut groups = configured_profile_groups(connection)?;
    for profile in list_saved_profiles(connection)? {
        push_unique_group(&mut groups, normalize_group_name(&profile.group));
    }
    groups.sort_by(|left, right| left.to_lowercase().cmp(&right.to_lowercase()));
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn profile(id: &str, group: &str) -> SavedProfile {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "name": id.to_uppercase(),
            "group": group,
            "connection_type": "ssh",
            "host": "example.com",
            "port": 22,
            "username": "root",
            "auth_method": "password",
            "private_key_path": null
        }))
        .expect("profile fixture")
    }

    fn ids(connection: &Connection) -> Vec<String> {
        list_saved_profiles(connection)
            .expect("list profiles")
            .into_iter()
            .map(|profile| profile.id)
            .collect()
    }

    #[test]
    fn upsert_appends_new_profiles_and_updates_in_place() {
        let database = Database::open_in_memory().expect("open database");
        database
            .write(|connection| {
                upsert_profile(connection, &profile("a", ""))?;
                upsert_profile(connection, &profile("b", ""))?;
                let mut updated = profile("a", "ops");
                updated.password = Some("secret".to_string());
                upsert_profile(connection, &updated)?;
                assert_eq!(ids(connection), ["a", "b"]);
                let stored = get_profile(connection, "a")?.expect("profile a");
                assert_eq!(stored.group, "ops");
                assert!(stored.password.is_none());
                Ok(())
            })
            .expect("write profiles");
    }

    #[test]
    fn move_profile_regroups_and_reorders() {
        let database = Database::open_in_memory().expect("open database");
        database
            .write(|connection| {
                for id in ["a", "b", "c", "d"] {
                    upsert_profile(connection, &profile(id, ""))?;
                }
                move_profile(connection, "d", "ops", Some("b"))?;
                assert_eq!(ids(connection), ["a", "d", "b", "c"]);
                assert_eq!(get_profile(connection, "d")?.unwrap().group, "ops");

                move_profile(connection, "a", "", None)?;
                assert_eq!(ids(connection), ["d", "b", "c", "a"]);

                assert!(move_profile(connection, "missing", "", None).is_err());
                Ok(())
            })
            .expect("move profiles");
    }

    #[test]
    fn group_rename_and_delete_touch_matching_profiles_only() {
        let database = Database::open_in_memory().expect("open database");
        database
            .write(|connection| {
                upsert_profile(connection, &profile("a", " ops "))?;
                upsert_profile(connection, &profile("b", "dev"))?;
                add_profile_group(connection, "ops")?;
                add_profile_group(connection, "empty")?;

                assert_eq!(regroup_profiles(connection, "ops", "prod")?, 1);
                remove_profile_group(connection, "ops")?;
                add_profile_group(connection, "prod")?;
                assert_eq!(get_profile(connection, "a")?.unwrap().group, "prod");
                assert_eq!(get_profile(connection, "b")?.unwrap().group, "dev");
                assert_eq!(all_profile_groups(connection)?, ["dev", "empty", "prod"]);

                regroup_profiles(connection, "prod", "")?;
                remove_profile_group(connection, "prod")?;
                assert_eq!(all_profile_groups(connection)?, ["dev", "empty"]);
                Ok(())
            })
            .expect("edit groups");
    }

    #[test]
    fn delete_profiles_removes_only_the_given_ids() {
        let database = Database::open_in_memory().expect("open database");
        database
            .write(|connection| {
                for id in ["a", "b", "c"] {
                    upsert_profile(connection, &profile(id, ""))?;
                }
                let deleted = delete_profiles(
                    connection,
                    &["a".to_string(), "c".to_string(), "x".to_string()],
                )?;
                assert_eq!(deleted, 2);
                assert_eq!(ids(connection), ["b"]);
                Ok(())
            })
            .expect("delete profiles");
    }

    #[test]
    fn legacy_jump_host_profiles_remain_enabled_after_migration() {
        let mut profile: SavedProfile = serde_json::from_value(serde_json::json!({
            "id": "legacy",
            "name": "Legacy",
            "connection_type": "ssh",
            "jump_hosts": [{
                "host": "bastion.example.com",
                "port": 22,
                "username": "ops"
            }]
        }))
        .expect("legacy profile should deserialize");

        normalize_profile(&mut profile);

        assert_eq!(profile.use_jump_host, Some(true));
        assert!(profile.uses_jump_host());
    }

    #[test]
    fn disabled_jump_host_profiles_keep_their_chain() {
        let mut profile: SavedProfile = serde_json::from_value(serde_json::json!({
            "id": "disabled",
            "name": "Disabled",
            "connection_type": "ssh",
            "use_jump_host": false,
            "jump_hosts": [{
                "host": "bastion.example.com",
                "port": 22,
                "username": "ops"
            }]
        }))
        .expect("disabled profile should deserialize");

        normalize_profile(&mut profile);

        assert_eq!(profile.use_jump_host, Some(false));
        assert_eq!(profile.jump_hosts.len(), 1);
        assert!(!profile.uses_jump_host());
    }
}
