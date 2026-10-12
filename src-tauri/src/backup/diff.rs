//! How an imported backup differs from what this device has.

use super::apply::{json_key, read_json_value, to_backup_value};
use super::remote;
use super::{BackupDiff, BackupPayload, CategoryDiff};
use crate::command_library::repository::CommandRepository;
use crate::config;
use serde_json::Value;
use std::collections::HashMap;

pub(super) fn calculate_diff(payload: &BackupPayload) -> Result<BackupDiff, String> {
    let existing_profiles = to_backup_value(&crate::profiles::storage::load_profiles()?)?;
    let profiles = calculate_json_array_diff(
        existing_profiles
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        payload
            .profiles
            .as_ref()
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]),
        &["id"],
    )?;

    let existing_commands = CommandRepository::new(crate::db::get()?).list()?;
    let existing_by_id = existing_commands
        .into_iter()
        .map(|command| (command.id.clone(), command))
        .collect::<HashMap<_, _>>();
    let mut commands = CategoryDiff::default();
    for command in payload.commands.as_deref().unwrap_or(&[]) {
        match existing_by_id.get(&command.id) {
            None => commands.added += 1,
            Some(existing) if existing == command => commands.unchanged += 1,
            Some(_) => commands.updated += 1,
        }
    }

    let settings_changed = match payload.config.as_ref() {
        Some(imported) => {
            read_json_value(&config::paths::get_config_path()?.join("config.json"))? != *imported
        }
        None => false,
    };
    let webdav_account = match payload.webdav_backup.as_ref() {
        Some(settings) => remote::account_change(settings)?,
        None => None,
    };
    Ok(BackupDiff {
        profiles,
        commands,
        settings_changed,
        webdav_account,
    })
}

fn calculate_json_array_diff(
    existing: &[Value],
    incoming: &[Value],
    keys: &[&str],
) -> Result<CategoryDiff, String> {
    let mut existing_by_key = HashMap::new();
    for value in existing {
        existing_by_key.insert(json_key(value, keys)?, value);
    }
    let mut diff = CategoryDiff::default();
    for value in incoming {
        match existing_by_key.get(&json_key(value, keys)?) {
            None => diff.added += 1,
            Some(existing) if **existing == *value => diff.unchanged += 1,
            Some(_) => diff.updated += 1,
        }
    }
    Ok(diff)
}

pub(super) fn value_array_len(value: Option<&Value>) -> usize {
    value.map_or(0, value_array_len_one)
}

pub(super) fn value_array_len_one(value: &Value) -> usize {
    value.as_array().map_or(0, Vec::len)
}
