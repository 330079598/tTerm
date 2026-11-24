//! The typed workspace shared by the Tauri and GPUI clients.
//!
//! `SessionData` keeps tabs as open JSON so older clients can preserve fields
//! they do not know. This module turns that JSON into a typed workspace without
//! silently discarding data that would be lost by writing it back.

mod load;
#[cfg(test)]
mod tests;

use std::collections::HashSet;

use serde_json::Value;

use crate::session_restore::SessionData;
use crate::workspace::tab::{Tab, TabType, is_page_tab};
use crate::workspace::tab_groups::{TabGroup, get_pane_ids};
use crate::workspace::tab_groups_persistence::{
    RestoredTabGroup, restore_tab_groups, serialize_tab_groups,
};

/// The newest saved-session schema this client can safely write.
pub const SUPPORTED_SESSION_SCHEMA_VERSION: u32 = 1;

/// A saved workspace after filtering transient tabs.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceSnapshot {
    /// The session tabs in tab-bar order, with unknown fields preserved.
    pub tabs: Vec<Tab>,
    /// The active tab, or the first surviving one when the saved id is gone.
    pub active_tab_id: Option<String>,
    /// The saved pane groups, keyed by persisted tab ids.
    pub groups: Vec<RestoredTabGroup>,
}

/// The result of reading a saved workspace.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceLoad {
    /// No session has been saved.
    Empty,
    /// A workspace that can be restored.
    Readable {
        /// The typed workspace.
        snapshot: WorkspaceSnapshot,
        /// Whether writing this workspace back would lose saved information.
        can_save: bool,
    },
    /// The stored session is malformed and must not be overwritten.
    Unreadable {
        /// Why the session cannot be read.
        reason: String,
    },
    /// A session written by a newer client.
    Future {
        /// The schema version that was found.
        schema_version: u32,
    },
}

/// Reads a saved session into the typed workspace.
pub fn decode_session(session: SessionData) -> WorkspaceLoad {
    if session.schema_version > SUPPORTED_SESSION_SCHEMA_VERSION {
        return WorkspaceLoad::Future {
            schema_version: session.schema_version,
        };
    }

    let raw_tabs = match session.tabs.as_array() {
        Some(tabs) => tabs,
        None => {
            return unreadable("The saved session's tabs are not an array.");
        }
    };

    let mut ids = HashSet::new();
    for (index, tab) in raw_tabs.iter().enumerate() {
        let object = match tab.as_object() {
            Some(object) => object,
            None => {
                return unreadable(format!("Saved tab {} is not an object.", index + 1));
            }
        };
        let id = match object.get("id").and_then(Value::as_str) {
            Some(id) if !id.is_empty() => id,
            _ => {
                return unreadable(format!(
                    "Saved tab {} has no non-empty string id.",
                    index + 1
                ));
            }
        };
        if !ids.insert(id.to_string()) {
            return unreadable(format!("Saved tab id {id:?} appears more than once."));
        }
    }

    // Reading sanitizes old secrets as well as preserving fields this client
    // does not understand.
    let sanitized = super::sanitize_session_tabs(session.tabs);
    let sanitized = sanitized.as_array().cloned().unwrap_or_default();
    let mut tabs = Vec::with_capacity(sanitized.len());
    for (index, value) in sanitized.into_iter().enumerate() {
        match serde_json::from_value::<Tab>(value) {
            Ok(tab) => tabs.push(tab),
            Err(error) => {
                return unreadable(format!("Saved tab {} is invalid: {error}", index + 1));
            }
        }
    }
    tabs.retain(|tab| !is_page_tab(tab) && tab.tab_type != TabType::RemoteFileEditor);

    let active_tab_id = session
        .active_tab_id
        .filter(|id| tabs.iter().any(|tab| tab.id == *id))
        .or_else(|| tabs.first().map(|tab| tab.id.clone()));
    let tab_ids: Vec<String> = tabs.iter().map(|tab| tab.id.clone()).collect();
    let (groups, can_save) = match session.layout.as_ref() {
        None | Some(Value::Null) => (Vec::new(), true),
        Some(layout) => match decode_layout(layout, &tab_ids) {
            Ok(decoded) => decoded,
            Err(reason) => return unreadable(reason),
        },
    };

    WorkspaceLoad::Readable {
        snapshot: WorkspaceSnapshot {
            tabs,
            active_tab_id,
            groups,
        },
        can_save,
    }
}

fn unreadable(reason: impl Into<String>) -> WorkspaceLoad {
    WorkspaceLoad::Unreadable {
        reason: reason.into(),
    }
}

fn decode_layout(
    layout: &Value,
    tab_ids: &[String],
) -> Result<(Vec<RestoredTabGroup>, bool), String> {
    let object = layout
        .as_object()
        .ok_or_else(|| "The saved session layout is not an object.".to_string())?;
    if object.get("version").and_then(Value::as_f64) == Some(2.0) {
        decode_current_layout(layout, object, tab_ids)
    } else {
        decode_dockview_layout(layout, object, tab_ids)
    }
}

