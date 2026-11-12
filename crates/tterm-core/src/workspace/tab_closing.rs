//! Close targets and menu items for a tab, from `src/lib/tabClosing.ts`.

use crate::keymap::actions::KeymapActionId;

use super::tab::TabContextMenuAction;

/// A closing action offered for a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabCloseAction {
    /// Close every tab but the chosen one.
    CloseOthers,
    /// Close the tabs to the left of the chosen one.
    CloseLeft,
    /// Close the tabs to the right of the chosen one.
    CloseRight,
}

/// The tabs a close action targets, from the tab's place in its group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabCloseContext {
    /// The tab's 0-based place in the group.
    pub index: usize,
    /// The group's other tabs.
    pub other_ids: Vec<String>,
    /// The tabs before the chosen one.
    pub left_ids: Vec<String>,
    /// The tabs after the chosen one.
    pub right_ids: Vec<String>,
}

/// The translated labels of the closing menu items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabCloseMenuLabels {
    /// The label of the item that closes the tab.
    pub close_tab: String,
    /// The label of the item that closes the other tabs.
    pub close_other_tabs: String,
    /// The label of the item that closes the tabs to the left.
    pub close_tabs_to_left: String,
    /// The label of the item that closes the tabs to the right.
    pub close_tabs_to_right: String,
}

/// The close targets around a tab, or `None` when it is not in the list.
pub fn get_tab_close_context(tab_ids: &[String], tab_id: &str) -> Option<TabCloseContext> {
    let index = tab_ids.iter().position(|id| id == tab_id)?;
    Some(TabCloseContext {
        index,
        other_ids: tab_ids
            .iter()
            .filter(|id| id.as_str() != tab_id)
            .cloned()
            .collect(),
        left_ids: tab_ids[..index].to_vec(),
        right_ids: tab_ids[index + 1..].to_vec(),
    })
}

/// The tab ids a closing action closes.
pub fn get_tab_ids_for_close_action(
    tab_ids: &[String],
    tab_id: &str,
    action: TabCloseAction,
) -> Vec<String> {
    let Some(context) = get_tab_close_context(tab_ids, tab_id) else {
        return Vec::new();
    };
    match action {
        TabCloseAction::CloseOthers => context.other_ids,
        TabCloseAction::CloseLeft => context.left_ids,
        TabCloseAction::CloseRight => context.right_ids,
    }
}

fn action(
    label: String,
    name: &str,
    icon: Option<&str>,
    disabled: Option<bool>,
    shortcut: Option<KeymapActionId>,
) -> TabContextMenuAction {
    TabContextMenuAction {
        label,
        action: name.to_owned(),
        icon: icon.map(str::to_owned),
        separator: false,
        disabled,
        shortcut,
    }
}

/// The closing items of a tab's menu. A split tab closes with all its panes,
/// so it shows the close shortcut only when it has a single pane.
pub fn get_tab_close_menu_actions(
    tab_ids: &[String],
    tab_id: &str,
    labels: &TabCloseMenuLabels,
    is_split: bool,
) -> Vec<TabContextMenuAction> {
    let context = get_tab_close_context(tab_ids, tab_id);
    vec![
        action(
            labels.close_tab.clone(),
            "close-tab",
            Some("x"),
            None,
            if is_split {
                None
            } else {
                Some(KeymapActionId::TabsCloseActive)
            },
        ),
        action(
            labels.close_other_tabs.clone(),
            "close-others",
            None,
            Some(
                !context
                    .as_ref()
                    .is_some_and(|context| !context.other_ids.is_empty()),
            ),
            None,
        ),
        action(
            labels.close_tabs_to_left.clone(),
            "close-left",
            None,
            Some(
                !context
                    .as_ref()
                    .is_some_and(|context| !context.left_ids.is_empty()),
            ),
            None,
        ),
        action(
            labels.close_tabs_to_right.clone(),
            "close-right",
            None,
            Some(
                !context
                    .as_ref()
                    .is_some_and(|context| !context.right_ids.is_empty()),
            ),
            None,
        ),
    ]
}
