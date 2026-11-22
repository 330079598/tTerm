//! Display-safe saved SSH profiles and launches for the native picker.
//!
//! Browsing reads only profile metadata. Secrets are resolved later by the
//! existing session authentication path when a confirmed tab starts.

use crate::connections::profile_connections::build_connection_from_profile;
use crate::core::session::types::PtyConnectionOptions;
use crate::db::sql_error;
use crate::profiles::connection::connection_options_for_profile;
use crate::profiles::storage::{find_profile, normalize_profile};
use crate::profiles::types::SavedProfile;
use crate::workspace::tab::Tab;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// The metadata shown for a saved SSH profile before it is opened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSshProfileSummary {
    pub id: String,
    pub name: String,
    pub username: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
}

impl SavedSshProfileSummary {
    /// The `user@host:port` label shown in the picker, without secrets.
    pub fn endpoint(&self) -> String {
        let mut endpoint = String::new();
        if let Some(username) = self
            .username
            .as_deref()
            .filter(|username| !username.trim().is_empty())
        {
            endpoint.push_str(username);
            endpoint.push('@');
        }
        let host = self
            .host
            .as_deref()
            .filter(|host| !host.trim().is_empty())
            .unwrap_or("not configured");
        if host.contains(':') && !host.starts_with('[') {
            endpoint.push('[');
            endpoint.push_str(host);
            endpoint.push(']');
        } else {
            endpoint.push_str(host);
        }
        if let Some(port) = self.port {
            endpoint.push(':');
            endpoint.push_str(&port.to_string());
        }
        endpoint
    }
}

/// A confirmed saved profile together with the tab and connection to open.
#[derive(Debug, Clone)]
pub struct SavedSshLaunch {
    pub summary: SavedSshProfileSummary,
    pub connection: PtyConnectionOptions,
    /// The saved tab fields; its id is assigned by the window.
    pub saved_tab: Tab,
}

/// Why a saved SSH profile could not become a launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavedSshLaunchError {
    /// The exact saved id no longer exists.
    NotFound,
    /// The exact saved id names a connection that is not SSH.
    NotSsh,
    /// The saved profile could not be read or prepared for a connection.
    Unavailable(String),
}

/// Reads the safe metadata of every saved SSH profile.
///
/// This only reads the isolated profile database: it does not touch the
/// keychain, resolve secrets, or open a network connection.
pub fn load_saved_ssh_profile_summaries() -> Result<Vec<SavedSshProfileSummary>, String> {
    crate::db::read(list_saved_ssh_profile_summaries)
}

