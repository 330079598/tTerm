use super::{UploadBatchResult, UploadFilePlanItem, ensure_remote_dir_all, is_cancelled};
use crate::sftp::internal::types::TransferCancelMap;
use russh_sftp::client::SftpSession;
use std::sync::Arc;
use tokio::sync::watch;
use tterm_core::events::EventSink;
use tterm_core::sftp::conflict::ConflictPolicy;
use tterm_core::sftp::internal::transfer::RemoteChannels;

pub(super) struct UploadBatch {
    pub cancel_map: TransferCancelMap,
    pub events: Arc<dyn EventSink>,
    pub tab_id: String,
    pub batch_id: String,
    pub directories: Vec<String>,
    pub files: Vec<UploadFilePlanItem>,
    pub batch_cancel_rx: Option<watch::Receiver<bool>>,
    pub batch_enabled: bool,
    pub policy: ConflictPolicy,
    pub skip_existing: Option<bool>,
}
pub(super) async fn run(
    batch: UploadBatch,
    sftp: &SftpSession,
    channels: &RemoteChannels,
) -> Result<UploadBatchResult, String> {
    let UploadBatch {
        cancel_map,
        events,
        tab_id,
        batch_id,
        directories,
        files,
        mut batch_cancel_rx,
        batch_enabled,
        policy,
        skip_existing,
    } = batch;

    let mut succeeded = 0usize;
    let mut failed = 0usize;
    let mut cancelled = false;

    for directory in directories {
        if batch_cancel_rx.as_mut().map(is_cancelled).unwrap_or(false) {
            cancelled = true;
            break;
        }

        match ensure_remote_dir_all(sftp, &directory, batch_cancel_rx.clone()).await {
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

            match super::items::upload_item(
                &super::items::UploadItems {
                    events: &events,
                    tab_id: &tab_id,
                    sftp,
                    channels,
                    cancel_map: &cancel_map,
                    batch_id: &batch_id,
                    batch_enabled,
                    policy,
                    skip_existing: skip_existing.unwrap_or(false),
                },
                plan_item,
                batch_cancel_rx.clone(),
            )
            .await
            {
                super::items::ItemOutcome::Succeeded => succeeded += 1,
                super::items::ItemOutcome::Failed => failed += 1,
                super::items::ItemOutcome::Cancelled => {
                    cancelled = true;
                    break;
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
