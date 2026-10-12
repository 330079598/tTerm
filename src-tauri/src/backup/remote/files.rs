//! Backup file names on the server, and which backups to keep.

use super::{RemoteBackupEntry, FILE_EXTENSION, FILE_PREFIX, TIMESTAMP_FORMAT, TIMESTAMP_LEN};
use crate::backup::webdav::DavEntry;
use chrono::{DateTime, NaiveDateTime, Utc};
use tauri::AppHandle;

pub(super) fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

pub(super) fn user_agent(app: &AppHandle) -> String {
    format!("tTerm/{}", app.package_info().version)
}

pub(super) fn device_name() -> String {
    sanitize_device_name(&tauri_plugin_os::hostname())
}

/// A file-name-safe device label: `Stone's MacBook.local` -> `stone-s-macbook`.
pub(super) fn sanitize_device_name(hostname: &str) -> String {
    let hostname = hostname.trim();
    let hostname = hostname
        .strip_suffix(".local")
        .or_else(|| hostname.strip_suffix(".lan"))
        .unwrap_or(hostname);
    let mut name = String::new();
    for ch in hostname.chars() {
        if ch.is_ascii_alphanumeric() {
            name.push(ch.to_ascii_lowercase());
        } else if !name.ends_with('-') {
            name.push('-');
        }
    }
    let name: String = name.trim_matches('-').chars().take(32).collect();
    let name = name.trim_end_matches('-');
    if name.is_empty() {
        "device".to_string()
    } else {
        name.to_string()
    }
}

pub(super) fn backup_file_name(device: &str, at: DateTime<Utc>) -> String {
    format!(
        "{FILE_PREFIX}{device}-{}{FILE_EXTENSION}",
        at.format(TIMESTAMP_FORMAT)
    )
}

/// The device and creation time encoded in a name made by `backup_file_name`.
pub(super) fn parse_backup_file_name(name: &str) -> Option<(String, i64)> {
    let stem = name
        .strip_prefix(FILE_PREFIX)?
        .strip_suffix(FILE_EXTENSION)?;
    let split = stem.len().checked_sub(TIMESTAMP_LEN + 1)?;
    let (device, timestamp) = (stem.get(..split)?, stem.get(split + 1..)?);
    if device.is_empty() || stem.as_bytes()[split] != b'-' {
        return None;
    }
    let at = NaiveDateTime::parse_from_str(timestamp, TIMESTAMP_FORMAT).ok()?;
    Some((device.to_string(), at.and_utc().timestamp_millis()))
}

pub(super) fn remote_backups(entries: &[DavEntry], device: &str) -> Vec<RemoteBackupEntry> {
    let mut backups: Vec<_> = entries
        .iter()
        .filter(|entry| !entry.is_collection && entry.name.ends_with(FILE_EXTENSION))
        .map(|entry| {
            let parsed = parse_backup_file_name(&entry.name);
            RemoteBackupEntry {
                file_name: entry.name.clone(),
                size_bytes: entry.size,
                created_at: parsed.as_ref().map(|(_, at)| *at).or(entry.modified_at),
                current_device: parsed.as_ref().is_some_and(|(name, _)| name == device),
                device: parsed.map(|(name, _)| name),
            }
        })
        .collect();
    backups.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.file_name.cmp(&left.file_name))
    });
    backups
}

/// This device's uploads beyond the newest `retention`.
pub(super) fn stale_backups(entries: &[DavEntry], device: &str, retention: usize) -> Vec<String> {
    remote_backups(entries, device)
        .into_iter()
        .filter(|backup| backup.current_device)
        .skip(retention)
        .map(|backup| backup.file_name)
        .collect()
}

pub(super) fn validate_file_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 255
        || !name.ends_with(FILE_EXTENSION)
        || name.contains(['/', '\\'])
        || name.starts_with('.')
        || name.chars().any(char::is_control)
    {
        return Err("Invalid remote backup name.".to_string());
    }
    Ok(())
}
