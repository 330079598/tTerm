//! Channel contracts captured before introducing the core search sink.
use super::*;
use std::sync::{Arc, Mutex};
use tauri::ipc::InvokeResponseBody;

#[test]
fn legacy_search_channel_json_and_limit() {
    let messages = Arc::new(Mutex::new(Vec::new()));
    let captured = messages.clone();
    let channel = Channel::new(move |body| {
        if let InvokeResponseBody::Json(json) = body {
            captured
                .lock()
                .unwrap()
                .push(serde_json::from_str::<serde_json::Value>(&json).unwrap());
        }
        Ok(())
    });
    let mut sink = ResultSink {
        channel,
        found: 999,
        truncated: false,
    };
    assert!(sink.send(vec![entry()]));
    assert!(!sink.truncated);
    sink.progress(25);
    assert!(!sink.send(vec![entry()]));
    assert_eq!(sink.found, 1000);
    assert_eq!(
        *messages.lock().unwrap(),
        vec![
            serde_json::json!({"kind":"matches","entries":[{"name":"中文.txt","path":"/tmp/中文.txt","isDir":false,"isSymlink":false,"size":null,"modifiedAt":null,"permissions":null,"mode":null,"owner":null,"group":null}]}),
            serde_json::json!({"kind":"progress","scannedFolders":25}),
        ]
    );
}

#[test]
fn legacy_search_ignores_channel_errors_and_counts_results() {
    let channel = Channel::new(|_| Err(tauri::Error::FailedToReceiveMessage));
    let mut sink = ResultSink {
        channel,
        found: 0,
        truncated: false,
    };
    assert!(sink.send(vec![entry()]));
    sink.progress(50);
    assert_eq!(sink.found, 1);
    assert!(!sink.truncated);
}

fn entry() -> SftpDirectoryEntry {
    SftpDirectoryEntry {
        name: "中文.txt".into(),
        path: "/tmp/中文.txt".into(),
        is_dir: false,
        is_symlink: false,
        size: None,
        modified_at: None,
        permissions: None,
        mode: None,
        owner: None,
        group: None,
    }
}
