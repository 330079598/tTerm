//! Opening remote files in programs on this computer, the way WinSCP and
//! FileZilla do: the file is downloaded into a folder of its own, opened
//! there, and the copy is watched. The drawer hears of each save that
//! differs from the server and uploads it, at once or after asking.

mod operations;
use operations::*;

#[cfg(all(test, target_os = "macos"))]
use tterm_platform::native::open::apps;

use tterm_core::events::sftp::external::{EditStatus, ExternalEdit};
mod local;
#[cfg(all(test, unix))]
mod real_sshd_tests;
mod remote;
mod sessions;
#[cfg(test)]
mod tests;
mod watch;

use crate::core::blocking::run_blocking;
use crate::core::session::types::{PtyConnectionOptions, SessionPlan};
use crate::services::AppServices;
use crate::sftp::internal::api::download::download_file_with_progress;
use crate::sftp::internal::api::prepare_transfer;
use crate::sftp::internal::connection::map_sftp_error;
use crate::sftp::internal::connection::SftpContext;
use crate::sftp::internal::types::{SftpConnectionPool, TransferCancelMap};
#[cfg(target_os = "macos")]
use base64::Engine;
use local::{FileStamp, Fingerprint};
use remote::RemoteStamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use tterm_core::events::sftp::SftpEvent;
use tterm_core::events::{AppEvent, EventSink};
#[cfg(target_os = "macos")]
use tterm_platform::open::ApplicationChoice;
use tterm_platform::open::OpenTargets;

struct EditSession {
    edit: ExternalEdit,
    /// The session's own folder, which holds only the copy.
    dir: PathBuf,
    local: PathBuf,
    baseline: RemoteStamp,
    /// The copy's content when it last matched the server.
    synced: Fingerprint,
    /// The copy's stamp at the last look that read it.
    observed: Option<FileStamp>,
    /// A changed stamp seen at the last look, read once it holds.
    settling: Option<FileStamp>,
}

impl EditSession {
    fn has_changes(&self) -> bool {
        self.edit.status != EditStatus::Synced
    }
}

#[derive(Default)]
pub struct ExternalEditState {
    sessions: Mutex<HashMap<String, EditSession>>,
    /// Whether the watch of the copies runs.
    watching: AtomicBool,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UploadOutcome {
    Uploaded {
        edit: ExternalEdit,
    },
    /// Someone changed or removed the remote file since it was downloaded,
    /// so nothing was written; uploading again with `overwrite` replaces it.
    RemoteChanged,
}

/// Where to open a file.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OpenIn {
    /// The file's default program.
    Default,
    /// A program the user picked: an application on macOS, an executable
    /// anywhere.
    Program { program: String },
    /// The system's own Open With dialog, on Windows.
    Chooser,
}

/// An application macOS offers for a kind of file.
#[cfg(target_os = "macos")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppChoice {
    name: String,
    path: String,
    is_default: bool,
    /// A PNG data URL.
    icon: Option<String>,
}

/// How the user can pick a program for a file on this system.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWithChoices {
    #[cfg(target_os = "macos")]
    apps: Vec<AppChoice>,
    /// Whether the system has an Open With dialog of its own.
    system_chooser: bool,
}

/// The tab's host, for moving a file to or from it.
struct Host<'a> {
    app: &'a SftpContext,
    events: Arc<dyn EventSink>,
    tab_id: &'a str,
    plan: SessionPlan,
    prompts: std::sync::Arc<tterm_core::prompts::PromptBroker>,
    pool: &'a SftpConnectionPool,
    cancel_map: &'a TransferCancelMap,
}

impl Host<'_> {
    /// Downloads the file to `local`. With a `transfer_id`, it goes through
    /// the parallel transfer engine, which reports its progress and can be
    /// cancelled under that id as any download can.
    async fn download(
        &self,
        remote_path: &str,
        local: &Path,
        transfer_id: Option<&str>,
    ) -> Result<RemoteStamp, String> {
        let Some(transfer_id) = transfer_id else {
            return with_sftp!(self.app, self.tab_id, &self.plan, self.prompts, self.pool, sftp => {
                remote::download(sftp, remote_path, local).await
            });
        };
        let (sftp, channels) = prepare_transfer(
            self.app,
            self.tab_id,
            &self.plan,
            self.prompts.clone(),
            self.pool,
        )
        .await?;
        let metadata = sftp.metadata(remote_path).await.map_err(map_sftp_error)?;
        if metadata.is_dir() {
            return Err(remote::FOLDER_ERROR.to_string());
        }
        let events = self.events.clone();
        download_file_with_progress(
            &events,
            self.tab_id,
            channels,
            self.cancel_map,
            transfer_id,
            remote_path,
            &local.to_string_lossy(),
        )
        .await?;
        Ok(RemoteStamp::of(&metadata))
    }

    async fn upload(
        &self,
        source: &sessions::UploadSource,
        overwrite: bool,
    ) -> Result<Option<RemoteStamp>, String> {
        with_sftp!(self.app, self.tab_id, &self.plan, self.prompts, self.pool, sftp => {
            remote::upload(sftp, &source.remote_path, &source.local, source.baseline, overwrite).await
        })
    }
}

