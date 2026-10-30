//! Opening remote files in programs on this computer, the way WinSCP and
//! FileZilla do: the file is downloaded into a folder of its own, opened
//! there, and the copy is watched. The drawer hears of each save that
//! differs from the server and uploads it, at once or after asking.

#[cfg(target_os = "macos")]
mod apps;
mod launch;
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
use crate::core::state::HostPromptMap;
use crate::sftp::internal::api::download::download_file_with_progress;
use crate::sftp::internal::api::prepare_transfer;
use crate::sftp::internal::connection::{ensure_ssh_plan, map_sftp_error};
use crate::sftp::internal::paths::normalize_remote_path;
use crate::sftp::internal::types::{SftpConnectionPool, TransferCancelMap};
use crate::ssh::secret_store::types::SecretStoreState;
use local::{FileStamp, Fingerprint};
use remote::RemoteStamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

/// Tells the drawer that an open file changed state; carries an
/// [`ExternalEdit`].
const EDIT_EVENT: &str = "sftp-external-edit";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EditStatus {
    /// The copy matches the server as last downloaded or uploaded.
    Synced,
    /// The copy has saved changes that are not on the server.
    Modified,
    Uploading,
    /// The last upload failed, so the changes are still only in the copy.
    Failed,
}

/// A remote file open in a program here, as the drawer lists it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalEdit {
    id: String,
    tab_id: String,
    remote_path: String,
    file_name: String,
    local_path: String,
    status: EditStatus,
    error: Option<String>,
}

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
    app: &'a AppHandle,
    tab_id: &'a str,
    plan: SessionPlan,
    prompts: HostPromptMap,
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
        download_file_with_progress(
            self.app,
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

fn emit(app: &AppHandle, edit: &ExternalEdit) {
    let _ = app.emit(EDIT_EVENT, edit);
}

/// Downloads the file into a new session's folder and starts watching it.
async fn start_session(
    host: &Host<'_>,
    remote_path: &str,
    file_name: &str,
    transfer_id: Option<&str>,
) -> Result<ExternalEdit, String> {
    let root = local::edit_root(host.app)?;
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
    host.app.state::<ExternalEditState>().insert(EditSession {
        edit: edit.clone(),
        dir,
        local,
        baseline,
        synced,
        observed: Some(synced.stamp),
        settling: None,
    });
    watch::ensure_running(host.app);
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
fn launch(app: &AppHandle, open_in: OpenIn, local: &Path) -> Result<bool, String> {
    match open_in {
        OpenIn::Default => launch::open_default(app, local).map(|()| true),
        OpenIn::Program { program } => launch::open_with(&program, local).map(|()| true),
        #[cfg(windows)]
        OpenIn::Chooser => launch::open_with_system_chooser(local),
        #[cfg(not(windows))]
        OpenIn::Chooser => Err("This system has no Open With dialog".to_string()),
    }
}

/// Opens the remote file at `path` as `open_in` says. A file already open
/// from this tab opens again as it is, downloaded afresh unless it has
/// changes of its own. Returns None when the user cancelled the system's
/// Open With dialog.
#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its IPC fields and injected state"
)]
pub async fn sftp_external_edit_open(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    open_in: OpenIn,
    transfer_id: Option<String>,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
    cancel_map: State<'_, TransferCancelMap>,
) -> Result<Option<ExternalEdit>, String> {
    let remote_path = normalize_remote_path(&path);
    let file_name = local::local_file_name(&file_name_of(&remote_path)?, cfg!(windows));
    let open_in = match open_in {
        OpenIn::Program { program } if program.trim().is_empty() => OpenIn::Default,
        OpenIn::Program { program } => OpenIn::Program {
            program: program.trim().to_string(),
        },
        other => other,
    };
    if matches!(open_in, OpenIn::Default) && local::runs_when_opened(&file_name) {
        return Err(format!(
            "'{file_name}' could run on this computer if opened in its default program. Open it in the external editor instead."
        ));
    }

    let host = Host {
        app: &app,
        tab_id: &tab_id,
        plan: ensure_ssh_plan(&app, &secret_state, connection)?,
        prompts: prompt_state.inner().clone(),
        pool: pool_state.inner(),
        cancel_map: cancel_map.inner(),
    };
    let transfer_id = transfer_id.as_deref();
    let state = app.state::<ExternalEditState>();
    let (edit, started) = match state.find(&tab_id, &remote_path) {
        Some(edit) if edit.status != EditStatus::Synced => (edit, false),
        Some(edit) => {
            let local = PathBuf::from(&edit.local_path);
            let baseline = host.download(&remote_path, &local, transfer_id).await?;
            let synced = fingerprint(local).await?;
            let edit = state.refreshed(&edit.id, baseline, synced).unwrap_or(edit);
            (edit, false)
        }
        None => (
            start_session(&host, &remote_path, &file_name, transfer_id).await?,
            true,
        ),
    };

    let local = PathBuf::from(&edit.local_path);
    let opener = app.clone();
    let launched = run_blocking(move || launch(&opener, open_in, &local)).await;
    if !matches!(launched, Ok(true)) && started {
        if let Some(session) = state.remove(&edit.id) {
            end_sessions(vec![session]).await;
        }
    }
    Ok(launched?.then_some(edit))
}

