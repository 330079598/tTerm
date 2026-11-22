//! Builds the native tab and split collections from a decoded workspace.

use std::collections::HashMap;

use gpui_kit::{Context, Entity};
use tterm_core::session_restore::workspace::WorkspaceSnapshot;

use super::super::{Locale, Prototype, TabFactory, Tabs, WindowTab, split_state};
use super::launch::launch_for_tab;

/// The restored tab collection, its split cards, and the next persisted id.
pub(in crate::terminal_demo::window) struct RestoredWorkspace {
    pub tabs: Tabs<WindowTab>,
    pub splits: split_state::SplitState,
    pub next_saved_tab: u64,
}

/// Opens every saved tab in order. Tabs that cannot be opened safely remain
/// inert placeholders so their saved fields survive the next write.
pub(in crate::terminal_demo::window) fn restore_workspace(
    factory: &TabFactory,
    snapshot: WorkspaceSnapshot,
    locale: &Entity<Locale>,
    cx: &mut Context<Prototype>,
) -> Option<RestoredWorkspace> {
    if snapshot.tabs.is_empty() {
        return None;
    }
    let config = factory.config.read(cx).config().clone();
    let mut entries = Vec::with_capacity(snapshot.tabs.len());
    let mut internal_ids = HashMap::with_capacity(snapshot.tabs.len());
    let mut next_saved_tab = 1u64;
    for (index, tab) in snapshot.tabs.iter().enumerate() {
        let internal = index + 1;
        internal_ids.insert(tab.id.clone(), internal);
        if let Some(number) = tab
            .id
            .strip_prefix("tab-")
            .and_then(|suffix| suffix.parse::<u64>().ok())
        {
            next_saved_tab = next_saved_tab.max(number.saturating_add(1));
        }
        let launch = launch_for_tab(tab, &config);
        entries.push(factory.open_launch(internal, launch, locale, cx));
    }
    let active = snapshot
        .active_tab_id
        .as_deref()
        .and_then(|id| internal_ids.get(id).copied())
        .unwrap_or(1);
    let tabs = Tabs::restore(entries, active)?;
    let mut splits =
        split_state::SplitState::restore(&snapshot.groups, |id| internal_ids.get(id).copied());
    for (id, _) in tabs.ordered() {
        splits.open(id);
    }
    Some(RestoredWorkspace {
        tabs,
        splits,
        next_saved_tab,
    })
}
