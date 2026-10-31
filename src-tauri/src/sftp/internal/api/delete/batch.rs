use super::*;
use crate::core::state::HostPromptMap;
use crate::sftp::internal::connection::{
    connect_authenticated_ssh, get_or_create_sftp_connection, map_sftp_error, pooled_handles,
};
use crate::sftp::internal::types::{SftpConnectionKey, SftpConnectionPool};
use russh::ChannelMsg;
use russh_sftp::client::SftpSession;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
pub(super) async fn preview_delete_entries(
    app: &AppHandle,
    tab_id: &str,
    plan: &crate::core::session::types::SessionPlan,
    prompts: HostPromptMap,
    pool: &SftpConnectionPool,
    entries: &[DeleteEntryRequest],
) -> Result<DeletePreviewResult, String> {
    let sftp = get_or_create_delete_sftp(app, tab_id, plan, prompts, pool).await?;
    let mut stats = DeleteStats::default();
    for entry in entries {
        if is_dangerous_delete_path(&entry.path) {
            return Err(format!("Refusing to delete unsafe path '{}'", entry.path));
        }
        collect_delete_stats(
            sftp.as_ref(),
            entry.path.clone(),
            entry.is_dir,
            &mut stats,
            DELETE_STATS_LIMIT,
        )
        .await?;
        if stats.truncated {
            break;
        }
    }

    let paths = entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    Ok(DeletePreviewResult {
        command: command_delete_script(&paths),
        should_prompt_for_command: stats.truncated
            || stats.entries > DELETE_COMMAND_PROMPT_THRESHOLD,
        total_directories: stats.directories,
        total_entries: stats.entries,
        total_files: stats.files,
        total_truncated: stats.truncated,
    })
}

async fn build_delete_plan(
    sftp: &SftpSession,
    entries: &[DeleteEntryRequest],
    use_command_delete: bool,
) -> Result<DeletePlan, String> {
    let mut stats = DeleteStats::default();
    for entry in entries {
        if is_dangerous_delete_path(&entry.path) {
            return Err(format!("Refusing to delete unsafe path '{}'", entry.path));
        }
        collect_delete_stats(
            sftp,
            entry.path.clone(),
            entry.is_dir,
            &mut stats,
            DELETE_STATS_LIMIT,
        )
        .await?;
        if stats.truncated {
            break;
        }
    }

    let method = if use_command_delete {
        DeleteMethod::Command
    } else {
        DeleteMethod::Sftp
    };

    Ok(DeletePlan { method, stats })
}

async fn run_remote_delete_command(
    batch: &DeleteBatch,
    stats: &DeleteStats,
) -> Result<DeleteProgress, String> {
    let DeleteBatch {
        app,
        tab_id,
        plan,
        prompts,
        batch_id,
        entries,
        options,
        ..
    } = batch;
    let paths = entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    let command = options
        .command
        .clone()
        .unwrap_or_else(|| command_delete_script(&paths));
    let (exit_status, stderr) = if plan.sftp_sudo {
        run_delete_command_as_root(batch, &command).await?
    } else {
        let (_jump_session, ssh) =
            connect_authenticated_ssh(app, tab_id, plan, prompts.clone()).await?;
        let mut channel = ssh
            .channel_open_session()
            .await
            .map_err(|err| format!("Failed to open SSH channel: {err}"))?;

        channel
            .exec(true, command)
            .await
            .map_err(|err| format!("Failed to execute delete command: {err}"))?;

        let mut stderr = String::new();
        let mut exit_status = None;
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { .. } => {}
                ChannelMsg::ExtendedData { data, .. } => {
                    stderr.push_str(&String::from_utf8_lossy(&data));
                }
                ChannelMsg::ExitStatus {
                    exit_status: status,
                } => {
                    exit_status = Some(status);
                }
                ChannelMsg::Close => break,
                _ => {}
            }
        }

        let _ = channel.close().await;
        let _ = ssh
            .disconnect(
                russh::Disconnect::ByApplication,
                "delete command completed",
                "en",
            )
            .await;
        (exit_status, stderr)
    };

    if exit_status.unwrap_or(0) == 0 {
        let progress = DeleteProgress {
            deleted_directories: stats.directories,
            deleted_files: stats.files,
            failed: 0,
        };
        emit_delete_progress(
            app,
            tab_id,
            batch_id,
            DeleteMethod::Command,
            stats,
            &progress,
            paths.join(", "),
        );
        Ok(progress)
    } else {
        let detail = stderr.trim();
        Err(if detail.is_empty() {
            format!(
                "Delete command failed with exit status {}",
                exit_status.unwrap_or(1)
            )
        } else {
            format!(
                "Delete command failed with exit status {}: {}",
                exit_status.unwrap_or(1),
                detail
            )
        })
    }
}

async fn delete_entries_with_sftp(
    app: &AppHandle,
    tab_id: &str,
    sftp: &SftpSession,
    batch_id: &str,
    entries: &[DeleteEntryRequest],
    stats: &DeleteStats,
) -> DeleteProgress {
    let mut progress = DeleteProgress::default();
    for entry in entries {
        let result = if entry.is_dir {
            delete_directory_recursive_with_progress(
                app,
                tab_id,
                sftp,
                batch_id,
                entry.path.clone(),
                stats,
                &mut progress,
            )
            .await
        } else {
            match sftp
                .remove_file(entry.path.clone())
                .await
                .map_err(map_sftp_error)
            {
                Ok(()) => {
                    progress.deleted_files += 1;
                    if should_emit_delete_progress(&progress) {
                        emit_delete_progress(
                            app,
                            tab_id,
                            batch_id,
                            DeleteMethod::Sftp,
                            stats,
                            &progress,
                            entry.path.clone(),
                        );
                    }
                    Ok(())
                }
                Err(error) => Err(error),
            }
        };

        if result.is_err() {
            progress.failed += 1;
            emit_delete_progress(
                app,
                tab_id,
                batch_id,
                DeleteMethod::Sftp,
                stats,
                &progress,
                entry.path.clone(),
            );
        }
    }

    emit_delete_progress(
        app,
        tab_id,
        batch_id,
        DeleteMethod::Sftp,
        stats,
        &progress,
        String::new(),
    );
    progress
}

