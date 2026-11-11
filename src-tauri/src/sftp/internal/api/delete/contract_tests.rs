use crate::event_sink::TauriEventSink;
use tterm_core::events::sftp::SftpEvent;
use tterm_core::events::{AppEvent, EventTarget};
// Legacy wire contracts captured before P1-04 transport changes.
use tterm_core::events::sftp::delete::{
    DeleteBatchCompleteEvent, DeleteBatchStartEvent, DeleteMethod, DeleteProgressEvent,
};

#[test]
fn legacy_r01_wire_contract() {
    let payload = DeleteProgressEvent {
        batch_id: "batch-1".into(),
        current_path: "current_path-value".into(),
        deleted_directories: 2,
        deleted_files: 2,
        failed: 2,
        method: DeleteMethod::Sftp,
        total_directories: 2,
        total_entries: 2,
        total_files: 2,
        total_truncated: false,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"batchId": "batch-1", "currentPath": "current_path-value", "deletedDirectories": 2, "deletedFiles": 2, "failed": 2, "method": "sftp", "totalDirectories": 2, "totalEntries": 2, "totalFiles": 2, "totalTruncated": false})
    );
    assert_eq!(
        format!("sftp-delete-progress-{}", "tab-中文-7"),
        "sftp-delete-progress-tab-中文-7"
    );
    let event = AppEvent::Sftp(SftpEvent::DeleteProgress {
        tab_id: "tab-中文-7".into(),
        payload,
    });
    let (name, target, json) = TauriEventSink::wire(&event).unwrap();
    assert_eq!(name.as_string(), "sftp-delete-progress-tab-中文-7");
    assert_eq!(target, EventTarget::Default);
    assert_eq!(target, event.target());
    assert_eq!(
        json,
        serde_json::json!({"batchId": "batch-1", "currentPath": "current_path-value", "deletedDirectories": 2, "deletedFiles": 2, "failed": 2, "method": "sftp", "totalDirectories": 2, "totalEntries": 2, "totalFiles": 2, "totalTruncated": false})
    );
}

#[test]
fn legacy_r02_wire_contract() {
    let payload = DeleteBatchStartEvent {
        batch_id: "batch-1".into(),
        entries: vec!["first".into(), "中文 second".into()],
        method: DeleteMethod::Sftp,
        total_directories: 2,
        total_entries: 2,
        total_files: 2,
        total_truncated: false,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"batchId": "batch-1", "entries": ["first", "中文 second"], "method": "sftp", "totalDirectories": 2, "totalEntries": 2, "totalFiles": 2, "totalTruncated": false})
    );
    assert_eq!(
        format!("sftp-delete-batch-start-{}", "tab-中文-7"),
        "sftp-delete-batch-start-tab-中文-7"
    );
    let event = AppEvent::Sftp(SftpEvent::DeleteBatchStart {
        tab_id: "tab-中文-7".into(),
        payload,
    });
    let (name, target, json) = TauriEventSink::wire(&event).unwrap();
    assert_eq!(name.as_string(), "sftp-delete-batch-start-tab-中文-7");
    assert_eq!(target, EventTarget::Default);
    assert_eq!(target, event.target());
    assert_eq!(
        json,
        serde_json::json!({"batchId": "batch-1", "entries": ["first", "中文 second"], "method": "sftp", "totalDirectories": 2, "totalEntries": 2, "totalFiles": 2, "totalTruncated": false})
    );
}

#[test]
fn legacy_r03_wire_contract() {
    let payload = DeleteBatchCompleteEvent {
        batch_id: "batch-1".into(),
        cancelled: false,
        deleted_directories: 2,
        deleted_files: 2,
        error: None,
        failed: 2,
        method: DeleteMethod::Sftp,
        total_directories: 2,
        total_entries: 2,
        total_files: 2,
        total_truncated: false,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"batchId": "batch-1", "cancelled": false, "deletedDirectories": 2, "deletedFiles": 2, "error": null, "failed": 2, "method": "sftp", "totalDirectories": 2, "totalEntries": 2, "totalFiles": 2, "totalTruncated": false})
    );
    assert_eq!(
        format!("sftp-delete-batch-complete-{}", "tab-中文-7"),
        "sftp-delete-batch-complete-tab-中文-7"
    );
    let event = AppEvent::Sftp(SftpEvent::DeleteBatchComplete {
        tab_id: "tab-中文-7".into(),
        payload,
    });
    let (name, target, json) = TauriEventSink::wire(&event).unwrap();
    assert_eq!(name.as_string(), "sftp-delete-batch-complete-tab-中文-7");
    assert_eq!(target, EventTarget::Default);
    assert_eq!(target, event.target());
    assert_eq!(
        json,
        serde_json::json!({"batchId": "batch-1", "cancelled": false, "deletedDirectories": 2, "deletedFiles": 2, "error": null, "failed": 2, "method": "sftp", "totalDirectories": 2, "totalEntries": 2, "totalFiles": 2, "totalTruncated": false})
    );
}

#[test]
fn delete_sampling_stays_at_twenty_five_items() {
    for (count, expected) in [(0, true), (24, false), (25, true), (26, false), (50, true)] {
        let progress = super::DeleteProgress {
            deleted_files: count,
            ..Default::default()
        };
        assert_eq!(super::should_emit_delete_progress(&progress), expected);
    }
}
