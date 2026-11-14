//! The profile-group tree the connection and settings screens show, from
//! `src/lib/profileGroups.ts`.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::js::trim_js;
use crate::profiles::groups::group_paths;
use crate::profiles::types::{ProfileGroup, SavedProfile};

/// Separates the names of a group path as typed.
pub const GROUP_PATH_SEPARATOR: char = '/';

/// A group in the tree, with the groups and profiles filed in it.
#[derive(Debug, Clone)]
pub struct GroupNode {
    /// The group this node shows.
    pub group: ProfileGroup,
    /// How many groups it sits in: zero at the top.
    pub depth: usize,
    /// The names from the top down to this group, joined for display.
    pub path: String,
    /// The groups filed directly inside this one.
    pub children: Vec<GroupNode>,
    /// The profiles filed directly in this group.
    pub profiles: Vec<SavedProfile>,
    /// The profiles in this group and in every group inside it.
    pub total: usize,
}

/// The groups arranged as the screens show them.
#[derive(Debug, Clone)]
pub struct GroupTree {
    /// The top-level groups, siblings by name.
    pub roots: Vec<GroupNode>,
    /// Profiles in no group, or in a group that no longer exists.
    pub ungrouped: Vec<SavedProfile>,
    /// Every node by group id.
    pub nodes: HashMap<String, GroupNode>,
}

/// A typed group path as the backend stores it: names trimmed, empty ones
/// dropped, and the rest joined for display.
pub fn normalize_group_path(path: &str) -> String {
    path.split(GROUP_PATH_SEPARATOR)
        .map(trim_js)
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>()
        .join(" / ")
}

/// Compares sibling names the way the WebView's default `localeCompare`
/// does for ASCII, then uses the exact text to break equal folded names.
///
/// Names outside ASCII fall back to code-point order, as the other migrated
/// frontend sorts do.
fn compare_text(left: &str, right: &str) -> Ordering {
    left.chars()
        .map(|character| character.to_ascii_lowercase())
        .cmp(
            right
                .chars()
                .map(|character| character.to_ascii_lowercase()),
        )
        .then_with(|| left.cmp(right))
}

fn compare_groups(left: &ProfileGroup, right: &ProfileGroup) -> Ordering {
    compare_text(&left.name, &right.name).then_with(|| left.id.cmp(&right.id))
}

fn place_group(
    group: ProfileGroup,
    children: &HashMap<Option<String>, Vec<ProfileGroup>>,
    placed: &mut HashSet<String>,
    parent_edges: &mut HashMap<String, Option<String>>,
    parent_id: Option<&str>,
) -> GroupNode {
    let id = group.id.clone();
    placed.insert(id.clone());
    parent_edges.insert(id.clone(), parent_id.map(str::to_owned));
    let mut descendants = Vec::new();
    for child in children.get(&Some(id.clone())).into_iter().flatten() {
        if !placed.contains(&child.id) {
            descendants.push(place_group(
                child.clone(),
                children,
                placed,
                parent_edges,
                Some(group.id.as_str()),
            ));
        }
    }
    GroupNode {
        group,
        depth: 0,
        path: String::new(),
        children: descendants,
        profiles: Vec::new(),
        total: 0,
    }
}

fn set_paths(node: &mut GroupNode, paths: &HashMap<String, String>, depth: usize) {
    node.depth = depth;
    node.path = paths.get(&node.group.id).cloned().unwrap_or_default();
    for child in &mut node.children {
        set_paths(child, paths, depth + 1);
    }
}

fn count_profiles(node: &mut GroupNode) -> usize {
    node.total = node.profiles.len() + node.children.iter_mut().map(count_profiles).sum::<usize>();
    node.total
}

