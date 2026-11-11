//! Making and unpacking archives on the host. SFTP can only move bytes, so
//! the work runs as `tar`, `zip` and friends over a command channel of the
//! tab's pooled connection, and nothing crosses the network but the result.

mod operations;
use operations::*;

#[cfg(all(test, unix))]
mod real_sshd_tests;
mod script;

use crate::core::session::types::PtyConnectionOptions;
use crate::services::AppServices;
use crate::sftp::internal::api::SftpHost;
use crate::sftp::internal::exec::{run_to_end, ScriptOutcome};
use crate::sftp::internal::paths::join_remote_path;
use script::{compress_script, extract_script, ArchiveScript, MISSING_TOOL_STATUS};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::watch;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveFormat {
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    /// Extract only: not every `tar` can write zstd.
    TarZst,
    /// Extract only: `7z` is rarely installed on servers.
    SevenZip,
    /// A single compressed file rather than an archive; extract only.
    Gzip,
    Bzip2,
    Xz,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressRequest {
    /// The folder holding the entries; the archive is made in it too.
    directory: String,
    names: Vec<String>,
    archive_name: String,
    format: ArchiveFormat,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractRequest {
    archive_path: String,
    destination: String,
    format: ArchiveFormat,
    overwrite: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteOperationResult {
    cancelled: bool,
}

fn check_outcome(
    outcome: &ScriptOutcome,
    script: &ArchiveScript,
    action: &str,
) -> Result<(), String> {
    if outcome.succeeded() {
        Ok(())
    } else if outcome.exit_status == Some(MISSING_TOOL_STATUS) {
        Err(format!(
            "{action} needs `{}`, which is not installed on the server",
            script.tool
        ))
    } else {
        Err(outcome.failure_message(action))
    }
}

async fn compress(
    host: SftpHost<'_>,
    request: CompressRequest,
    mut cancel: watch::Receiver<bool>,
) -> Result<RemoteOperationResult, String> {
    let script = compress_script(
        &request.directory,
        &request.names,
        &request.archive_name,
        request.format,
    )?;
    let handles = host.handles().await?;
    let sftp = &handles.sftp;

    // tar and zip would add to or replace an existing file of that name.
    let archive_path = join_remote_path(&request.directory, &request.archive_name);
    if sftp.symlink_metadata(archive_path.clone()).await.is_ok() {
        return Err(format!(
            "An item named '{}' already exists in this folder",
            request.archive_name
        ));
    }

    match run_to_end(&handles.launcher, &handles.ssh, &script.text, &mut cancel).await? {
        Some(outcome) => {
            check_outcome(&outcome, &script, "Compressing")?;
            Ok(RemoteOperationResult { cancelled: false })
        }
        None => {
            // A half-written archive is of no use. The host may still hold
            // it open when it ignored the signal, so this may fail.
            let _ = sftp.remove_file(archive_path).await;
            Ok(RemoteOperationResult { cancelled: true })
        }
    }
}

async fn extract(
    host: SftpHost<'_>,
    request: ExtractRequest,
    mut cancel: watch::Receiver<bool>,
) -> Result<RemoteOperationResult, String> {
    let script = extract_script(
        &request.archive_path,
        request.destination.trim(),
        request.format,
        request.overwrite,
    )?;
    let handles = host.handles().await?;
    match run_to_end(&handles.launcher, &handles.ssh, &script.text, &mut cancel).await? {
        Some(outcome) => {
            check_outcome(&outcome, &script, "Extracting")?;
            Ok(RemoteOperationResult { cancelled: false })
        }
        None => Ok(RemoteOperationResult { cancelled: true }),
    }
}

/// Packs entries of one folder into a new archive beside them.
#[tauri::command]
pub async fn sftp_compress_entries(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    operation_id: String,
    request: CompressRequest,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<RemoteOperationResult, String> {
    sftp_compress_entries_service(
        SftpCompressEntriesDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
        },
        SftpCompressEntriesInput {
            tab_id,
            connection,
            operation_id,
            request,
        },
    )
    .await
}

/// Unpacks an archive into a folder, which is created when missing.
#[tauri::command]
pub async fn sftp_extract_archive(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    operation_id: String,
    request: ExtractRequest,
    services: State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<RemoteOperationResult, String> {
    sftp_extract_archive_service(
        SftpExtractArchiveDependencies {
            context: crate::connection_context::sftp(&services),
            prompts: services.shared.prompts.clone(),
            secrets: services.shared.secrets.clone(),
            pool: services.sftp.pool.clone(),
            cancel: services.sftp.transfer_cancel.clone(),
        },
        SftpExtractArchiveInput {
            tab_id,
            connection,
            operation_id,
            request,
        },
    )
    .await
}

/// Stops a running compress, extract or search.
#[tauri::command]
pub async fn sftp_cancel_remote_operation(
    operation_id: String,
    services: State<'_, std::sync::Arc<AppServices>>,
) -> Result<(), String> {
    sftp_cancel_remote_operation_service(
        SftpCancelRemoteOperationDependencies {
            cancel: services.sftp.transfer_cancel.clone(),
        },
        SftpCancelRemoteOperationInput { operation_id },
    )
    .await
}
