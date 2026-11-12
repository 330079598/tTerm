use super::targets::resolve_upload_target;
use super::{UploadFilePlanItem, next_transfer_id, upload_single_file_with_progress};
use crate::sftp::internal::types::TransferCancelMap;
use russh_sftp::client::SftpSession;
use std::sync::Arc;
use tokio::sync::watch;
use tterm_core::events::sftp::SftpEvent;
use tterm_core::events::sftp::upload::{UploadItemCompleteEvent, UploadItemStartEvent};
use tterm_core::events::{AppEvent, EventSink};
use tterm_core::sftp::conflict::ConflictPolicy;
use tterm_core::sftp::internal::transfer::RemoteChannels;

pub(super) struct UploadItems<'a> {
    pub events: &'a Arc<dyn EventSink>,
    pub tab_id: &'a str,
    pub sftp: &'a SftpSession,
    pub channels: &'a RemoteChannels,
    pub cancel_map: &'a TransferCancelMap,
    pub batch_id: &'a str,
    pub batch_enabled: bool,
    pub policy: ConflictPolicy,
    pub skip_existing: bool,
}

pub(super) enum ItemOutcome {
    Succeeded,
    Failed,
    Cancelled,
}

pub(super) async fn upload_item(
    context: &UploadItems<'_>,
    plan_item: UploadFilePlanItem,
    batch_cancel_rx: Option<watch::Receiver<bool>>,
) -> ItemOutcome {
    let UploadItems {
        events,
        tab_id,
        sftp,
        channels,
        cancel_map,
        batch_id,
        batch_enabled,
        policy,
        skip_existing,
    } = *context;
    let transfer_id = next_transfer_id();
    let resolved = resolve_upload_target(sftp, &plan_item, policy, skip_existing).await;
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
    let _ = events.emit(AppEvent::Sftp(SftpEvent::UploadItemStart {
        tab_id: tab_id.to_string(),
        payload: UploadItemStartEvent {
            transfer_id: transfer_id.clone(),
            batch_id: if batch_enabled {
                Some(batch_id.to_string())
            } else {
                None
            },
            file_name: plan_item.file_name.clone(),
            file_size: plan_item.file_size,
            local_path: plan_item.local_path.clone(),
            remote_path: plan_item.remote_path.clone(),
        },
    }));

    let resolved = match resolved {
        Ok(resolved) => resolved,
        Err(error) => {
            let _ = events.emit(AppEvent::Sftp(SftpEvent::UploadItemComplete {
                tab_id: tab_id.to_string(),
                payload: UploadItemCompleteEvent {
                    transfer_id,
                    error: Some(error),
                    local_path: plan_item.local_path,
                    remote_path: plan_item.remote_path,
                    cancelled: false,
                    success: false,
                    skipped: false,
                },
            }));
            return ItemOutcome::Failed;
        }
    };

    if resolved.is_none() {
        let _ = events.emit(AppEvent::Sftp(SftpEvent::UploadItemComplete {
            tab_id: tab_id.to_string(),
            payload: UploadItemCompleteEvent {
                transfer_id,
                error: None,
                local_path: plan_item.local_path.clone(),
                remote_path: plan_item.remote_path.clone(),
                cancelled: false,
                success: true,
                skipped: true,
            },
        }));
        return ItemOutcome::Succeeded;
    }

    let result = upload_single_file_with_progress(
        events,
        tab_id,
        channels.instance(),
        cancel_map,
        batch_cancel_rx.clone(),
        &plan_item,
        &transfer_id,
    )
    .await;

    match result {
        Ok(()) => {
            let _ = events.emit(AppEvent::Sftp(SftpEvent::UploadItemComplete {
                tab_id: tab_id.to_string(),
                payload: UploadItemCompleteEvent {
                    transfer_id,
                    error: None,
                    local_path: plan_item.local_path,
                    remote_path: plan_item.remote_path,
                    cancelled: false,
                    success: true,
                    skipped: false,
                },
            }));
        }
        Err(error) => {
            let item_cancelled = error.contains("cancelled");

            let _ = events.emit(AppEvent::Sftp(SftpEvent::UploadItemComplete {
                tab_id: tab_id.to_string(),
                payload: UploadItemCompleteEvent {
                    transfer_id,
                    error: Some(error),
                    local_path: plan_item.local_path,
                    remote_path: plan_item.remote_path,
                    cancelled: item_cancelled,
                    success: false,
                    skipped: false,
                },
            }));

            return if item_cancelled {
                ItemOutcome::Cancelled
            } else {
                ItemOutcome::Failed
            };
        }
    }
    ItemOutcome::Succeeded
}
