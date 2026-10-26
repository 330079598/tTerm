//! Profile groups: a tree of named groups that profiles are filed in.
//!
//! The commands that change groups keep the tree a tree and sibling names
//! distinct. Sync and backups write groups as they come, so [`repair`] mends
//! what they can leave behind.

use super::storage::{list_saved_profiles, upsert_profile};
use super::types::{ProfileGroup, SavedProfile};
use crate::db::sql_error;
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};

/// Separates the names in a group path.
const PATH_SEPARATOR: char = '/';
/// Joins the names of a group path for display; it reads back as the same path.
const PATH_JOINER: &str = " / ";

/// Every group with when it last changed, siblings by name.
pub(crate) fn group_rows(
    connection: &Connection,
) -> Result<Vec<(ProfileGroup, Option<i64>)>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, parent_id, updated_at FROM profile_groups \
             ORDER BY lower(name), name, id",
        )
        .map_err(sql_error("Failed to read profile groups"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                ProfileGroup {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                },
                row.get(3)?,
            ))
        })
        .map_err(sql_error("Failed to read profile groups"))?;
    rows.collect::<Result<_, _>>()
        .map_err(sql_error("Failed to read profile groups"))
}

/// Every group, siblings by name.
pub(crate) fn list_groups(connection: &Connection) -> Result<Vec<ProfileGroup>, String> {
    Ok(group_rows(connection)?
        .into_iter()
        .map(|(group, _)| group)
        .collect())
}

/// Saves a group as given, over the one with its id.
pub(crate) fn put_group(connection: &Connection, group: &ProfileGroup) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO profile_groups (id, name, parent_id) VALUES (?1, ?2, ?3) \
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, parent_id = excluded.parent_id",
            params![group.id, group.name, group.parent_id],
        )
        .map_err(sql_error("Failed to save profile group"))?;
    Ok(())
}

/// Removes the group row only; see [`delete_group`] for what the user deletes.
pub(crate) fn delete_group_row(connection: &Connection, id: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM profile_groups WHERE id = ?1", [id])
        .map_err(sql_error("Failed to delete profile group"))?;
    Ok(())
}

/// Replaces every group as given, leaving out any without an id or a name.
/// Callers run [`repair`] once the profiles that travel with the groups are
/// written too.
pub(crate) fn replace_groups(
    connection: &Connection,
    groups: &[ProfileGroup],
) -> Result<(), String> {
    connection
        .execute("DELETE FROM profile_groups", [])
        .map_err(sql_error("Failed to clear profile groups"))?;
    for group in groups
        .iter()
        .filter(|group| !group.id.is_empty() && !group.name.trim().is_empty())
    {
        put_group(connection, group)?;
    }
    Ok(())
}

/// `groups` and a top-level group for every name an older profile was filed
/// under that they lack: older versions made a group of every name a
/// profile carried.
pub(crate) fn with_legacy_groups(
    mut groups: Vec<ProfileGroup>,
    profiles: &[SavedProfile],
) -> Vec<ProfileGroup> {
    for profile in profiles.iter().filter(|profile| profile.group_id.is_none()) {
        let name = profile.legacy_group.trim();
        if !name.is_empty() && !groups.iter().any(|group| group.id == name) {
            groups.push(ProfileGroup {
                id: name.to_string(),
                name: name.to_string(),
                parent_id: None,
            });
        }
    }
    groups
}

/// A group name as stored: trimmed, not empty, and free of the path separator.
fn group_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Group name is required".to_string());
    }
    if name.contains(PATH_SEPARATOR) {
        return Err(format!(
            "A group name cannot contain \"{PATH_SEPARATOR}\"; it separates the groups of a path"
        ));
    }
    Ok(name.to_string())
}

fn find_group<'a>(groups: &'a [ProfileGroup], id: &str) -> Result<&'a ProfileGroup, String> {
    groups
        .iter()
        .find(|group| group.id == id)
        .ok_or_else(|| "This group no longer exists".to_string())
}

/// Fails when a group other than `except` under `parent_id` has `name`.
fn ensure_name_free(
    groups: &[ProfileGroup],
    parent_id: Option<&str>,
    name: &str,
    except: Option<&str>,
) -> Result<(), String> {
    let taken = groups.iter().any(|group| {
        group.parent_id.as_deref() == parent_id
            && group.name == name
            && Some(group.id.as_str()) != except
    });
    if taken {
        return Err(format!("A group named \"{name}\" already exists here"));
    }
    Ok(())
}

fn parent_map(groups: &[ProfileGroup]) -> HashMap<&str, Option<&str>> {
    groups
        .iter()
        .map(|group| (group.id.as_str(), group.parent_id.as_deref()))
        .collect()
}

/// The ids of a group and of the groups above it, nearest first. Stops at a
/// parent that is gone or at a loop.
fn lineage<'a>(parents: &HashMap<&'a str, Option<&'a str>>, id: &'a str) -> Vec<&'a str> {
    let mut lineage = Vec::new();
    let mut current = Some(id);
    while let Some(id) = current {
        if lineage.contains(&id) {
            break;
        }
        let Some(parent) = parents.get(id) else {
            break;
        };
        lineage.push(id);
        current = *parent;
    }
    lineage
}

