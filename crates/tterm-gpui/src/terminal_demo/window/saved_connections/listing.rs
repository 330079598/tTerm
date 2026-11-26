//! Browsing the saved SSH connections: grouping, searching and ordering.
//!
//! Search and grouping use the shared core rules the Tauri connections panel
//! uses, so both clients find and arrange the same connections. Only display
//! metadata is kept here; no secret is read or retained.

use std::collections::{HashMap, HashSet};

use tterm_core::connections::profile_groups::{
    GroupNode, GroupTree, build_group_tree, normalize_group_path, visible_tree_profiles,
};
use tterm_core::connections::profile_recents::{
    RecentProfile, profile_last_used, read_recent_profiles,
};
use tterm_core::connections::profile_search::{profile_matches_query, rank_profiles};
use tterm_core::profiles::launch::SavedSshProfileSummary;
use tterm_core::profiles::types::{ProfileGroup, SavedProfile};

mod body;
pub(super) mod navigation;
#[cfg(test)]
mod navigation_tests;
mod search;
mod sort_store;
#[cfg(test)]
mod tests;

/// The key of the section that holds the profiles in no group.
pub(super) const UNGROUPED_KEY: &str = "__ungrouped__";
/// The search field and its accessible name.
pub(super) const SEARCH_INPUT_ID: &str = "saved-connections-search";
/// The empty state shown when a search has no matches.
pub(super) const SEARCH_EMPTY_ID: &str = "saved-connections-search-empty";
/// The two order choices in the sort toggle.
pub(super) const SORT_GROUP_ID: &str = "saved-connections-sort-group";
pub(super) const SORT_RECENT_ID: &str = "saved-connections-sort-recent";
/// A group heading is identified by its canonical path.
pub(super) const GROUP_ID_PREFIX: &str = "saved-connections-group-";
/// The Ungrouped heading.
pub(super) const UNGROUPED_ID: &str = "saved-connections-ungrouped";

/// How the picker lists the saved connections.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum ProfileSort {
    /// The group tree, as `profiles.sort.default` names it.
    #[default]
    Group,
    /// One list with the connection used last first.
    Recent,
}

impl ProfileSort {
    /// Reads the stored `profile_sort` value, defaulting to the group tree.
    pub(super) fn from_config(value: &str) -> Self {
        if value == "recent" {
            Self::Recent
        } else {
            Self::Group
        }
    }

    /// The value the shared config file stores.
    pub(super) fn config_value(self) -> &'static str {
        match self {
            Self::Group => "default",
            Self::Recent => "recent",
        }
    }
}

/// The settings read aside from the profiles themselves.
#[derive(Clone, Debug, Default)]
pub(super) struct ListingOptions {
    pub(super) sort: ProfileSort,
    pub(super) recents: Vec<RecentProfile>,
}

/// One saved connection with the metadata-only projection the core rules use.
struct ListedProfile {
    summary: SavedSshProfileSummary,
    search: SavedProfile,
}

impl ListedProfile {
    /// Projects the summary without reading or holding any secret.
    fn new(summary: SavedSshProfileSummary) -> Self {
        let group_path = summary
            .group_path
            .as_deref()
            .map(normalize_group_path)
            .filter(|path| !path.is_empty());
        let search = SavedProfile {
            id: summary.id.clone(),
            name: summary.name.clone(),
            group_id: group_path.clone(),
            group_path: group_path.clone(),
            legacy_group: String::new(),
            connection_type: "ssh".to_string(),
            host: summary.host.clone(),
            port: summary.port,
            username: summary.username.clone(),
            password: None,
            ignore_saved_password: false,
            remember_password: false,
            auth_method: None,
            agent_forward: false,
            private_key_path: None,
            private_key_passphrase: None,
            certificate_path: None,
            keepalive_interval_secs: 15,
            keepalive_count_max: 3,
            server_monitor_visible: false,
            use_jump_host: None,
            legacy_jump_host: None,
            jump_hosts: Vec::new(),
            sudo_autofill: true,
            sudo_password: None,
            clear_sudo_password: false,
            encoding: None,
            terminal_theme: None,
            login_script: None,
            ssh_shell_integration: None,
            legacy_shell_integration: false,
            session_log: None,
            sftp_start_directory: None,
            highlight_set: None,
            proxy: Default::default(),
            tab_icon: None,
            distro: None,
            serial: None,
            local_terminal: None,
        };
        Self { summary, search }
    }

    fn summary(&self) -> &SavedSshProfileSummary {
        &self.summary
    }

    fn group_path(&self) -> Option<&str> {
        self.search.group_path.as_deref()
    }
}

/// The profiles as the picker currently shows them, and the filters that
/// decide that order.
pub(super) struct ProfileListing {
    entries: Vec<ListedProfile>,
    tree: GroupTree,
    query: String,
    collapsed: HashSet<String>,
    sort: ProfileSort,
    last_used: HashMap<String, i64>,
    /// Stable ids in the order the rows are shown and the keyboard walks.
    visible: Vec<String>,
}

impl Default for ProfileListing {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            tree: build_group_tree(&[], &[]),
            query: String::new(),
            collapsed: HashSet::new(),
            sort: ProfileSort::Group,
            last_used: HashMap::new(),
            visible: Vec::new(),
        }
    }
}

impl ProfileListing {
    /// Replaces the profiles while keeping the search, order and collapsed
    /// groups, so an asynchronous reload does not reset the user's view.
    pub(super) fn replace_profiles(&mut self, profiles: &[SavedSshProfileSummary]) {
        self.entries = profiles.iter().cloned().map(ListedProfile::new).collect();
        let groups = projected_groups(&self.entries);
        let searchable: Vec<SavedProfile> = self
            .entries
            .iter()
            .map(|entry| entry.search.clone())
            .collect();
        self.tree = build_group_tree(&groups, &searchable);
        self.refresh_visible();
    }

