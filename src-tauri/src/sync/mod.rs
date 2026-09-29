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
mod snapshot;
mod state;

use crate::backup::webdav::{Fetched, Stored, WebDavClient};
use crate::config;
use crate::core::blocking::run_blocking;
use crate::ssh::SecretStoreState;
use crate::tunnel::TunnelManager;
use document::{Collections, MAX_DOCUMENT_SIZE, SYNC_FILE_NAME};
use merge::{merge_collection, same_data};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snapshot::{removed_ids, SETTINGS, THEMES, TUNNELS};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, State};

pub use snapshot::SyncSelection;

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
    /// The custom theme list to store, when themes changed.
    pub custom_themes: Option<Value>,
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
        save_settings(&settings)?;
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
/// set. `frontend_state` carries the custom themes kept in the web view.
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
    let categories = settings.selection.categories(frontend_state.is_some());

    let (local, base) = {
        let data_key = data_key.clone();
        let categories = categories.clone();
        let frontend_state = frontend_state.clone();
        let target_key = target.key.clone();
        run_blocking(move || {
            crate::db::read(|connection| {
                Ok((
                    snapshot::collect(connection, &data_key, &categories, frontend_state.as_ref())?,
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
    data_key: crate::ssh::secret_store::DataKey,
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
    let deletes = changed
        .iter()
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
            crate::backup::create_recovery_backup(
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
    if changed.contains(&THEMES) {
        outcome.custom_themes = Some(snapshot::themes_value(&normalized[THEMES]));
    }

    run_blocking(move || {
        crate::db::write(|transaction| {
            // Settings live in a file; writing them first means a failure
            // leaves the database, and so the base, untouched.
            if changed.contains(&SETTINGS) {
                snapshot::apply_settings(&normalized[SETTINGS])?;
            }
            for &category in &changed {
                if category != SETTINGS && category != THEMES {
                    snapshot::apply_database(
                        transaction,
                        &data_key,
                        category,
                        &local[category],
                        &normalized[category],
                    )?;
                }
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

fn settings_path() -> Result<PathBuf, String> {
    Ok(config::ensure_config_dir()?.join(SETTINGS_FILE))
}

fn load_settings() -> Result<SyncSettings, String> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(SyncSettings::default());
    }
    let bytes =
        fs::read(&path).map_err(|error| format!("Failed to read sync settings: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Failed to parse sync settings: {error}"))
}

fn save_settings(settings: &SyncSettings) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Failed to serialize sync settings: {error}"))?;
    config::atomic_write(&settings_path()?, bytes)
}

#[cfg(test)]
mod tests {
    use super::snapshot::{COMMANDS, KNOWN_HOSTS, PROFILES, PROFILE_GROUPS, SECRETS};
    use super::*;
    use crate::command_library::{CommandRepository, SavedCommand};
    use crate::db::Database;
    use crate::ssh::secret_store::{get_secret, put_secret, DataKey};
    use crate::ssh::store::KnownHostRecord;
    use serde_json::json;

    const CATEGORIES: [&str; 6] = [
        PROFILES,
        PROFILE_GROUPS,
        TUNNELS,
        SECRETS,
        COMMANDS,
        KNOWN_HOSTS,
    ];

    /// The server's file: collections and revision history.
    type Remote = Option<(Collections, Vec<String>)>;

    /// One device: its database, data key and last synced base.
    struct Device {
        database: Database,
        key: DataKey,
        base: Option<(Collections, Vec<String>)>,
        /// The base when syncing through a real server.
        live_base: Option<state::SyncBase>,
    }

    impl Device {
        fn new() -> Self {
            Self {
                database: Database::open_in_memory().unwrap(),
                key: DataKey::generate(),
                base: None,
                live_base: None,
            }
        }

        fn collect(&self) -> Collections {
            self.database
                .read(|connection| snapshot::collect(connection, &self.key, &CATEGORIES, None))
                .unwrap()
        }

        /// The sync without the network: merge with `remote`, apply, upload.
        fn sync(&mut self, remote: &mut Remote) -> Vec<&'static str> {
            let (merged, history) = self.merge(remote.as_ref());
            let changed = self.apply(&merged);
            *remote = Some((merged.clone(), history.clone()));
            self.base = Some((merged, history));
            changed
        }

        /// Merges with `remote` and returns the result and its new history.
        fn merge(&self, remote: Option<&(Collections, Vec<String>)>) -> (Collections, Vec<String>) {
            let local = self.collect();
            let base_revision = self.base.as_ref().and_then(|(_, history)| history.last());
            let base = self
                .base
                .as_ref()
                .filter(|_| {
                    descends_from(
                        remote.map(|(_, history)| history.as_slice()),
                        base_revision.map(String::as_str),
                    )
                })
                .map(|(base, _)| normalize_lenient(base, &CATEGORIES));
            let merged = merge_all(&CATEGORIES, base.as_ref(), &local, remote.map(|(c, _)| c));
            let mut history = remote
                .map(|(_, history)| history.clone())
                .unwrap_or_default();
            history.push(uuid::Uuid::new_v4().to_string());
            (merged, history)
        }

        /// A sync through a real server, as `sync` does it minus the app.
        async fn sync_live(&mut self, remote: &RemoteFile<'_>) -> Exchanged {
            let local = self.collect();
            let base_normalized = self
                .live_base
                .as_ref()
                .map(|base| normalize_lenient(&base.collections, &CATEGORIES));
            let exchanged = exchange(
                remote,
                &CATEGORIES,
                &local,
                self.live_base.as_ref(),
                base_normalized.as_ref(),
            )
            .await
            .unwrap();
            self.apply(&exchanged.agreed.collections);
            self.live_base = Some(state::SyncBase {
                collections: exchanged.agreed.collections.clone(),
                kdf: exchanged.agreed.kdf.clone(),
                history: exchanged.agreed.history.clone(),
                etag: exchanged.agreed.etag.clone(),
            });
            exchanged
        }

        fn apply(&self, merged: &Collections) -> Vec<&'static str> {
            let local = self.collect();
            let mut changed = Vec::new();
            self.database
                .write(|transaction| {
                    for category in CATEGORIES {
                        let normalized = snapshot::normalize(category, &merged[category])?;
                        if !same_data(&local[category], &normalized) {
                            changed.push(category);
                            snapshot::apply_database(
                                transaction,
                                &self.key,
                                category,
                                &local[category],
                                &normalized,
                            )?;
                        }
                    }
                    Ok(())
                })
                .unwrap();
            changed
        }

        fn save_profile(&self, id: &str, host: &str, group: &str) {
            let profile = serde_json::from_value(json!({
                "id": id, "name": id, "group": group, "connection_type": "ssh",
                "host": host, "port": 22, "username": "root",
                "auth_method": "password", "private_key_path": null
            }))
            .unwrap();
            self.database
                .write(|transaction| crate::profiles::upsert_profile(transaction, &profile))
                .unwrap();
        }

        fn profile(&self, id: &str) -> Option<crate::profiles::SavedProfile> {
            self.database
                .read(|connection| crate::profiles::get_profile(connection, id))
                .unwrap()
        }

        fn delete_profile(&self, id: &str) {
            self.database
                .write(|transaction| {
                    crate::profiles::delete_profile_rows(transaction, &[id.to_string()])
                })
                .unwrap();
        }

        fn save_secret(&self, key: &str, value: &str) {
            self.database
                .write(|transaction| put_secret(transaction, &self.key, key, value))
                .unwrap();
        }

        fn secret(&self, key: &str) -> Option<String> {
            self.database
                .read(|connection| get_secret(connection, &self.key, key))
                .unwrap()
                .map(|value| value.to_string())
        }
    }

    fn command(id: &str, text: &str) -> SavedCommand {
        serde_json::from_value(json!({
            "id": id, "name": id, "commandText": text, "description": "",
            "scopeType": "global", "scopeId": null, "shellType": "any", "platform": "any",
            "isFavorite": false, "confirmBeforeRun": false, "sortOrder": 0,
            "useCount": 0, "lastUsedAt": null, "createdAt": 1, "updatedAt": 1,
            "tags": ["ops"], "variables": []
        }))
        .unwrap()
    }

    #[test]
    fn two_devices_converge_and_deletions_travel() {
        let mut remote = None;
        let (mut a, mut b) = (Device::new(), Device::new());
        a.save_profile("p1", "one.example", "prod");
        a.save_profile("p2", "two.example", "prod");
        a.save_secret("p1", "secret-1");
        a.save_secret("webdav:password", "never-synced");
        b.save_profile("p3", "three.example", "dev");

        assert!(a.sync(&mut remote).is_empty());
        assert_eq!(b.sync(&mut remote), [PROFILES, SECRETS]);
        assert_eq!(b.secret("p1").as_deref(), Some("secret-1"));
        assert_eq!(a.sync(&mut remote), [PROFILES]);
        assert!(a.profile("p3").is_some());
        assert_eq!(
            b.profile("p1").unwrap().host.as_deref(),
            Some("one.example")
        );
        assert!(b.secret("webdav:password").is_none());

        // A deletes a profile; B deletes nothing but receives the deletion.
        a.delete_profile("p2");
        a.sync(&mut remote);
        b.sync(&mut remote);
        assert!(b.profile("p2").is_none());
        assert!(b.profile("p1").is_some());

        // Both edit different fields of the same profile.
        let mut edited = b.profile("p1").unwrap();
        edited.port = Some(2222);
        b.database
            .write(|transaction| crate::profiles::upsert_profile(transaction, &edited))
            .unwrap();
        a.save_profile("p1", "renamed.example", "prod");
        a.sync(&mut remote);
        b.sync(&mut remote);
        a.sync(&mut remote);
        for device in [&a, &b] {
            let profile = device.profile("p1").unwrap();
            assert_eq!(profile.host.as_deref(), Some("renamed.example"));
            assert_eq!(profile.port, Some(2222));
        }

        // Nothing left to do on either side.
        assert!(a.sync(&mut remote).is_empty());
        assert!(b.sync(&mut remote).is_empty());
        assert!(same_collections(&a.collect(), &b.collect()));
    }

    #[test]
    fn an_overwritten_upload_is_merged_back_instead_of_deleted() {
        let mut remote = None;
        let (mut a, mut b) = (Device::new(), Device::new());
        a.save_profile("p1", "one.example", "");
        a.sync(&mut remote);
        b.sync(&mut remote);

        // Both read the same file; A uploads first, then B uploads over it
        // (a server that ignores If-Match lets the stale upload through).
        let seen_by_b = remote.clone();
        a.save_profile("p2", "from-a.example", "");
        a.sync(&mut remote);
        b.save_profile("p3", "from-b.example", "");
        let (merged, history) = b.merge(seen_by_b.as_ref());
        b.apply(&merged);
        remote = Some((merged.clone(), history.clone()));
        b.base = Some((merged, history));

        // A's profile is missing from the file, but A must not take that as a deletion.
        a.sync(&mut remote);
        b.sync(&mut remote);
        for device in [&a, &b] {
            for id in ["p1", "p2", "p3"] {
                assert!(device.profile(id).is_some(), "{id} lost");
            }
        }

        // Deletions work again once the lineage is back.
        a.delete_profile("p1");
        a.sync(&mut remote);
        b.sync(&mut remote);
        assert!(b.profile("p1").is_none());
    }

    #[test]
    fn sync_settings_default_and_validate_intervals() {
        let settings: SyncSettings = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
        assert_eq!(
            (settings.push_interval_secs, settings.pull_interval_mins),
            (30, 5)
        );
        assert!(settings.validate().is_ok());
        let odd = SyncSettings {
            push_interval_secs: 1,
            ..SyncSettings::default()
        };
        assert!(odd.validate().is_err());
        let odd = SyncSettings {
            pull_interval_mins: 0,
            ..SyncSettings::default()
        };
        assert!(odd.validate().is_err());
    }

    #[test]
    fn lineage_decides_whether_the_base_applies() {
        let history = ["r1".to_string(), "r2".to_string()];
        assert!(descends_from(Some(&history), Some("r1")));
        assert!(descends_from(Some(&history), Some("r2")));
        assert!(!descends_from(Some(&history), Some("r0")));
        assert!(!descends_from(Some(&history), None));
        assert!(descends_from(None, None));
    }

    #[test]
    fn passwords_sync_and_follow_their_profile() {
        let mut remote = None;
        let (mut a, mut b) = (Device::new(), Device::new());
        a.save_profile("p1", "one.example", "");
        a.save_secret("p1", "first");
        a.save_secret("p1:sudo", "sudo-pass");
        a.sync(&mut remote);
        b.sync(&mut remote);
        assert_eq!(b.secret("p1").as_deref(), Some("first"));
        assert_eq!(b.secret("p1:sudo").as_deref(), Some("sudo-pass"));

        b.save_secret("p1", "second");
        b.sync(&mut remote);
        a.sync(&mut remote);
        assert_eq!(a.secret("p1").as_deref(), Some("second"));

        // Deleting the profile deletes its passwords everywhere.
        a.delete_profile("p1");
        a.sync(&mut remote);
        b.sync(&mut remote);
        assert!(b.profile("p1").is_none());
        assert!(b.secret("p1").is_none());
        assert!(b.secret("p1:sudo").is_none());
    }

    #[test]
    fn commands_sync_but_usage_counts_stay_per_device() {
        let mut remote = None;
        let (mut a, mut b) = (Device::new(), Device::new());
        CommandRepository::new(&a.database)
            .save(&command("c1", "uptime"))
            .unwrap();
        a.sync(&mut remote);
        b.sync(&mut remote);
        let command_b = |device: &Device| {
            CommandRepository::new(&device.database)
                .get("c1")
                .unwrap()
                .unwrap()
        };
        assert_eq!(command_b(&b).tags, ["ops"]);

        // Using a command changes nothing to sync.
        CommandRepository::new(&b.database)
            .record_use("c1", 99)
            .unwrap();
        assert!(b.sync(&mut remote).is_empty());

        let mut edited = command("c1", "uptime -p");
        edited.updated_at = 2;
        CommandRepository::new(&a.database).save(&edited).unwrap();
        a.sync(&mut remote);
        b.sync(&mut remote);
        let synced = command_b(&b);
        assert_eq!(synced.command_text, "uptime -p");
        assert_eq!(synced.use_count, 1);
        assert_eq!(synced.last_used_at, Some(99));
    }

    #[test]
    fn groups_tunnels_and_known_hosts_sync() {
        let mut remote = None;
        let (mut a, mut b) = (Device::new(), Device::new());
        let rule: crate::tunnel::TunnelRule = serde_json::from_value(json!({
            "id": "t1", "name": "db", "profileId": "p1", "kind": "local",
            "bindHost": "127.0.0.1", "bindPort": 8080, "destHost": "db", "destPort": 5432
        }))
        .unwrap();
        a.database
            .write(|transaction| {
                crate::profiles::replace_profile_groups(transaction, &["Empty group".to_string()])?;
                crate::tunnel::replace_tunnels(transaction, &[rule])?;
                crate::ssh::store::merge_known_host(
                    transaction,
                    &KnownHostRecord {
                        profile_id: Some("p1".to_string()),
                        profile_name: "p1".to_string(),
                        host: "one.example".to_string(),
                        port: 22,
                        algorithm: "ssh-ed25519".to_string(),
                        fingerprint: "SHA256:abc".to_string(),
                        trusted_at: 5,
                    },
                )
            })
            .unwrap();
        a.sync(&mut remote);
        assert_eq!(b.sync(&mut remote), [PROFILE_GROUPS, TUNNELS, KNOWN_HOSTS]);
        b.database
            .read(|connection| {
                assert_eq!(
                    crate::profiles::configured_profile_groups(connection)?,
                    ["Empty group"]
                );
                assert_eq!(
                    crate::tunnel::list_tunnel_rules(connection)?[0].dest_port,
                    5432
                );
                assert_eq!(
                    crate::ssh::store::list_known_hosts(connection)?.entries[0].fingerprint,
                    "SHA256:abc"
                );
                Ok(())
            })
            .unwrap();
        assert!(b.sync(&mut remote).is_empty());
    }

    #[test]
    fn categories_this_device_skips_are_carried_over() {
        let mut remote = Some(Collections::from([(
            "themes".to_string(),
            merge::Collection::from([(
                "t".to_string(),
                merge::SyncRecord::new(json!({"id": "t"}), None),
            )]),
        )]));
        let local = Collections::from([(PROFILES.to_string(), merge::Collection::new())]);
        let merged = merge_all(&[PROFILES], None, &local, remote.as_ref());
        assert_eq!(merged["themes"], remote.as_ref().unwrap()["themes"]);
        assert!(merged[PROFILES].is_empty());

        // A remote file without a category never deletes local records.
        let local = Collections::from([(
            PROFILES.to_string(),
            merge::Collection::from([(
                "p".to_string(),
                merge::SyncRecord::new(json!({"a": 1}), None),
            )]),
        )]);
        let base = local.clone();
        remote.as_mut().unwrap().remove(PROFILES);
        let merged = merge_all(&[PROFILES], Some(&base), &local, remote.as_ref());
        assert_eq!(merged[PROFILES], local[PROFILES]);
    }

    #[test]
    fn fields_from_a_newer_version_survive_an_older_device() {
        let mut remote = None;
        let (mut a, mut b) = (Device::new(), Device::new());
        a.save_profile("p1", "one.example", "");
        a.sync(&mut remote);
        // A newer tTerm adds a field B does not know.
        remote
            .as_mut()
            .unwrap()
            .0
            .get_mut(PROFILES)
            .unwrap()
            .get_mut("p1")
            .unwrap()
            .data["futureField"] = json!("keep me");
        b.sync(&mut remote);
        // B edits the profile; the unknown field must stay in the remote file.
        let mut edited = b.profile("p1").unwrap();
        edited.port = Some(2200);
        b.database
            .write(|transaction| crate::profiles::upsert_profile(transaction, &edited))
            .unwrap();
        b.sync(&mut remote);
        let record = &remote.as_ref().unwrap().0[PROFILES]["p1"].data;
        assert_eq!(record["futureField"], "keep me");
        assert_eq!(record["port"], 2200);
        assert!(b.sync(&mut remote).is_empty());
    }

    /// Two devices syncing through a real server:
    /// `TTERM_WEBDAV_URL=... TTERM_WEBDAV_USER=... TTERM_WEBDAV_PASSWORD=... cargo test live_devices -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn live_devices_sync_through_a_server() {
        let env = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
        let client = WebDavClient::new(
            &env("TTERM_WEBDAV_URL"),
            &env("TTERM_WEBDAV_USER"),
            zeroize::Zeroizing::new(env("TTERM_WEBDAV_PASSWORD")),
            "tTerm-test",
        )
        .unwrap();
        let directory = vec![format!("tterm-sync-{}", std::process::id())];
        client.ensure_directory(&directory).await.unwrap();
        let remote = |device| RemoteFile {
            client: &client,
            directory: &directory,
            password: "sync-password",
            device,
            app_version: "test",
        };
        let (mut a, mut b) = (Device::new(), Device::new());

        a.save_profile("p1", "one.example", "prod");
        a.save_profile("p2", "two.example", "prod");
        a.save_secret("p1", "secret-1");
        b.save_profile("p3", "three.example", "dev");
        let first = a.sync_live(&remote("a")).await;
        assert!(first.uploaded && first.union);
        let joined = b.sync_live(&remote("b")).await;
        assert!(
            joined.uploaded && joined.union,
            "B adds p3 on its first sync"
        );
        a.sync_live(&remote("a")).await;
        for device in [&a, &b] {
            for id in ["p1", "p2", "p3"] {
                assert!(device.profile(id).is_some(), "{id} missing");
            }
        }
        assert_eq!(b.secret("p1").as_deref(), Some("secret-1"));

        // Nothing changed: no upload, and the file is not downloaded again
        // when the server honors If-None-Match.
        let idle = b.sync_live(&remote("b")).await;
        assert!(!idle.uploaded && !idle.union);

        // A deletes, B edits another field of a profile both have.
        a.delete_profile("p2");
        let mut edited = b.profile("p1").unwrap();
        edited.port = Some(2222);
        b.database
            .write(|transaction| crate::profiles::upsert_profile(transaction, &edited))
            .unwrap();
        a.sync_live(&remote("a")).await;
        let merged = b.sync_live(&remote("b")).await;
        assert!(merged.uploaded && !merged.union);
        a.sync_live(&remote("a")).await;
        for device in [&a, &b] {
            assert!(device.profile("p2").is_none(), "deletion travels");
            assert_eq!(device.profile("p1").unwrap().port, Some(2222));
        }
        assert!(same_collections(&a.collect(), &b.collect()));

        // A device that forgot its base merges without deleting anything.
        b.save_profile("p9", "local-only.example", "");
        b.live_base = None;
        let rejoined = b.sync_live(&remote("b")).await;
        assert!(rejoined.union);
        a.sync_live(&remote("a")).await;
        assert!(a.profile("p9").is_some());
        assert!(b.profile("p3").is_some());

        client.delete_directory(&directory).await.unwrap();
    }

    /// Both devices change and sync at the same moment, repeatedly; whatever
    /// the server does with If-Match, nothing may be lost.
    #[tokio::test]
    #[ignore]
    async fn live_devices_syncing_at_once_lose_nothing() {
        let env = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
        let client = WebDavClient::new(
            &env("TTERM_WEBDAV_URL"),
            &env("TTERM_WEBDAV_USER"),
            zeroize::Zeroizing::new(env("TTERM_WEBDAV_PASSWORD")),
            "tTerm-test",
        )
        .unwrap();
        let directory = vec![format!("tterm-race-{}", std::process::id())];
        client.ensure_directory(&directory).await.unwrap();
        let remote = |device| RemoteFile {
            client: &client,
            directory: &directory,
            password: "sync-password",
            device,
            app_version: "test",
        };
        let (mut a, mut b) = (Device::new(), Device::new());
        a.sync_live(&remote("a")).await;
        b.sync_live(&remote("b")).await;
        for round in 0..6 {
            a.save_profile(&format!("a{round}"), "a.example", "");
            b.save_profile(&format!("b{round}"), "b.example", "");
            let (ra, rb) = (remote("a"), remote("b"));
            tokio::join!(a.sync_live(&ra), b.sync_live(&rb));
        }
        for _ in 0..2 {
            a.sync_live(&remote("a")).await;
            b.sync_live(&remote("b")).await;
        }
        for device in [&a, &b] {
            for round in 0..6 {
                for id in [format!("a{round}"), format!("b{round}")] {
                    assert!(device.profile(&id).is_some(), "{id} lost");
                }
            }
        }
        assert!(same_collections(&a.collect(), &b.collect()));
        client.delete_directory(&directory).await.unwrap();
    }

    #[test]
    fn triggers_record_when_rows_change() {
        let device = Device::new();
        device.save_profile("p1", "one.example", "");
        let updated_at = |device: &Device| -> Option<i64> {
            device
                .database
                .read(|connection| {
                    connection
                        .query_row(
                            "SELECT updated_at FROM profiles WHERE id = 'p1'",
                            [],
                            |row| row.get(0),
                        )
                        .map_err(|error| error.to_string())
                })
                .unwrap()
        };
        let first = updated_at(&device).expect("stamped on insert");
        device
            .database
            .write(|transaction| {
                transaction
                    .execute("UPDATE profiles SET updated_at = 1 WHERE id = 'p1'", [])
                    .map_err(|error| error.to_string())
            })
            .unwrap();
        device.save_profile("p1", "one.example", "");
        assert_eq!(
            updated_at(&device),
            Some(1),
            "an unchanged save keeps the time"
        );
        device.save_profile("p1", "two.example", "");
        assert!(updated_at(&device).unwrap() >= first);
    }
}