/// How the user can pick a program for `file_name` here.
#[tauri::command]
pub async fn sftp_external_open_with_choices(file_name: String) -> Result<OpenWithChoices, String> {
    #[cfg(target_os = "macos")]
    let apps = run_blocking(move || Ok(apps::apps_for(&file_name))).await?;
    #[cfg(not(target_os = "macos"))]
    let _ = file_name;
    Ok(OpenWithChoices {
        #[cfg(target_os = "macos")]
        apps,
        system_chooser: cfg!(windows),
    })
}

/// Uploads the copy's content over the remote file.
#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its IPC fields and injected state"
)]
pub async fn sftp_external_edit_upload(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    id: String,
    overwrite: bool,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
    cancel_map: State<'_, TransferCancelMap>,
) -> Result<UploadOutcome, String> {
    let state = app.state::<ExternalEditState>();
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;
    let (source, edit) = state.begin_upload(&id)?;
    emit(&app, &edit);

    let host = Host {
        app: &app,
        tab_id: &tab_id,
        plan,
        prompts: prompt_state.inner().clone(),
        pool: pool_state.inner(),
        cancel_map: cancel_map.inner(),
    };
    let result = async {
        let uploaded = fingerprint(source.local.clone()).await?;
        Ok::<_, String>(
            host.upload(&source, overwrite)
                .await?
                .map(|stamp| (stamp, uploaded)),
        )
    }
    .await;

    let (edit, outcome) = match result {
        Ok(Some((stamp, uploaded))) => match state.uploaded(&id, stamp, uploaded) {
            Some(edit) => (Some(edit.clone()), Ok(UploadOutcome::Uploaded { edit })),
            None => (
                None,
                Err("The file was uploaded, but it was closed meanwhile".to_string()),
            ),
        },
        Ok(None) => (
            state.upload_stopped(&id, None),
            Ok(UploadOutcome::RemoteChanged),
        ),
        Err(error) => (state.upload_stopped(&id, Some(error.clone())), Err(error)),
    };
    if let Some(edit) = edit {
        emit(&app, &edit);
    }
    outcome
}

/// The files open from the tab.
#[tauri::command]
pub async fn sftp_external_edit_list(app: AppHandle, tab_id: String) -> Vec<ExternalEdit> {
    app.state::<ExternalEditState>().list(&tab_id)
}

/// Stops watching the file and removes its copy, changes and all.
#[tauri::command]
pub async fn sftp_external_edit_stop(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(session) = app.state::<ExternalEditState>().remove(&id) {
        run_blocking(move || {
            local::remove_session_dir(&session.dir);
            Ok(())
        })
        .await?;
    }
    Ok(())
}

/// Called when the tab closes. Returns the files whose copies were kept,
/// as their changes were never uploaded.
#[tauri::command]
pub async fn sftp_external_edit_release_tab(app: AppHandle, tab_id: String) -> Vec<ExternalEdit> {
    end_sessions(app.state::<ExternalEditState>().remove_tab(&tab_id)).await
}

#[tauri::command]
pub async fn sftp_external_edit_reveal(app: AppHandle, id: String) -> Result<(), String> {
    let edit = app
        .state::<ExternalEditState>()
        .get(&id)
        .ok_or("This file is no longer open in another program")?;
    app.opener()
        .reveal_item_in_dir(&edit.local_path)
        .map_err(|error| format!("Failed to show '{}': {error}", edit.local_path))
}

/// The icons of the applications at `paths`, as PNG data URLs by path;
/// none outside macOS, which is the only system listing applications.
#[tauri::command]
pub async fn sftp_external_app_icons(
    paths: Vec<String>,
) -> Result<HashMap<String, Option<String>>, String> {
    #[cfg(target_os = "macos")]
    return run_blocking(move || Ok(apps::icons_for(paths))).await;
    #[cfg(not(target_os = "macos"))]
    {
        let _ = paths;
        Ok(HashMap::new())
    }
}

/// The names of the open files whose changes are not on the server, which
/// a quit would leave behind.
pub fn unuploaded_files(app: &AppHandle) -> Vec<String> {
    app.state::<ExternalEditState>().unuploaded_names()
}

/// Removes, at startup, copies left behind long ago.
pub fn clean_up_on_start(app: &AppHandle) {
    let Ok(root) = local::edit_root(app) else {
        return;
    };
    tauri::async_runtime::spawn_blocking(move || local::remove_stale(&root));
}

/// Removes, at exit, the copies without changes of their own.
pub fn shutdown(app: &AppHandle) {
    for session in app.state::<ExternalEditState>().remove_all() {
        if !session.has_changes() {
            local::remove_session_dir(&session.dir);
        }
    }
}
