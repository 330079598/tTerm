use tauri::State;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::sftp::internal::api::permissions::operations::SftpSetPermissionsDependencies;
use tterm_core::sftp::internal::api::permissions::operations::SftpSetPermissionsInput;
use tterm_core::sftp::internal::api::permissions::operations::sftp_set_permissions_service;

#[tauri::command]
pub async fn sftp_set_permissions(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    paths: Vec<String>,
    mode: u32,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    sftp_set_permissions_service(
        SftpSetPermissionsDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
        },
        SftpSetPermissionsInput {
            tab_id,
            connection,
            paths,
            mode,
        },
    )
    .await
}