fn delete_directory_recursive_with_progress<'a>(
    app: &'a AppHandle,
    tab_id: &'a str,
    sftp: &'a SftpSession,
    batch_id: &'a str,
    path: String,
    stats: &'a DeleteStats,
    progress: &'a mut DeleteProgress,
) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
    Box::pin(async move {
        let entries = sftp.read_dir(&path).await.map_err(map_sftp_error)?;
        for entry in entries {
            let name = entry.file_name();
            let entry_path = crate::sftp::internal::paths::join_remote_path(&path, &name);

            if entry.metadata().is_dir() {
                delete_directory_recursive_with_progress(
                    app, tab_id, sftp, batch_id, entry_path, stats, progress,
                )
                .await?;
            } else {
                sftp.remove_file(entry_path.clone())
                    .await
                    .map_err(map_sftp_error)?;
                progress.deleted_files += 1;
                if should_emit_delete_progress(progress) {
                    emit_delete_progress(
                        app,
                        tab_id,
                        batch_id,
                        DeleteMethod::Sftp,
                        stats,
                        progress,
                        entry_path,
                    );
                }
            }
        }

        sftp.remove_dir(path.clone())
            .await
            .map_err(map_sftp_error)?;
        progress.deleted_directories += 1;
        if should_emit_delete_progress(progress) {
            emit_delete_progress(
                app,
                tab_id,
                batch_id,
                DeleteMethod::Sftp,
                stats,
                progress,
                path,
            );
        }
        Ok(())
    })
}

/// One delete the user started: what to delete, how, and on which connection.
pub(super) struct DeleteBatch {
    pub(super) app: AppHandle,
    pub(super) tab_id: String,
    pub(super) plan: crate::core::session::types::SessionPlan,
    pub(super) prompts: HostPromptMap,
    pub(super) pool: SftpConnectionPool,
    pub(super) batch_id: String,
    pub(super) entries: Vec<DeleteEntryRequest>,
    pub(super) options: DeleteOptions,
}

pub(super) async fn run_delete_batch(batch: DeleteBatch) {
    let mut method = DeleteMethod::Sftp;
    let mut stats = DeleteStats::default();
    let mut progress = DeleteProgress::default();
    let result: Result<(), String> = async {
        let sftp = get_or_create_delete_sftp(
            &batch.app,
            &batch.tab_id,
            &batch.plan,
            batch.prompts.clone(),
            &batch.pool,
        )
        .await?;
        let delete_plan =
            build_delete_plan(&sftp, &batch.entries, batch.options.use_command_delete).await?;
        method = delete_plan.method;
        stats = delete_plan.stats;

        let _ = batch.app.emit(
            &format!("sftp-delete-batch-start-{}", batch.tab_id),
            DeleteBatchStartEvent {
                batch_id: batch.batch_id.clone(),
                entries: batch
                    .entries
                    .iter()
                    .map(|entry| entry.name.clone())
                    .collect(),
                method,
                total_directories: stats.directories,
                total_entries: stats.entries,
                total_files: stats.files,
                total_truncated: stats.truncated,
            },
        );

        progress = match method {
            DeleteMethod::Command => run_remote_delete_command(&batch, &stats).await?,
            DeleteMethod::Sftp => {
                delete_entries_with_sftp(
                    &batch.app,
                    &batch.tab_id,
                    &sftp,
                    &batch.batch_id,
                    &batch.entries,
                    &stats,
                )
                .await
            }
        };

        Ok(())
    }
    .await;

    let error = result.err();
    if error.is_some() {
        progress.failed += 1;
    }

    let _ = batch.app.emit(
        &format!("sftp-delete-batch-complete-{}", batch.tab_id),
        build_delete_complete_event(batch.batch_id, method, &stats, &progress, false, error),
    );
}

/// Runs the delete command as root on the tab's sudo-mode connection. The
/// command goes to `sh` on standard input rather than through the login
/// shell, so it reads the same whichever shell the account logs in with.
async fn run_delete_command_as_root(
    batch: &DeleteBatch,
    command: &str,
) -> Result<(Option<u32>, String), String> {
    let handles = pooled_handles(
        &batch.app,
        &batch.tab_id,
        &batch.plan,
        batch.prompts.clone(),
        &batch.pool,
    )
    .await?;
    let outcome = handles
        .launcher
        .start_script(&handles.ssh, command)
        .await?
        .finish()
        .await;
    Ok((outcome.exit_status, outcome.stderr))
}

async fn get_or_create_delete_sftp(
    app: &AppHandle,
    tab_id: &str,
    plan: &crate::core::session::types::SessionPlan,
    prompts: HostPromptMap,
    pool: &SftpConnectionPool,
) -> Result<Arc<SftpSession>, String> {
    let key = SftpConnectionKey::from_plan(tab_id, plan)?;

    get_or_create_sftp_connection(app, tab_id, plan, prompts, pool).await?;
    let pool_guard = pool.read().await;
    let cached = pool_guard.get(&key).ok_or("Connection not found")?;
    Ok(Arc::clone(&cached.connection.sftp))
}
