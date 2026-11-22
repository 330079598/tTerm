use std::path::PathBuf;

use serde_json::Value;

use super::*;
use crate::session_restore::{read_session, write_session};

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("session")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn fixture_value(name: &str) -> Value {
    serde_json::from_str(&fixture(name)).expect("fixture JSON")
}

fn fixture_session(name: &str) -> SessionData {
    serde_json::from_str(&fixture(name)).expect("session fixture")
}

fn readable(load: WorkspaceLoad) -> (WorkspaceSnapshot, bool) {
    match load {
        WorkspaceLoad::Readable { snapshot, can_save } => (snapshot, can_save),
        other => panic!("expected a readable workspace, got {other:?}"),
    }
}

#[test]
fn reads_a_tauri_session_with_unknown_fields_and_non_numeric_ids() {
    let (snapshot, can_save) = readable(decode_session(fixture_session("tauri-v1-workspace.json")));
    assert!(can_save);
    assert_eq!(snapshot.active_tab_id.as_deref(), Some("ssh:prod host"));
    assert_eq!(
        snapshot
            .tabs
            .iter()
            .map(|tab| tab.id.as_str())
            .collect::<Vec<_>>(),
        ["local shell 1", "ssh:prod host"]
    );
    assert_eq!(
        snapshot.tabs[0].saved_field("futureTabField"),
        Some(&serde_json::json!(["kept"]))
    );
    let connection = snapshot.tabs[1].connection.as_ref().expect("connection");
    assert_eq!(
        connection.saved_field("futureConnectionField"),
        Some(&serde_json::json!("kept"))
    );
    assert_eq!(
        connection.agent_resume,
        Some(serde_json::json!({
            "agent": "codex",
            "sessionId": "session-alpha-42",
            "cwd": "/srv/app"
        }))
    );
    assert!(
        snapshot.tabs[1]
            .connection
            .as_ref()
            .unwrap()
            .password
            .is_none()
    );
    assert_eq!(snapshot.groups.len(), 1);
    assert_eq!(
        snapshot.groups[0].active_tab_id,
        "ssh:prod host".to_string()
    );
    assert_eq!(
        snapshot.groups[0].zoomed_tab_id.as_deref(),
        Some("ssh:prod host")
    );
}

#[test]
fn the_native_writer_matches_the_cross_client_fixture() {
    let session = fixture_session("tauri-v1-workspace.json");
    let last_saved = session.last_saved;
    let (snapshot, _) = readable(decode_session(session));
    let encoded = serde_json::to_value(encode_session(snapshot, last_saved)).unwrap();
    assert_eq!(encoded, fixture_value("gpui-v1-workspace.json"));
}

#[test]
fn the_native_writer_keeps_resume_and_removes_credentials() {
    let session = fixture_session("tauri-v1-workspace.json");
    let (snapshot, _) = readable(decode_session(session));
    let encoded = encode_session(snapshot, 7);
    assert_eq!(encoded.schema_version, 1);
    assert_eq!(encoded.last_saved, 7);
    assert_eq!(encoded.tabs.as_array().unwrap().len(), 2);
    let ssh = &encoded.tabs[1];
    assert_eq!(ssh["isActive"], true);
    assert_eq!(ssh["hasConnected"], true);
    assert_eq!(ssh["sessionNonce"], 0);
    assert_eq!(
        ssh["connection"]["agentResume"]["sessionId"],
        "session-alpha-42"
    );
    assert!(ssh["connection"].get("password").is_none());
    assert!(ssh["connection"].get("privateKeyPassphrase").is_none());
    assert!(ssh["connection"]["jumpHosts"][0].get("password").is_none());
    assert_eq!(encoded.tabs[0]["isActive"], false);
    assert_eq!(encoded.tabs[0]["hasConnected"], false);
}

#[test]
fn a_missing_layout_is_readable_and_saveable() {
    let (snapshot, can_save) = readable(decode_session(fixture_session("missing-layout.json")));
    assert!(can_save);
    assert_eq!(snapshot.tabs.len(), 1);
    assert!(snapshot.groups.is_empty());
}

#[test]
fn a_legacy_dockview_layout_is_readable_and_saveable() {
    let (snapshot, can_save) = readable(decode_session(fixture_session("legacy-dockview.json")));
    assert!(can_save);
    assert_eq!(snapshot.groups.len(), 1);
    assert_eq!(snapshot.groups[0].active_tab_id, "right");
    assert_eq!(
        get_pane_ids(&snapshot.groups[0].root),
        vec!["left", "right"]
    );
}

#[test]
fn a_future_schema_is_not_decoded() {
    assert_eq!(
        decode_session(fixture_session("future-schema.json")),
        WorkspaceLoad::Future { schema_version: 2 }
    );
}

#[test]
fn duplicate_ids_and_malformed_tabs_fail_closed() {
    assert!(matches!(
        decode_session(fixture_session("duplicate-ids.json")),
        WorkspaceLoad::Unreadable { .. }
    ));
    assert!(matches!(
        decode_session(fixture_session("malformed-tabs.json")),
        WorkspaceLoad::Unreadable { .. }
    ));
}

#[test]
fn a_malformed_top_level_layout_fails_closed() {
    assert!(matches!(
        decode_session(fixture_session("malformed-layout.json")),
        WorkspaceLoad::Unreadable { .. }
    ));
}

