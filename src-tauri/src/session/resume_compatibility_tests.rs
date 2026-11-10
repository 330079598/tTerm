//! The dev baseline 5a7e90ba stores tabs as serde_json::Value. Its unchanged
//! read_session/write_session functions are used below to simulate an old
//! backend; typed projections model an old UI that ignores the added field.

use super::*;
use crate::core::agent_resume::{build_resume_command, ResumeAgent, ResumeRequest};
use crate::db::Database;

#[derive(Deserialize)]
struct CurrentTab {
    id: String,
    connection: CurrentConnection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurrentConnection {
    #[serde(default)]
    agent_resume: Option<SavedResume>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedResume {
    agent: ResumeAgent,
    session_id: String,
    cwd: String,
}

// Old dev backend's SessionData is unchanged: tabs is an open JSON value,
// so read_session/write_session preserve fields the old UI does not know.
#[derive(Deserialize)]
struct DevTab {
    id: String,
    connection: DevConnection,
}

#[derive(Deserialize)]
struct DevConnection {
    host: Option<String>,
}

fn saved_file(resume: Option<serde_json::Value>) -> Vec<u8> {
    let mut connection =
        serde_json::json!({"type":"ssh", "host":"test.invalid", "password":"secret"});
    if let Some(resume) = resume {
        connection["agentResume"] = resume;
    }
    serde_json::to_vec(&serde_json::json!({
        "tabs": [{"id":"tab-1", "connection":connection}],
        "active_tab_id":"tab-1", "schema_version":1, "last_saved":5,
        "layout":{"root":"tab-1"}
    }))
    .unwrap()
}

fn round_trip_file(bytes: &[u8]) -> SessionData {
    let database = Database::open_in_memory().unwrap();
    database
        .write(|connection| {
            import_session(connection, bytes)?;
            let loaded = read_session(connection)?.unwrap();
            write_session(connection, loaded)?;
            Ok(read_session(connection)?.unwrap())
        })
        .unwrap()
}

#[test]
fn new_reader_loads_old_workspace_without_resume_and_keeps_l1_metadata() {
    let saved = round_trip_file(&saved_file(None));
    let tabs: Vec<CurrentTab> = serde_json::from_value(saved.tabs.clone()).unwrap();
    assert_eq!(tabs[0].id, "tab-1");
    assert!(tabs[0].connection.agent_resume.is_none());
    assert_eq!(saved.active_tab_id.as_deref(), Some("tab-1"));
    assert_eq!(saved.layout.unwrap()["root"], "tab-1");
    assert_eq!(saved.schema_version, 1);
    assert!(saved.tabs[0]["connection"].get("password").is_none());
}

#[test]
fn unchanged_dev_reader_accepts_new_workspace_and_round_trips_optional_field() {
    let resume = serde_json::json!({"agent":"codex", "sessionId":"abc-123", "cwd":"/remote work"});
    let saved = round_trip_file(&saved_file(Some(resume.clone())));
    let old_ui: Vec<DevTab> = serde_json::from_value(saved.tabs.clone()).unwrap();
    assert_eq!(old_ui[0].id, "tab-1");
    assert_eq!(old_ui[0].connection.host.as_deref(), Some("test.invalid"));
    assert_eq!(saved.tabs[0]["connection"]["agentResume"], resume);
    assert!(saved.tabs[0]["connection"].get("password").is_none());
    let new_ui: Vec<CurrentTab> = serde_json::from_value(saved.tabs).unwrap();
    let restored = new_ui[0].connection.agent_resume.as_ref().unwrap();
    assert_eq!(restored.session_id, "abc-123");
    assert_eq!(restored.cwd, "/remote work");
    // Only one connection key is added; schema and layout are unchanged.
    let old: serde_json::Value = serde_json::from_slice(&saved_file(None)).unwrap();
    let new: serde_json::Value = serde_json::from_slice(&saved_file(Some(resume))).unwrap();
    let mut projected = new.clone();
    projected["tabs"][0]["connection"]
        .as_object_mut()
        .unwrap()
        .remove("agentResume");
    assert_eq!(projected, old);
}

#[test]
fn tampered_saved_workspaces_never_construct_a_resume_command() {
    for (id, cwd) in [
        ("abc; rm -rf ~", "/safe"),
        ("abc-123", "/safe\nrm -rf ~"),
        ("abc-123", "/safe\rnext"),
        ("abc-123", "/safe\tname"),
        ("abc-123", "/safe\0name"),
        ("abc-123", "/safe\u{1b}name"),
        ("abc-123", "/safe\u{85}name"),
    ] {
        let saved = round_trip_file(&saved_file(Some(serde_json::json!({
            "agent":"claudeCode", "sessionId":id, "cwd":cwd,
            "shell":"bash", "command":"rm -rf ~"
        }))));
        let tabs: Vec<CurrentTab> = serde_json::from_value(saved.tabs).unwrap();
        let resume = tabs[0].connection.agent_resume.as_ref().unwrap();
        for shell in ["bash", "zsh", "fish", "sh", "pwsh", "cmd"] {
            assert_eq!(
                build_resume_command(&ResumeRequest {
                    agent: resume.agent,
                    session_id: &resume.session_id,
                    cwd: &resume.cwd,
                    shell,
                }),
                None,
                "{id:?} {cwd:?} {shell}"
            );
        }
    }
}
