//! Legacy wire contracts captured before P1-04 transport changes.
use super::*;

#[test]
fn legacy_u01_wire_contract() {
    let payload = UploadBatchStartEvent {
        batch_id: "batch-1".into(),
        display_name: "display_name-value".into(),
        local_path: "/tmp/中文 file.txt".into(),
        local_paths: vec!["first".into(), "中文 second".into()],
        remote_base_path: "remote_base_path-value".into(),
        conflict_policy: super::super::conflict::ConflictPolicy::Rename,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"batchId": "batch-1", "displayName": "display_name-value", "localPath": "/tmp/中文 file.txt", "localPaths": ["first", "中文 second"], "remoteBasePath": "remote_base_path-value", "conflictPolicy": "rename"})
    );
    assert_eq!(
        format!("sftp-upload-batch-start-{}", "tab-中文-7"),
        "sftp-upload-batch-start-tab-中文-7"
    );
}

#[test]
fn legacy_u02_wire_contract() {
    let payload = UploadBatchCompleteEvent {
        batch_id: "batch-1".into(),
        cancelled: true,
        error: None,
        failed: 0,
        succeeded: 0,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"batchId": "batch-1", "cancelled": true, "error": null, "failed": 0, "succeeded": 0})
    );
    assert_eq!(
        format!("sftp-upload-batch-complete-{}", "tab-中文-7"),
        "sftp-upload-batch-complete-tab-中文-7"
    );
}

#[test]
fn legacy_u03_wire_contract() {
    let payload = UploadItemStartEvent {
        transfer_id: "transfer-1".into(),
        batch_id: Some("batch-1".into()),
        file_name: "file_name-value".into(),
        file_size: 2,
        local_path: "/tmp/中文 file.txt".into(),
        remote_path: "/home/alice/中文 file.txt".into(),
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"transferId": "transfer-1", "batchId": "batch-1", "fileName": "file_name-value", "fileSize": 2, "localPath": "/tmp/中文 file.txt", "remotePath": "/home/alice/中文 file.txt"})
    );
    assert_eq!(
        format!("sftp-upload-item-start-{}", "tab-中文-7"),
        "sftp-upload-item-start-tab-中文-7"
    );
}

#[test]
fn legacy_u04_wire_contract() {
    let payload = UploadItemCompleteEvent {
        transfer_id: "transfer-1".into(),
        error: Some("resolve failed".into()),
        local_path: "/tmp/中文 file.txt".into(),
        remote_path: "/home/alice/中文 file.txt".into(),
        cancelled: false,
        success: false,
        skipped: false,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"transferId": "transfer-1", "error": "resolve failed", "localPath": "/tmp/中文 file.txt", "remotePath": "/home/alice/中文 file.txt", "cancelled": false, "success": false, "skipped": false})
    );
    assert_eq!(
        format!("sftp-upload-item-complete-{}", "tab-中文-7"),
        "sftp-upload-item-complete-tab-中文-7"
    );
}

#[test]
fn legacy_u05_wire_contract() {
    let payload = UploadItemCompleteEvent {
        transfer_id: "transfer-1".into(),
        error: None,
        local_path: "/tmp/中文 file.txt".into(),
        remote_path: "/home/alice/中文 file.txt".into(),
        cancelled: false,
        success: true,
        skipped: true,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"transferId": "transfer-1", "error": null, "localPath": "/tmp/中文 file.txt", "remotePath": "/home/alice/中文 file.txt", "cancelled": false, "success": true, "skipped": true})
    );
    assert_eq!(
        format!("sftp-upload-item-complete-{}", "tab-中文-7"),
        "sftp-upload-item-complete-tab-中文-7"
    );
}

#[test]
fn legacy_u06_wire_contract() {
    let payload = UploadItemCompleteEvent {
        transfer_id: "transfer-1".into(),
        error: None,
        local_path: "/tmp/中文 file.txt".into(),
        remote_path: "/home/alice/中文 file.txt".into(),
        cancelled: false,
        success: true,
        skipped: false,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"transferId": "transfer-1", "error": null, "localPath": "/tmp/中文 file.txt", "remotePath": "/home/alice/中文 file.txt", "cancelled": false, "success": true, "skipped": false})
    );
    assert_eq!(
        format!("sftp-upload-item-complete-{}", "tab-中文-7"),
        "sftp-upload-item-complete-tab-中文-7"
    );
}

#[test]
fn legacy_u07_wire_contract() {
    let payload = UploadItemCompleteEvent {
        transfer_id: "transfer-1".into(),
        error: Some("Upload cancelled by user".into()),
        local_path: "/tmp/中文 file.txt".into(),
        remote_path: "/home/alice/中文 file.txt".into(),
        cancelled: true,
        success: false,
        skipped: false,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"transferId": "transfer-1", "error": "Upload cancelled by user", "localPath": "/tmp/中文 file.txt", "remotePath": "/home/alice/中文 file.txt", "cancelled": true, "success": false, "skipped": false})
    );
    assert_eq!(
        format!("sftp-upload-item-complete-{}", "tab-中文-7"),
        "sftp-upload-item-complete-tab-中文-7"
    );
}

#[test]
fn legacy_u08_wire_contract() {
    let payload = UploadBatchCompleteEvent {
        batch_id: "batch-1".into(),
        cancelled: false,
        error: None,
        failed: 2,
        succeeded: 2,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"batchId": "batch-1", "cancelled": false, "error": null, "failed": 2, "succeeded": 2})
    );
    assert_eq!(
        format!("sftp-upload-batch-complete-{}", "tab-中文-7"),
        "sftp-upload-batch-complete-tab-中文-7"
    );
}

#[test]
fn legacy_u09_wire_contract() {
    let payload = UploadItemProgressEvent {
        transfer_id: "transfer-1".into(),
        local_path: "/tmp/中文 file.txt".into(),
        transferred: 100,
        total: 200,
        progress: 50,
        resumed_from: 2,
        parallelism: 4,
        completed_chunks: 2,
        chunk_count: 2,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"transferId": "transfer-1", "localPath": "/tmp/中文 file.txt", "transferred": 100, "total": 200, "progress": 50, "resumedFrom": 2, "parallelism": 4, "completedChunks": 2, "chunkCount": 2})
    );
    assert_eq!(
        format!("sftp-upload-progress-{}", "tab-中文-7"),
        "sftp-upload-progress-tab-中文-7"
    );
}
