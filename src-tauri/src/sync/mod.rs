//! Two-way sync of profiles, passwords, commands, known hosts, settings and
//! themes between devices, through one encrypted file in the WebDAV folder.
//!
//! A sync reads this device's data, fetches the remote file (skipped with
//! `If-None-Match` when it has not changed), merges both against the base
//! from the last sync, uploads the result with `If-Match` so a concurrent
//! upload from another device is merged rather than overwritten, and then
//! writes the result locally together with the new base.

mod document;
mod merge;
pub(crate) mod snapshot;
mod state;

use crate::backup::webdav::{Fetched, Stored, WebDavClient};
use crate::core::blocking::run_blocking;
use crate::db::meta;
use crate::ssh::secret_store::types::SecretStoreState;
use crate::tunnel::commands::TunnelManager;
use document::{Collections, MAX_DOCUMENT_SIZE, SYNC_FILE_NAME};
use merge::{merge_collection, same_data};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snapshot::{removed_ids, PROFILES, PROFILE_GROUPS, SERIAL_SENDS, SETTINGS, TUNNELS};
use std::sync::Mutex;
use tauri::{AppHandle, State};

use snapshot::SyncSelection;

pub(crate) const SETTINGS_KEY: &str = "settings.sync";
/// Where older versions kept the settings.
const SETTINGS_FILE: &str = "sync_settings.json";
/// Uploads retried after another device uploaded in between.
const MAX_ATTEMPTS: u32 = 4;
const RECOVERY_PREFIX: &str = "pre-sync";
const RECOVERY_KEEP: usize = 5;

/// One sync at a time; a tick that finds one running is skipped.
static SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Choices offered for how often local changes are looked for (seconds).
const PUSH_INTERVALS_SECS: [u32; 4] = [30, 60, 300, 900];
/// Choices offered for how often the server is checked (minutes).
const PULL_INTERVALS_MINS: [u32; 4] = [5, 15, 30, 60];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncSettings {
    pub enabled: bool,
    pub selection: SyncSelection,
    /// How often this device looks for its own changes and uploads them.
    /// Looking costs no network request.
    pub push_interval_secs: u32,
    /// How often this device downloads other devices' changes.
    pub pull_interval_mins: u32,
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            selection: SyncSelection::default(),
            push_interval_secs: PUSH_INTERVALS_SECS[0],
            pull_interval_mins: PULL_INTERVALS_MINS[0],
        }
    }
}

