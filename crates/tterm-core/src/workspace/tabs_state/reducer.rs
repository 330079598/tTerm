//! The action cases and final reconciliation of `tabsReducer`.

use crate::workspace::local_tab_title::with_distinct_local_titles;
use crate::workspace::tab::{TabType, is_page_tab};
use crate::workspace::tab_groups::{
    break_out_pane, find_group_of_tab, get_fallback_tab_id, get_pane_ids, insert_next_to,
    merge_group, move_group, pane_leaf, place_pane, remove_tabs_from_groups, resize_split,
    toggle_zoom, ungroup_panes,
};
use crate::workspace::tab_groups_persistence::restore_tab_groups;
use crate::workspace::tabs_state::action::TabsAction;
use crate::workspace::tabs_state::reopen::{place_reopened_tabs, reopened_active_tab_id};
use crate::workspace::tabs_state::transitions::{activate, apply_template, duplicate, settle};
use crate::workspace::tabs_state::{TabsState, ensure_tab_defaults, remove_tabs_from_state};

pub(super) fn reduce(mut state: TabsState, action: TabsAction) -> TabsState {
    let changed = apply(&mut state, action);
    let reordered = settle(&mut state);
    if changed || reordered {
        state.tabs = with_distinct_local_titles(&state.tabs, None);
    }
    state
}

fn apply(state: &mut TabsState, action: TabsAction) -> bool {
    match action {
        TabsAction::Restore {
            tabs,
            active_tab_id,
            layout,
        } => restore(state, tabs, active_tab_id, layout),
        TabsAction::Add(mut tab) => {
            for previous in &mut state.tabs {
                previous.is_active = false;
                previous.forget_saved_field("isActive");
            }
            state.active_tab_id = Some(tab.id.clone());
            tab.is_active = true;
            tab.has_connected = Some(true);
            tab.forget_saved_field("isActive");
            tab.forget_saved_field("hasConnected");
            state.tabs.push(tab);
            true
        }
        TabsAction::OpenPage(tab) => open_page(state, tab),
        TabsAction::RenamePage { tab_type, title } => {
            let mut changed = false;
            for tab in &mut state.tabs {
                if tab.tab_type == tab_type && tab.title != title {
                    tab.title = title.clone();
                    changed = true;
                }
            }
            changed
        }
        TabsAction::Remove { ids, options } => remove(state, &ids, options),
        TabsAction::SetActive(id) => activate(state, &id),
        TabsAction::Move {
            from_index,
            to_index,
        } => {
            state.groups = move_group(&state.groups, from_index, to_index);
            false
        }
        TabsAction::Duplicate {
            source_id,
            new_id,
            split,
        } => duplicate_action(state, &source_id, &new_id, split),
        TabsAction::Rename { id, title } => {
            for tab in state.tabs.iter_mut().filter(|tab| tab.id == id) {
                tab.title = title.clone();
                tab.duplicate_base_title = None;
                tab.auto_title = None;
                tab.forget_saved_field("duplicateBaseTitle");
                tab.forget_saved_field("autoTitle");
            }
            true
        }
        TabsAction::Update { id, updater } => {
            for tab in state.tabs.iter_mut().filter(|tab| tab.id == id) {
                *tab = ensure_tab_defaults(&updater(tab));
                tab.is_active = state.active_tab_id.as_deref() == Some(tab.id.as_str());
                tab.forget_saved_field("isActive");
            }
            true
        }
        TabsAction::PlacePane {
            id,
            target_id,
            direction,
        } => {
            state.groups = place_pane(&state.groups, &id, &target_id, direction);
            activate(state, &id)
        }
        TabsAction::MergeGroup {
            id,
            target_id,
            direction,
        } => merge(state, &id, &target_id, direction),
        TabsAction::BreakOut { id, index } => {
            if find_group_of_tab(&state.groups, &id)
                .is_none_or(|group| get_pane_ids(&group.root).len() < 2)
            {
                return false;
            }
            state.group_count += 1;
            state.groups = break_out_pane(
                &state.groups,
                &id,
                &format!("group-{}", state.group_count),
                index,
            );
            activate(state, &id)
        }
        TabsAction::Ungroup(id) => {
            let mut count = state.group_count;
            state.groups = ungroup_panes(&state.groups, &id, &mut || {
                count += 1;
                format!("group-{count}")
            });
            state.group_count = count;
            false
        }
        TabsAction::ToggleZoom(id) => {
            state.groups = toggle_zoom(&state.groups, &id);
            activate(state, &id)
        }
        TabsAction::ResizeSplit {
            group_id,
            path,
            sizes,
        } => {
            for group in state.groups.iter_mut().filter(|group| group.id == group_id) {
                group.root = resize_split(&group.root, &path, &sizes);
            }
            false
        }
        TabsAction::ApplyTemplate { id, root, new_ids } => {
            apply_template(state, &id, &root, &new_ids)
        }
        TabsAction::Reopen { tabs, layout } => reopen(state, tabs, layout),
    }
}

