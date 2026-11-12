//! Actions accepted by the pure workspace reducer.

use serde_json::Value;

use crate::workspace::pane_templates::LayoutShape;
use crate::workspace::tab::{Tab, TabType};
use crate::workspace::tab_groups::SplitDirection;
use crate::workspace::tabs_state::RemoveTabsOptions;
use crate::workspace::tabs_state::reopen::ClosedLayout;

/// Every case of the TypeScript `TabsAction` union.
pub enum TabsAction {
    /// Restores saved tabs and a current or legacy layout.
    Restore {
        /// The restored tabs.
        tabs: Vec<Tab>,
        /// The requested active tab.
        active_tab_id: Option<String>,
        /// The saved layout, or null when absent.
        layout: Value,
    },
    /// Adds and activates a tab.
    Add(Tab),
    /// Opens a singleton page or focuses its existing tab.
    OpenPage(Tab),
    /// Renames every page of the requested type.
    RenamePage {
        /// The page type.
        tab_type: TabType,
        /// The new title.
        title: String,
    },
    /// Removes tabs and chooses their replacement focus.
    Remove {
        /// The ids to remove.
        ids: Vec<String>,
        /// Focus preferences.
        options: RemoveTabsOptions,
    },
    /// Activates the tab with this id.
    SetActive(String),
    /// Moves one group in the tab bar.
    Move {
        /// The old group index.
        from_index: i64,
        /// The new group index.
        to_index: i64,
    },
    /// Duplicates a session, optionally into its source split.
    Duplicate {
        /// The source tab id.
        source_id: String,
        /// The duplicate's id.
        new_id: String,
        /// The side of the source pane on which to split.
        split: Option<SplitDirection>,
    },
    /// Sets a manual title and clears automatic naming metadata.
    Rename {
        /// The tab id.
        id: String,
        /// The new title.
        title: String,
    },
    /// Applies a pure updater to a tab and fills its defaults.
    Update {
        /// The tab id.
        id: String,
        /// The same pure callback accepted by the TypeScript action.
        updater: Box<dyn Fn(&Tab) -> Tab>,
    },
    /// Places a pane beside another pane.
    PlacePane {
        /// The moved pane.
        id: String,
        /// The target pane.
        target_id: String,
        /// The target side.
        direction: SplitDirection,
    },
    /// Merges an entire group beside a pane.
    MergeGroup {
        /// A pane in the source group.
        id: String,
        /// The target pane.
        target_id: String,
        /// The target side.
        direction: SplitDirection,
    },
    /// Gives a pane a group of its own.
    BreakOut {
        /// The pane id.
        id: String,
        /// The requested position, or immediately after the source group.
        index: Option<i64>,
    },
    /// Gives every pane of this pane's group its own group.
    Ungroup(String),
    /// Toggles a pane's zoom and activates it.
    ToggleZoom(String),
    /// Resizes one split.
    ResizeSplit {
        /// The group containing the split.
        group_id: String,
        /// The child-index path to the split.
        path: Vec<i64>,
        /// The new proportions.
        sizes: Vec<f64>,
    },
    /// Applies a layout template, copying the source tab when needed.
    ApplyTemplate {
        /// The source pane.
        id: String,
        /// The requested shape.
        root: LayoutShape,
        /// Ids available for copies.
        new_ids: Vec<String>,
    },
    /// Reopens tabs according to their previous layout.
    Reopen {
        /// Tabs with newly allocated ids.
        tabs: Vec<Tab>,
        /// Their old group shape or pane anchor.
        layout: ClosedLayout,
    },
}
