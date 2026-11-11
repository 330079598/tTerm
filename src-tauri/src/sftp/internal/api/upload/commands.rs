#[path = "operations.rs"]
mod operations;
use super::*;
use crate::core::session::types::PtyConnectionOptions;
use crate::services::AppServices;
use crate::sftp::internal::api::conflict::{
    self, CandidateDecision, ConflictReport, SourceStat, TransferConflict,
};
use operations::*;
use std::collections::HashMap;
use tauri::State;
use tterm_core::sftp::conflict::ConflictPolicy;

/// Where a planned upload should land under `policy`, or `None` to skip it.
///
/// `skip_existing` is set by a batch retry, so files an earlier attempt
/// already finished are passed over instead of re-sent (see
/// [`conflict::decide_candidate`]). A stat error is treated as "nothing
/// there" and the transfer itself surfaces the real problem.
pub(super) async fn resolve_upload_target(
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
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    local_paths: Vec<String>,
    remote_base_path: String,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<ConflictReport, String> {
    sftp_check_upload_conflicts_service(
        SftpCheckUploadConflictsDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
        },
        SftpCheckUploadConflictsInput {
            tab_id,
            connection,
            local_paths,
            remote_base_path,
        },
    )
    .await
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
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    local_path: String,
    remote_path: String,
    transfer_id: String,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    sftp_upload_file_service(
        SftpUploadFileDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
            events: services.shared.events.clone(),
        },
        SftpUploadFileInput {
            tab_id,
            connection,
            local_path,
            remote_path,
            transfer_id,
        },
    )
    .await
}

#[tauri::command]
pub async fn sftp_upload_paths(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    local_paths: Vec<String>,
    remote_base_path: String,
    skip_existing: Option<bool>,
    conflict_policy: Option<ConflictPolicy>,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<UploadBatchResult, String> {
    sftp_upload_paths_service(
        SftpUploadPathsDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
            events: services.shared.events.clone(),
        },
        SftpUploadPathsInput {
            tab_id,
            connection,
            local_paths,
            remote_base_path,
            skip_existing,
            conflict_policy,
        },
    )
    .await
}

#[tauri::command]
pub async fn sftp_cancel_upload(
    transfer_id: String,
    services: State<'_, std::sync::Arc<AppServices>>,
) -> Result<(), String> {
    sftp_cancel_upload_service(
        SftpCancelUploadDependencies {
            cancel: services.sftp.transfer_cancel.clone(),
        },
        SftpCancelUploadInput { transfer_id },
    )
    .await
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
