use super::base::operations::{
    SftpCreateDirectoryDependencies, SftpCreateDirectoryInput, SftpListDirectoryDependencies,
    SftpListDirectoryInput, SftpReleaseTabConnectionsDependencies, SftpReleaseTabConnectionsInput,
    SftpRenameEntryDependencies, SftpRenameEntryInput, sftp_create_directory_service,
    sftp_list_directory_service, sftp_release_tab_connections_service, sftp_rename_entry_service,
};
use crate::core::session::types::PtyConnectionOptions;
use crate::services::AppServices;
use crate::sftp::internal::types::SftpDirectoryListing;
use tauri::State;

/// Called when a tab's SFTP drawer unmounts, i.e. the tab has closed.
#[tauri::command]
pub async fn sftp_release_tab_connections(
    tab_id: String,
    services: State<'_, std::sync::Arc<AppServices>>,
) -> Result<(), String> {
    sftp_release_tab_connections_service(
        SftpReleaseTabConnectionsDependencies {
            pool: services.sftp.pool.clone(),
        },
        SftpReleaseTabConnectionsInput { tab_id },
    )
    .await
}

#[tauri::command]
pub async fn sftp_list_directory(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: Option<String>,
    services: State<'_, std::sync::Arc<AppServices>>,
) -> Result<SftpDirectoryListing, String> {
    sftp_list_directory_service(
        SftpListDirectoryDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
        },
        SftpListDirectoryInput {
            tab_id,
            connection,
            path,
        },
    )
    .await
}

#[tauri::command]
pub async fn sftp_create_directory(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    path: String,
    services: State<'_, std::sync::Arc<AppServices>>,
) -> Result<(), String> {
    sftp_create_directory_service(
        SftpCreateDirectoryDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
        },
        SftpCreateDirectoryInput {
            tab_id,
            connection,
            path,
        },
    )
    .await
}

#[tauri::command]
pub async fn sftp_rename_entry(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    old_path: String,
    new_path: String,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    sftp_rename_entry_service(
        SftpRenameEntryDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
        },
        SftpRenameEntryInput {
            tab_id,
            connection,
            old_path,
            new_path,
        },
    )
    .await
}
