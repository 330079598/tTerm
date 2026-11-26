use super::{
    SavedSshLaunchError, launch_from_lookup, launch_from_saved_profile,
    list_saved_ssh_profile_summaries, summarize_profiles,
};
use crate::db::Database;
use crate::profiles::groups::put_group;
use crate::profiles::types::{ProfileGroup, SavedProfile};
use rusqlite::params;
use serde_json::{Value, json};

fn profile(overrides: Value) -> SavedProfile {
    let mut value = json!({
        "id": "prod",
        "name": "Production",
        "connection_type": "ssh",
        "host": "example.com",
        "port": 2222,
        "username": "deploy",
        "auth_method": "password",
        "password": "stored-password",
        "private_key_passphrase": "stored-passphrase"
    });
    value
        .as_object_mut()
        .expect("profile object")
        .extend(overrides.as_object().cloned().unwrap_or_default());
    serde_json::from_value(value).expect("profile")
}

#[test]
fn the_picker_lists_only_ssh_profiles_and_shows_a_safe_endpoint() {
    let mut local = profile(json!({
        "id": "local",
        "name": "Local",
        "connection_type": "terminal"
    }));
    local.password = Some("local-secret".to_string());
    local.private_key_passphrase = Some("key-secret".to_string());
    let summaries = summarize_profiles(&[profile(json!({})), local]);

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "prod");
    assert_eq!(summaries[0].endpoint(), "deploy@example.com:2222");
    let serialized = serde_json::to_string(&summaries).expect("summaries");
    assert!(!serialized.contains("stored-password"));
    assert!(!serialized.contains("stored-passphrase"));
}

#[test]
fn launches_use_the_exact_saved_id_and_never_a_same_named_profile() {
    let production = profile(json!({}));
    let mut renamed = profile(json!({ "id": "other", "name": "prod" }));
    renamed.host = Some("other.example.com".to_string());

    let launch =
        launch_from_lookup("prod", Ok(Some(production))).expect("exact profile should load");
    assert_eq!(launch.connection.profile_id.as_deref(), Some("prod"));
    assert_eq!(
        launch_from_lookup("prod", Ok(Some(renamed))).unwrap_err(),
        SavedSshLaunchError::NotFound
    );
    assert_eq!(
        launch_from_lookup("missing", Ok(None)).unwrap_err(),
        SavedSshLaunchError::NotFound
    );
}

#[test]
fn non_ssh_and_unreadable_profiles_do_not_open_a_tab() {
    assert_eq!(
        launch_from_saved_profile(&profile(json!({ "connection_type": "terminal" }))).unwrap_err(),
        SavedSshLaunchError::NotSsh
    );
    assert_eq!(
        launch_from_lookup("prod", Err("profile data is corrupt".to_string())).unwrap_err(),
        SavedSshLaunchError::Unavailable("profile data is corrupt".to_string())
    );
}

#[test]
fn a_confirmed_launch_carries_connection_details_but_no_saved_secrets() {
    let launch = launch_from_saved_profile(&profile(json!({
        "auth_method": "key",
        "private_key_path": "/keys/id",
        "agent_forward": true
    })))
    .expect("launch");
    assert_eq!(launch.summary.id, "prod");
    assert_eq!(launch.connection.host.as_deref(), Some("example.com"));
    assert_eq!(
        launch.connection.private_key_path.as_deref(),
        Some("/keys/id")
    );
    assert!(launch.connection.agent_forward);
    let serialized = serde_json::to_string(&launch.saved_tab).expect("saved tab");
    assert!(!serialized.contains("stored-password"));
    assert!(!serialized.contains("stored-passphrase"));
}

#[test]
fn an_empty_endpoint_does_not_invent_a_host() {
    let summary = super::SavedSshProfileSummary {
        id: "blank".to_string(),
        name: "Blank".to_string(),
        username: None,
        host: None,
        port: None,
        group_path: None,
    };
    assert_eq!(summary.endpoint(), "not configured");
}

#[test]
fn summarizing_profiles_keeps_the_group_path_listing_filled_in() {
    let grouped = profile(json!({
        "id": "web",
        "name": "Web",
        "group_path": "Prod / Web"
    }));
    let plain = profile(json!({ "id": "plain", "name": "Plain" }));

    let summaries = summarize_profiles(&[grouped.clone(), plain]);

    assert_eq!(summaries[0].group_path.as_deref(), Some("Prod / Web"));
    assert_eq!(summaries[1].group_path, None);
    let grouped_json = serde_json::to_value(&summaries[0]).expect("grouped summary");
    assert_eq!(grouped_json["groupPath"], "Prod / Web");
    let plain_json = serde_json::to_value(&summaries[1]).expect("plain summary");
    assert!(plain_json.get("groupPath").is_none());

    let launch = launch_from_saved_profile(&grouped).expect("launch");
    assert_eq!(launch.summary.group_path.as_deref(), Some("Prod / Web"));
}

#[test]
fn browsing_projects_metadata_and_nested_group_paths_in_saved_order() {
    let database = Database::open_in_memory().expect("isolated database");
    database
        .write(|connection| {
            for group in [
                ProfileGroup {
                    id: "prod".to_string(),
                    name: "Prod".to_string(),
                    parent_id: None,
                },
                ProfileGroup {
                    id: "web".to_string(),
                    name: "Web".to_string(),
                    parent_id: Some("prod".to_string()),
                },
            ] {
                put_group(connection, &group)?;
            }
            let insert = |id: &str, position: i64, data: Value| -> Result<(), String> {
                connection
                    .execute(
                        "INSERT INTO profiles (id, position, data) VALUES (?1, ?2, ?3)",
                        params![id, position, data.to_string()],
                    )
                    .map_err(|error| error.to_string())?;
                Ok(())
            };
            insert(
                "later",
                2,
                json!({
                    "id": "later",
                    "name": "Later",
                    "group_id": "web",
                    "connection_type": "ssh",
                    "host": "later.example.com",
                    "port": 2200,
                    "username": "deploy",
                    "password": "stored-password"
                }),
            )?;
            insert(
                "first",
                0,
                json!({
                    "id": "first",
                    "name": "First",
                    "connection_type": "ssh"
                }),
            )?;
            insert(
                "local",
                1,
                json!({
                    "id": "local",
                    "name": "Local",
                    "connection_type": "terminal"
                }),
            )?;
            insert(
                "legacy",
                3,
                json!({
                    "id": "legacy",
                    "name": "Legacy",
                    "group": "prod",
                    "connection_type": "ssh"
                }),
            )?;
            insert(
                "unknown",
                4,
                json!({
                    "id": "unknown",
                    "name": "Unknown",
                    "connection_type": "future-connection"
                }),
            )?;

            let summaries = list_saved_ssh_profile_summaries(connection)?;

            assert_eq!(
                summaries
                    .iter()
                    .map(|entry| entry.id.as_str())
                    .collect::<Vec<_>>(),
                vec!["first", "later", "legacy"]
            );
            assert_eq!(summaries[0].endpoint(), "not configured");
            assert_eq!(summaries[0].group_path, None);
            assert_eq!(summaries[1].endpoint(), "deploy@later.example.com:2200");
            assert_eq!(summaries[1].group_path.as_deref(), Some("Prod / Web"));
            assert_eq!(summaries[2].group_path.as_deref(), Some("Prod"));
            let serialized = serde_json::to_string(&summaries).expect("summaries");
            assert!(!serialized.contains("stored-password"));
            Ok(())
        })
        .expect("projected summaries");
}
