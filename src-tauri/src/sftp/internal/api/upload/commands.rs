use super::*;
use crate::core::session::PtyConnectionOptions;
use crate::core::state::HostPromptMap;
use crate::sftp::internal::api::conflict::{
    self, CandidateDecision, ConflictPolicy, ConflictReport, SourceStat, TransferConflict,
};
use crate::sftp::internal::api::prepare_transfer;
use crate::sftp::internal::connection::ensure_ssh_plan;
use crate::sftp::internal::types::{SftpConnectionPool, TransferCancelMap};
use crate::ssh::SecretStoreState;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::watch;

/// Where a planned upload should land under `policy`, or `None` to skip it.
///
/// `skip_existing` is set by a batch retry, so files an earlier attempt
/// already finished are passed over instead of re-sent (see
/// [`conflict::decide_candidate`]). A stat error is treated as "nothing
/// there" and the transfer itself surfaces the real problem.
async fn resolve_upload_target(
    sftp: &SftpSession,
    plan_item: &UploadFilePlanItem,
    policy: ConflictPolicy,
    skip_existing: bool,
) -> Result<Option<String>, String> {
    // The historical behaviour needs no stat round trip at all.
    if policy == ConflictPolicy::Overwrite && !skip_existing {
        return Ok(Some(plan_item.remote_path.clone()));
    }

    let source = SourceStat {
        size: plan_item.file_size,
        mtime: fs::metadata(&plan_item.local_path)
            .await
            .ok()
            .and_then(|metadata| conflict::local_mtime_secs(&metadata)),
    };

    for attempt in 0..=conflict::MAX_RENAME_ATTEMPTS {
        let candidate = if attempt == 0 {
            plan_item.remote_path.clone()
        } else {
            crate::sftp::internal::paths::join_remote_path(
                &plan_item.remote_dir,
                &conflict::candidate_file_name(&plan_item.file_name, attempt),
            )
        };
        let target = conflict::remote_target_stat(sftp, &candidate).await;
        match conflict::decide_candidate(policy, source, target, skip_existing) {
            CandidateDecision::Use => return Ok(Some(candidate)),
            CandidateDecision::Skip => return Ok(None),
            CandidateDecision::NextName => {}
        }
    }

    Err(format!(
        "No free name left for '{}' in '{}'",
        plan_item.file_name, plan_item.remote_dir
    ))
}

/// Pre-flight for `sftp_upload_paths`: which planned files already exist on
/// the remote. Each destination directory is listed once rather than every
/// file being stat'ed, so a large folder costs as many round trips as the
/// upload spends creating its directories.
#[tauri::command]
pub async fn sftp_check_upload_conflicts(
    app: AppHandle,
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    local_paths: Vec<String>,
    remote_base_path: String,
    prompt_state: State<'_, HostPromptMap>,
    secret_state: State<'_, SecretStoreState>,
    pool_state: State<'_, SftpConnectionPool>,
) -> Result<ConflictReport, String> {
    let plan = ensure_ssh_plan(&app, &secret_state, connection)?;
    let upload_plan = collect_upload_plan(&local_paths, &remote_base_path, None).await?;

    with_sftp!(&app, &tab_id, &plan, prompt_state.inner().clone(), pool_state.inner(), sftp => {
        Ok(collect_upload_conflicts(sftp, &upload_plan).await)
    })
}