impl SyncSettings {
    fn validate(&self) -> Result<(), String> {
        if !PUSH_INTERVALS_SECS.contains(&self.push_interval_secs) {
            return Err(format!(
                "Upload check interval must be one of {PUSH_INTERVALS_SECS:?} seconds."
            ));
        }
        if !PULL_INTERVALS_MINS.contains(&self.pull_interval_mins) {
            return Err(format!(
                "Download interval must be one of {PULL_INTERVALS_MINS:?} minutes."
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub settings: SyncSettings,
    pub last_synced_at: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOutcome {
    /// `disabled`, `locked`, `busy`, `unchanged` or `synced`.
    pub state: &'static str,
    /// Categories whose local data changed.
    pub changed: Vec<String>,
    pub uploaded: bool,
    /// The recovery point saved before local data was changed.
    pub recovery_backup: Option<String>,
}

impl SyncOutcome {
    fn skipped(state: &'static str) -> Self {
        Self {
            state,
            ..Self::default()
        }
    }
}

#[tauri::command]
pub async fn get_sync_status() -> Result<SyncStatus, String> {
    run_blocking(status).await
}

#[tauri::command]
pub async fn save_sync_settings(settings: SyncSettings) -> Result<SyncStatus, String> {
    run_blocking(move || {
        settings.validate()?;
        crate::db::write(|transaction| write_settings(transaction, &settings))?;
        status()
    })
    .await
}

/// Forgets what was last synced, so the next sync merges like a first one:
/// both sides' data is kept and nothing is deleted.
#[tauri::command]
pub async fn reset_sync() -> Result<SyncStatus, String> {
    let _guard = SYNC_LOCK.lock().await;
    run_blocking(|| {
        crate::db::write(|transaction| state::clear(transaction))?;
        set_last_error(None);
        status()
    })
    .await
}

/// Syncs when this device has changes, or always when `check_remote` is
/// set. `frontend_state` is what the web view keeps, for the recovery backup
/// made before a sync deletes anything.
#[tauri::command]
pub async fn run_sync(
    app: AppHandle,
    frontend_state: Option<Value>,
    check_remote: bool,
    secret_state: State<'_, SecretStoreState>,
    tunnels: State<'_, TunnelManager>,
) -> Result<SyncOutcome, String> {
    let Ok(_guard) = SYNC_LOCK.try_lock() else {
        return Ok(SyncOutcome::skipped("busy"));
    };
    let result = sync(
        &app,
        frontend_state,
        check_remote,
        secret_state.inner(),
        tunnels.inner(),
    )
    .await;
    match &result {
        Ok(outcome) if matches!(outcome.state, "synced" | "unchanged") => set_last_error(None),
        Ok(_) => {}
        Err(error) => set_last_error(Some(error.clone())),
    }
    result
}

async fn sync(
    app: &AppHandle,
    frontend_state: Option<Value>,
    check_remote: bool,
    secret_state: &SecretStoreState,
    tunnels: &TunnelManager,
) -> Result<SyncOutcome, String> {
    let settings = run_blocking(load_settings).await?;
    if !settings.enabled {
        return Ok(SyncOutcome::skipped("disabled"));
    }
    let Ok(data_key) = secret_state.data_key() else {
        return Ok(SyncOutcome::skipped("locked"));
    };
    let Some(target) = crate::backup::remote::sync_target(app, secret_state).await? else {
        return Ok(SyncOutcome::skipped("disabled"));
    };
    let (categories, local, base) = {
        let data_key = data_key.clone();
        let target_key = target.key.clone();
        run_blocking(move || {
            crate::db::read(|connection| {
                let categories = settings
                    .selection
                    .categories(crate::themes::themes_imported(connection)?);
                let local = snapshot::collect(connection, &data_key, &categories)?;
                Ok((
                    categories,
                    local,
                    state::load(connection, &data_key, &target_key)?,
                ))
            })
        })
        .await?
    };
    let base_normalized = base
        .as_ref()
        .map(|base| normalize_lenient(&base.collections, &categories));
    if !check_remote && base_normalized.as_ref().is_some_and(|base| {
        categories
            .iter()
            .all(|category| matches!((local.get(*category), base.get(*category)), (Some(l), Some(b)) if same_data(l, b)))
    }) {
        return Ok(SyncOutcome::skipped("unchanged"));
    }

    let remote = RemoteFile {
        client: &target.client,
        directory: &target.directory,
        password: &target.password,
        device: &target.device,
        app_version: &target.app_version,
    };
    let exchanged = exchange(
        &remote,
        &categories,
        &local,
        base.as_ref(),
        base_normalized.as_ref(),
    )
    .await?;
    let uploaded = exchanged.uploaded;
    let mut outcome = apply(
        app,
        secret_state,
        tunnels,
        ApplyInput {
            data_key,
            target_key: target.key.clone(),
            categories,
            local,
            union: exchanged.union,
            agreed: exchanged.agreed,
            frontend_state,
        },
    )
    .await?;
    outcome.uploaded = uploaded;
    Ok(outcome)
}

/// The sync file on the server and what is needed to read and write it.
struct RemoteFile<'a> {
    client: &'a WebDavClient,
    directory: &'a [String],
    password: &'a str,
    device: &'a str,
    app_version: &'a str,
}

/// What this device and the server agreed on.
struct Exchanged {
    /// The merged collections as they are now on the server, with the
    /// file's key settings, revision history and ETag: the next base.
    agreed: state::SyncBase,
    /// Merged without a base (first sync or broken lineage).
    union: bool,
    uploaded: bool,
}

/// Downloads the sync file when it changed, merges it with `local`, and
/// uploads the result when it differs from the server's.
async fn exchange(
    remote: &RemoteFile<'_>,
    categories: &[&'static str],
    local: &Collections,
    base: Option<&state::SyncBase>,
    base_normalized: Option<&Collections>,
) -> Result<Exchanged, String> {
    let mut etag_hint = base.and_then(|base| base.etag.clone());
    for attempt in 0..MAX_ATTEMPTS {
        if attempt > 0 {
            // Let the other device's upload finish before reading again.
            tokio::time::sleep(std::time::Duration::from_millis(250 * u64::from(attempt))).await;
        }
        let fetched = remote
            .client
            .get_if_changed(
                remote.directory,
                SYNC_FILE_NAME,
                etag_hint.as_deref().filter(|_| base.is_some()),
                MAX_DOCUMENT_SIZE,
            )
            .await?;
        let (file, file_history, file_etag, kdf) = match fetched {
            Fetched::NotModified => match base {
                Some(base) => (
                    Some(base.collections.clone()),
                    base.history.clone(),
                    etag_hint.clone(),
                    base.kdf.clone(),
                ),
                None => {
                    etag_hint = None;
                    continue;
                }
            },
            Fetched::NotFound => (
                None,
                Vec::new(),
                None,
                base.and_then(|base| base.kdf.clone()),
            ),
            Fetched::Found { data, etag } => {
                let password = remote.password.to_string();
                let decoded = run_blocking(move || document::decode(&data, &password)).await?;
                (
                    Some(decoded.collections),
                    decoded.history,
                    etag,
                    Some(decoded.kdf),
                )
            }
        };
        // The base only tells what changed if the file was made from it.
        // Otherwise (another device overwrote our upload on a server
        // without If-Match, or this device was away for a long time) merge
        // like a first sync, which never deletes anything.
        let merge_base = base_normalized.filter(|_| {
            descends_from(
                file.is_some().then_some(file_history.as_slice()),
                base.and_then(|base| base.revision()),
            )
        });
        let merged = merge_all(categories, merge_base, local, file.as_ref());
        let uploaded = file
            .as_ref()
            .is_none_or(|file| !same_collections(file, &merged));
        let mut agreed = state::SyncBase {
            collections: merged,
            kdf,
            history: file_history,
            etag: file_etag,
        };
        if uploaded {
            let encoded = {
                let merged = agreed.collections.clone();
                let parent = agreed.history.clone();
                let kdf = agreed.kdf.clone();
                let password = remote.password.to_string();
                let (device, version) = (remote.device.to_string(), remote.app_version.to_string());
                run_blocking(move || {
                    document::encode(&merged, &parent, &password, kdf.as_ref(), &device, &version)
                })
                .await?
            };
            match remote
                .client
                .put_if_match(
                    remote.directory,
                    SYNC_FILE_NAME,
                    encoded.bytes,
                    agreed.etag.as_deref(),
                )
                .await?
            {
                Stored::Conflict => {
                    // Another device uploaded since the download; merge again.
                    etag_hint = None;
                    continue;
                }
                Stored::Stored { etag } => {
                    agreed.etag = etag;
                    agreed.kdf = Some(encoded.kdf);
                    agreed.history = encoded.history;
                }
            }
        }
        return Ok(Exchanged {
            agreed,
            union: merge_base.is_none(),
            uploaded,
        });
    }
    Err("The sync file kept changing on the server. Try again.".to_string())
}

struct ApplyInput {
    data_key: crate::ssh::secret_store::crypto::SecretKey,
    target_key: String,
    categories: Vec<&'static str>,
    local: Collections,
    /// Merged without a base (first sync or broken lineage).
    union: bool,
    agreed: state::SyncBase,
    frontend_state: Option<Value>,
}

/// Writes the merge result locally and saves it as the new base.
async fn apply(
    app: &AppHandle,
    secret_state: &SecretStoreState,
    tunnels: &TunnelManager,
    input: ApplyInput,
) -> Result<SyncOutcome, String> {
    let ApplyInput {
        data_key,
        target_key,
        categories,
        local,
        union,
        agreed,
        frontend_state,
    } = input;
    let mut normalized = Collections::new();
    for &category in &categories {
        let collection = agreed
            .collections
            .get(category)
            .cloned()
            .unwrap_or_default();
        normalized.insert(
            category.to_string(),
            snapshot::normalize(category, &collection)?,
        );
    }
    let changed: Vec<&'static str> = categories
        .iter()
        .copied()
        .filter(|category| !same_data(&local[*category], &normalized[*category]))
        .collect();
    // A full send history drops its oldest send on every new one, which is
    // no reason for a recovery backup.
    let deletes = changed
        .iter()
        .filter(|category| **category != SERIAL_SENDS)
        .any(|category| !removed_ids(&local[*category], &normalized[*category]).is_empty());

    let mut outcome = SyncOutcome {
        state: "synced",
        changed: changed
            .iter()
            .map(|category| category.to_string())
            .collect(),
        ..SyncOutcome::default()
    };
    if !changed.is_empty() && (union || deletes) {
        let (app, secret_state) = (app.clone(), secret_state.clone());
        let path = run_blocking(move || {
            crate::backup::automatic::create_recovery_backup(
                &app,
                RECOVERY_PREFIX,
                frontend_state,
                &secret_state,
                RECOVERY_KEEP,
            )
        })
        .await?;
        outcome.recovery_backup = Some(path.to_string_lossy().into_owned());
    }
    if changed.contains(&TUNNELS) {
        for id in removed_ids(&local[TUNNELS], &normalized[TUNNELS]) {
            tunnels.discard(&id).await;
        }
    }

    run_blocking(move || {
        crate::db::write(|transaction| {
            // Settings live in a file; writing them first means a failure
            // leaves the database, and so the base, untouched.
            if changed.contains(&SETTINGS) {
                snapshot::apply_settings(&normalized[SETTINGS])?;
            }
            for &category in &changed {
                if category != SETTINGS {
                    snapshot::apply_database(
                        transaction,
                        &data_key,
                        category,
                        &local[category],
                        &normalized[category],
                    )?;
                }
            }
            // Groups and profiles merged apart can leave a group in a loop or
            // a profile in a group another device deleted.
            if changed
                .iter()
                .any(|category| [PROFILES, PROFILE_GROUPS].contains(category))
            {
                crate::profiles::groups::repair(transaction)?;
            }
            state::save(
                transaction,
                &data_key,
                &target_key,
                &agreed,
                chrono::Utc::now().timestamp_millis(),
            )
        })
    })
    .await?;
    Ok(outcome)
}

/// Whether the remote file (its revision history, `None` when there is no
/// file) was made from the revision the base equals.
fn descends_from(remote_history: Option<&[String]>, base_revision: Option<&str>) -> bool {
    match remote_history {
        None => true,
        Some(history) => {
            base_revision.is_some_and(|revision| history.iter().any(|r| r == revision))
        }
    }
}

/// Merges the categories this device syncs and carries the others over
/// from the remote file unchanged. A category missing from the remote file
/// (it was deleted or never had it) is taken from this device as is.
fn merge_all(
    categories: &[&str],
    base: Option<&Collections>,
    local: &Collections,
    remote: Option<&Collections>,
) -> Collections {
    let mut merged = remote.cloned().unwrap_or_default();
    for &category in categories {
        let local = local.get(category).cloned().unwrap_or_default();
        let collection = match remote.and_then(|remote| remote.get(category)) {
            Some(remote) => {
                merge_collection(base.and_then(|base| base.get(category)), &local, remote).merged
            }
            None => local,
        };
        merged.insert(category.to_string(), collection);
    }
    merged
}

fn same_collections(left: &Collections, right: &Collections) -> bool {
    left.len() == right.len()
        && left.iter().all(|(category, collection)| {
            right
                .get(category)
                .is_some_and(|other| same_data(collection, other))
        })
}

/// Normalizes the base like the local side. A record that no longer fits
/// the model is kept as stored; it then just counts as changed remotely.
fn normalize_lenient(collections: &Collections, categories: &[&str]) -> Collections {
    let mut normalized = collections.clone();
    for &category in categories {
        if let Some(collection) = collections.get(category) {
            let entry = normalized.entry(category.to_string()).or_default();
            for (id, record) in collection {
                let single = merge::Collection::from([(id.clone(), record.clone())]);
                if let Ok(mut done) = snapshot::normalize(category, &single) {
                    if let Some(record) = done.remove(id) {
                        entry.insert(id.clone(), record);
                    }
                }
            }
        }
    }
    normalized
}

fn status() -> Result<SyncStatus, String> {
    Ok(SyncStatus {
        settings: load_settings()?,
        last_synced_at: crate::db::read(|connection| state::synced_at(connection, None))?,
        last_error: LAST_ERROR.lock().ok().and_then(|error| error.clone()),
    })
}

fn set_last_error(error: Option<String>) {
    if let Ok(mut last) = LAST_ERROR.lock() {
        *last = error;
    }
}

/// Applies the settings a backup carries.
pub(crate) fn restore_settings(
    connection: &rusqlite::Connection,
    settings: &SyncSettings,
) -> Result<(), String> {
    settings.validate()?;
    write_settings(connection, settings)
}

pub(crate) fn load_settings() -> Result<SyncSettings, String> {
    crate::db::read(read_settings)
}

pub(crate) fn read_settings(connection: &rusqlite::Connection) -> Result<SyncSettings, String> {
    Ok(meta::get(connection, SETTINGS_KEY)?.unwrap_or_default())
}

fn write_settings(
    connection: &rusqlite::Connection,
    settings: &SyncSettings,
) -> Result<(), String> {
    meta::set(connection, SETTINGS_KEY, settings)
}

/// Moves `sync_settings.json` from older versions into the database.
pub(crate) fn import_settings_file(
    database: &crate::db::Database,
    config_dir: &std::path::Path,
) -> Result<(), String> {
    meta::import_file(
        database,
        &config_dir.join(SETTINGS_FILE),
        |connection, bytes| {
            meta::set_if_absent(
                connection,
                SETTINGS_KEY,
                &meta::parse_file::<SyncSettings>(bytes)?,
            )
        },
    )
}

#[cfg(test)]
mod tests;