    /// Drops the rows without forgetting the chosen order or collapsed groups.
    pub(super) fn clear_profiles(&mut self) {
        self.entries.clear();
        self.tree = build_group_tree(&[], &[]);
        self.visible.clear();
    }

    /// Replaces the recent uses without changing a sort the user chose.
    pub(super) fn set_recents(&mut self, recents: &[RecentProfile]) {
        self.last_used = profile_last_used(recents);
        if self.sort == ProfileSort::Recent {
            self.refresh_visible();
        }
    }

    /// Changes the order, reporting whether it changed the visible list.
    pub(super) fn set_sort(&mut self, sort: ProfileSort) -> bool {
        if self.sort == sort {
            return false;
        }
        self.sort = sort;
        self.refresh_visible();
        true
    }

    /// Replaces the search text, reporting whether it changed the list.
    pub(super) fn set_query(&mut self, query: String) -> bool {
        if self.query == query {
            return false;
        }
        self.query = query;
        self.refresh_visible();
        true
    }

    /// Collapses or expands one group, reporting whether it changed.
    pub(super) fn toggle_group(&mut self, key: &str) -> bool {
        let changed = if self.collapsed.remove(key) {
            true
        } else {
            self.collapsed.insert(key.to_string());
            true
        };
        if changed {
            self.refresh_visible();
        }
        changed
    }

    pub(super) fn sort(&self) -> ProfileSort {
        self.sort
    }

    /// Whether a search is entered; groups give way to one ranked list.
    pub(super) fn is_filtering(&self) -> bool {
        !self.query.trim().is_empty()
    }

    /// Whether the rows form one list rather than the group tree.
    pub(super) fn is_flat(&self) -> bool {
        self.is_filtering() || self.sort == ProfileSort::Recent
    }

    pub(super) fn visible_is_empty(&self) -> bool {
        self.visible.is_empty()
    }

    pub(super) fn visible_ids(&self) -> &[String] {
        &self.visible
    }

    pub(super) fn is_visible(&self, id: &str) -> bool {
        self.visible.iter().any(|visible| visible == id)
    }

    pub(super) fn first_visible_id(&self) -> Option<&str> {
        self.visible.first().map(String::as_str)
    }

    /// The stable id to act on: the selected one while it is visible, else
    /// the first row in sight. There is no hidden fallback.
    pub(super) fn active_id(&self, selected: Option<&str>) -> Option<String> {
        match selected {
            Some(id) if self.is_visible(id) => Some(id.to_string()),
            _ => self.visible.first().cloned(),
        }
    }

    pub(super) fn row_index(&self, id: &str) -> Option<usize> {
        self.visible.iter().position(|visible| visible == id)
    }

    fn profile(&self, id: &str) -> Option<&ListedProfile> {
        self.entries.iter().find(|entry| entry.summary.id == id)
    }

    pub(super) fn is_collapsed(&self, key: &str) -> bool {
        self.collapsed.contains(key)
    }

    pub(super) fn roots(&self) -> &[GroupNode] {
        &self.tree.roots
    }

    pub(super) fn ungrouped(&self) -> &[SavedProfile] {
        &self.tree.ungrouped
    }

    fn refresh_visible(&mut self) {
        if self.is_flat() {
            let matches: Vec<SavedProfile> = self
                .entries
                .iter()
                .map(|entry| entry.search.clone())
                .filter(|profile| profile_matches_query(profile, &self.query))
                .collect();
            let empty = HashMap::new();
            let last_used = if self.sort == ProfileSort::Recent {
                &self.last_used
            } else {
                &empty
            };
            self.visible = rank_profiles(&matches, &self.query, last_used)
                .into_iter()
                .map(|profile| profile.id)
                .collect();
        } else {
            self.visible = visible_tree_profiles(&self.tree, &self.collapsed, UNGROUPED_KEY)
                .into_iter()
                .map(|profile| profile.id)
                .collect();
        }
    }
}

/// Builds the tree from the canonical group paths the summaries carry.
///
/// The tree only needs nested names for the picker, and the paths are already
/// canonical, so each prefix is used as the group id. The shared tree builder
/// then decides siblings, counts, repair of missing parents and display paths.
fn projected_groups(profiles: &[ListedProfile]) -> Vec<ProfileGroup> {
    let mut groups: HashMap<String, ProfileGroup> = HashMap::new();
    for profile in profiles {
        let Some(path) = profile.group_path() else {
            continue;
        };
        let mut prefix = String::new();
        for name in path
            .split('/')
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            let id = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix} / {name}")
            };
            let parent_id = (!prefix.is_empty()).then(|| prefix.clone());
            groups.entry(id.clone()).or_insert_with(|| ProfileGroup {
                id: id.clone(),
                name: name.to_string(),
                parent_id,
            });
            prefix = id;
        }
    }
    groups.into_values().collect()
}

/// Reads the saved order and recent uses. A failed optional read falls back
/// to the group tree and no recent ordering instead of blocking browsing.
pub(super) fn load_options() -> ListingOptions {
    let sort = sort_store::load().unwrap_or_else(|error| {
        eprintln!("tterm-gpui: failed to read the profile order: {error}");
        ProfileSort::Group
    });
    let recents = read_recent_profiles().unwrap_or_else(|error| {
        eprintln!("tterm-gpui: failed to read recent saved connections: {error}");
        Vec::new()
    });
    ListingOptions { sort, recents }
}
