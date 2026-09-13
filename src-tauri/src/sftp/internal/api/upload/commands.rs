use super::*;
use crate::core::session::PtyConnectionOptions;
use crate::core::state::HostPromptMap;
use crate::sftp::internal::api::prepare_transfer;
use crate::sftp::internal::connection::ensure_ssh_plan;
use crate::sftp::internal::types::{SftpConnectionPool, TransferCancelMap};
use crate::ssh::SecretStoreState;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::watch;

/// Seconds since the Unix epoch for a local file's mtime.
async fn local_mtime_secs(path: &str) -> Option<i64> {
    let modified = fs::metadata(path).await.ok()?.modified().ok()?;
    Some(
        modified
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs() as i64,
    )
}

/// Whether the remote copy provably already holds this file's bytes: an
/// identical length and a remote mtime at least as new as the local one.
///
/// Only consulted for a run that explicitly opted into skipping existing
/// files (a batch retry). A size match alone is not evidence: an edited file
/// that kept its length would be reported as uploaded while the remote still
/// holds stale bytes. Clock skew between client and server can only make this
/// check stricter (an apparently older remote copy is simply re-transferred).
///
/// Two caveats inherent to the mtime+size evidence: uploads do not setstat
/// the local mtime, so a finished upload's remote mtime is the server-side
/// upload time — which is what makes it compare as current here. And a local
/// rewrite that keeps the length while restoring an older mtime (sync tools,
/// `git checkout`, `touch -d`) wrongly compares as already uploaded, the
/// same blind spot rsync's quick check has.
async fn remote_already_holds_source(sftp: &SftpSession, plan_item: &UploadFilePlanItem) -> bool {
    let Some(local_mtime) = local_mtime_secs(&plan_item.local_path).await else {
        return false;
    };
    let Ok(attrs) = sftp.metadata(&plan_item.remote_path).await else {
        return false;
    };
    attrs.size == Some(plan_item.file_size)
        && attrs
            .mtime
            .is_some_and(|remote_mtime| i64::from(remote_mtime) >= local_mtime)
}

#[tauri::command]
pub async fn sftp_upload_file(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    local_path: String,
    remote_path: String,
    transfer_id: String,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
    cancel_map: State<'_, TransferCancelMap>,
) -> Result<(), String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;

    let metadata = fs::metadata(&local_path)
        .await
        .map_err(|err| format!("Failed to get file metadata '{local_path}': {err}"))?;
    let file_name = file_name_from_path(Path::new(&local_path))?;
    let plan_item = UploadFilePlanItem {
        file_name,
        file_size: metadata.len(),
        local_path,
        remote_path,
    };

    let (_sftp, channels) = prepare_transfer(
        &app,
        &tab_id,
        &plan,
        prompt_state.inner().clone(),
        pool_state.inner(),
    )
    .await?;

    upload_single_file_with_progress(
        &app,
        &tab_id,
        channels,
        cancel_map.inner(),
        None,
        &plan_item,
        &transfer_id,
    )
    .await
}