fn decode_current_layout(
    layout: &Value,
    object: &serde_json::Map<String, Value>,
    tab_ids: &[String],
) -> Result<(Vec<RestoredTabGroup>, bool), String> {
    let saved_groups = object
        .get("groups")
        .and_then(Value::as_array)
        .ok_or_else(|| "The saved session layout has no groups array.".to_string())?;
    let mut lossless = true;
    let mut raw_ids = Vec::new();
    for group in saved_groups {
        let Some(group) = group.as_object() else {
            lossless = false;
            continue;
        };
        let Some(root) = group.get("root") else {
            lossless = false;
            continue;
        };
        let Some(ids) = current_pane_ids(root) else {
            lossless = false;
            continue;
        };
        let active_ok = group
            .get("activeTabId")
            .and_then(Value::as_str)
            .is_some_and(|active| ids.iter().any(|id| id == active));
        if !active_ok {
            lossless = false;
        }
        match group.get("zoomedTabId") {
            None => {}
            Some(Value::String(zoomed)) if ids.iter().any(|id| id == zoomed) => {}
            Some(_) => lossless = false,
        }
        raw_ids.extend(ids);
    }

    let groups = restore_tab_groups(layout, tab_ids);
    if restored_ids(&groups) != raw_ids {
        lossless = false;
    }
    Ok((groups, lossless))
}

fn current_pane_ids(node: &Value) -> Option<Vec<String>> {
    let object = node.as_object()?;
    match object.get("type")?.as_str()? {
        "pane" => {
            let id = object.get("tabId")?.as_str()?;
            (!id.is_empty()).then(|| vec![id.to_string()])
        }
        "split" => {
            match object.get("orientation")?.as_str()? {
                "horizontal" | "vertical" => {}
                _ => return None,
            }
            let children = object.get("children")?.as_array()?;
            let sizes = object.get("sizes")?.as_array()?;
            if children.len() < 2 || children.len() != sizes.len() {
                return None;
            }
            let mut total = 0.0;
            for size in sizes {
                let size = size.as_f64()?;
                if !size.is_finite() || size <= 0.0 {
                    return None;
                }
                total += size;
            }
            if (total - 1.0).abs() > 1e-6 {
                return None;
            }
            let mut ids = Vec::new();
            for child in children {
                let same_orientation_split = child.as_object().is_some_and(|child| {
                    child.get("type").and_then(Value::as_str) == Some("split")
                        && child.get("orientation").and_then(Value::as_str)
                            == object.get("orientation").and_then(Value::as_str)
                });
                if same_orientation_split {
                    return None;
                }
                ids.extend(current_pane_ids(child)?);
            }
            Some(ids)
        }
        _ => None,
    }
}

fn decode_dockview_layout(
    layout: &Value,
    object: &serde_json::Map<String, Value>,
    tab_ids: &[String],
) -> Result<(Vec<RestoredTabGroup>, bool), String> {
    let grid = object
        .get("grid")
        .and_then(Value::as_object)
        .ok_or_else(|| "The saved session layout is neither version 2 nor Dockview.".to_string())?;
    let root = grid
        .get("root")
        .filter(|root| root.is_object())
        .ok_or_else(|| "The saved Dockview layout has no grid root object.".to_string())?;
    let orientation = if grid.get("orientation").and_then(Value::as_str) == Some("VERTICAL") {
        crate::workspace::tab_groups::SplitOrientation::Vertical
    } else {
        crate::workspace::tab_groups::SplitOrientation::Horizontal
    };
    let mut lossless = true;
    let mut raw_ids = Vec::new();
    let mut group_ids = HashSet::new();
    inspect_dockview_node(
        root,
        orientation,
        &mut raw_ids,
        &mut group_ids,
        &mut lossless,
    );
    match object.get("activeGroup").and_then(Value::as_str) {
        None => {}
        Some(active) if group_ids.contains(active) => {}
        Some(_) => lossless = false,
    }
    if object
        .get("activeGroup")
        .is_some_and(|active| !active.is_string())
    {
        lossless = false;
    }

    let groups = restore_tab_groups(layout, tab_ids);
    let mut saved_ids = raw_ids;
    let mut restored = restored_ids(&groups);
    saved_ids.sort();
    restored.sort();
    if saved_ids != restored {
        lossless = false;
    }
    Ok((groups, lossless))
}

