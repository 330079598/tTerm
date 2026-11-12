//! Tab and pane context-menu items, from `src/lib/tabMenuActions.ts`.

use crate::keymap::actions::KeymapActionId;

use super::tab::{Tab, TabContextMenuAction, TabType, is_page_tab, is_session_tab};

/// Resolves a translation key, falling back to the English label when the
/// caller gives one.
pub type Translate<'a> = &'a dyn Fn(&str, Option<&str>) -> String;

struct SplitAction {
    key: &'static str,
    label: &'static str,
    action: &'static str,
    shortcut: Option<KeymapActionId>,
}

const SPLITS: &[SplitAction] = &[
    SplitAction {
        key: "splitRight",
        label: "Split Right",
        action: "split-right",
        shortcut: Some(KeymapActionId::WorkspaceSplitRight),
    },
    SplitAction {
        key: "splitLeft",
        label: "Split Left",
        action: "split-left",
        shortcut: None,
    },
    SplitAction {
        key: "splitDown",
        label: "Split Down",
        action: "split-down",
        shortcut: Some(KeymapActionId::WorkspaceSplitBelow),
    },
    SplitAction {
        key: "splitAbove",
        label: "Split Above",
        action: "split-above",
        shortcut: None,
    },
];

fn action(
    label: String,
    name: &str,
    icon: Option<&str>,
    shortcut: Option<KeymapActionId>,
) -> TabContextMenuAction {
    TabContextMenuAction {
        label,
        action: name.to_owned(),
        icon: icon.map(str::to_owned),
        separator: false,
        disabled: None,
        shortcut,
    }
}

/// The items that split a terminal tab's pane, none for other tabs.
pub fn get_tab_split_menu_actions(
    tab: &Tab,
    translate: Translate<'_>,
) -> Vec<TabContextMenuAction> {
    if !is_session_tab(tab) {
        return Vec::new();
    }
    SPLITS
        .iter()
        .map(|split| {
            action(
                translate(&format!("contextMenu.{}", split.key), Some(split.label)),
                split.action,
                Some(split.action),
                split.shortcut,
            )
        })
        .collect()
}

/// The item that renames a tab; a terminal tab's also picks its icon.
pub fn get_tab_rename_menu_action(tab: &Tab, translate: Translate<'_>) -> TabContextMenuAction {
    let label = if is_session_tab(tab) {
        translate("contextMenu.editTab", None)
    } else {
        translate("contextMenu.renameTab", None)
    };
    action(
        label,
        "rename",
        Some("edit"),
        Some(KeymapActionId::TabsRenameActive),
    )
}

/// The item that pins or unpins an SSH tab's connection header, none for
/// other tabs.
pub fn get_tab_pin_menu_action(tab: &Tab, translate: Translate<'_>) -> Vec<TabContextMenuAction> {
    if tab.tab_type != TabType::Ssh {
        return Vec::new();
    }
    if tab.connection_header_pinned == Some(false) {
        vec![action(
            translate("contextMenu.pinConnectionHeader", None),
            "pin-header",
            Some("pin"),
            None,
        )]
    } else {
        vec![action(
            translate("contextMenu.unpinConnectionHeader", None),
            "unpin-header",
            Some("pin-off"),
            None,
        )]
    }
}

/// The item that edits the saved connection a session tab came from, or saves
/// an SSH or local tab that has none as one.
pub fn get_tab_connection_menu_action(
    tab: &Tab,
    translate: Translate<'_>,
) -> Vec<TabContextMenuAction> {
    let Some(label) = tab_connection_action_label(tab) else {
        return Vec::new();
    };
    vec![action(
        translate(label, None),
        "edit-connection",
        Some("edit"),
        Some(KeymapActionId::TabsSaveConnection),
    )]
}

/// The label key of the connection menu item, or `None` when the tab has
/// none.
pub fn tab_connection_action_label(tab: &Tab) -> Option<&'static str> {
    if !is_session_tab(tab) {
        return None;
    }
    if tab
        .connection
        .as_ref()
        .and_then(|connection| connection.profile_id.as_deref())
        .is_some_and(|profile_id| !profile_id.is_empty())
    {
        return Some("contextMenu.editConnection");
    }
    match tab.tab_type {
        TabType::Ssh => Some("contextMenu.saveConnection"),
        TabType::Terminal => Some("contextMenu.saveLocalConnection"),
        _ => None,
    }
}

