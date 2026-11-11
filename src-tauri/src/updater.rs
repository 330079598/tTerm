//! Update IPC keeps its wire DTOs and pending-channel map while using owned platform releases.
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::{ipc::Channel, Manager};
use tterm_core::updates::{
    pending_update_download_path, remove_pending_update_file, update_endpoint,
    write_pending_update_download,
};
use tterm_platform::updates::{DownloadEvent, DownloadedUpdate, UpdateProgress, Updates};

#[derive(Default)]
pub struct PendingUpdateDownloads(
    pub tokio::sync::RwLock<std::collections::HashMap<String, Arc<PendingUpdateDownload>>>,
);
pub struct PendingUpdateDownload {
    path: PathBuf,
    update: Arc<dyn DownloadedUpdate>,
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

struct Progress(Channel<AppUpdateDownloadEvent>);
impl UpdateProgress for Progress {
    fn report(&self, event: DownloadEvent) {
        let event = match event {
            DownloadEvent::Started { content_length } => {
                AppUpdateDownloadEvent::Started { content_length }
            }
            DownloadEvent::Progress { chunk_length } => {
                AppUpdateDownloadEvent::Progress { chunk_length }
            }
            DownloadEvent::Finished => AppUpdateDownloadEvent::Finished,
        };
        let _ = self.0.send(event);
    }
}
#[tauri::command]
pub async fn check_app_update(
    app: tauri::AppHandle,
    channel: String,
) -> Result<Option<AppUpdateMetadata>, String> {
    Ok(app
        .state::<Arc<dyn Updates>>()
        .check(update_endpoint(&channel).into())
        .await?
        .map(|update| {
            let meta = update.metadata();
            AppUpdateMetadata {
                version: meta.version,
                current_version: meta.current_version,
                body: meta.body,
                date: meta.date,
            }
        }))
}
#[tauri::command]
pub async fn download_app_update(
    app: tauri::AppHandle,
    downloads: tauri::State<'_, Arc<PendingUpdateDownloads>>,
    channel: String,
    on_event: Channel<AppUpdateDownloadEvent>,
) -> Result<bool, String> {
    let Some(update) = app
        .state::<Arc<dyn Updates>>()
        .check(update_endpoint(&channel).into())
        .await?
    else {
        return Ok(false);
    };
    let mut download = update.download(Arc::new(Progress(on_event))).await?;
    let paths = crate::app_paths::get(&app)?.inner().clone();
    let cache_channel = channel.clone();
    let path = crate::core::blocking::run_blocking(move || {
        pending_update_download_path(&paths, &cache_channel)
    })
    .await?;
    write_pending_update_download(&channel, &path, &download.take_bytes()).await?;
    let previous = downloads.0.write().await.insert(
        channel,
        Arc::new(PendingUpdateDownload {
            path,
            update: Arc::from(download),
        }),
    );
    remove_pending_update_download(previous).await;
    Ok(true)
}
#[tauri::command]
pub async fn install_downloaded_app_update(
    app: tauri::AppHandle,
    downloads: tauri::State<'_, Arc<PendingUpdateDownloads>>,
    channel: String,
) -> Result<bool, String> {
    let Some(download) = downloads.0.read().await.get(&channel).cloned() else {
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
    let bundle = current_macos_app_bundle_path();
    crate::window_state::save_now(app.clone()).await;
    download.update.clone().install(bytes).await?;
    clear_macos_quarantine_after_update(bundle.as_deref());
    remove_cached_update(downloads.inner(), &channel).await;
    Ok(true)
}
#[tauri::command]
pub async fn download_install_app_update(
    app: tauri::AppHandle,
    downloads: tauri::State<'_, Arc<PendingUpdateDownloads>>,
    channel: String,
    on_event: Channel<AppUpdateDownloadEvent>,
) -> Result<bool, String> {
    let Some(update) = app
        .state::<Arc<dyn Updates>>()
        .check(update_endpoint(&channel).into())
        .await?
    else {
        return Ok(false);
    };
    let mut download = update.download(Arc::new(Progress(on_event))).await?;
    let bytes = download.take_bytes();
    let download: Arc<dyn DownloadedUpdate> = Arc::from(download);
    let bundle = current_macos_app_bundle_path();
    crate::window_state::save_now(app.clone()).await;
    download.install(bytes).await?;
    clear_macos_quarantine_after_update(bundle.as_deref());
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

async fn remove_cached_update(downloads: &PendingUpdateDownloads, channel: &str) {
    let download = downloads.0.write().await.remove(channel);
    remove_pending_update_download(download).await;
}
async fn remove_pending_update_download(download: Option<Arc<PendingUpdateDownload>>) {
    if let Some(download) = download {
        remove_pending_update_file(&download.path).await;
    }
}
pub fn cleanup_stale_pending_update_files(app: &tauri::AppHandle) -> Result<(), String> {
    tterm_core::updates::cleanup_stale_pending_update_files(crate::app_paths::get(app)?.inner())
}
