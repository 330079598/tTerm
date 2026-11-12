#[cfg(target_os = "macos")]
use super::{AppChoice, png_data_url};
use super::{
    ExternalEditState, Host, OpenIn, OpenWithChoices, UploadOutcome, emit, end_sessions,
    file_name_of, fingerprint, launch, local, start_session,
};
use crate::core::blocking::run_blocking;
use crate::core::session::types::PtyConnectionOptions;
use crate::sftp::internal::connection::{SftpContext, ensure_ssh_plan};
use crate::sftp::internal::paths::normalize_remote_path;
use crate::sftp::internal::types::SftpConnectionPool;
use crate::sftp::internal::types::TransferCancelMap;
use crate::ssh::secret_store::types::SecretStoreState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tterm_core::events::EventSink;
use tterm_core::events::sftp::external::{EditStatus, ExternalEdit};
use tterm_core::paths::AppPaths;
use tterm_core::prompts::PromptBroker;
use tterm_platform::open::OpenTargets;

pub(super) struct SftpExternalEditOpenInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) path: String,
    pub(super) open_in: OpenIn,
    pub(super) transfer_id: Option<String>,
}
pub(super) struct SftpExternalEditOpenDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
    pub(super) cancel: TransferCancelMap,
    pub(super) events: Arc<dyn EventSink>,
    pub(super) state: Arc<ExternalEditState>,
    pub(super) opener: Arc<dyn OpenTargets>,
    pub(super) paths: Result<AppPaths, String>,
}
pub(super) async fn sftp_external_edit_open_service(
    dependencies: SftpExternalEditOpenDependencies,
    input: SftpExternalEditOpenInput,
) -> Result<Option<ExternalEdit>, String> {
    let SftpExternalEditOpenDependencies {
        context,
        prompts,
        secrets,
        pool,
        cancel,
        events,
        state,
        opener,
        paths,
    } = dependencies;
    let SftpExternalEditOpenInput {
        tab_id,
        connection,
        path,
        open_in,
        transfer_id,
    } = input;

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
        app: &context,
        events,
        tab_id: &tab_id,
        plan: ensure_ssh_plan(&secrets, connection)?,
        prompts,
        pool: &pool,
        cancel_map: &cancel,
    };
    let transfer_id = transfer_id.as_deref();
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
            start_session(
                &host,
                &state,
                paths.as_ref().map_err(Clone::clone)?,
                &remote_path,
                &file_name,
                transfer_id,
            )
            .await?,
            true,
        ),
    };

    let local = PathBuf::from(&edit.local_path);
    let launched = launch(opener.as_ref(), open_in, &local).await;
    if !matches!(launched, Ok(true))
        && started
        && let Some(session) = state.remove(&edit.id)
    {
        end_sessions(vec![session]).await;
    }
    Ok(launched?.then_some(edit))
}

pub(super) struct SftpExternalOpenWithChoicesInput {
    pub(super) file_name: String,
}
pub(super) async fn sftp_external_open_with_choices_service(
    opener: Arc<dyn OpenTargets>,
    input: SftpExternalOpenWithChoicesInput,
) -> Result<OpenWithChoices, String> {
    let SftpExternalOpenWithChoicesInput { file_name } = input;

    #[cfg(target_os = "macos")]
    let apps = opener
        .applications_for(file_name)
        .await?
        .into_iter()
        .map(AppChoice::from)
        .collect();
    #[cfg(not(target_os = "macos"))]
    let _ = (opener, file_name);
    Ok(OpenWithChoices {
        #[cfg(target_os = "macos")]
        apps,
        system_chooser: cfg!(windows),
    })
}

