use crate::services::AppServices;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::events::sftp::external::ExternalEdit;
use tterm_core::sftp::internal::api::external::OpenIn;
use tterm_core::sftp::internal::api::external::OpenWithChoices;
use tterm_core::sftp::internal::api::external::UploadOutcome;
use tterm_core::sftp::internal::api::external::operations::SftpExternalAppIconsInput;
use tterm_core::sftp::internal::api::external::operations::SftpExternalEditListDependencies;
use tterm_core::sftp::internal::api::external::operations::SftpExternalEditListInput;
use tterm_core::sftp::internal::api::external::operations::SftpExternalEditOpenDependencies;
use tterm_core::sftp::internal::api::external::operations::SftpExternalEditOpenInput;
use tterm_core::sftp::internal::api::external::operations::SftpExternalEditReleaseTabDependencies;
use tterm_core::sftp::internal::api::external::operations::SftpExternalEditReleaseTabInput;
use tterm_core::sftp::internal::api::external::operations::SftpExternalEditRevealInput;
use tterm_core::sftp::internal::api::external::operations::SftpExternalEditStopDependencies;
use tterm_core::sftp::internal::api::external::operations::SftpExternalEditStopInput;
use tterm_core::sftp::internal::api::external::operations::SftpExternalEditUploadDependencies;
use tterm_core::sftp::internal::api::external::operations::SftpExternalEditUploadInput;
use tterm_core::sftp::internal::api::external::operations::SftpExternalOpenWithChoicesInput;
use tterm_core::sftp::internal::api::external::operations::sftp_external_app_icons_service;
use tterm_core::sftp::internal::api::external::operations::sftp_external_edit_list_service;
use tterm_core::sftp::internal::api::external::operations::sftp_external_edit_open_service;
use tterm_core::sftp::internal::api::external::operations::sftp_external_edit_release_tab_service;
use tterm_core::sftp::internal::api::external::operations::sftp_external_edit_reveal_service;
use tterm_core::sftp::internal::api::external::operations::sftp_external_edit_stop_service;
use tterm_core::sftp::internal::api::external::operations::sftp_external_edit_upload_service;
use tterm_core::sftp::internal::api::external::operations::sftp_external_open_with_choices_service;

/// Opens the remote file at `path` as `open_in` says. A file already open
/// from this tab opens again as it is, downloaded afresh unless it has
/// changes of its own. Returns None when the user cancelled the system's
/// Open With dialog.
#[tauri::command]
pub async fn sftp_external_edit_open(
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
            paths: services.shared.paths.clone(),
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
