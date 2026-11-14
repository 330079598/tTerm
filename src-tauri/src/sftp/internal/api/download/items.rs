use crate::sftp::internal::api::download::next_transfer_id;
use crate::{
    sftp::internal::api::download::directory::BatchProgress,
    sftp::internal::api::download::directory::DirectoryDownloadItem,
    sftp::internal::api::download::directory::DownloadTarget,
    sftp::internal::api::download::directory::download_file_into_directory,
    sftp::internal::api::download::directory::resolve_download_target,
};
use std::path::Path;
use tokio::sync::watch;
use tterm_core::events::AppEvent;
use tterm_core::events::sftp::SftpEvent;
use tterm_core::events::sftp::download::{
    DownloadItemCompleteEvent, DownloadItemStartEvent, DownloadProgressEvent,
};
use tterm_core::sftp::conflict::ConflictPolicy;
use tterm_core::sftp::internal::transfer::RemoteChannels;

#[derive(Default)]
pub(super) struct DownloadCounts {
    pub aggregate_transferred: u64,
    pub skipped_bytes: u64,
    pub resumed_bytes: u64,
}
#[derive(Clone, Copy)]
pub(super) struct DownloadItemContext<'a> {
    pub target: DownloadTarget<'a>,
    pub channels: &'a RemoteChannels,
    pub cancel_rx: &'a watch::Receiver<bool>,
    pub transfer_id: &'a str,
    pub local_parent: &'a Path,
    pub policy: ConflictPolicy,
    pub skip_existing: Option<bool>,
    pub total_size: u64,
}
pub(super) async fn download_item(
    context: DownloadItemContext<'_>,
    item: &DirectoryDownloadItem,
    counts: &mut DownloadCounts,
) -> Result<(), String> {
    let DownloadItemContext {
        target,
        channels,
        cancel_rx,
        transfer_id,
        local_parent,
        policy,
        skip_existing,
        total_size,
    } = context;
    let DownloadTarget { events, tab_id, .. } = target;
    let local_path = local_parent.join(&item.relative_path);
    if let Some(parent) = local_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|err| {
            format!(
                "Failed to create local directory '{}': {err}",
                parent.display()
            )
        })?;
    }

    let item_transfer_id = next_transfer_id();
    let resolved =
        resolve_download_target(&local_path, item, policy, skip_existing.unwrap_or(false)).await;
    // The start event already carries the resolved name, so the
    // transfer list and a per-item retry both use the path the file
    // actually lands on.
    let local_path = match &resolved {
        Ok(Some(resolved_path)) => resolved_path.clone(),
        _ => local_path,
    };
    let local_path_string = local_path.display().to_string();
    let _ = events.emit(AppEvent::Sftp(SftpEvent::DownloadItemStart {
        tab_id: tab_id.to_string(),
        payload: DownloadItemStartEvent {
            transfer_id: item_transfer_id.clone(),
            batch_id: transfer_id.to_string(),
            file_name: item.file_name.clone(),
            file_size: item.size,
            local_path: local_path_string.clone(),
            remote_path: item.remote_path.clone(),
        },
    }));

    let resolved = match resolved {
        Ok(resolved) => resolved,
        Err(error) => {
            let _ = events.emit(AppEvent::Sftp(SftpEvent::DownloadItemComplete {
                tab_id: tab_id.to_string(),
                payload: DownloadItemCompleteEvent {
                    transfer_id: item_transfer_id,
                    error: Some(error.clone()),
                    local_path: local_path_string,
                    remote_path: item.remote_path.clone(),
                    cancelled: false,
                    success: false,
                    skipped: false,
                },
            }));
            return Err(error);
        }
    };

    // Skipped by the conflict policy, or by a retry because the local
    // file provably already holds the remote bytes.
    if resolved.is_none() {
        report_skipped(&context, item, counts, item_transfer_id, local_path_string);
        return Ok(());
    }

    let item_result = download_file_into_directory(
        target,
        channels.instance(),
        Some(cancel_rx.clone()),
        BatchProgress {
            batch_transfer_id: transfer_id,
            total_size,
            aggregate_base: counts.aggregate_transferred,
            existing_base: counts.skipped_bytes.saturating_add(counts.resumed_bytes),
        },
        &item_transfer_id,
        item,
        &local_path,
    )
    .await;

    match item_result {
        Ok(item_resumed) => {
            counts.resumed_bytes = counts.resumed_bytes.saturating_add(item_resumed);
            counts.aggregate_transferred = counts.aggregate_transferred.saturating_add(item.size);
            let _ = events.emit(AppEvent::Sftp(SftpEvent::DownloadItemComplete {
                tab_id: tab_id.to_string(),
                payload: DownloadItemCompleteEvent {
                    transfer_id: item_transfer_id,
                    error: None,
                    local_path: local_path_string,
                    remote_path: item.remote_path.clone(),
                    cancelled: false,
                    success: true,
                    skipped: false,
                },
            }));
        }
        Err(error) => {
            let cancelled = error.contains("cancelled");
            let _ = events.emit(AppEvent::Sftp(SftpEvent::DownloadItemComplete {
                tab_id: tab_id.to_string(),
                payload: DownloadItemCompleteEvent {
                    transfer_id: item_transfer_id,
                    error: if cancelled { None } else { Some(error.clone()) },
                    local_path: local_path_string,
                    remote_path: item.remote_path.clone(),
                    cancelled,
                    success: false,
                    skipped: false,
                },
            }));
            return Err(error);
        }
    }
    Ok(())
}

fn report_skipped(
    context: &DownloadItemContext<'_>,
    item: &DirectoryDownloadItem,
    counts: &mut DownloadCounts,
    item_transfer_id: String,
    local_path_string: String,
) {
    let DownloadItemContext {
        target,
        transfer_id,
        total_size,
        ..
    } = *context;
    let DownloadTarget { events, tab_id, .. } = target;
    counts.aggregate_transferred = counts.aggregate_transferred.saturating_add(item.size);
    counts.skipped_bytes = counts.skipped_bytes.saturating_add(item.size);
    let _ = events.emit(AppEvent::Sftp(SftpEvent::DownloadItemComplete {
        tab_id: tab_id.to_string(),
        payload: DownloadItemCompleteEvent {
            transfer_id: item_transfer_id,
            error: None,
            local_path: local_path_string.clone(),
            remote_path: item.remote_path.clone(),
            cancelled: false,
            success: true,
            skipped: true,
        },
    }));
    let progress = if total_size > 0 {
        ((counts.aggregate_transferred as f64 / total_size as f64) * 100.0).min(100.0) as u32
    } else {
        0
    };
    let _ = events.emit(AppEvent::Sftp(SftpEvent::DownloadProgress {
        tab_id: tab_id.to_string(),
        payload: DownloadProgressEvent {
            transfer_id: transfer_id.to_string(),
            local_path: local_path_string,
            remote_path: item.remote_path.clone(),
            transferred: counts.aggregate_transferred,
            total: total_size,
            progress,
            // Counted in `transferred` for the progress bar, but no
            // bytes moved for it in this run. Earlier resumed bytes
            // are just as absent, hence the same sum as the
            // aggregate events.
            resumed_from: counts.skipped_bytes.saturating_add(counts.resumed_bytes),
            parallelism: 0,
            completed_chunks: 0,
            chunk_count: 0,
        },
    }));
}
