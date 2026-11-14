//! Workspace models shared by the tab bar, split panes and the native view.
//!
//! The TypeScript implementations live in `src/lib/`. Only the parts this
//! migration card needs are installed here; P1-11d's second group completes
//! the tab state and tab group persistence models in the same modules.

/// Requests that open a singleton app page, from `src/lib/appNavigation.ts`.
pub mod app_navigation;
/// Local tab titles, directories and running commands, from
/// `src/lib/localTabTitle.ts`.
pub mod local_tab_title;
/// Pane and sash geometry, from `src/lib/paneLayout.ts`.
pub mod pane_layout;
/// Directional pane navigation, from `src/lib/paneNavigation.ts`.
pub mod pane_navigation;
/// Split layout templates, from `src/lib/paneTemplates.ts`.
pub mod pane_templates;
/// Tab and context-menu value types, from `src/types/tab.ts`.
pub mod tab;
/// Agent indicator styles and normalization, from
/// `src/lib/tabAgentIndicator.ts`.
pub mod tab_agent_indicator;
/// Attention marks and agent state, from `src/lib/tabAttention.ts`.
pub mod tab_attention;
/// Tab close targets and menu items, from `src/lib/tabClosing.ts`.
pub mod tab_closing;
/// Split pane trees and tab groups, from `src/lib/tabGroups.ts`.
pub mod tab_groups;
/// Saved tab group layouts, from `src/lib/tabGroupsPersistence.ts`.
pub mod tab_groups_persistence;
/// Tab icons and icon settings, from `src/lib/tabIcons.ts`.
pub mod tab_icons;
/// Tab and pane context-menu items, from `src/lib/tabMenuActions.ts`.
pub mod tab_menu_actions;
/// Tab ordering for next, previous and nth-tab shortcuts, from
/// `src/lib/tabNavigation.ts`.
pub mod tab_navigation;
/// Tab-bar nudge state, from `src/lib/tabNudge.ts`.
pub mod tab_nudge;
/// Tab width, hover tooltips, icons, agent indicators and pane settings,
/// from `src/lib/tabSettings.ts`.
pub mod tab_settings;
/// Pure tab-list transitions, from `src/lib/tabsState.ts`.
pub mod tabs_state;

pub(crate) mod js_number;

#[cfg(test)]
mod tests;