fn inspect_dockview_node(
    node: &Value,
    orientation: crate::workspace::tab_groups::SplitOrientation,
    ids: &mut Vec<String>,
    group_ids: &mut HashSet<String>,
    lossless: &mut bool,
) {
    let Some(object) = node.as_object() else {
        *lossless = false;
        return;
    };
    match object.get("type").and_then(Value::as_str) {
        Some("leaf") => {
            let Some(data) = object.get("data").and_then(Value::as_object) else {
                *lossless = false;
                return;
            };
            let Some(views) = data.get("views").and_then(Value::as_array) else {
                *lossless = false;
                return;
            };
            let mut leaf_views = Vec::new();
            for view in views {
                match view.as_str() {
                    Some(view) if !view.is_empty() => leaf_views.push(view.to_string()),
                    _ => *lossless = false,
                }
            }
            if leaf_views.is_empty() {
                *lossless = false;
            }
            let active_ok = data
                .get("activeView")
                .and_then(Value::as_str)
                .is_some_and(|active| leaf_views.iter().any(|view| view == active));
            if !active_ok {
                *lossless = false;
            }
            if let Some(id) = data.get("id").and_then(Value::as_str) {
                group_ids.insert(id.to_string());
            }
            ids.extend(leaf_views);
        }
        Some("branch") => {
            let Some(children) = object.get("data").and_then(Value::as_array) else {
                *lossless = false;
                return;
            };
            if children.len() < 2 {
                *lossless = false;
            }
            let crossed = match orientation {
                crate::workspace::tab_groups::SplitOrientation::Horizontal => {
                    crate::workspace::tab_groups::SplitOrientation::Vertical
                }
                crate::workspace::tab_groups::SplitOrientation::Vertical => {
                    crate::workspace::tab_groups::SplitOrientation::Horizontal
                }
            };
            for child in children {
                let size_ok = child
                    .as_object()
                    .and_then(|child| child.get("size"))
                    .and_then(Value::as_f64)
                    .is_some_and(|size| size.is_finite() && size > 0.0);
                if !size_ok {
                    *lossless = false;
                }
                inspect_dockview_node(child, crossed, ids, group_ids, lossless);
            }
        }
        _ => *lossless = false,
    }
}

fn restored_ids(groups: &[RestoredTabGroup]) -> Vec<String> {
    groups
        .iter()
        .flat_map(|group| get_pane_ids(&group.root))
        .collect()
}

/// Writes a workspace in the current session format.
pub fn encode_session(snapshot: WorkspaceSnapshot, now_ms: i64) -> SessionData {
    let tabs: Vec<Tab> = snapshot
        .tabs
        .into_iter()
        .filter(|tab| !is_page_tab(tab) && tab.tab_type != TabType::RemoteFileEditor)
        .collect();
    let active_tab_id = snapshot
        .active_tab_id
        .filter(|id| tabs.iter().any(|tab| tab.id == *id))
        .or_else(|| tabs.first().map(|tab| tab.id.clone()));
    let saved_ids: HashSet<String> = tabs.iter().map(|tab| tab.id.clone()).collect();
    let groups: Vec<TabGroup> = snapshot
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| TabGroup {
            id: format!("group-{}", index + 1),
            root: group.root.clone(),
            active_tab_id: group.active_tab_id.clone(),
            zoomed_tab_id: group.zoomed_tab_id.clone(),
        })
        .collect();
    let layout =
        serde_json::to_value(serialize_tab_groups(&groups, &saved_ids)).unwrap_or(Value::Null);
    let encoded_tabs: Vec<Value> = tabs
        .into_iter()
        .map(|tab| {
            let is_active = active_tab_id.as_deref() == Some(tab.id.as_str());
            serde_json::to_value(prepare_tab(tab, is_active)).unwrap_or(Value::Null)
        })
        .collect();

    SessionData {
        tabs: Value::Array(encoded_tabs),
        active_tab_id,
        layout: Some(layout),
        schema_version: SUPPORTED_SESSION_SCHEMA_VERSION,
        last_saved: now_ms,
    }
}

fn prepare_tab(mut tab: Tab, is_active: bool) -> Tab {
    tab.set_is_active(is_active);
    tab.set_has_connected(Some(is_active));
    tab.set_session_nonce(Some(0));
    tab.set_connection_header_pinned(Some(tab.connection_header_pinned.unwrap_or(true)));
    if let Some(connection) = tab.connection.clone() {
        let mut connection = connection;
        connection.set_password(None);
        connection.set_private_key_passphrase(None);
        if let Some(jump_hosts) = connection.jump_hosts.clone() {
            connection.set_jump_hosts(Some(strip_jump_host_secrets(jump_hosts)));
        }
        tab.set_connection(Some(connection));
    }
    if let Some(jump_host) = tab.saved_field("jumpHost").cloned() {
        tab.set_saved_field("jumpHost", Some(strip_jump_host_secrets(jump_host)));
    }
    tab
}

fn strip_jump_host_secrets(value: Value) -> Value {
    match value {
        Value::Array(hosts) => {
            Value::Array(hosts.into_iter().map(strip_jump_host_secrets).collect())
        }
        Value::Object(mut host) => {
            host.remove("password");
            host.remove("privateKeyPassphrase");
            Value::Object(host)
        }
        other => other,
    }
}

/// Reads the saved workspace from the app database.
pub async fn load_workspace_service() -> Result<WorkspaceLoad, String> {
    load::service().await
}

/// Saves a workspace through the session store without pruning snapshots.
///
/// The GPUI client only reads Tauri-era snapshots in this stage, so a metadata
/// save must not delete files it does not own. The Tauri session save still
/// prunes through [`crate::session_restore::save_session_service`].
pub async fn save_workspace_service(snapshot: WorkspaceSnapshot) -> Result<(), String> {
    let saved_at = chrono::Utc::now().timestamp_millis();
    super::write_session_service(encode_session(snapshot, saved_at), false).await
}