async fn collect_upload_conflicts(sftp: &SftpSession, upload_plan: &UploadPlan) -> ConflictReport {
    let mut report = ConflictReport {
        file_count: upload_plan.files.len(),
        ..ConflictReport::default()
    };
    let mut listings: HashMap<String, HashMap<String, conflict::TargetStat>> = HashMap::new();

    for plan_item in &upload_plan.files {
        if !listings.contains_key(&plan_item.remote_dir) {
            // A directory that does not exist yet (or cannot be listed)
            // holds nothing to collide with.
            let mut entries = HashMap::new();
            if let Ok(read_dir) = sftp.read_dir(&plan_item.remote_dir).await {
                for entry in read_dir {
                    let metadata = entry.metadata();
                    entries.insert(
                        entry.file_name(),
                        conflict::TargetStat {
                            size: metadata.size.unwrap_or(0),
                            mtime: metadata.mtime.map(i64::from),
                            is_dir: metadata.is_dir(),
                        },
                    );
                }
            }
            listings.insert(plan_item.remote_dir.clone(), entries);
        }

        let Some(target) = listings
            .get(&plan_item.remote_dir)
            .and_then(|entries| entries.get(&plan_item.file_name))
        else {
            continue;
        };

        let source_mtime = fs::metadata(&plan_item.local_path)
            .await
            .ok()
            .and_then(|metadata| conflict::local_mtime_secs(&metadata));
        report.push(TransferConflict {
            source_path: plan_item.local_path.clone(),
            target_path: plan_item.remote_path.clone(),
            source_size: plan_item.file_size,
            source_mtime,
            target_size: target.size,
            target_mtime: target.mtime,
            target_is_dir: target.is_dir,
        });
    }

    report
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
        remote_dir: crate::sftp::internal::paths::parent_remote_path(&remote_path)
            .unwrap_or_default(),
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
    conflict_policy: Option<ConflictPolicy>,
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
                conflict_policy: conflict_policy.unwrap_or_default(),
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
            let policy = conflict_policy.unwrap_or_default();

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
                        let resolved = resolve_upload_target(
                            &sftp,
                            &plan_item,
                            policy,
                            skip_existing.unwrap_or(false),
                        )
                        .await;
                        // The start event already carries the resolved name, so
                        // the transfer list and a per-item retry both use the
                        // path the file actually lands on.
                        let plan_item = match &resolved {
                            Ok(Some(remote_path)) => UploadFilePlanItem {
                                remote_path: remote_path.clone(),
                                ..plan_item
                            },
                            _ => plan_item,
                        };
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

                        let resolved = match resolved {
                            Ok(resolved) => resolved,
                            Err(error) => {
                                failed += 1;
                                let _ = app.emit(
                                    &format!("sftp-upload-item-complete-{}", tab_id),
                                    UploadItemCompleteEvent {
                                        transfer_id,
                                        error: Some(error),
                                        local_path: plan_item.local_path,
                                        remote_path: plan_item.remote_path,
                                        cancelled: false,
                                        success: false,
                                        skipped: false,
                                    },
                                );
                                continue;
                            }
                        };

                        if resolved.is_none() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sftp::internal::transfer::test_server::{ServerOptions, TestServer};
    use std::time::{Duration, SystemTime};

    fn write_with_mtime(path: &Path, contents: &[u8], mtime: SystemTime) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
    }

    fn plan_item(local: &Path, remote_dir: &str, name: &str) -> UploadFilePlanItem {
        UploadFilePlanItem {
            file_name: name.to_string(),
            file_size: std::fs::metadata(local).unwrap().len(),
            local_path: local.to_string_lossy().into_owned(),
            remote_dir: remote_dir.to_string(),
            remote_path: format!("{remote_dir}/{name}"),
        }
    }

    #[tokio::test]
    async fn preflight_reports_only_files_that_exist_remotely() {
        let server = TestServer::start(ServerOptions::default()).await;
        let sftp = server.sftp_session().await;
        let now = SystemTime::now();

        write_with_mtime(&server.root().join("dst/nginx.conf"), b"remote", now);
        write_with_mtime(&server.root().join("dst/site/index.html"), b"remote", now);
        let local = server.local_dir();
        write_with_mtime(&local.join("nginx.conf"), b"local!", now);
        write_with_mtime(&local.join("site/index.html"), b"local!", now);
        write_with_mtime(&local.join("site/new.txt"), b"new", now);

        let upload_plan = collect_upload_plan(
            &[
                local.join("nginx.conf").to_string_lossy().into_owned(),
                local.join("site").to_string_lossy().into_owned(),
            ],
            "/dst",
            None,
        )
        .await
        .unwrap();
        let report = collect_upload_conflicts(&sftp, &upload_plan).await;

        assert_eq!(report.file_count, 3);
        assert_eq!(report.total, 2);
        let mut targets: Vec<_> = report
            .conflicts
            .iter()
            .map(|conflict| conflict.target_path.as_str())
            .collect();
        targets.sort();
        assert_eq!(targets, ["/dst/nginx.conf", "/dst/site/index.html"]);
        let conflict = &report.conflicts[0];
        assert_eq!(conflict.source_size, 6);
        assert_eq!(conflict.target_size, 6);
        assert!(conflict.source_mtime.is_some() && conflict.target_mtime.is_some());
    }

    #[tokio::test]
    async fn policies_resolve_against_a_live_sftp_server() {
        let server = TestServer::start(ServerOptions::default()).await;
        let sftp = server.sftp_session().await;
        let now = SystemTime::now();
        let hour = Duration::from_secs(3600);

        // The server's copy was edited an hour ago.
        write_with_mtime(
            &server.root().join("dst/app.conf"),
            b"server edit",
            now - hour,
        );
        let local = server.local_dir().join("app.conf");

        // A local copy older than the server's must not replace it.
        write_with_mtime(&local, b"stale local", now - hour * 2);
        let item = plan_item(&local, "/dst", "app.conf");
        let resolve = |policy, skip_existing| {
            let sftp = &sftp;
            let item = &item;
            async move {
                resolve_upload_target(sftp, item, policy, skip_existing)
                    .await
                    .unwrap()
            }
        };
        assert_eq!(resolve(ConflictPolicy::OverwriteIfNewer, false).await, None);
        assert_eq!(resolve(ConflictPolicy::Skip, false).await, None);
        assert_eq!(
            resolve(ConflictPolicy::Overwrite, false).await.as_deref(),
            Some("/dst/app.conf")
        );
        assert_eq!(
            resolve(ConflictPolicy::Rename, false).await.as_deref(),
            Some("/dst/app (1).conf")
        );

        // Edited locally after the server's copy: "newer" now overwrites.
        write_with_mtime(&local, b"fresh local", now);
        let item = plan_item(&local, "/dst", "app.conf");
        assert_eq!(
            resolve_upload_target(&sftp, &item, ConflictPolicy::OverwriteIfNewer, false)
                .await
                .unwrap()
                .as_deref(),
            Some("/dst/app.conf")
        );

        // A retry of a "keep both" batch whose copy already made it: the
        // finished copy is recognised and no "app (2).conf" is created.
        write_with_mtime(
            &server.root().join("dst/app (1).conf"),
            b"fresh local",
            now + hour,
        );
        assert_eq!(
            resolve_upload_target(&sftp, &item, ConflictPolicy::Rename, true)
                .await
                .unwrap(),
            None
        );
        // Without the retry flag the next free name is taken instead.
        assert_eq!(
            resolve_upload_target(&sftp, &item, ConflictPolicy::Rename, false)
                .await
                .unwrap()
                .as_deref(),
            Some("/dst/app (2).conf")
        );
    }
}
