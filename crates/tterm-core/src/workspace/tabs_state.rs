//! Pure tab-list transitions, from `src/lib/tabsState.ts`.
//!
//! The reducer, its action enum and the state it holds are React state
//! management. The native workspace entity owns that state instead; the
//! transitions here are the parts that do not depend on it.

use std::collections::HashSet;

use super::tab::{Tab, TabType, is_page_tab};

/// How removing tabs chooses the tab to focus next.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoveTabsOptions {
    /// The tab to focus when it survives.
    pub preferred_active_tab_id: Option<String>,
    /// Whether the preferred tab wins even when the active tab survives.
    pub activate_preferred: bool,
}

/// The tab list after removing tabs.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoveTabsResult {
    /// The remaining tabs.
    pub tabs: Vec<Tab>,
    /// The tab to focus, or `None` when no tab is left.
    pub active_tab_id: Option<String>,
}

/// Removes tabs and picks the tab to focus next.
pub fn remove_tabs_from_state(
    tabs: Vec<Tab>,
    active_tab_id: Option<&str>,
    ids: &[String],
    options: &RemoveTabsOptions,
) -> RemoveTabsResult {
    let ids_to_remove: HashSet<&str> = ids.iter().map(String::as_str).collect();
    if !tabs
        .iter()
        .any(|tab| ids_to_remove.contains(tab.id.as_str()))
    {
        return RemoveTabsResult {
            tabs,
            active_tab_id: active_tab_id.map(str::to_owned),
        };
    }

    let active_tab_index = active_tab_id.and_then(|active| {
        tabs.iter()
            .position(|tab| tab.id == active)
            .map(|index| index as isize)
    });
    let remaining_tabs: Vec<Tab> = tabs
        .into_iter()
        .filter(|tab| !ids_to_remove.contains(tab.id.as_str()))
        .collect();
    if remaining_tabs.is_empty() {
        return RemoveTabsResult {
            tabs: Vec::new(),
            active_tab_id: None,
        };
    }

    let preferred_tab = options
        .preferred_active_tab_id
        .as_deref()
        .and_then(|preferred| remaining_tabs.iter().find(|tab| tab.id == preferred));
    let current_active_tab =
        active_tab_id.and_then(|active| remaining_tabs.iter().find(|tab| tab.id == active));
    let fallback_index = active_tab_index
        .unwrap_or(-1)
        .max(0)
        .min(remaining_tabs.len() as isize - 1) as usize;
    let fallback_tab = &remaining_tabs[fallback_index];
    let next_active_tab = if options.activate_preferred {
        preferred_tab
    } else {
        current_active_tab
    }
    .or(preferred_tab)
    .or(current_active_tab)
    .unwrap_or(fallback_tab);
    let next_active_tab_id = next_active_tab.id.clone();

    RemoveTabsResult {
        tabs: remaining_tabs
            .into_iter()
            .map(|mut tab| {
                tab.is_active = tab.id == next_active_tab_id;
                if tab.is_active {
                    tab.has_connected = Some(true);
                }
                tab
            })
            .collect(),
        active_tab_id: Some(next_active_tab_id),
    }
}

/// Fills in the defaults a newly restored or created tab needs.
pub fn ensure_tab_defaults(tab: &Tab) -> Tab {
    let mut next = tab.clone();
    if is_page_tab(tab) || tab.tab_type == TabType::RemoteFileEditor {
        next.has_connected = Some(true);
        next.session_nonce = Some(0);
        if tab.tab_type != TabType::RemoteFileEditor {
            next.connection = None;
        }
        return next;
    }
    next.has_connected = Some(tab.has_connected.unwrap_or(tab.is_active));
    next.session_nonce = Some(tab.session_nonce.unwrap_or(0));
    next.connection_header_pinned = Some(tab.connection_header_pinned.unwrap_or(true));
    next
}

/// Numbers a copy of a tab: `web` becomes `web-2`, then `web-3`.
pub fn get_duplicate_tab_title(base: &str, existing_titles: &[String]) -> String {
    let titles: HashSet<&str> = existing_titles.iter().map(String::as_str).collect();
    let mut index = 2u64;
    loop {
        let candidate = format!("{base}-{index}");
        if !titles.contains(candidate.as_str()) {
            return candidate;
        }
        index = index.saturating_add(1);
    }
}
