//! Composite tab transitions used by the reducer.

use crate::workspace::pane_templates::{
    LayoutShape, apply_template_to_groups, count_template_panes,
};
use crate::workspace::tab::{ConnectionType, Tab, TabConnection, TabType, is_page_tab};
use crate::workspace::tab_groups::{
    find_group_of_tab, focus_pane_in_groups, get_pane_ids, reconcile_groups,
};
use crate::workspace::tabs_state::{TabsState, ensure_tab_defaults, get_duplicate_tab_title};

pub(super) fn activate(state: &mut TabsState, id: &str) -> bool {
    if !state.tabs.iter().any(|tab| tab.id == id) {
        return false;
    }
    state.active_tab_id = Some(id.to_string());
    for tab in &mut state.tabs {
        tab.set_is_active(tab.id == id);
        if tab.is_active {
            tab.set_has_connected(Some(true));
        }
    }
    true
}

pub(super) fn duplicate(source: &Tab, new_id: &str, tabs: &[Tab]) -> Tab {
    let mut next = source.clone();
    let base = if super::js_value::truthy(source.saved_field("autoTitle")) {
        None
    } else {
        Some(
            source
                .duplicate_base_title
                .clone()
                .unwrap_or_else(|| source.title.clone()),
        )
    };
    next.set_saved_field("id", None);
    next.set_saved_field("isActive", None);
    if next
        .saved_field("connection")
        .is_none_or(serde_json::Value::is_null)
    {
        let mut connection = TabConnection::default();
        connection.set_connection_type(Some(if source.tab_type == TabType::Terminal {
            ConnectionType::Terminal
        } else {
            ConnectionType::Ssh
        }));
        next.set_connection(Some(connection));
    }
    next.set_session_nonce(Some(super::js_value::next_nonce(
        source.saved_field("sessionNonce"),
    )));
    next.set_id(new_id.to_string());
    next.set_title(base.as_ref().map_or_else(
        || source.title.clone(),
        |base| {
            get_duplicate_tab_title(
                base,
                &tabs.iter().map(|tab| tab.title.clone()).collect::<Vec<_>>(),
            )
        },
    ));
    next.set_duplicate_base_title(base);
    next.set_is_active(true);
    next.set_has_connected(Some(true));
    ensure_tab_defaults(&next)
}

pub(super) fn apply_template(
    state: &mut TabsState,
    id: &str,
    root: &LayoutShape,
    new_ids: &[String],
) -> bool {
    let Some(source) = state.tabs.iter().find(|tab| tab.id == id).cloned() else {
        return false;
    };
    let can_copy = !is_page_tab(&source) && source.tab_type != TabType::RemoteFileEditor;
    let mut new_tabs = Vec::new();
    if can_copy {
        for new_id in new_ids {
            let mut available = state.tabs.clone();
            available.extend(new_tabs.clone());
            let mut tab = duplicate(&source, new_id, &available);
            tab.set_is_active(false);
            new_tabs.push(tab);
        }
    }
    let Some(group) = find_group_of_tab(&state.groups, id) else {
        return false;
    };
    if get_pane_ids(&group.root).len() + new_tabs.len() < count_template_panes(root) {
        return false;
    }
    let mut count = state.group_count;
    state.groups = apply_template_to_groups(
        &state.groups,
        id,
        root,
        &new_tabs
            .iter()
            .map(|tab| tab.id.clone())
            .collect::<Vec<_>>(),
        &mut || {
            count += 1;
            format!("group-{count}")
        },
    );
    state.group_count = count;
    state.tabs.extend(new_tabs);
    activate(state, id)
}

pub(super) fn settle(state: &mut TabsState) -> bool {
    let mut count = state.group_count;
    state.groups = reconcile_groups(
        &state.groups,
        &state
            .tabs
            .iter()
            .map(|tab| tab.id.clone())
            .collect::<Vec<_>>(),
        &mut || {
            count += 1;
            format!("group-{count}")
        },
    );
    state.group_count = count;
    if let Some(active) = state.active_tab_id.as_deref().filter(|id| !id.is_empty()) {
        state.groups = focus_pane_in_groups(&state.groups, active);
    }
    let order: Vec<String> = state
        .groups
        .iter()
        .flat_map(|group| get_pane_ids(&group.root))
        .collect();
    let in_order = order
        .iter()
        .enumerate()
        .all(|(index, id)| state.tabs.get(index).is_some_and(|tab| tab.id == *id));
    if in_order {
        return false;
    }
    let by_id: indexmap::IndexMap<String, Tab> = state
        .tabs
        .iter()
        .map(|tab| (tab.id.clone(), tab.clone()))
        .collect();
    state.tabs = order
        .iter()
        .filter_map(|id| by_id.get(id).cloned())
        .collect();
    true
}
