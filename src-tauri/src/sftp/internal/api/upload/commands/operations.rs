use crate::core::session::types::PtyConnectionOptions;
use crate::sftp::internal::api::conflict::ConflictReport;
use crate::sftp::internal::api::prepare_transfer;
use crate::sftp::internal::api::upload::commands::collect_upload_conflicts;
use crate::sftp::internal::api::upload::{
    collect_upload_plan, file_name_from_path, inspect_upload_roots, next_transfer_id,
    upload_single_file_with_progress, UploadBatchResult, UploadFilePlanItem,
};
use crate::sftp::internal::connection::{ensure_ssh_plan, SftpContext};
use crate::sftp::internal::types::SftpConnectionPool;
use crate::sftp::internal::types::TransferCancelMap;
use crate::ssh::secret_store::types::SecretStoreState;
use std::path::Path;
use std::sync::Arc;
use tokio::{fs, sync::watch};
use tterm_core::events::sftp::{
    upload::{UploadBatchCompleteEvent, UploadBatchStartEvent},
    SftpEvent,
};
use tterm_core::events::{AppEvent, EventSink};
use tterm_core::prompts::PromptBroker;
use tterm_core::sftp::conflict::ConflictPolicy;

pub(super) struct SftpCheckUploadConflictsInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) local_paths: Vec<String>,
    pub(super) remote_base_path: String,
}
pub(super) struct SftpCheckUploadConflictsDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
}
pub(super) async fn sftp_check_upload_conflicts_service(
    dependencies: SftpCheckUploadConflictsDependencies,
    input: SftpCheckUploadConflictsInput,
) -> Result<ConflictReport, String> {
    let SftpCheckUploadConflictsDependencies {
        context,
        prompts,
        secrets,
        pool,
    } = dependencies;
    let SftpCheckUploadConflictsInput {
        tab_id,
        connection,
        local_paths,
        remote_base_path,
    } = input;

    let plan = ensure_ssh_plan(&secrets, connection)?;
    let upload_plan = collect_upload_plan(&local_paths, &remote_base_path, None).await?;

    with_sftp!(&context, &tab_id, &plan, prompts, &pool, sftp => {
        Ok(collect_upload_conflicts(sftp, &upload_plan).await)
    })
}

pub(super) struct SftpUploadFileInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) local_path: String,
    pub(super) remote_path: String,
    pub(super) transfer_id: String,
}
pub(super) struct SftpUploadFileDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
    pub(super) cancel: TransferCancelMap,
    pub(super) events: Arc<dyn EventSink>,
}
pub(super) async fn sftp_upload_file_service(
    dependencies: SftpUploadFileDependencies,
    input: SftpUploadFileInput,
) -> Result<(), String> {
    let SftpUploadFileDependencies {
        context,
        prompts,
        secrets,
        pool,
        cancel,
        events,
    } = dependencies;
    let SftpUploadFileInput {
        tab_id,
        connection,
        local_path,
        remote_path,
        transfer_id,
    } = input;

    let plan = ensure_ssh_plan(&secrets, connection)?;

    let metadata = fs::metadata(&local_path)
        .await
        .map_err(|err| format!("Failed to get file metadata '{local_path}': {err}"))?;
    let file_name = file_name_from_path(Path::new(&local_path))?;
    let plan_item = UploadFilePlanItem {
        file_name,
        file_size: metadata.len(),
        local_path,
        remote_dir: crate::sftp::internal::paths::parent_remote_path(&remote_path)
            .unwrap_or_default(),
        remote_path,
    };

    let (_sftp, channels) = prepare_transfer(&context, &tab_id, &plan, prompts, &pool).await?;

    upload_single_file_with_progress(
        &events,
        &tab_id,
        channels,
        &cancel,
        None,
        &plan_item,
        &transfer_id,
    )
    .await
}

