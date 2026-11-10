//! Legacy wire contracts captured before P1-04 transport changes.
use tterm_core::events::sftp::download::{
    DownloadBatchCompleteEvent, DownloadItemCompleteEvent, DownloadItemStartEvent,
    DownloadProgressEvent,
};

#[test]
fn legacy_d01_wire_contract() {
    let payload = DownloadProgressEvent {
        transfer_id: "transfer-1".into(),
        local_path: "/tmp/中文 file.txt".into(),
        remote_path: "/home/alice/中文 file.txt".into(),
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
        serde_json::json!({"transferId": "transfer-1", "localPath": "/tmp/中文 file.txt", "remotePath": "/home/alice/中文 file.txt", "transferred": 100, "total": 200, "progress": 50, "resumedFrom": 2, "parallelism": 4, "completedChunks": 2, "chunkCount": 2})
    );
    assert_eq!(
        format!("sftp-download-progress-{}", "tab-中文-7"),
        "sftp-download-progress-tab-中文-7"
    );
}

#[test]
fn legacy_d02_wire_contract() {
    let payload = DownloadProgressEvent {
        transfer_id: "item-1".into(),
        local_path: "/tmp/中文 file.txt".into(),
        remote_path: "/home/alice/中文 file.txt".into(),
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
        serde_json::json!({"transferId": "item-1", "localPath": "/tmp/中文 file.txt", "remotePath": "/home/alice/中文 file.txt", "transferred": 100, "total": 200, "progress": 50, "resumedFrom": 2, "parallelism": 4, "completedChunks": 2, "chunkCount": 2})
    );
    assert_eq!(
        format!("sftp-download-progress-{}", "tab-中文-7"),
        "sftp-download-progress-tab-中文-7"
    );
}

#[test]
fn legacy_d03_wire_contract() {
    let payload = DownloadProgressEvent {
        transfer_id: "batch-1".into(),
        local_path: "/tmp/中文 file.txt".into(),
        remote_path: "/home/alice/中文 file.txt".into(),
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
        serde_json::json!({"transferId": "batch-1", "localPath": "/tmp/中文 file.txt", "remotePath": "/home/alice/中文 file.txt", "transferred": 100, "total": 200, "progress": 50, "resumedFrom": 2, "parallelism": 4, "completedChunks": 2, "chunkCount": 2})
    );
    assert_eq!(
        format!("sftp-download-progress-{}", "tab-中文-7"),
        "sftp-download-progress-tab-中文-7"
    );
}

#[test]
fn legacy_d04_wire_contract() {
    let payload = DownloadItemStartEvent {
        transfer_id: "transfer-1".into(),
        batch_id: "batch-1".into(),
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
        format!("sftp-download-item-start-{}", "tab-中文-7"),
        "sftp-download-item-start-tab-中文-7"
    );
}

#[test]
fn legacy_d05_wire_contract() {
    let payload = DownloadItemCompleteEvent {
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
        format!("sftp-download-item-complete-{}", "tab-中文-7"),
        "sftp-download-item-complete-tab-中文-7"
    );
}

#[test]
fn legacy_d06_wire_contract() {
    let payload = DownloadItemCompleteEvent {
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
        format!("sftp-download-item-complete-{}", "tab-中文-7"),
        "sftp-download-item-complete-tab-中文-7"
    );
}

#[test]
fn legacy_d07_wire_contract() {
    let payload = DownloadProgressEvent {
        transfer_id: "transfer-1".into(),
        local_path: "/tmp/中文 file.txt".into(),
        remote_path: "/home/alice/中文 file.txt".into(),
        transferred: 100,
        total: 200,
        progress: 50,
        resumed_from: 2,
        parallelism: 0,
        completed_chunks: 0,
        chunk_count: 0,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"transferId": "transfer-1", "localPath": "/tmp/中文 file.txt", "remotePath": "/home/alice/中文 file.txt", "transferred": 100, "total": 200, "progress": 50, "resumedFrom": 2, "parallelism": 0, "completedChunks": 0, "chunkCount": 0})
    );
    assert_eq!(
        format!("sftp-download-progress-{}", "tab-中文-7"),
        "sftp-download-progress-tab-中文-7"
    );
}

#[test]
fn legacy_d08_wire_contract() {
    let payload = DownloadItemCompleteEvent {
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
        format!("sftp-download-item-complete-{}", "tab-中文-7"),
        "sftp-download-item-complete-tab-中文-7"
    );
}

#[test]
fn legacy_d09_wire_contract() {
    let payload = DownloadItemCompleteEvent {
        transfer_id: "transfer-1".into(),
        error: None,
        local_path: "/tmp/中文 file.txt".into(),
        remote_path: "/home/alice/中文 file.txt".into(),
        cancelled: true,
        success: false,
        skipped: false,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"transferId": "transfer-1", "error": null, "localPath": "/tmp/中文 file.txt", "remotePath": "/home/alice/中文 file.txt", "cancelled": true, "success": false, "skipped": false})
    );
    assert_eq!(
        format!("sftp-download-item-complete-{}", "tab-中文-7"),
        "sftp-download-item-complete-tab-中文-7"
    );
}

#[test]
fn legacy_d10_wire_contract() {
    let payload = DownloadProgressEvent {
        transfer_id: "transfer-1".into(),
        local_path: "/tmp/中文 file.txt".into(),
        remote_path: "/home/alice/中文 file.txt".into(),
        transferred: 100,
        total: 200,
        progress: 100,
        resumed_from: 2,
        parallelism: 0,
        completed_chunks: 0,
        chunk_count: 0,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"transferId": "transfer-1", "localPath": "/tmp/中文 file.txt", "remotePath": "/home/alice/中文 file.txt", "transferred": 100, "total": 200, "progress": 100, "resumedFrom": 2, "parallelism": 0, "completedChunks": 0, "chunkCount": 0})
    );
    assert_eq!(
        format!("sftp-download-progress-{}", "tab-中文-7"),
        "sftp-download-progress-tab-中文-7"
    );
}

#[test]
fn legacy_d11_wire_contract() {
    let payload = DownloadBatchCompleteEvent {
        batch_id: "batch-1".into(),
        cancelled: false,
        error: None,
        transferred: 100,
        total: 200,
        skipped: 2,
        resumed: 2,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"batchId": "batch-1", "cancelled": false, "error": null, "transferred": 100, "total": 200, "skipped": 2, "resumed": 2})
    );
    assert_eq!(
        format!("sftp-download-batch-complete-{}", "tab-中文-7"),
        "sftp-download-batch-complete-tab-中文-7"
    );
}