fn restore(
    state: &mut TabsState,
    tabs: Vec<crate::workspace::tab::Tab>,
    active_tab_id: Option<String>,
    layout: serde_json::Value,
) -> bool {
    let restored = restore_tab_groups(
        &layout,
        &tabs.iter().map(|tab| tab.id.clone()).collect::<Vec<_>>(),
    );
    state.groups = restored
        .into_iter()
        .enumerate()
        .map(|(index, group)| crate::workspace::tab_groups::TabGroup {
            id: format!("group-{}", index + 1),
            root: group.root,
            active_tab_id: group.active_tab_id,
            zoomed_tab_id: group.zoomed_tab_id,
        })
        .collect();
    state.group_count = state.groups.len() as u64;
    state.tabs = tabs
        .into_iter()
        .map(|mut tab| {
            tab.is_active = active_tab_id.as_deref() == Some(tab.id.as_str());
            tab.has_connected = Some(tab.is_active);
            tab.forget_saved_field("isActive");
            tab.forget_saved_field("hasConnected");
            ensure_tab_defaults(&tab)
        })
        .collect();
    state.active_tab_id = active_tab_id;
    true
}

fn remove(state: &mut TabsState, ids: &[String], mut options: super::RemoveTabsOptions) -> bool {
    let groups = remove_tabs_from_groups(&state.groups, ids);
    options.preferred_active_tab_id = options
        .preferred_active_tab_id
        .or_else(|| get_fallback_tab_id(&state.groups, &groups, state.active_tab_id.as_deref()));
    let changed = state.tabs.iter().any(|tab| ids.contains(&tab.id));
    let result = remove_tabs_from_state(
        std::mem::take(&mut state.tabs),
        state.active_tab_id.as_deref(),
        ids,
        &options,
    );
    state.tabs = result.tabs;
    state.active_tab_id = result.active_tab_id;
    state.groups = groups;
    changed
}

fn merge(
    state: &mut TabsState,
    id: &str,
    target_id: &str,
    direction: crate::workspace::tab_groups::SplitDirection,
) -> bool {
    let source = find_group_of_tab(&state.groups, id);
    let target = find_group_of_tab(&state.groups, target_id);
    if source.is_none()
        || target.is_none()
        || source
            .zip(target)
            .is_some_and(|(source, target)| std::ptr::eq(source, target))
    {
        return false;
    }
    state.groups = merge_group(&state.groups, id, target_id, direction);
    let active =
        find_group_of_tab(&state.groups, target_id).map(|group| group.active_tab_id.clone());
    active.is_some_and(|active| activate(state, &active))
}

fn reopen(
    state: &mut TabsState,
    tabs: Vec<crate::workspace::tab::Tab>,
    layout: super::reopen::ClosedLayout,
) -> bool {
    if tabs.is_empty() {
        return false;
    }
    let ids = tabs.iter().map(|tab| tab.id.clone()).collect::<Vec<_>>();
    let mut count = state.group_count;
    state.groups = place_reopened_tabs(&state.groups, &layout, &ids, &mut || {
        count += 1;
        format!("group-{count}")
    });
    state.group_count = count;
    state.tabs.extend(tabs);
    reopened_active_tab_id(&layout, &ids).is_some_and(|active| activate(state, &active))
}

fn open_page(state: &mut TabsState, mut tab: crate::workspace::tab::Tab) -> bool {
    if let Some(existing) = state
        .tabs
        .iter()
        .find(|existing| existing.tab_type == tab.tab_type)
    {
        let id = existing.id.clone();
        if tab
            .settings_section
            .as_deref()
            .is_some_and(|section| !section.is_empty())
        {
            for existing in state.tabs.iter_mut().filter(|existing| existing.id == id) {
                existing.settings_section = tab.settings_section.clone();
                let next = u64::from(existing.settings_request.unwrap_or(0)) + 1;
                existing
                    .raw_fields
                    .insert("settingsRequest".into(), next.into());
                existing.settings_request = u32::try_from(next).ok();
            }
        }
        return activate(state, &id);
    }
    for previous in &mut state.tabs {
        previous.is_active = false;
        previous.forget_saved_field("isActive");
    }
    state.active_tab_id = Some(tab.id.clone());
    tab.is_active = true;
    tab.forget_saved_field("isActive");
    state.tabs.push(tab);
    true
}

fn duplicate_action(
    state: &mut TabsState,
    source_id: &str,
    new_id: &str,
    split: Option<crate::workspace::tab_groups::SplitDirection>,
) -> bool {
    let Some(source) = state.tabs.iter().find(|tab| tab.id == source_id) else {
        return false;
    };
    if is_page_tab(source) || source.tab_type == TabType::RemoteFileEditor {
        return false;
    }
    let next = duplicate(source, new_id, &state.tabs);
    let source_index = state
        .groups
        .iter()
        .position(|group| get_pane_ids(&group.root).iter().any(|id| id == source_id));
    if let Some((index, direction)) = source_index.zip(split) {
        state.groups[index].root = insert_next_to(
            &state.groups[index].root,
            source_id,
            &pane_leaf(new_id),
            direction,
        );
        state.groups[index].zoomed_tab_id = None;
    }
    for previous in &mut state.tabs {
        previous.is_active = false;
        previous.forget_saved_field("isActive");
    }
    state.tabs.push(next);
    state.active_tab_id = Some(new_id.to_string());
    true
}
