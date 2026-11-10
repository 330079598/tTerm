//! Legacy wire contracts captured before P1-04 transport changes.
use super::*;
use tterm_core::events::sftp::external::{EditStatus, ExternalEdit};

#[test]
fn legacy_e01_wire_contract() {
    let payload = ExternalEdit {
        id: "edit-1".into(),
        tab_id: "tab-中文-7".into(),
        remote_path: "/home/alice/中文 file.txt".into(),
        file_name: "file_name-value".into(),
        local_path: "/tmp/中文 file.txt".into(),
        status: EditStatus::Uploading,
        error: None,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"id": "edit-1", "tabId": "tab-中文-7", "remotePath": "/home/alice/中文 file.txt", "fileName": "file_name-value", "localPath": "/tmp/中文 file.txt", "status": "uploading", "error": null})
    );
    assert_eq!(EDIT_EVENT, "sftp-external-edit");
}

#[test]
fn legacy_e02_wire_contract() {
    let payload = ExternalEdit {
        id: "edit-1".into(),
        tab_id: "tab-中文-7".into(),
        remote_path: "/home/alice/中文 file.txt".into(),
        file_name: "file_name-value".into(),
        local_path: "/tmp/中文 file.txt".into(),
        status: EditStatus::Modified,
        error: None,
    };
    assert_eq!(
        serde_json::to_value(&payload).unwrap(),
        serde_json::json!({"id": "edit-1", "tabId": "tab-中文-7", "remotePath": "/home/alice/中文 file.txt", "fileName": "file_name-value", "localPath": "/tmp/中文 file.txt", "status": "modified", "error": null})
    );
    assert_eq!(EDIT_EVENT, "sftp-external-edit");
}