/// The item that opens the tab closed last again.
pub fn get_reopen_closed_tab_menu_action(translate: Translate<'_>) -> TabContextMenuAction {
    action(
        translate("contextMenu.reopenClosedTab", None),
        "reopen-closed",
        Some("rotate-ccw"),
        Some(KeymapActionId::TabsReopenClosed),
    )
}

/// The item that undoes a tab's split, giving each pane a tab of its own.
pub fn get_ungroup_menu_action(translate: Translate<'_>) -> TabContextMenuAction {
    action(
        translate("workspace.unsplit", None),
        "ungroup",
        Some("unsplit"),
        Some(KeymapActionId::WorkspaceRestoreSingleGroup),
    )
}

/// The item that opens the layout templates for the tab holding this pane.
pub fn get_layout_templates_menu_action(translate: Translate<'_>) -> TabContextMenuAction {
    action(
        translate("layoutTemplates.menu", None),
        "layout-templates",
        Some("layout"),
        Some(KeymapActionId::WorkspaceChooseLayout),
    )
}

/// The tab log items. `recording` is supplied by the caller until
/// `terminalLogRecording.ts` is ported with P1-11g.
pub fn get_tab_log_menu_actions(
    tab: &Tab,
    translate: Translate<'_>,
    recording: bool,
) -> Vec<TabContextMenuAction> {
    if !is_session_tab(tab) {
        return Vec::new();
    }
    vec![
        if recording {
            action(
                translate("contextMenu.stopLogging", None),
                "log-stop",
                Some("record-stop"),
                None,
            )
        } else {
            action(
                translate("contextMenu.startLogging", None),
                "log-start",
                Some("record"),
                None,
            )
        },
        action(
            translate("contextMenu.viewLogs", None),
            "view-logs",
            Some("scroll-text"),
            None,
        ),
    ]
}

fn separator() -> TabContextMenuAction {
    TabContextMenuAction {
        label: String::new(),
        action: String::new(),
        icon: None,
        separator: true,
        disabled: None,
        shortcut: None,
    }
}

/// The context menu of a pane's header in a tab split into panes.
pub fn get_pane_menu_actions(
    tab: &Tab,
    is_zoomed: bool,
    translate: Translate<'_>,
    log_recording: bool,
) -> Vec<TabContextMenuAction> {
    let layout_actions = vec![
        separator(),
        action(
            if is_zoomed {
                translate("workspace.unzoomPane", None)
            } else {
                translate("workspace.zoomPane", None)
            },
            "toggle-zoom",
            Some(if is_zoomed { "minimize" } else { "maximize" }),
            Some(KeymapActionId::WorkspaceTogglePaneZoom),
        ),
        action(
            translate("workspace.movePaneToNewTab", None),
            "move-to-new-tab",
            Some("move-out"),
            Some(KeymapActionId::WorkspaceMovePaneToNewTab),
        ),
        get_ungroup_menu_action(translate),
        get_layout_templates_menu_action(translate),
    ];
    let close_actions = vec![
        separator(),
        action(
            translate("workspace.closePane", None),
            "close",
            Some("x"),
            Some(KeymapActionId::TabsCloseActive),
        ),
    ];

    if is_page_tab(tab) {
        let mut result: Vec<TabContextMenuAction> = layout_actions.into_iter().skip(1).collect();
        result.extend(close_actions);
        return result;
    }

    let mut result = vec![action(
        translate("contextMenu.duplicateTab", None),
        "duplicate",
        Some("copy"),
        Some(KeymapActionId::TabsDuplicateActive),
    )];
    result.extend(get_tab_split_menu_actions(tab, translate));
    result.push(get_tab_rename_menu_action(tab, translate));
    result.extend(get_tab_connection_menu_action(tab, translate));
    result.extend(get_tab_pin_menu_action(tab, translate));
    result.extend(get_tab_log_menu_actions(tab, translate, log_recording));
    result.extend(layout_actions);
    result.extend(close_actions);
    result
}
