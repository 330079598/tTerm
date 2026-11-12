//! State transitions for tab-bar nudges, from `src/lib/tabNudge.ts`.
//!
//! The TypeScript module manipulates DOM classes. Native code keeps the same
//! state here and the GPUI tab bar applies it to its own elements.

use std::collections::BTreeSet;

/// The element an animation-end event belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NudgeAnimationTarget {
    /// The tab's own animation ended.
    Tab(String),
    /// A child of the tab, such as its agent spinner, ended.
    TabChild(String),
    /// The tab bar's own animation ended.
    Bar,
}

/// Which nudges are running and what the view should show.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TabNudgeState {
    /// Tab ids present in the bar.
    pub tab_ids: Vec<String>,
    /// Whether the tab bar exists.
    pub bar_present: bool,
    flashing: BTreeSet<String>,
    shaking: bool,
    bar_pending: bool,
    scroll_requests: u64,
    pending: BTreeSet<String>,
}

impl TabNudgeState {
    /// A bar holding `tab_ids`.
    pub fn new(tab_ids: Vec<String>) -> Self {
        Self {
            tab_ids,
            bar_present: true,
            ..Self::default()
        }
    }

    /// Whether the tab currently shows the flash animation.
    pub fn is_flashing(&self, tab_id: &str) -> bool {
        self.flashing.contains(tab_id)
    }

    /// Whether the tab bar currently shows the shake animation.
    pub fn is_shaking(&self) -> bool {
        self.shaking
    }

    /// How many times the bar was asked to scroll the named tab into view.
    pub fn scroll_requests(&self) -> u64 {
        self.scroll_requests
    }

    /// Flashes the named tab and asks the bar to scroll it into view.
    pub fn nudge_tab(&mut self, tab_id: &str) {
        if !self.tab_ids.iter().any(|id| id == tab_id) {
            return;
        }
        self.scroll_requests += 1;
        self.flashing.remove(tab_id);
        self.flashing.insert(tab_id.to_owned());
        self.pending.insert(tab_id.to_owned());
    }

    /// Shakes the tab bar.
    pub fn nudge_tab_bar(&mut self) {
        if !self.bar_present {
            return;
        }
        self.shaking = true;
        self.bar_pending = true;
    }

    /// Handles an animation-end event. Only the element's own event clears a
    /// nudge; child animations bubble but do not end it.
    pub fn animation_end(&mut self, target: NudgeAnimationTarget) {
        match target {
            NudgeAnimationTarget::Tab(tab_id) => {
                if self.pending.remove(&tab_id) {
                    self.flashing.remove(&tab_id);
                }
            }
            NudgeAnimationTarget::TabChild(_) => {}
            NudgeAnimationTarget::Bar => {
                if self.bar_pending {
                    self.bar_pending = false;
                    self.shaking = false;
                }
            }
        }
    }
}
