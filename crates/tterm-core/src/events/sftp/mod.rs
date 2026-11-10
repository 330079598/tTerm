//! SFTP event payloads shared by the clients (P1-04).
pub mod delete;
pub mod download;
pub mod external;
pub mod upload;

use crate::events::{EventName, EventTarget};
use delete::DeleteBatchCompleteEvent;
use delete::DeleteBatchStartEvent;
use delete::DeleteProgressEvent;
use download::DownloadBatchCompleteEvent;
use download::DownloadItemCompleteEvent;
use download::DownloadItemStartEvent;
use download::DownloadProgressEvent;
use external::ExternalEdit;
use upload::UploadBatchCompleteEvent;
use upload::UploadBatchStartEvent;
use upload::UploadItemCompleteEvent;
use upload::UploadItemProgressEvent;
use upload::UploadItemStartEvent;

/// SFTP notifications owned by P1-04.
#[derive(Debug, Clone, PartialEq)]
pub enum SftpEvent {
    /// Announces the original inputs and conflict policy of an upload batch.
    UploadBatchStart {
        tab_id: String,
        payload: UploadBatchStartEvent,
    },
    /// Reports final upload batch counters, cancellation and errors.
    UploadBatchComplete {
        tab_id: String,
        payload: UploadBatchCompleteEvent,
    },
    /// Announces an upload item with its resolved remote destination.
    UploadItemStart {
        tab_id: String,
        payload: UploadItemStartEvent,
    },
    /// Reports an upload item's outcome, including skipped destinations.
    UploadItemComplete {
        tab_id: String,
        payload: UploadItemCompleteEvent,
    },
    /// Reports sampled byte and chunk progress for an upload item.
    UploadProgress {
        tab_id: String,
        payload: UploadItemProgressEvent,
    },
    /// Reports single-file or aggregate download progress under its transfer ID.
    DownloadProgress {
        tab_id: String,
        payload: DownloadProgressEvent,
    },
    /// Announces a download item with its resolved local destination.
    DownloadItemStart {
        tab_id: String,
        payload: DownloadItemStartEvent,
    },
    /// Reports a download item's outcome, including skipped local copies.
    DownloadItemComplete {
        tab_id: String,
        payload: DownloadItemCompleteEvent,
    },
    /// Reports final download totals, including skipped and resumed bytes.
    DownloadBatchComplete {
        tab_id: String,
        payload: DownloadBatchCompleteEvent,
    },
    /// Announces selected deletion entries and their bounded pre-scan totals.
    DeleteBatchStart {
        tab_id: String,
        payload: DeleteBatchStartEvent,
    },
    /// Reports sampled deletion counters and the current path.
    DeleteProgress {
        tab_id: String,
        payload: DeleteProgressEvent,
    },
    /// Reports final deletion counters and any error.
    DeleteBatchComplete {
        tab_id: String,
        payload: DeleteBatchCompleteEvent,
    },
    /// Publishes a complete external-edit snapshot on the shared event name.
    ExternalEditChanged(ExternalEdit),
}
impl SftpEvent {
    /// Preserves the application-default scope of all SFTP notifications.
    pub fn target(&self) -> EventTarget {
        EventTarget::Default
    }
    /// Returns the existing client event name (P1-04).
    pub fn name(&self) -> EventName {
        let (prefix, tab_id) = match self {
            Self::UploadBatchStart { tab_id, .. } => ("sftp-upload-batch-start", tab_id),
            Self::UploadBatchComplete { tab_id, .. } => ("sftp-upload-batch-complete", tab_id),
            Self::UploadItemStart { tab_id, .. } => ("sftp-upload-item-start", tab_id),
            Self::UploadItemComplete { tab_id, .. } => ("sftp-upload-item-complete", tab_id),
            Self::UploadProgress { tab_id, .. } => ("sftp-upload-progress", tab_id),
            Self::DownloadProgress { tab_id, .. } => ("sftp-download-progress", tab_id),
            Self::DownloadItemStart { tab_id, .. } => ("sftp-download-item-start", tab_id),
            Self::DownloadItemComplete { tab_id, .. } => ("sftp-download-item-complete", tab_id),
            Self::DownloadBatchComplete { tab_id, .. } => ("sftp-download-batch-complete", tab_id),
            Self::DeleteBatchStart { tab_id, .. } => ("sftp-delete-batch-start", tab_id),
            Self::DeleteProgress { tab_id, .. } => ("sftp-delete-progress", tab_id),
            Self::DeleteBatchComplete { tab_id, .. } => ("sftp-delete-batch-complete", tab_id),
            Self::ExternalEditChanged(_) => return EventName::Static("sftp-external-edit"),
        };
        EventName::TabScoped {
            prefix,
            tab_id: tab_id.clone(),
        }
    }
}