/// Projects only the display fields of SSH profiles, in their saved order.
///
/// The query keeps the rest of each JSON document, including secrets and
/// connection options, out of memory. Non-SSH records are filtered in SQL so a
/// malformed or future record type cannot break browsing.
pub fn list_saved_ssh_profile_summaries(
    connection: &Connection,
) -> Result<Vec<SavedSshProfileSummary>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, \
                    COALESCE(json_extract(data, '$.name'), id), \
                    json_extract(data, '$.username'), \
                    json_extract(data, '$.host'), \
                    json_extract(data, '$.port') \
             FROM profiles \
             WHERE json_extract(data, '$.connection_type') = 'ssh' \
             ORDER BY position, rowid",
        )
        .map_err(sql_error("Failed to read saved SSH profiles"))?;
    let rows = statement
        .query_map([], |row| {
            let port = row
                .get::<_, Option<i64>>(4)?
                .and_then(|port| u16::try_from(port).ok());
            Ok(SavedSshProfileSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                username: row.get(2)?,
                host: row.get(3)?,
                port,
            })
        })
        .map_err(sql_error("Failed to read saved SSH profiles"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(sql_error("Failed to read saved SSH profiles"))
}

/// Builds the launch for one exact saved profile id.
///
/// There is deliberately no name fallback here. A caller that already knows
/// a profile id must never open a different profile after a rename or delete.
pub fn load_saved_ssh_launch(profile_id: &str) -> Result<SavedSshLaunch, SavedSshLaunchError> {
    launch_from_lookup(profile_id, find_profile(profile_id))
}

/// Keeps SSH profiles in their saved order and maps only display-safe fields.
pub fn summarize_profiles(profiles: &[SavedProfile]) -> Vec<SavedSshProfileSummary> {
    profiles
        .iter()
        .filter(|profile| profile.connection_type == "ssh")
        .map(summarize_profile)
        .collect()
}

/// Builds a launch from a profile that was already read from storage.
pub fn launch_from_saved_profile(
    profile: &SavedProfile,
) -> Result<SavedSshLaunch, SavedSshLaunchError> {
    if profile.connection_type != "ssh" {
        return Err(SavedSshLaunchError::NotSsh);
    }
    let mut normalized = profile.clone();
    normalize_profile(&mut normalized);
    let connection =
        connection_options_for_profile(&normalized).map_err(SavedSshLaunchError::Unavailable)?;
    Ok(SavedSshLaunch {
        summary: summarize_profile(&normalized),
        connection,
        saved_tab: build_connection_from_profile(&normalized),
    })
}

fn summarize_profile(profile: &SavedProfile) -> SavedSshProfileSummary {
    SavedSshProfileSummary {
        id: profile.id.clone(),
        name: profile.name.clone(),
        username: profile.username.clone(),
        host: profile.host.clone(),
        port: profile.port,
    }
}

fn launch_from_lookup(
    profile_id: &str,
    lookup: Result<Option<SavedProfile>, String>,
) -> Result<SavedSshLaunch, SavedSshLaunchError> {
    match lookup {
        Ok(Some(profile)) if profile.id == profile_id => launch_from_saved_profile(&profile),
        Ok(Some(_)) | Ok(None) => Err(SavedSshLaunchError::NotFound),
        Err(error) => Err(SavedSshLaunchError::Unavailable(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SavedSshLaunchError, launch_from_lookup, launch_from_saved_profile,
        list_saved_ssh_profile_summaries, summarize_profiles,
    };
    use crate::profiles::types::SavedProfile;
    use rusqlite::{Connection, params};
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
            launch_from_saved_profile(&profile(json!({ "connection_type": "terminal" })))
                .unwrap_err(),
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
        };
        assert_eq!(summary.endpoint(), "not configured");
    }

    #[test]
    fn browsing_projects_stored_metadata_in_saved_order() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE profiles (\
                     id TEXT PRIMARY KEY NOT NULL, \
                     position INTEGER NOT NULL, \
                     data TEXT NOT NULL\
                 )",
            )
            .expect("profiles schema");
        let insert = |id: &str, position: i64, data: Value| {
            connection
                .execute(
                    "INSERT INTO profiles (id, position, data) VALUES (?1, ?2, ?3)",
                    params![id, position, data.to_string()],
                )
                .expect("insert profile");
        };
        insert(
            "later",
            2,
            json!({
                "id": "later",
                "name": "Later",
                "connection_type": "ssh",
                "host": "later.example.com",
                "port": 2200,
                "username": "deploy",
                "password": "must-not-be-selected"
            }),
        );
        insert(
            "first",
            0,
            json!({
                "id": "first",
                "name": "First",
                "connection_type": "ssh"
            }),
        );
        insert(
            "local",
            1,
            json!({
                "id": "local",
                "name": "Local",
                "connection_type": "terminal"
            }),
        );
        insert(
            "unknown",
            3,
            json!({
                "id": "unknown",
                "name": "Unknown",
                "connection_type": "future-connection"
            }),
        );

        let summaries = list_saved_ssh_profile_summaries(&connection).expect("projected summaries");

        assert_eq!(
            summaries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec!["first", "later"]
        );
        assert_eq!(summaries[0].endpoint(), "not configured");
        assert_eq!(summaries[1].endpoint(), "deploy@later.example.com:2200");
    }
}