#[tauri::command]
pub async fn sftp_upload_paths(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    local_paths: Vec<String>,
    remote_base_path: String,
    skip_existing: Option<bool>,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
    cancel_map: State<'_, TransferCancelMap>,
) -> Result<UploadBatchResult, String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;
    let root_summary = inspect_upload_roots(&local_paths).await?;
    let batch_id = next_transfer_id();

    let mut batch_cancel_rx = None;
    if root_summary.has_directories {
        let (batch_cancel_tx, next_batch_cancel_rx) = watch::channel(false);
        cancel_map
            .write()
            .await
            .insert(batch_id.clone(), batch_cancel_tx);
        batch_cancel_rx = Some(next_batch_cancel_rx);

        let _ = app.emit(
            &format!("sftp-upload-batch-start-{}", tab_id),
            UploadBatchStartEvent {
                batch_id: batch_id.clone(),
                display_name: root_summary.label.clone(),
                local_path: root_summary.local_path.clone(),
                local_paths: local_paths.clone(),
                remote_base_path: remote_base_path.clone(),
            },
        );
    }

    let upload_plan =
        match collect_upload_plan(&local_paths, &remote_base_path, batch_cancel_rx.clone()).await {
            Ok(plan) => plan,
            Err(error) => {
                if root_summary.has_directories {
                    let cancelled = error.contains("cancelled");
                    let _ = app.emit(
                        &format!("sftp-upload-batch-complete-{}", tab_id),
                        UploadBatchCompleteEvent {
                            batch_id: batch_id.clone(),
                            cancelled,
                            error: if cancelled { None } else { Some(error.clone()) },
                            failed: 0,
                            succeeded: 0,
                        },
                    );
                    cancel_map.write().await.remove(&batch_id);

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

    let result: Result<UploadBatchResult, String> = match prepare_transfer(
        &app,
        &tab_id,
        &plan,
        prompt_state.inner().clone(),
        pool_state.inner(),
    )
    .await
    {
        Ok((sftp, channels)) => {
            let cancel_map = cancel_map.inner().clone();
            let app = app.clone();
            let tab_id = tab_id.clone();
            let batch_id = batch_id.clone();
            let directories = upload_plan.directories.clone();
            let files = upload_plan.files.clone();
            let mut batch_cancel_rx = batch_cancel_rx.clone();
            let batch_enabled = root_summary.has_directories;

            async move {
                let mut succeeded = 0usize;
                let mut failed = 0usize;
                let mut cancelled = false;

                for directory in directories {
                    if batch_cancel_rx.as_mut().map(is_cancelled).unwrap_or(false) {
                        cancelled = true;
                        break;
                    }

                    match ensure_remote_dir_all(&sftp, &directory, batch_cancel_rx.clone()).await {
                        Ok(()) => {}
                        Err(error) => {
                            if error.contains("cancelled") {
                                cancelled = true;
                                break;
                            }
                            return Err(error);
                        }
                    }
                }

                if !cancelled {
                    for plan_item in files {
                        if batch_cancel_rx.as_mut().map(is_cancelled).unwrap_or(false) {
                            cancelled = true;
                            break;
                        }

                        let transfer_id = next_transfer_id();
                        let _ = app.emit(
                            &format!("sftp-upload-item-start-{}", tab_id),
                            UploadItemStartEvent {
                                transfer_id: transfer_id.clone(),
                                batch_id: if batch_enabled {
                                    Some(batch_id.clone())
                                } else {
                                    None
                                },
                                file_name: plan_item.file_name.clone(),
                                file_size: plan_item.file_size,
                                local_path: plan_item.local_path.clone(),
                                remote_path: plan_item.remote_path.clone(),
                            },
                        );

                        // Skipping is opt-in (a batch retry re-runs the whole
                        // batch) and even then only when the remote copy
                        // provably holds the local bytes. A plain size match
                        // is not enough: an edited file that kept its length
                        // would be reported as uploaded while the remote still
                        // has the old bytes. A stat error is treated as "not
                        // uploaded" and simply re-transfers.
                        let already_uploaded = skip_existing.unwrap_or(false)
                            && remote_already_holds_source(&sftp, &plan_item).await;
                        if already_uploaded {
                            succeeded += 1;
                            let _ = app.emit(
                                &format!("sftp-upload-item-complete-{}", tab_id),
                                UploadItemCompleteEvent {
                                    transfer_id,
                                    error: None,
                                    local_path: plan_item.local_path.clone(),
                                    remote_path: plan_item.remote_path.clone(),
                                    cancelled: false,
                                    success: true,
                                    skipped: true,
                                },
                            );
                            continue;
                        }

                        let result = upload_single_file_with_progress(
                            &app,
                            &tab_id,
                            channels.instance(),
                            &cancel_map,
                            batch_cancel_rx.clone(),
                            &plan_item,
                            &transfer_id,
                        )
                        .await;

                        match result {
                            Ok(()) => {
                                succeeded += 1;
                                let _ = app.emit(
                                    &format!("sftp-upload-item-complete-{}", tab_id),
                                    UploadItemCompleteEvent {
                                        transfer_id,
                                        error: None,
                                        local_path: plan_item.local_path,
                                        remote_path: plan_item.remote_path,
                                        cancelled: false,
                                        success: true,
                                        skipped: false,
                                    },
                                );
                            }
                            Err(error) => {
                                let item_cancelled = error.contains("cancelled");
                                if item_cancelled {
                                    cancelled = true;
                                } else {
                                    failed += 1;
                                }

                                let _ = app.emit(
                                    &format!("sftp-upload-item-complete-{}", tab_id),
                                    UploadItemCompleteEvent {
                                        transfer_id,
                                        error: Some(error),
                                        local_path: plan_item.local_path,
                                        remote_path: plan_item.remote_path,
                                        cancelled: item_cancelled,
                                        success: false,
                                        skipped: false,
                                    },
                                );

                                if item_cancelled {
                                    break;
                                }
                            }
                        }
                    }
                }

                Ok(UploadBatchResult {
                    cancelled,
                    failed,
                    succeeded,
                })
            }
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

        let _ = app.emit(&format!("sftp-upload-batch-complete-{}", tab_id), event);
        cancel_map.write().await.remove(&batch_id);
    }

    result
}

#[tauri::command]
pub async fn sftp_cancel_upload(
    transfer_id: String,
    cancel_map: State<'_, TransferCancelMap>,
) -> Result<(), String> {
    let map = cancel_map.read().await;
    if let Some(sender) = map.get(&transfer_id) {
        let _ = sender.send(true);
        Ok(())
    } else {
        Err("Transfer not found or already completed".to_string())
    }
}
