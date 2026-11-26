//! Display-safe saved SSH profiles and launches for the native picker.
//!
//! Browsing reads only profile metadata. Secrets are resolved later by the
//! existing session authentication path when a confirmed tab starts.

use crate::connections::profile_connections::build_connection_from_profile;
use crate::core::session::types::PtyConnectionOptions;
use crate::db::sql_error;
use crate::profiles::connection::connection_options_for_profile;
use crate::profiles::groups::{group_paths, list_groups};
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
    /// The names of the group and the groups above it, from the top, joined
    /// by ` / `; `None` when the profile is ungrouped or its group is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_path: Option<String>,
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
    let paths = group_paths(&list_groups(connection)?);
    let mut statement = connection
        .prepare(
            "SELECT id, \
                    COALESCE(json_extract(data, '$.name'), id), \
                    json_extract(data, '$.username'), \
                    json_extract(data, '$.host'), \
                    json_extract(data, '$.port'), \
                    COALESCE(json_extract(data, '$.group_id'), json_extract(data, '$.group')) \
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
            let group_path = row
                .get::<_, Option<String>>(5)?
                .and_then(|id| paths.get(id.trim()).cloned());
            Ok(SavedSshProfileSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                username: row.get(2)?,
                host: row.get(3)?,
                port,
                group_path,
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
        group_path: profile.group_path.clone(),
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
mod tests;
