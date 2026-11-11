//! Serialization of SFTP notifications (P1-04).
use serde_json::Value;
use tterm_core::events::sftp::SftpEvent;

pub(super) fn wire(event: &SftpEvent) -> Result<Value, String> {
    match event {
        SftpEvent::UploadBatchStart { payload, .. } => serde_json::to_value(payload),
        SftpEvent::UploadBatchComplete { payload, .. } => serde_json::to_value(payload),
        SftpEvent::UploadItemStart { payload, .. } => serde_json::to_value(payload),
        SftpEvent::UploadItemComplete { payload, .. } => serde_json::to_value(payload),
        SftpEvent::UploadProgress { payload, .. } => serde_json::to_value(payload),
        SftpEvent::DownloadProgress { payload, .. } => serde_json::to_value(payload),
        SftpEvent::DownloadItemStart { payload, .. } => serde_json::to_value(payload),
        SftpEvent::DownloadItemComplete { payload, .. } => serde_json::to_value(payload),
        SftpEvent::DownloadBatchComplete { payload, .. } => serde_json::to_value(payload),
        SftpEvent::DeleteBatchStart { payload, .. } => serde_json::to_value(payload),
        SftpEvent::DeleteProgress { payload, .. } => serde_json::to_value(payload),
        SftpEvent::DeleteBatchComplete { payload, .. } => serde_json::to_value(payload),
        SftpEvent::ExternalEditChanged(payload) => serde_json::to_value(payload),
    }
    .map_err(|error| error.to_string())
}
