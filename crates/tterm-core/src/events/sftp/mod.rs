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
    UploadBatchStart {
        tab_id: String,
        payload: UploadBatchStartEvent,
    },
    UploadBatchComplete {
        tab_id: String,
        payload: UploadBatchCompleteEvent,
    },
    UploadItemStart {
        tab_id: String,
        payload: UploadItemStartEvent,
    },
    UploadItemComplete {
        tab_id: String,
        payload: UploadItemCompleteEvent,
    },
    UploadProgress {
        tab_id: String,
        payload: UploadItemProgressEvent,
    },
    DownloadProgress {
        tab_id: String,
        payload: DownloadProgressEvent,
    },
    DownloadItemStart {
        tab_id: String,
        payload: DownloadItemStartEvent,
    },
    DownloadItemComplete {
        tab_id: String,
        payload: DownloadItemCompleteEvent,
    },
    DownloadBatchComplete {
        tab_id: String,
        payload: DownloadBatchCompleteEvent,
    },
    DeleteBatchStart {
        tab_id: String,
        payload: DeleteBatchStartEvent,
    },
    DeleteProgress {
        tab_id: String,
        payload: DeleteProgressEvent,
    },
    DeleteBatchComplete {
        tab_id: String,
        payload: DeleteBatchCompleteEvent,
    },
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
