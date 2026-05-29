use crate::sftp::internal::connection::map_sftp_error;
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use tauri::{AppHandle, Emitter};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteEntryRequest {
    path: String,
    name: String,
    is_dir: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOptions {
    command: Option<String>,
    use_command_delete: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletePreviewResult {
    command: String,
    should_prompt_for_command: bool,
    total_directories: usize,
    total_entries: usize,
    total_files: usize,
    total_truncated: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteBatchStartResult {
    batch_id: String,
}

#[derive(Default)]
struct DeleteStats {
    directories: usize,
    entries: usize,
    files: usize,
    truncated: bool,
}

#[derive(Default)]
struct DeleteProgress {
    deleted_directories: usize,
    deleted_files: usize,
    failed: usize,
}

struct DeletePlan {
    method: DeleteMethod,
    stats: DeleteStats,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum DeleteMethod {
    Sftp,
    Command,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteBatchStartEvent {
    batch_id: String,
    entries: Vec<String>,
    method: DeleteMethod,
    total_directories: usize,
    total_entries: usize,
    total_files: usize,
    total_truncated: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteProgressEvent {
    batch_id: String,
    current_path: String,
    deleted_directories: usize,
    deleted_files: usize,
    failed: usize,
    method: DeleteMethod,
    total_directories: usize,
    total_entries: usize,
    total_files: usize,
    total_truncated: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteBatchCompleteEvent {
    batch_id: String,
    cancelled: bool,
    deleted_directories: usize,
    deleted_files: usize,
    error: Option<String>,
    failed: usize,
    method: DeleteMethod,
    total_directories: usize,
    total_entries: usize,
    total_files: usize,
    total_truncated: bool,
}

const DELETE_COMMAND_PROMPT_THRESHOLD: usize = 300;
const DELETE_STATS_LIMIT: usize = 301;

fn next_delete_batch_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn should_emit_delete_progress(progress: &DeleteProgress) -> bool {
    let total = progress.deleted_directories + progress.deleted_files + progress.failed;
    total == 0 || total.is_multiple_of(25)
}

fn add_delete_entry_stats(stats: &mut DeleteStats, is_dir: bool) {
    if is_dir {
        stats.directories += 1;
    } else {
        stats.files += 1;
    }
    stats.entries += 1;
}

fn is_dangerous_delete_path(path: &str) -> bool {
    let normalized = crate::sftp::internal::paths::normalize_remote_path(path);
    normalized.is_empty() || normalized == "/" || normalized == "." || normalized == ".."
}

fn shell_quote_single(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn command_delete_script(paths: &[String]) -> String {
    let quoted_paths = paths
        .iter()
        .map(|path| shell_quote_single(path))
        .collect::<Vec<_>>()
        .join(" ");

    format!("rm -rf -- {quoted_paths}")
}

fn build_delete_complete_event(
    batch_id: String,
    method: DeleteMethod,
    stats: &DeleteStats,
    progress: &DeleteProgress,
    cancelled: bool,
    error: Option<String>,
) -> DeleteBatchCompleteEvent {
    DeleteBatchCompleteEvent {
        batch_id,
        cancelled,
        deleted_directories: progress.deleted_directories,
        deleted_files: progress.deleted_files,
        error,
        failed: progress.failed,
        method,
        total_directories: stats.directories,
        total_entries: stats.entries,
        total_files: stats.files,
        total_truncated: stats.truncated,
    }
}

fn emit_delete_progress(
    app: &AppHandle,
    tab_id: &str,
    batch_id: &str,
    method: DeleteMethod,
    stats: &DeleteStats,
    progress: &DeleteProgress,
    current_path: String,
) {
    let _ = app.emit(
        &format!("sftp-delete-progress-{tab_id}"),
        DeleteProgressEvent {
            batch_id: batch_id.to_string(),
            current_path,
            deleted_directories: progress.deleted_directories,
            deleted_files: progress.deleted_files,
            failed: progress.failed,
            method,
            total_directories: stats.directories,
            total_entries: stats.entries,
            total_files: stats.files,
            total_truncated: stats.truncated,
        },
    );
}

fn collect_delete_stats<'a>(
    sftp: &'a SftpSession,
    path: String,
    is_dir: bool,
    stats: &'a mut DeleteStats,
    limit: usize,
) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
    Box::pin(async move {
        if stats.truncated {
            return Ok(());
        }

        add_delete_entry_stats(stats, is_dir);
        if stats.entries >= limit {
            stats.truncated = true;
            return Ok(());
        }

        if !is_dir {
            return Ok(());
        }

        let entries = sftp.read_dir(&path).await.map_err(map_sftp_error)?;
        for entry in entries {
            let name = entry.file_name();
            let entry_path = crate::sftp::internal::paths::join_remote_path(&path, &name);
            collect_delete_stats(sftp, entry_path, entry.metadata().is_dir(), stats, limit).await?;
            if stats.truncated {
                break;
            }
        }

        Ok(())
    })
}

mod batch;
pub mod commands;