/// Builds the group tree, siblings by name and each holding its profiles in
/// the order given. A group whose parent is gone sits at the top, as does the
/// group with the smallest id in a loop.
pub fn build_group_tree(groups: &[ProfileGroup], profiles: &[SavedProfile]) -> GroupTree {
    let known: HashSet<&str> = groups.iter().map(|group| group.id.as_str()).collect();
    let mut ordered = groups.to_vec();
    ordered.sort_by(compare_groups);
    let mut children: HashMap<Option<String>, Vec<ProfileGroup>> = HashMap::new();
    for group in ordered {
        let parent = group
            .parent_id
            .as_deref()
            .filter(|parent| *parent != group.id && known.contains(parent))
            .map(str::to_owned);
        children.entry(parent).or_default().push(group);
    }

    let mut placed = HashSet::new();
    let mut parent_edges = HashMap::new();
    let mut roots = Vec::new();
    for group in children.get(&None).into_iter().flatten() {
        if !placed.contains(&group.id) {
            roots.push(place_group(
                group.clone(),
                &children,
                &mut placed,
                &mut parent_edges,
                None,
            ));
        }
    }

    loop {
        let mut unplaced: Vec<&ProfileGroup> = groups
            .iter()
            .filter(|group| !placed.contains(&group.id))
            .collect();
        unplaced.sort_by(|left, right| left.id.cmp(&right.id));
        let Some(cut) = unplaced.first() else {
            break;
        };
        roots.push(place_group(
            (*cut).clone(),
            &children,
            &mut placed,
            &mut parent_edges,
            None,
        ));
    }
    roots.sort_by(|left, right| compare_groups(&left.group, &right.group));

    let repaired_groups: Vec<ProfileGroup> = groups
        .iter()
        .map(|group| ProfileGroup {
            parent_id: parent_edges.get(&group.id).cloned().flatten(),
            ..group.clone()
        })
        .collect();
    let paths = group_paths(&repaired_groups);
    for root in &mut roots {
        set_paths(root, &paths, 0);
    }

    let mut ungrouped = Vec::new();
    for profile in profiles {
        fn find_node<'a>(roots: &'a mut [GroupNode], id: &str) -> Option<&'a mut GroupNode> {
            for root in roots {
                if root.group.id == id {
                    return Some(root);
                }
                if let Some(node) = find_node(&mut root.children, id) {
                    return Some(node);
                }
            }
            None
        }

        match profile
            .group_id
            .as_deref()
            .and_then(|id| find_node(&mut roots, id))
        {
            Some(node) => node.profiles.push(profile.clone()),
            None => ungrouped.push(profile.clone()),
        }
    }
    for root in &mut roots {
        count_profiles(root);
    }

    let mut by_id = HashMap::new();
    fn index_nodes(nodes: &[GroupNode], by_id: &mut HashMap<String, GroupNode>) {
        for node in nodes {
            by_id.insert(node.group.id.clone(), node.clone());
            index_nodes(&node.children, by_id);
        }
    }
    index_nodes(&roots, &mut by_id);

    GroupTree {
        roots,
        ungrouped,
        nodes: by_id,
    }
}

/// The ids of a group and of every group inside it.
pub fn group_and_descendant_ids(tree: &GroupTree, group_id: &str) -> HashSet<String> {
    fn visit(node: &GroupNode, ids: &mut HashSet<String>) {
        ids.insert(node.group.id.clone());
        for child in &node.children {
            visit(child, ids);
        }
    }

    let mut ids = HashSet::new();
    if let Some(node) = tree.nodes.get(group_id) {
        visit(node, &mut ids);
    }
    ids
}

/// The groups in the order the tree lists them, skipping what sits in a
/// collapsed group.
pub fn visible_group_nodes(tree: &GroupTree, collapsed_ids: &HashSet<String>) -> Vec<GroupNode> {
    fn visit(node: &GroupNode, collapsed: &HashSet<String>, result: &mut Vec<GroupNode>) {
        result.push(node.clone());
        if !collapsed.contains(&node.group.id) {
            for child in &node.children {
                visit(child, collapsed, result);
            }
        }
    }

    let mut result = Vec::new();
    for root in &tree.roots {
        visit(root, collapsed_ids, &mut result);
    }
    result
}

/// The profiles in the order the tree shows them: in each group the groups
/// inside it first, then its own profiles; ungrouped profiles last.
pub fn visible_tree_profiles(
    tree: &GroupTree,
    collapsed_keys: &HashSet<String>,
    ungrouped_key: &str,
) -> Vec<SavedProfile> {
    fn visit(node: &GroupNode, collapsed: &HashSet<String>, result: &mut Vec<SavedProfile>) {
        if collapsed.contains(&node.group.id) {
            return;
        }
        for child in &node.children {
            visit(child, collapsed, result);
        }
        result.extend(node.profiles.iter().cloned());
    }

    let mut result = Vec::new();
    for root in &tree.roots {
        visit(root, collapsed_keys, &mut result);
    }
    if !collapsed_keys.contains(ungrouped_key) {
        result.extend(tree.ungrouped.iter().cloned());
    }
    result
}

