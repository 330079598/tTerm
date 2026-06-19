mod clipboard_files;
mod config;
mod core;
mod fonts;
mod monitor;
mod profiles;
mod session;
mod sftp;
mod ssh;
mod terminal;

use core::PtyMap;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{ipc::Channel, Manager};
#[cfg(any(target_os = "windows", target_os = "macos"))]
use tauri_plugin_frame::FramePluginBuilder;
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::RwLock;

pub struct TokioRuntimeState {
    pub runtime: tokio::runtime::Runtime,
}

#[derive(Default)]
pub struct PendingUpdateDownloads(pub RwLock<HashMap<String, PendingUpdateDownload>>);

#[derive(Clone)]
pub struct PendingUpdateDownload {
    path: PathBuf,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateMetadata {
    version: String,
    current_version: String,
    body: Option<String>,
    date: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum AppUpdateDownloadEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        content_length: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        chunk_length: usize,
    },
    Finished,
}

fn update_endpoint(channel: &str) -> &'static str {
    match channel {
        "beta-dev" => "https://330079598.github.io/tTerm/update/beta-dev/latest.json",
        _ => "https://330079598.github.io/tTerm/update/stable/latest.json",
    }
}

async fn find_app_update(
    app: &tauri::AppHandle,
    channel: &str,
) -> Result<Option<tauri_plugin_updater::Update>, String> {
    let endpoint = tauri::Url::parse(update_endpoint(channel))
        .map_err(|e| format!("Invalid update endpoint: {e}"))?;

    app.updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_app_update(
    app: tauri::AppHandle,
    channel: String,
) -> Result<Option<AppUpdateMetadata>, String> {
    Ok(find_app_update(&app, &channel)
        .await?
        .map(|update| AppUpdateMetadata {
            version: update.version,
            current_version: update.current_version,
            body: update.body,
            date: update.date.map(|date| date.to_string()),
        }))
}

#[tauri::command]
async fn download_app_update(
    app: tauri::AppHandle,
    downloads: tauri::State<'_, PendingUpdateDownloads>,
    channel: String,
    on_event: Channel<AppUpdateDownloadEvent>,
) -> Result<bool, String> {
    let Some(update) = find_app_update(&app, &channel).await? else {
        return Ok(false);
    };

    let mut started = false;
    let bytes = update
        .download(
            |chunk_length, content_length| {
                if !started {
                    let _ = on_event.send(AppUpdateDownloadEvent::Started { content_length });
                    started = true;
                }
                let _ = on_event.send(AppUpdateDownloadEvent::Progress { chunk_length });
            },
            || {
                let _ = on_event.send(AppUpdateDownloadEvent::Finished);
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    let update_path = pending_update_download_path(&app, &channel)?;
    write_pending_update_download(&channel, &update_path, &bytes).await?;

    let previous_download = downloads
        .0
        .write()
        .await
        .insert(channel, PendingUpdateDownload { path: update_path });
    remove_pending_update_download(previous_download).await;
    Ok(true)
}

#[tauri::command]
async fn install_downloaded_app_update(
    app: tauri::AppHandle,
    downloads: tauri::State<'_, PendingUpdateDownloads>,
    channel: String,
) -> Result<bool, String> {
    let Some(download) = downloads.0.read().await.get(&channel).cloned() else {
        return Ok(false);
    };
    let Some(update) = find_app_update(&app, &channel).await? else {
        remove_cached_update(downloads.inner(), &channel).await;
        return Ok(false);
    };

    let bytes = match tokio::fs::read(&download.path).await {
        Ok(bytes) => bytes,
        Err(err) => {
            remove_cached_update(downloads.inner(), &channel).await;
            return Err(format!(
                "Failed to read downloaded update '{}': {}",
                download.path.display(),
                err
            ));
        }
    };
    let app_bundle_path = current_macos_app_bundle_path();
    update.install(bytes).map_err(|e| e.to_string())?;
    clear_macos_quarantine_after_update(app_bundle_path.as_deref());
    remove_cached_update(downloads.inner(), &channel).await;
    Ok(true)
}

#[tauri::command]
async fn download_install_app_update(
    app: tauri::AppHandle,
    downloads: tauri::State<'_, PendingUpdateDownloads>,
    channel: String,
    on_event: Channel<AppUpdateDownloadEvent>,
) -> Result<bool, String> {
    let Some(update) = find_app_update(&app, &channel).await? else {
        return Ok(false);
    };

    let mut started = false;
    let app_bundle_path = current_macos_app_bundle_path();
    update
        .download_and_install(
            |chunk_length, content_length| {
                if !started {
                    let _ = on_event.send(AppUpdateDownloadEvent::Started { content_length });
                    started = true;
                }
                let _ = on_event.send(AppUpdateDownloadEvent::Progress { chunk_length });
            },
            || {
                let _ = on_event.send(AppUpdateDownloadEvent::Finished);
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    clear_macos_quarantine_after_update(app_bundle_path.as_deref());
    remove_cached_update(downloads.inner(), &channel).await;
    Ok(true)
}

#[cfg(target_os = "macos")]
fn current_macos_app_bundle_path() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    macos_app_bundle_path_from_executable(&executable)
}

#[cfg(not(target_os = "macos"))]
fn current_macos_app_bundle_path() -> Option<PathBuf> {
    None
}

#[cfg(target_os = "macos")]
fn macos_app_bundle_path_from_executable(executable: &Path) -> Option<PathBuf> {
    let macos_dir = executable.parent()?;
    if macos_dir.file_name().and_then(|value| value.to_str()) != Some("MacOS") {
        return None;
    }

    let contents_dir = macos_dir.parent()?;
    if contents_dir.file_name().and_then(|value| value.to_str()) != Some("Contents") {
        return None;
    }

    let app_bundle = contents_dir.parent()?;
    if app_bundle.extension().and_then(|value| value.to_str()) != Some("app") {
        return None;
    }

    Some(app_bundle.to_path_buf())
}

fn clear_macos_quarantine_after_update(app_bundle_path: Option<&Path>) {
    #[cfg(target_os = "macos")]
    {
        let Some(app_bundle_path) = app_bundle_path else {
            return;
        };

        match std::process::Command::new("/usr/bin/xattr")
            .args(["-rd", "com.apple.quarantine"])
            .arg(app_bundle_path)
            .status()
        {
            Ok(status) if status.success() => {}
            Ok(status) => eprintln!(
                "Failed to clear macOS quarantine attribute for '{}': xattr exited with {}",
                app_bundle_path.display(),
                status
            ),
            Err(err) => eprintln!(
                "Failed to clear macOS quarantine attribute for '{}': {}",
                app_bundle_path.display(),
                err
            ),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app_bundle_path;
    }
}

fn sanitize_update_channel(channel: &str) -> String {
    channel
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn pending_update_download_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("Failed to resolve app cache dir: {e}"))?
        .join("updates"))
}

fn pending_update_download_path(app: &tauri::AppHandle, channel: &str) -> Result<PathBuf, String> {
    let update_dir = pending_update_download_dir(app)?;

    std::fs::create_dir_all(&update_dir).map_err(|e| {
        format!(
            "Failed to create update cache dir '{}': {}",
            update_dir.display(),
            e
        )
    })?;

    Ok(update_dir.join(format!(
        "{}-{}.bin",
        sanitize_update_channel(channel),
        uuid::Uuid::new_v4()
    )))
}

async fn write_pending_update_download(
    channel: &str,
    update_path: &Path,
    bytes: &[u8],
) -> Result<(), String> {
    let update_dir = update_path
        .parent()
        .ok_or_else(|| format!("Invalid update cache path '{}'", update_path.display()))?;
    let temp_path = update_dir.join(format!(
        "{}-{}.tmp",
        sanitize_update_channel(channel),
        uuid::Uuid::new_v4()
    ));

    let write_result = async {
        tokio::fs::write(&temp_path, bytes).await.map_err(|e| {
            format!(
                "Failed to cache downloaded update '{}': {}",
                temp_path.display(),
                e
            )
        })?;
        tokio::fs::rename(&temp_path, update_path)
            .await
            .map_err(|e| {
                format!(
                    "Failed to finalize downloaded update '{}' from '{}': {}",
                    update_path.display(),
                    temp_path.display(),
                    e
                )
            })
    }
    .await;

    if write_result.is_err() {
        remove_pending_update_file(&temp_path).await;
    }

    write_result
}

async fn remove_cached_update(downloads: &PendingUpdateDownloads, channel: &str) {
    let download = downloads.0.write().await.remove(channel);
    remove_pending_update_download(download).await;
}

async fn remove_pending_update_download(download: Option<PendingUpdateDownload>) {
    let Some(download) = download else {
        return;
    };

    remove_pending_update_file(&download.path).await;
}

async fn remove_pending_update_file(path: &Path) {
    if let Err(err) = tokio::fs::remove_file(path).await {
        if err.kind() != std::io::ErrorKind::NotFound {
            eprintln!(
                "Failed to remove cached update '{}': {}",
                path.display(),
                err
            );
        }
    }
}

fn cleanup_stale_pending_update_files(app: &tauri::AppHandle) -> Result<(), String> {
    let update_dir = pending_update_download_dir(app)?;
    let Ok(entries) = std::fs::read_dir(&update_dir) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let is_update_cache_file = matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("bin" | "tmp")
        );
        if is_update_cache_file {
            if let Err(err) = std::fs::remove_file(&path) {
                if err.kind() != std::io::ErrorKind::NotFound {
                    eprintln!(
                        "Failed to remove stale update '{}': {}",
                        path.display(),
                        err
                    );
                }
            }
        }
    }

    Ok(())
}

const MIGRATED_CONFIG_FILES: &[&str] = &[
    "config.json",
    "profiles.json",
    "profile_groups.json",
    "session.json",
    "ssh_known_hosts.json",
    "ssh_profiles.json",
    "sftp_directories.json",
];

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let pty_map: PtyMap = Arc::new(RwLock::new(std::collections::HashMap::new()));
    let host_prompt_map: core::HostPromptMap =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    let sftp_pool: sftp::SftpConnectionPool =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    let monitor_sessions: monitor::MonitorSessionMap =
        Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
    let transfer_cancel_map: sftp::TransferCancelMap =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");
    let secret_store = ssh::SecretStoreState::new();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build());

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let builder = builder.plugin(
        FramePluginBuilder::new()
            .titlebar_height(32)
            .button_width(46)
            .auto_titlebar(true)
            .snap_overlay_delay_ms(15)
            .close_hover_bg("rgba(196,43,28,1)")
            .button_hover_bg("rgba(255,255,255,0.1)")
            .build(),
    );

    builder
        .manage(pty_map)
        .manage(host_prompt_map)
        .manage(sftp_pool)
        .manage(monitor_sessions)
        .manage(transfer_cancel_map)
        .manage(TokioRuntimeState { runtime })
        .manage(PendingUpdateDownloads::default())
        .manage(secret_store)
        .invoke_handler(tauri::generate_handler![
            config::load_config,
            config::save_config,
            clipboard_files::read_clipboard_file_paths,
            session::load_session,
            session::save_session,
            session::clear_session,
            core::commands::create_pty,
            core::commands::write_pty,
            core::commands::resize_pty,
            core::commands::kill_pty,
            core::commands::respond_ssh_host_key_prompt,
            core::commands::has_saved_password,
            core::commands::has_saved_jump_host_password,
            core::commands::write_saved_password_for_sudo,
            fonts::list_fonts,
            monitor::get_server_metrics_snapshot,
            monitor::release_server_monitor_session,
            profiles::list_profiles,
            profiles::preview_ssh_config_import,
            profiles::import_ssh_config_profiles,
            profiles::list_profile_groups,
            profiles::save_profile_group,
            profiles::rename_profile_group,
            profiles::delete_profile_group,
            profiles::move_profile_to_group,
            profiles::save_profile,
            profiles::delete_profile,
            profiles::set_profile_server_monitor_visible,
            profiles::test_connection,
            sftp::internal::api::base::sftp_list_directory,
            sftp::internal::api::base::sftp_create_directory,
            sftp::internal::api::delete::commands::sftp_delete_entry,
            sftp::internal::api::delete::commands::sftp_delete_entries,
            sftp::internal::api::delete::commands::sftp_preview_delete_entries,
            sftp::internal::api::base::sftp_rename_entry,
            sftp::internal::api::edit::sftp_open_file_for_edit,
            sftp::internal::api::edit::sftp_save_edited_file,
            sftp::internal::api::upload::commands::sftp_upload_file,
            sftp::internal::api::upload::commands::sftp_upload_paths,
            sftp::internal::api::upload::commands::sftp_cancel_upload,
            sftp::internal::api::base::sftp_download_file,
            sftp::internal::api::base::sftp_download_directory,
            sftp::internal::api::base::get_file_size,
            ssh::secret_commands::get_secret_backend_status,
            ssh::secret_commands::unlock_secret_vault,
            ssh::secret_commands::lock_secret_vault,
            ssh::secret_commands::set_secret_vault_enabled,
            ssh::secret_commands::set_secret_storage_mode,
            ssh::secret_commands::copy_secret_store,
            ssh::secret_commands::list_saved_secrets,
            ssh::secret_commands::delete_saved_secret,
            check_app_update,
            download_app_update,
            install_downloaded_app_update,
            download_install_app_update,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            config::init_config_dir(&app_handle)?;

            if let Err(err) = cleanup_stale_pending_update_files(&app_handle) {
                eprintln!("Failed to clean stale update cache files: {}", err);
            }

            if let Err(err) = migrate_legacy_config_files(&app_handle) {
                eprintln!("Failed to migrate legacy config files: {}", err);
            }

            if let Err(err) = migrate_legacy_ssh_passwords(&app_handle) {
                eprintln!("Failed to migrate legacy SSH passwords: {}", err);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn migrate_legacy_config_files(app: &tauri::AppHandle) -> Result<(), String> {
    let new_dir = config::ensure_config_dir()?;
    let old_dir = config::legacy_config_path()?;

    if same_path(&new_dir, &old_dir) || !old_dir.exists() {
        return Ok(());
    }

    for name in MIGRATED_CONFIG_FILES {
        let old_path = old_dir.join(name);
        if !old_path.exists() {
            continue;
        }

        let new_path = new_dir.join(name);
        if let Some(parent) = new_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create migration target dir: {}", e))?;
        }

        if new_path.exists() {
            merge_migrated_config_file(name, &old_path, &new_path)?;
            continue;
        }

        std::fs::copy(&old_path, &new_path).map_err(|e| {
            format!(
                "Failed to migrate '{}' from '{}' to '{}': {}",
                name,
                old_path.display(),
                new_path.display(),
                e
            )
        })?;
    }

    migrate_legacy_secret_vault(app)?;

    Ok(())
}

fn merge_migrated_config_file(name: &str, old_path: &Path, new_path: &Path) -> Result<(), String> {
    match name {
        "profiles.json" => merge_profiles_file(old_path, new_path),
        "profile_groups.json" => merge_profile_groups_file(old_path, new_path),
        "session.json" => merge_session_file(old_path, new_path),
        "ssh_known_hosts.json" => merge_known_hosts_file(old_path, new_path),
        "ssh_profiles.json" => merge_legacy_password_store_file(old_path, new_path),
        "sftp_directories.json" => merge_sftp_directory_store_file(old_path, new_path),
        _ => Ok(()),
    }
}

fn merge_profile_groups_file(old_path: &Path, new_path: &Path) -> Result<(), String> {
    let old_groups = read_json_file::<Vec<String>>(old_path, "profile groups")?;
    let mut new_groups = read_json_file::<Vec<String>>(new_path, "profile groups")?;
    let mut changed = false;

    for group in old_groups {
        let group = group.trim();
        if group.is_empty() || new_groups.iter().any(|existing| existing == group) {
            continue;
        }

        new_groups.push(group.to_string());
        changed = true;
    }

    if changed {
        new_groups.sort_by(|left, right| left.to_lowercase().cmp(&right.to_lowercase()));
        write_json_file(new_path, &new_groups, "profile groups")?;
    }

    Ok(())
}

fn merge_profiles_file(old_path: &Path, new_path: &Path) -> Result<(), String> {
    let old_profiles = read_json_file::<Vec<profiles::SavedProfile>>(old_path, "profiles")?;
    let mut new_profiles = read_json_file::<Vec<profiles::SavedProfile>>(new_path, "profiles")?;
    let mut changed = false;

    for profile in old_profiles {
        let duplicate = new_profiles.iter().any(|existing| {
            existing.id == profile.id
                || (!profile.name.trim().is_empty() && existing.name == profile.name)
        });
        if !duplicate {
            new_profiles.push(profile);
            changed = true;
        }
    }

    if changed {
        write_json_file(new_path, &new_profiles, "profiles")?;
    }

    Ok(())
}

fn merge_session_file(old_path: &Path, new_path: &Path) -> Result<(), String> {
    let old_session = read_json_file::<session::SessionData>(old_path, "session")?;
    let new_session = read_json_file::<session::SessionData>(new_path, "session")?;

    if old_session.last_saved > new_session.last_saved {
        write_json_file(new_path, &old_session, "session")?;
    }

    Ok(())
}

fn merge_known_hosts_file(old_path: &Path, new_path: &Path) -> Result<(), String> {
    let old_store = read_json_file::<ssh::store::KnownHostStore>(old_path, "known hosts")?;
    let mut new_store = read_json_file::<ssh::store::KnownHostStore>(new_path, "known hosts")?;
    let mut changed = false;

    for entry in old_store.entries {
        let duplicate = new_store.entries.iter().any(|existing| {
            (entry.profile_id.is_some() && existing.profile_id == entry.profile_id)
                || (existing.profile_name == entry.profile_name
                    && existing.host == entry.host
                    && existing.port == entry.port
                    && existing.algorithm == entry.algorithm
                    && existing.fingerprint == entry.fingerprint)
        });
        if !duplicate {
            new_store.entries.push(entry);
            changed = true;
        }
    }

    if changed {
        write_json_file(new_path, &new_store, "known hosts")?;
    }

    Ok(())
}

fn merge_legacy_password_store_file(old_path: &Path, new_path: &Path) -> Result<(), String> {
    let old_store = read_json_file::<ssh::store::LegacySshPasswordStore>(
        old_path,
        "legacy SSH password store",
    )?;
    let mut new_store = read_json_file::<ssh::store::LegacySshPasswordStore>(
        new_path,
        "legacy SSH password store",
    )?;
    let mut changed = false;

    for record in old_store.profiles {
        let duplicate = new_store
            .profiles
            .iter()
            .any(|existing| existing.profile_name == record.profile_name);
        if !duplicate {
            new_store.profiles.push(record);
            changed = true;
        }
    }

    if changed {
        write_json_file(new_path, &new_store, "legacy SSH password store")?;
    }

    Ok(())
}

fn merge_sftp_directory_store_file(old_path: &Path, new_path: &Path) -> Result<(), String> {
    let old_store =
        read_json_file::<sftp::store::SftpDirectoryStore>(old_path, "SFTP directory store")?;
    let mut new_store =
        read_json_file::<sftp::store::SftpDirectoryStore>(new_path, "SFTP directory store")?;
    let mut changed = false;

    for entry in old_store.entries {
        let duplicate = new_store.entries.iter().any(|existing| {
            existing.host == entry.host
                && existing.port == entry.port
                && existing.username == entry.username
        });
        if !duplicate {
            new_store.entries.push(entry);
            changed = true;
        }
    }

    if changed {
        write_json_file(new_path, &new_store, "SFTP directory store")?;
    }

    Ok(())
}

fn read_json_file<T>(path: &Path, label: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {} file '{}': {}", label, path.display(), e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse {} file '{}': {}", label, path.display(), e))
}

