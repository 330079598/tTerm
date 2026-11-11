use super::download_file_with_progress;
use crate::core::session::types::PtyConnectionOptions;
use crate::sftp::internal::api::conflict::{self, ConflictReport, TransferConflict};
use crate::sftp::internal::api::download::directory::{
    collect_download_plan, download_directory_with_progress, remote_basename,
    DirectoryDownloadRequest, DownloadTarget,
};
use crate::sftp::internal::api::prepare_transfer;
use crate::sftp::internal::connection::{ensure_ssh_plan, SftpContext};
use crate::sftp::internal::types::SftpConnectionPool;
use crate::sftp::internal::types::TransferCancelMap;
use crate::ssh::secret_store::types::SecretStoreState;
use std::path::PathBuf;
use std::sync::Arc;
use tterm_core::events::EventSink;
use tterm_core::prompts::PromptBroker;
use tterm_core::sftp::conflict::ConflictPolicy;

pub(super) struct SftpDownloadFileInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) transfer_id: String,
    pub(super) remote_path: String,
    pub(super) local_path: String,
}
pub(super) struct SftpDownloadFileDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
    pub(super) cancel: TransferCancelMap,
    pub(super) events: Arc<dyn EventSink>,
}
pub(super) async fn sftp_download_file_service(
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

pub(super) struct SftpDownloadPathsInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) transfer_id: String,
    pub(super) remote_paths: Vec<String>,
    pub(super) local_parent_path: String,
    pub(super) skip_existing: Option<bool>,
    pub(super) conflict_policy: Option<ConflictPolicy>,
}
pub(super) struct SftpDownloadPathsDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
    pub(super) cancel: TransferCancelMap,
    pub(super) events: Arc<dyn EventSink>,
}
pub(super) async fn sftp_download_paths_service(
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

pub(super) struct SftpCheckDownloadConflictsInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) remote_paths: Vec<String>,
    pub(super) local_parent_path: String,
}
pub(super) struct SftpCheckDownloadConflictsDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
}
pub(super) async fn sftp_check_download_conflicts_service(
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
pub(super) async fn get_file_size_service(local_path: String) -> Result<u64, String> {
    tokio::task::spawn_blocking(move || {
        std::fs::metadata(&local_path)
            .map(|metadata| metadata.len())
            .map_err(|err| format!("Failed to get file size for '{local_path}': {err}"))
    })
    .await
    .map_err(|err| format!("Task join error: {err}"))?
}