pub(super) struct SftpExternalEditUploadInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) id: String,
    pub(super) overwrite: bool,
}
pub(super) struct SftpExternalEditUploadDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
    pub(super) cancel: TransferCancelMap,
    pub(super) events: Arc<dyn EventSink>,
    pub(super) state: Arc<ExternalEditState>,
}
pub(super) async fn sftp_external_edit_upload_service(
    dependencies: SftpExternalEditUploadDependencies,
    input: SftpExternalEditUploadInput,
) -> Result<UploadOutcome, String> {
    let SftpExternalEditUploadDependencies {
        context,
        prompts,
        secrets,
        pool,
        cancel,
        events,
        state,
    } = dependencies;
    let SftpExternalEditUploadInput {
        tab_id,
        connection,
        id,
        overwrite,
    } = input;

    let plan = ensure_ssh_plan(&secrets, connection)?;
    let (source, edit) = state.begin_upload(&id)?;
    emit(events.as_ref(), &edit);

    let host = Host {
        app: &context,
        events: events.clone(),
        tab_id: &tab_id,
        plan,
        prompts,
        pool: &pool,
        cancel_map: &cancel,
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
        emit(events.as_ref(), &edit);
    }
    outcome
}

pub(super) struct SftpExternalEditListInput {
    pub(super) tab_id: String,
}
pub(super) struct SftpExternalEditListDependencies {
    pub(super) state: Arc<ExternalEditState>,
}
pub(super) async fn sftp_external_edit_list_service(
    dependencies: SftpExternalEditListDependencies,
    input: SftpExternalEditListInput,
) -> Vec<ExternalEdit> {
    let SftpExternalEditListDependencies { state } = dependencies;
    let SftpExternalEditListInput { tab_id } = input;

    state.list(&tab_id)
}

pub(super) struct SftpExternalEditStopInput {
    pub(super) id: String,
}
pub(super) struct SftpExternalEditStopDependencies {
    pub(super) state: Arc<ExternalEditState>,
}
pub(super) async fn sftp_external_edit_stop_service(
    dependencies: SftpExternalEditStopDependencies,
    input: SftpExternalEditStopInput,
) -> Result<(), String> {
    let SftpExternalEditStopDependencies { state } = dependencies;
    let SftpExternalEditStopInput { id } = input;

    if let Some(session) = state.remove(&id) {
        run_blocking(move || {
            local::remove_session_dir(&session.dir);
            Ok(())
        })
        .await?;
    }
    Ok(())
}

pub(super) struct SftpExternalEditReleaseTabInput {
    pub(super) tab_id: String,
}
pub(super) struct SftpExternalEditReleaseTabDependencies {
    pub(super) state: Arc<ExternalEditState>,
}
pub(super) async fn sftp_external_edit_release_tab_service(
    dependencies: SftpExternalEditReleaseTabDependencies,
    input: SftpExternalEditReleaseTabInput,
) -> Vec<ExternalEdit> {
    let SftpExternalEditReleaseTabDependencies { state } = dependencies;
    let SftpExternalEditReleaseTabInput { tab_id } = input;

    end_sessions(state.remove_tab(&tab_id)).await
}

pub(super) struct SftpExternalEditRevealInput {
    pub(super) id: String,
}
pub(super) async fn sftp_external_edit_reveal_service(
    state: Arc<ExternalEditState>,
    opener: Arc<dyn OpenTargets>,
    input: SftpExternalEditRevealInput,
) -> Result<(), String> {
    let SftpExternalEditRevealInput { id } = input;

    let edit = state
        .get(&id)
        .ok_or("This file is no longer open in another program")?;
    opener
        .reveal(PathBuf::from(&edit.local_path))
        .await
        .map_err(|error| format!("Failed to show '{}': {error}", edit.local_path))
}

pub(super) struct SftpExternalAppIconsInput {
    pub(super) paths: Vec<String>,
}
pub(super) async fn sftp_external_app_icons_service(
    opener: Arc<dyn OpenTargets>,
    input: SftpExternalAppIconsInput,
) -> Result<HashMap<String, Option<String>>, String> {
    let SftpExternalAppIconsInput { paths } = input;

    #[cfg(target_os = "macos")]
    return Ok(opener
        .application_icons(paths)
        .await?
        .into_iter()
        .map(|(path, bytes)| (path, bytes.map(png_data_url)))
        .collect());
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (opener, paths);
        Ok(HashMap::new())
    }
}