fn file_name_of(remote_path: &str) -> Result<String, String> {
    remote_path
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .map(ToString::to_string)
        .ok_or_else(|| format!("'{remote_path}' is not a file"))
}

async fn fingerprint(path: PathBuf) -> Result<Fingerprint, String> {
    run_blocking(move || local::fingerprint(&path)).await
}

fn emit(events: &dyn EventSink, edit: &ExternalEdit) {
    let _ = events.emit(AppEvent::Sftp(SftpEvent::ExternalEditChanged(edit.clone())));
}

/// Downloads the file into a new session's folder and starts watching it.
async fn start_session(
    host: &Host<'_>,
    state: &Arc<ExternalEditState>,
    paths: &tterm_core::paths::AppPaths,
    remote_path: &str,
    file_name: &str,
    transfer_id: Option<&str>,
) -> Result<ExternalEdit, String> {
    let root = local::edit_root(paths);
    let id = uuid::Uuid::new_v4().to_string();
    let dir = {
        let id = id.clone();
        run_blocking(move || local::create_session_dir(&root, &id)).await?
    };
    let local = dir.join(file_name);
    let downloaded = async {
        let baseline = host.download(remote_path, &local, transfer_id).await?;
        Ok::<_, String>((baseline, fingerprint(local.clone()).await?))
    }
    .await;
    let (baseline, synced) = match downloaded {
        Ok(downloaded) => downloaded,
        Err(error) => {
            let _ = run_blocking(move || {
                local::remove_session_dir(&dir);
                Ok(())
            })
            .await;
            return Err(error);
        }
    };
    let edit = ExternalEdit {
        id,
        tab_id: host.tab_id.to_string(),
        remote_path: remote_path.to_string(),
        file_name: file_name.to_string(),
        local_path: local.to_string_lossy().into_owned(),
        status: EditStatus::Synced,
        error: None,
    };
    state.insert(EditSession {
        edit: edit.clone(),
        dir,
        local,
        baseline,
        synced,
        observed: Some(synced.stamp),
        settling: None,
    });
    watch::ensure_running(state.clone(), host.events.clone());
    Ok(edit)
}

/// Ends sessions: their copies go, except those with changes that were
/// never uploaded, which stay where they are. Returns the sessions kept.
async fn end_sessions(sessions: Vec<EditSession>) -> Vec<ExternalEdit> {
    let (kept, removed): (Vec<_>, Vec<_>) =
        sessions.into_iter().partition(EditSession::has_changes);
    let dirs: Vec<_> = removed.into_iter().map(|session| session.dir).collect();
    let _ = run_blocking(move || {
        dirs.iter().for_each(|dir| local::remove_session_dir(dir));
        Ok(())
    })
    .await;
    kept.into_iter().map(|session| session.edit).collect()
}

/// Opens the file at `local` as `open_in` says. Returns `false` when the
/// user cancelled the system's dialog.
async fn launch(opener: &dyn OpenTargets, open_in: OpenIn, local: &Path) -> Result<bool, String> {
    match open_in {
        OpenIn::Default => opener
            .open_file(local.to_path_buf())
            .await
            .map(|()| true)
            .map_err(|e| format!("Failed to open '{}': {e}", local.display())),
        OpenIn::Program { program } => opener
            .open_with(program, local.to_path_buf())
            .await
            .map(|()| true),
        OpenIn::Chooser => opener.choose_application(local.to_path_buf()).await,
    }
}

/// Opens the remote file at `path` as `open_in` says. A file already open
/// from this tab opens again as it is, downloaded afresh unless it has
/// changes of its own. Returns None when the user cancelled the system's
/// Open With dialog.
#[tauri::command]
pub async fn sftp_external_edit_open(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    open_in: OpenIn,
    transfer_id: Option<String>,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<Option<ExternalEdit>, String> {
    sftp_external_edit_open_service(
        SftpExternalEditOpenDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
            events: services.shared.events.clone(),
            state: services.sftp.external_edit.clone(),
            opener: services.shared.platform.open.clone(),
            paths: crate::app_paths::get(&app).map(|paths| paths.as_ref().clone()),
        },
        SftpExternalEditOpenInput {
            tab_id,
            connection,
            path,
            open_in,
            transfer_id,
        },
    )
    .await
}