#[test]
fn an_invalid_group_is_kept_readable_but_not_saved_over() {
    let session: SessionData = serde_json::from_value(serde_json::json!({
        "tabs": [{"id": "a", "title": "A", "type": "terminal", "isActive": true}],
        "active_tab_id": "a",
        "layout": {
            "version": 2,
            "groups": [{
                "root": {
                    "type": "split",
                    "orientation": "horizontal",
                    "children": [{"type": "pane", "tabId": "a"}],
                    "sizes": ["broken"]
                },
                "activeTabId": "a"
            }]
        },
        "schema_version": 1,
        "last_saved": 1
    }))
    .unwrap();
    let (snapshot, can_save) = readable(decode_session(session));
    assert!(!can_save);
    assert_eq!(snapshot.groups.len(), 1);
    assert_eq!(get_pane_ids(&snapshot.groups[0].root), vec!["a"]);
    assert_eq!(snapshot.tabs.len(), 1);
}

#[test]
fn reads_the_actual_sqlite_fixture_and_reports_malformed_rows() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("session")
        .join("tauri-session.db");
    let connection = rusqlite::Connection::open(&path).unwrap();
    let (snapshot, can_save) = readable(super::load::from_connection(&connection).unwrap());
    assert!(can_save);
    assert_eq!(snapshot.active_tab_id.as_deref(), Some("ssh:prod host"));
    assert_eq!(snapshot.tabs.len(), 2);
    drop(connection);

    let directory =
        std::env::temp_dir().join(format!("tterm-workspace-corrupt-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let copy = directory.join("session.db");
    std::fs::copy(&path, &copy).unwrap();
    let connection = rusqlite::Connection::open(&copy).unwrap();
    connection
        .execute(
            "UPDATE app_meta SET value = '{ not json' WHERE key = 'session'",
            [],
        )
        .unwrap();
    assert!(matches!(
        super::load::from_connection(&connection).unwrap(),
        WorkspaceLoad::Unreadable { .. }
    ));
    connection
        .execute("DELETE FROM app_meta WHERE key = 'session'", [])
        .unwrap();
    assert_eq!(
        super::load::from_connection(&connection).unwrap(),
        WorkspaceLoad::Empty
    );
    drop(connection);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_disk_session_round_trips_through_the_same_row_reader() {
    let directory = std::env::temp_dir().join(format!("tterm-workspace-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("session.db");
    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch("CREATE TABLE app_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap();
        let (snapshot, _) = readable(decode_session(fixture_session("tauri-v1-workspace.json")));
        write_session(&connection, encode_session(snapshot, 99)).unwrap();
    }
    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        let (snapshot, can_save) = readable(super::load::from_connection(&connection).unwrap());
        assert!(can_save);
        assert_eq!(snapshot.active_tab_id.as_deref(), Some("ssh:prod host"));
        assert_eq!(
            snapshot.tabs[1].connection.as_ref().unwrap().agent_resume,
            Some(serde_json::json!({
                "agent": "codex",
                "sessionId": "session-alpha-42",
                "cwd": "/srv/app"
            }))
        );
        let saved = read_session(&connection).unwrap().unwrap();
        assert_eq!(saved.last_saved, 99);
        assert_eq!(saved.schema_version, 1);
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn active_falls_back_to_the_first_surviving_tab() {
    let session: SessionData = serde_json::from_value(serde_json::json!({
        "tabs": [
            {"id": "settings", "title": "Settings", "type": "settings", "isActive": true},
            {"id": "shell", "title": "Shell", "type": "terminal", "isActive": false}
        ],
        "active_tab_id": "settings",
        "schema_version": 0,
        "last_saved": 1
    }))
    .unwrap();
    let (snapshot, can_save) = readable(decode_session(session));
    assert!(can_save);
    assert_eq!(snapshot.active_tab_id.as_deref(), Some("shell"));
    assert_eq!(snapshot.tabs.len(), 1);
}

#[test]
fn unsupported_tabs_are_preserved_rather_than_rejected() {
    let session: SessionData = serde_json::from_value(serde_json::json!({
        "tabs": [{
            "id": "future",
            "title": "Future",
            "type": "future-pane",
            "isActive": true,
            "futureField": {"kept": true}
        }],
        "active_tab_id": "future",
        "schema_version": 1,
        "last_saved": 1
    }))
    .unwrap();
    let (snapshot, can_save) = readable(decode_session(session));
    assert!(can_save);
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(
        snapshot.tabs[0].tab_type,
        TabType::Unknown("future-pane".into())
    );
    assert_eq!(
        snapshot.tabs[0].saved_field("futureField"),
        Some(&serde_json::json!({"kept": true}))
    );
    let encoded = encode_session(snapshot, 2);
    assert_eq!(encoded.tabs[0]["type"], "future-pane");
    assert_eq!(encoded.tabs[0]["futureField"]["kept"], true);
}

#[test]
fn duplicate_panes_are_readable_but_marked_lossy() {
    let session: SessionData = serde_json::from_value(serde_json::json!({
        "tabs": [
            {"id": "a", "title": "A", "type": "terminal", "isActive": true},
            {"id": "b", "title": "B", "type": "ssh", "isActive": false}
        ],
        "active_tab_id": "a",
        "layout": {
            "version": 2,
            "groups": [
                {
                    "root": {"type": "pane", "tabId": "a"},
                    "activeTabId": "a"
                },
                {
                    "root": {"type": "pane", "tabId": "a"},
                    "activeTabId": "a"
                }
            ]
        },
        "schema_version": 1,
        "last_saved": 1
    }))
    .unwrap();
    let (snapshot, can_save) = readable(decode_session(session));
    assert!(!can_save);
    assert_eq!(snapshot.groups.len(), 1);
    assert_eq!(get_pane_ids(&snapshot.groups[0].root), vec!["a"]);
}