/// The ids of the groups that hold no profiles, in them or below them.
pub fn empty_group_ids(tree: &GroupTree) -> HashSet<String> {
    tree.nodes
        .values()
        .filter(|node| node.total == 0)
        .map(|node| node.group.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{
        GroupNode, build_group_tree, empty_group_ids, group_and_descendant_ids,
        normalize_group_path, visible_group_nodes, visible_tree_profiles,
    };
    use crate::profiles::types::{ProfileGroup, SavedProfile};

    const UNGROUPED: &str = "__ungrouped__";

    fn group(id: &str, name: &str, parent_id: Option<&str>) -> ProfileGroup {
        ProfileGroup {
            id: id.to_owned(),
            name: name.to_owned(),
            parent_id: parent_id.map(str::to_owned),
        }
    }

    fn profile(id: &str, group_id: Option<&str>) -> SavedProfile {
        serde_json::from_value(json!({
            "id": id,
            "name": id,
            "group_id": group_id,
            "connection_type": "ssh",
            "keepalive_interval_secs": 15,
            "keepalive_count_max": 3
        }))
        .expect("a test profile")
    }

    fn outline(nodes: &[GroupNode]) -> Value {
        Value::Array(
            nodes
                .iter()
                .map(|node| {
                    json!([
                        node.path,
                        node.total,
                        node.profiles
                            .iter()
                            .map(|item| &item.id)
                            .collect::<Vec<_>>(),
                        outline(&node.children)
                    ])
                })
                .collect(),
        )
    }

    fn ids(profiles: &[SavedProfile]) -> Vec<&str> {
        profiles.iter().map(|profile| profile.id.as_str()).collect()
    }

    #[test]
    fn trims_the_names_and_drops_empty_ones() {
        assert_eq!(normalize_group_path(" prod /web// "), "prod / web");
        assert_eq!(normalize_group_path(" / "), "");
    }

    #[test]
    fn nests_groups_by_name_and_counts_the_profiles_inside_each() {
        let tree = build_group_tree(
            &[
                group("w", "web", Some("p")),
                group("p", "prod", None),
                group("a", "api", Some("p")),
                group("d", "dev", None),
            ],
            &[
                profile("1", Some("w")),
                profile("2", Some("p")),
                profile("3", None),
                profile("4", Some("gone")),
                profile("5", Some("w")),
            ],
        );
        assert_eq!(
            outline(&tree.roots),
            json!([
                ["dev", 0, [], []],
                [
                    "prod",
                    3,
                    ["2"],
                    [["prod / api", 0, [], []], ["prod / web", 2, ["1", "5"], []]]
                ]
            ])
        );
        assert_eq!(ids(&tree.ungrouped), ["3", "4"]);
        assert_eq!(tree.nodes["w"].depth, 1);
    }

    #[test]
    fn puts_a_group_whose_parent_is_gone_or_in_a_loop_at_the_top() {
        let tree = build_group_tree(
            &[
                group("o", "orphan", Some("gone")),
                group("x", "x", Some("y")),
                group("y", "y", Some("x")),
            ],
            &[],
        );
        assert_eq!(
            outline(&tree.roots),
            json!([["orphan", 0, [], []], ["x", 0, [], [["x / y", 0, [], []]]]])
        );
    }

    fn sample_tree() -> super::GroupTree {
        build_group_tree(
            &[
                group("p", "prod", None),
                group("w", "web", Some("p")),
                group("d", "dev", None),
            ],
            &[
                profile("1", Some("w")),
                profile("2", Some("p")),
                profile("3", None),
                profile("4", Some("d")),
            ],
        )
    }

    #[test]
    fn orders_ascii_sibling_names_like_locale_compare() {
        let tree = build_group_tree(
            &[
                group("w", "Web", None),
                group("a", "api", None),
                group("z", "Zebra", None),
                group("p", "prod", None),
            ],
            &[],
        );
        assert_eq!(
            tree.roots
                .iter()
                .map(|node| node.group.name.as_str())
                .collect::<Vec<_>>(),
            ["api", "prod", "Web", "Zebra"]
        );
    }

    #[test]
    fn lists_a_groups_own_id_and_those_inside_it() {
        let tree = sample_tree();
        let mut found = group_and_descendant_ids(&tree, "p")
            .into_iter()
            .collect::<Vec<_>>();
        found.sort();
        assert_eq!(found, ["p", "w"]);
        assert!(group_and_descendant_ids(&tree, "missing").is_empty());
    }

    #[test]
    fn lists_the_groups_in_sight_skipping_those_in_a_collapsed_one() {
        let tree = sample_tree();
        let visible = |collapsed: &[&str]| {
            visible_group_nodes(
                &tree,
                &collapsed.iter().map(|id| (*id).to_owned()).collect(),
            )
            .into_iter()
            .map(|node| node.group.id)
            .collect::<Vec<_>>()
        };
        assert_eq!(visible(&[]), ["d", "p", "w"]);
        assert_eq!(visible(&["p"]), ["d", "p"]);
    }

    #[test]
    fn lists_the_profiles_in_sight_in_tree_order() {
        let tree = sample_tree();
        let visible = |collapsed: &[&str]| {
            let collapsed = collapsed.iter().map(|id| (*id).to_owned()).collect();
            ids(&visible_tree_profiles(&tree, &collapsed, UNGROUPED))
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        assert_eq!(visible(&[]), ["4", "1", "2", "3"]);
        assert_eq!(visible(&["p", UNGROUPED]), ["4"]);
        assert_eq!(visible(&["w"]), ["4", "2", "3"]);
    }

    #[test]
    fn lists_groups_that_hold_no_profiles_counting_the_groups_inside_them() {
        let tree = build_group_tree(
            &[
                group("p", "prod", None),
                group("w", "web", Some("p")),
                group("e", "empty", Some("p")),
                group("d", "dev", None),
            ],
            &[profile("1", Some("w"))],
        );
        let mut empty = empty_group_ids(&tree).into_iter().collect::<Vec<_>>();
        empty.sort();
        assert_eq!(empty, ["d", "e"]);
    }
}
