use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{ipc::Channel, Manager};
use tauri_plugin_updater::UpdaterExt;

#[derive(Default)]
pub struct PendingUpdateDownloads(
    pub tokio::sync::RwLock<std::collections::HashMap<String, PendingUpdateDownload>>,
);

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
pub async fn check_app_update(
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
pub async fn download_app_update(
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
pub async fn install_downloaded_app_update(
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
pub async fn download_install_app_update(
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

pub fn cleanup_stale_pending_update_files(app: &tauri::AppHandle) -> Result<(), String> {
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