pub(super) struct SftpUploadPathsInput {
    pub(super) tab_id: String,
    pub(super) connection: Option<PtyConnectionOptions>,
    pub(super) local_paths: Vec<String>,
    pub(super) remote_base_path: String,
    pub(super) skip_existing: Option<bool>,
    pub(super) conflict_policy: Option<ConflictPolicy>,
}
pub(super) struct SftpUploadPathsDependencies {
    pub(super) context: SftpContext,
    pub(super) prompts: Arc<PromptBroker>,
    pub(super) secrets: SecretStoreState,
    pub(super) pool: SftpConnectionPool,
    pub(super) cancel: TransferCancelMap,
    pub(super) events: Arc<dyn EventSink>,
}
pub(super) async fn sftp_upload_paths_service(
    dependencies: SftpUploadPathsDependencies,
    input: SftpUploadPathsInput,
) -> Result<UploadBatchResult, String> {
    let SftpUploadPathsDependencies {
        context,
        prompts,
        secrets,
        pool,
        cancel,
        events,
    } = dependencies;
    let SftpUploadPathsInput {
        tab_id,
        connection,
        local_paths,
        remote_base_path,
        skip_existing,
        conflict_policy,
    } = input;

    let plan = ensure_ssh_plan(&secrets, connection)?;
    let root_summary = inspect_upload_roots(&local_paths).await?;
    let batch_id = next_transfer_id();

    let mut batch_cancel_rx = None;
    if root_summary.has_directories {
        let (batch_cancel_tx, next_batch_cancel_rx) = watch::channel(false);
        cancel
            .write()
            .await
            .insert(batch_id.clone(), batch_cancel_tx);
        batch_cancel_rx = Some(next_batch_cancel_rx);

        let _ = events.emit(AppEvent::Sftp(SftpEvent::UploadBatchStart {
            tab_id: tab_id.to_string(),
            payload: UploadBatchStartEvent {
                batch_id: batch_id.clone(),
                display_name: root_summary.label.clone(),
                local_path: root_summary.local_path.clone(),
                local_paths: local_paths.clone(),
                remote_base_path: remote_base_path.clone(),
                conflict_policy: conflict_policy.unwrap_or_default(),
            },
        }));
    }

    let upload_plan =
        match collect_upload_plan(&local_paths, &remote_base_path, batch_cancel_rx.clone()).await {
            Ok(plan) => plan,
            Err(error) => {
                if root_summary.has_directories {
                    let cancelled = error.contains("cancelled");
                    let _ = events.emit(AppEvent::Sftp(SftpEvent::UploadBatchComplete {
                        tab_id: tab_id.to_string(),
                        payload: UploadBatchCompleteEvent {
                            batch_id: batch_id.clone(),
                            cancelled,
                            error: if cancelled { None } else { Some(error.clone()) },
                            failed: 0,
                            succeeded: 0,
                        },
                    }));
                    cancel.write().await.remove(&batch_id);

                    if cancelled {
                        return Ok(UploadBatchResult {
                            cancelled: true,
                            failed: 0,
                            succeeded: 0,
                        });
                    }
                }

                return Err(error);
            }
        };

    let result: Result<UploadBatchResult, String> =
        match prepare_transfer(&context, &tab_id, &plan, prompts, &pool).await {
            Ok((sftp, channels)) => {
                let cancel = cancel.clone();
                let tab_id = tab_id.clone();
                let batch_id = batch_id.clone();
                let directories = upload_plan.directories.clone();
                let files = upload_plan.files.clone();
                let batch_cancel_rx = batch_cancel_rx.clone();
                let batch_enabled = root_summary.has_directories;
                let policy = conflict_policy.unwrap_or_default();

                super::batch::run(
                    super::batch::UploadBatch {
                        cancel_map: cancel,
                        events: events.clone(),
                        tab_id,
                        batch_id,
                        directories,
                        files,
                        batch_cancel_rx,
                        batch_enabled,
                        policy,
                        skip_existing,
                    },
                    &sftp,
                    &channels,
                )
                .await
            }
            Err(error) => Err(error),
        };

    if root_summary.has_directories {
        let event = match &result {
            Ok(batch_result) => UploadBatchCompleteEvent {
                batch_id: batch_id.clone(),
                cancelled: batch_result.cancelled,
                error: None,
                failed: batch_result.failed,
                succeeded: batch_result.succeeded,
            },
            Err(error) => UploadBatchCompleteEvent {
                batch_id: batch_id.clone(),
                cancelled: false,
                error: Some(error.clone()),
                failed: 0,
                succeeded: 0,
            },
        };

        let _ = events.emit(AppEvent::Sftp(SftpEvent::UploadBatchComplete {
            tab_id: tab_id.to_string(),
            payload: event,
        }));
        cancel.write().await.remove(&batch_id);
    }

    result
}

pub(super) struct SftpCancelUploadInput {
    pub(super) transfer_id: String,
}
pub(super) struct SftpCancelUploadDependencies {
    pub(super) cancel: TransferCancelMap,
}
pub(super) async fn sftp_cancel_upload_service(
    dependencies: SftpCancelUploadDependencies,
    input: SftpCancelUploadInput,
) -> Result<(), String> {
    let SftpCancelUploadDependencies { cancel } = dependencies;
    let SftpCancelUploadInput { transfer_id } = input;

    let map = cancel.read().await;
    if let Some(sender) = map.get(&transfer_id) {
        let _ = sender.send(true);
        Ok(())
    } else {
        Err("Transfer not found or already completed".to_string())
    }
}
