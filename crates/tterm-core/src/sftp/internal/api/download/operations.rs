use crate::core::session::types::PtyConnectionOptions;
use crate::events::EventSink;
use crate::prompts::PromptBroker;
use crate::sftp::conflict::ConflictPolicy;
use crate::sftp::internal::api::conflict;
use crate::sftp::internal::api::conflict::ConflictReport;
use crate::sftp::internal::api::conflict::TransferConflict;
use crate::sftp::internal::api::download::directory::DirectoryDownloadRequest;
use crate::sftp::internal::api::download::directory::DownloadTarget;
use crate::sftp::internal::api::download::directory::collect_download_plan;
use crate::sftp::internal::api::download::directory::download_directory_with_progress;
use crate::sftp::internal::api::download::directory::remote_basename;
use crate::sftp::internal::api::download::download_file_with_progress;
use crate::sftp::internal::api::prepare_transfer;
use crate::sftp::internal::connection::SftpContext;
use crate::sftp::internal::connection::ensure_ssh_plan;
use crate::sftp::internal::types::SftpConnectionPool;
use crate::sftp::internal::types::TransferCancelMap;
use crate::ssh::secret_store::types::SecretStoreState;
use std::path::PathBuf;
use std::sync::Arc;

pub struct SftpDownloadFileInput {
    pub tab_id: String,
    pub connection: Option<PtyConnectionOptions>,
    pub transfer_id: String,
    pub remote_path: String,
    pub local_path: String,
}
pub struct SftpDownloadFileDependencies {
    pub context: SftpContext,
    pub prompts: Arc<PromptBroker>,
    pub secrets: SecretStoreState,
    pub pool: SftpConnectionPool,
    pub cancel: TransferCancelMap,
    pub events: Arc<dyn EventSink>,
}
pub async fn sftp_download_file_service(
    dependencies: SftpDownloadFileDependencies,
    input: SftpDownloadFileInput,
) -> Result<(), String> {
    let SftpDownloadFileDependencies {
        context,
        prompts,
        secrets,
        pool,
        cancel,
        events,
    } = dependencies;
    let SftpDownloadFileInput {
        tab_id,
        connection,
        transfer_id,
        remote_path,
        local_path,
    } = input;

    let plan = ensure_ssh_plan(&secrets, connection)?;

    let (_sftp, channels) = prepare_transfer(&context, &tab_id, &plan, prompts, &pool).await?;

    download_file_with_progress(
        &events,
        &tab_id,
        channels,
        &cancel,
        &transfer_id,
        &remote_path,
        &local_path,
    )
    .await
}

pub struct SftpDownloadPathsInput {
    pub tab_id: String,
    pub connection: Option<PtyConnectionOptions>,
    pub transfer_id: String,
    pub remote_paths: Vec<String>,
    pub local_parent_path: String,
    pub skip_existing: Option<bool>,
    pub conflict_policy: Option<ConflictPolicy>,
}
pub struct SftpDownloadPathsDependencies {
    pub context: SftpContext,
    pub prompts: Arc<PromptBroker>,
    pub secrets: SecretStoreState,
    pub pool: SftpConnectionPool,
    pub cancel: TransferCancelMap,
    pub events: Arc<dyn EventSink>,
}
pub async fn sftp_download_paths_service(
    dependencies: SftpDownloadPathsDependencies,
    input: SftpDownloadPathsInput,
) -> Result<(), String> {
    let SftpDownloadPathsDependencies {
        context,
        prompts,
        secrets,
        pool,
        cancel,
        events,
    } = dependencies;
    let SftpDownloadPathsInput {
        tab_id,
        connection,
        transfer_id,
        remote_paths,
        local_parent_path,
        skip_existing,
        conflict_policy,
    } = input;

    let plan = ensure_ssh_plan(&secrets, connection)?;

    let (sftp, channels) = prepare_transfer(&context, &tab_id, &plan, prompts, &pool).await?;

    download_directory_with_progress(
        DownloadTarget {
            events: &events,
            tab_id: &tab_id,
            cancel_map: &cancel,
        },
        &sftp,
        &channels,
        DirectoryDownloadRequest {
            transfer_id: &transfer_id,
            remote_paths: &remote_paths,
            local_parent_path: &local_parent_path,
            skip_existing,
            policy: conflict_policy.unwrap_or_default(),
        },
    )
    .await
}

pub struct SftpCheckDownloadConflictsInput {
    pub tab_id: String,
    pub connection: Option<PtyConnectionOptions>,
    pub remote_paths: Vec<String>,
    pub local_parent_path: String,
}
pub struct SftpCheckDownloadConflictsDependencies {
    pub context: SftpContext,
    pub prompts: Arc<PromptBroker>,
    pub secrets: SecretStoreState,
    pub pool: SftpConnectionPool,
}
pub async fn sftp_check_download_conflicts_service(
    dependencies: SftpCheckDownloadConflictsDependencies,
    input: SftpCheckDownloadConflictsInput,
) -> Result<ConflictReport, String> {
    let SftpCheckDownloadConflictsDependencies {
        context,
        prompts,
        secrets,
        pool,
    } = dependencies;
    let SftpCheckDownloadConflictsInput {
        tab_id,
        connection,
        remote_paths,
        local_parent_path,
    } = input;

    let local_parent = PathBuf::from(&local_parent_path);
    let mut any_root_exists = false;
    for remote_path in &remote_paths {
        let root_name = remote_basename(remote_path)?;
        if tokio::fs::metadata(local_parent.join(&root_name))
            .await
            .is_ok()
        {
            any_root_exists = true;
            break;
        }
    }
    if !any_root_exists {
        return Ok(ConflictReport::default());
    }

    let plan = ensure_ssh_plan(&secrets, connection)?;
    with_sftp!(&context, &tab_id, &plan, prompts, &pool, sftp => {
        let download_plan = collect_download_plan(sftp, &remote_paths).await?;
        let mut report = ConflictReport {
            file_count: download_plan.files.len(),
            ..ConflictReport::default()
        };

        for item in &download_plan.files {
            let local_path = local_parent.join(&item.relative_path);
            let Some(target) = conflict::local_target_stat(&local_path).await else {
                continue;
            };
            report.push(TransferConflict {
                source_path: item.remote_path.clone(),
                target_path: local_path.display().to_string(),
                source_size: item.size,
                source_mtime: item.mtime,
                target_size: target.size,
                target_mtime: target.mtime,
                target_is_dir: target.is_dir,
            });
        }

        Ok(report)
    })
}

/// Reads file metadata away from the calling thread.
pub async fn get_file_size_service(local_path: String) -> Result<u64, String> {
    tokio::task::spawn_blocking(move || {
        std::fs::metadata(&local_path)
            .map(|metadata| metadata.len())
            .map_err(|err| format!("Failed to get file size for '{local_path}': {err}"))
    })
    .await
    .map_err(|err| format!("Task join error: {err}"))?
}