/// Each group's path from the top, joined for display.
pub(crate) fn group_paths(groups: &[ProfileGroup]) -> HashMap<String, String> {
    let parents = parent_map(groups);
    let names: HashMap<&str, &str> = groups
        .iter()
        .map(|group| (group.id.as_str(), group.name.as_str()))
        .collect();
    groups
        .iter()
        .map(|group| {
            let path = lineage(&parents, &group.id)
                .into_iter()
                .rev()
                .filter_map(|id| names.get(id).copied())
                .collect::<Vec<_>>();
            (group.id.clone(), path.join(PATH_JOINER))
        })
        .collect()
}

pub(crate) fn create_group(
    connection: &Connection,
    name: &str,
    parent_id: Option<&str>,
) -> Result<ProfileGroup, String> {
    let name = group_name(name)?;
    let groups = list_groups(connection)?;
    if let Some(parent_id) = parent_id {
        find_group(&groups, parent_id)?;
    }
    ensure_name_free(&groups, parent_id, &name, None)?;
    let group = ProfileGroup {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        parent_id: parent_id.map(str::to_string),
    };
    put_group(connection, &group)?;
    Ok(group)
}

pub(crate) fn rename_group(connection: &Connection, id: &str, name: &str) -> Result<(), String> {
    let name = group_name(name)?;
    let groups = list_groups(connection)?;
    let group = find_group(&groups, id)?;
    ensure_name_free(&groups, group.parent_id.as_deref(), &name, Some(id))?;
    put_group(
        connection,
        &ProfileGroup {
            name,
            ..group.clone()
        },
    )
}

/// Moves a group, with everything in it, under `parent_id` or to the top.
pub(crate) fn move_group(
    connection: &Connection,
    id: &str,
    parent_id: Option<&str>,
) -> Result<(), String> {
    let groups = list_groups(connection)?;
    let group = find_group(&groups, id)?;
    if let Some(parent_id) = parent_id {
        find_group(&groups, parent_id)?;
        if lineage(&parent_map(&groups), parent_id).contains(&id) {
            return Err("A group cannot move into itself or into a group inside it".to_string());
        }
    }
    ensure_name_free(&groups, parent_id, &group.name, Some(id))?;
    put_group(
        connection,
        &ProfileGroup {
            parent_id: parent_id.map(str::to_string),
            ..group.clone()
        },
    )
}

/// Deletes a group. Its profiles and the groups in it move up to its parent,
/// so deleting a group never deletes a profile.
pub(crate) fn delete_group(connection: &Connection, id: &str) -> Result<(), String> {
    let groups = list_groups(connection)?;
    let parent_id = find_group(&groups, id)?.parent_id.clone();
    for child in groups
        .iter()
        .filter(|group| group.parent_id.as_deref() == Some(id))
    {
        put_group(
            connection,
            &ProfileGroup {
                parent_id: parent_id.clone(),
                ..child.clone()
            },
        )?;
    }
    for mut profile in list_saved_profiles(connection)? {
        if profile.group_id.as_deref() == Some(id) {
            profile.group_id = parent_id.clone();
            upsert_profile(connection, &profile)?;
        }
    }
    delete_group_row(connection, id)
}

/// The group a path such as `prod / web` names, creating the groups along it
/// that are missing; `None` for an empty path.
pub(crate) fn resolve_path(connection: &Connection, path: &str) -> Result<Option<String>, String> {
    let mut groups = list_groups(connection)?;
    let mut parent_id: Option<String> = None;
    for name in path
        .split(PATH_SEPARATOR)
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        let existing = groups
            .iter()
            .find(|group| group.parent_id == parent_id && group.name == name)
            .map(|group| group.id.clone());
        let id = match existing {
            Some(id) => id,
            None => {
                let group = create_group(connection, name, parent_id.as_deref())?;
                let id = group.id.clone();
                groups.push(group);
                id
            }
        };
        parent_id = Some(id);
    }
    Ok(parent_id)
}

/// Mends what sync and backups can leave behind. A group whose parent is
/// gone moves to the top. A loop of groups, made by two devices moving
/// groups into each other, is cut at its group with the smallest id, so
/// every device cuts it the same way. A profile filed in a group that is
/// gone becomes ungrouped.
pub(crate) fn repair(connection: &Connection) -> Result<(), String> {
    let groups = list_groups(connection)?;
    let ids: HashSet<&str> = groups.iter().map(|group| group.id.as_str()).collect();
    let mut parents: HashMap<&str, Option<&str>> = groups
        .iter()
        .map(|group| {
            let parent = group
                .parent_id
                .as_deref()
                .filter(|parent| ids.contains(parent));
            (group.id.as_str(), parent)
        })
        .collect();
    let mut starts = ids.iter().copied().collect::<Vec<_>>();
    starts.sort_unstable();
    for start in starts {
        let mut chain: Vec<&str> = Vec::new();
        let mut current = Some(start);
        while let Some(id) = current {
            if let Some(index) = chain.iter().position(|seen| *seen == id) {
                if let Some(&cut) = chain[index..].iter().min() {
                    parents.insert(cut, None);
                }
                break;
            }
            chain.push(id);
            current = parents.get(id).copied().flatten();
        }
    }
    for group in &groups {
        let parent = parents.get(group.id.as_str()).copied().flatten();
        if group.parent_id.as_deref() != parent {
            put_group(
                connection,
                &ProfileGroup {
                    parent_id: parent.map(str::to_string),
                    ..group.clone()
                },
            )?;
        }
    }
    for mut profile in list_saved_profiles(connection)? {
        if profile
            .group_id
            .as_deref()
            .is_some_and(|id| !ids.contains(id))
        {
            profile.group_id = None;
            upsert_profile(connection, &profile)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