/// How the user can pick a program for `file_name` here.
#[tauri::command]
pub async fn sftp_external_open_with_choices(
    file_name: String,
    services: State<'_, Arc<AppServices>>,
) -> Result<OpenWithChoices, String> {
    sftp_external_open_with_choices_service(
        services.shared.platform.open.clone(),
        SftpExternalOpenWithChoicesInput { file_name },
    )
    .await
}

/// Uploads the copy's content over the remote file.
#[tauri::command]
pub async fn sftp_external_edit_upload(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    id: String,
    overwrite: bool,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<UploadOutcome, String> {
    sftp_external_edit_upload_service(
        SftpExternalEditUploadDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
            events: services.shared.events.clone(),
            state: services.sftp.external_edit.clone(),
        },
        SftpExternalEditUploadInput {
            tab_id,
            connection,
            id,
            overwrite,
        },
    )
    .await
}

/// The files open from the tab.
#[tauri::command]
pub async fn sftp_external_edit_list(tab_id: String, app: AppHandle) -> Vec<ExternalEdit> {
    let Some(services) = app.try_state::<Arc<AppServices>>() else {
        return Vec::new();
    };
    sftp_external_edit_list_service(
        SftpExternalEditListDependencies {
            state: services.sftp.external_edit.clone(),
        },
        SftpExternalEditListInput { tab_id },
    )
    .await
}

/// Stops watching the file and removes its copy, changes and all.
#[tauri::command]
pub async fn sftp_external_edit_stop(
    id: String,
    services: State<'_, Arc<AppServices>>,
) -> Result<(), String> {
    sftp_external_edit_stop_service(
        SftpExternalEditStopDependencies {
            state: services.sftp.external_edit.clone(),
        },
        SftpExternalEditStopInput { id },
    )
    .await
}

/// Called when the tab closes. Returns the files whose copies were kept,
/// as their changes were never uploaded.
#[tauri::command]
pub async fn sftp_external_edit_release_tab(tab_id: String, app: AppHandle) -> Vec<ExternalEdit> {
    let Some(services) = app.try_state::<Arc<AppServices>>() else {
        return Vec::new();
    };
    sftp_external_edit_release_tab_service(
        SftpExternalEditReleaseTabDependencies {
            state: services.sftp.external_edit.clone(),
        },
        SftpExternalEditReleaseTabInput { tab_id },
    )
    .await
}

#[tauri::command]
pub async fn sftp_external_edit_reveal(
    id: String,
    services: State<'_, Arc<AppServices>>,
) -> Result<(), String> {
    sftp_external_edit_reveal_service(
        services.sftp.external_edit.clone(),
        services.shared.platform.open.clone(),
        SftpExternalEditRevealInput { id },
    )
    .await
}

/// The icons of the applications at `paths`, as PNG data URLs by path;
/// none outside macOS, which is the only system listing applications.
#[tauri::command]
pub async fn sftp_external_app_icons(
    paths: Vec<String>,
    services: State<'_, Arc<AppServices>>,
) -> Result<HashMap<String, Option<String>>, String> {
    sftp_external_app_icons_service(
        services.shared.platform.open.clone(),
        SftpExternalAppIconsInput { paths },
    )
    .await
}

/// The names of the open files whose changes are not on the server, which
/// a quit would leave behind.
pub fn unuploaded_files(state: &ExternalEditState) -> Vec<String> {
    state.unuploaded_names()
}

/// Removes, at startup, copies left behind long ago.
pub fn clean_up_on_start(paths: &tterm_core::paths::AppPaths, runtime: &tokio::runtime::Handle) {
    let root = local::edit_root(paths);
    runtime.spawn_blocking(move || local::remove_stale(&root));
}

/// Removes, at exit, the copies without changes of their own.
pub fn shutdown(state: &ExternalEditState) {
    for session in state.remove_all() {
        if !session.has_changes() {
            local::remove_session_dir(&session.dir);
        }
    }
}

#[cfg(target_os = "macos")]
fn png_data_url(bytes: Vec<u8>) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}
#[cfg(target_os = "macos")]
impl From<ApplicationChoice> for AppChoice {
    fn from(app: ApplicationChoice) -> Self {
        Self {
            name: app.name,
            path: app.path,
            is_default: app.is_default,
            icon: app.icon.map(png_data_url),
        }
    }
}
#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod container_tests;