fn write_json_file<T>(path: &Path, value: &T, label: &str) -> Result<(), String>
where
    T: Serialize,
{
    let content = serde_json::to_string_pretty(value).map_err(|e| {
        format!(
            "Failed to serialize {} file '{}': {}",
            label,
            path.display(),
            e
        )
    })?;
    std::fs::write(path, content)
        .map_err(|e| format!("Failed to write {} file '{}': {}", label, path.display(), e))
}

fn migrate_legacy_secret_vault(app: &tauri::AppHandle) -> Result<(), String> {
    let new_secret_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {}", e))?
        .join("secrets");

    if !new_secret_dir.exists() {
        std::fs::create_dir_all(&new_secret_dir)
            .map_err(|e| format!("Failed to create secret dir: {}", e))?;
    }

    let old_secret_dir = config::legacy_config_path()?.join("secrets");
    if same_path(&new_secret_dir, &old_secret_dir) || !old_secret_dir.exists() {
        return Ok(());
    }

    for name in ["secret_vault.json", "secret_vault_config.json"] {
        let old_path = old_secret_dir.join(name);
        if !old_path.exists() {
            continue;
        }

        let new_path = new_secret_dir.join(name);
        if new_path.exists() {
            continue;
        }

        std::fs::copy(&old_path, &new_path).map_err(|e| {
            format!(
                "Failed to migrate '{}' from '{}' to '{}': {}",
                name,
                old_path.display(),
                new_path.display(),
                e
            )
        })?;
    }

    Ok(())
}

fn same_path(left: &Path, right: &Path) -> bool {
    normalize_path(left) == normalize_path(right)
}

fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn migrate_legacy_ssh_passwords(app: &tauri::AppHandle) -> Result<(), String> {
    let secret_state = app.state::<ssh::SecretStoreState>();
    let store = ssh::load_legacy_password_store()?;
    if store.profiles.is_empty() {
        return Ok(());
    }

    if !secret_state.keyring_available()? {
        return Ok(());
    }

    for record in store.profiles {
        if record.password.is_empty() {
            continue;
        }
        let password = secret_state.get_password(app, &record.profile_name)?;
        if password.is_none() {
            secret_state.save_password(app, &record.profile_name, &record.password)?;
        }
    }

    ssh::remove_legacy_password_store()?;
    Ok(())
}
