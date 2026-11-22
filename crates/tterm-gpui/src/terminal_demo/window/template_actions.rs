//! Layout templates rearrange a card, copying only live default shells.
//!
//! A template stores a shape, never connection or session data. Filling its
//! extra panes therefore reruns only the safe local default shell; SSH panes
//! keep their live sessions and `--exec` programs are never started again.

use gpui_kit::{Context, Window};
use tterm_core::workspace::pane_templates::{LayoutShape, count_template_panes};
use tterm_core::workspace::tab_groups::get_pane_ids;

use super::{CommandSpec, Prototype, WindowTab};

impl Prototype {
    /// Whether a template may add panes to `tab` without reconnecting or
    /// rerunning its program.
    pub(super) fn template_can_add_panes(&self, tab: usize) -> bool {
        self.tabs
            .get(tab)
            .is_some_and(WindowTab::live_default_shell)
    }

    /// Applies `shape` to the active card.
    pub(super) fn apply_layout_template(
        &mut self,
        shape: &LayoutShape,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.apply_layout_template_to(self.tabs.active_id(), shape, window, cx)
    }

    /// Applies `shape` to the card holding `tab`, opening local shells for its
    /// missing panes and leaving surplus panes as standalone cards.
    pub(super) fn apply_layout_template_to(
        &mut self,
        tab: usize,
        shape: &LayoutShape,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.tabs.get(tab).is_none() {
            return false;
        }
        let active = self.tabs.active_id();
        let previous = self.visible_ids();
        let panes = self
            .splits
            .root(tab)
            .map_or(0, |root| get_pane_ids(root).len());
        let missing = count_template_panes(shape).saturating_sub(panes);
        if missing > 0 && !self.template_can_add_panes(tab) {
            return false;
        }

        let mut opened = Vec::with_capacity(missing);
        let mut failed = false;
        for _ in 0..missing {
            let previous = self.tabs.active_id();
            self.tabs.push(|id| {
                self.factory
                    .open(id, CommandSpec::Shell, &self.ui.locale, cx)
            });
            let new = self.tabs.active_id();
            if new == previous {
                failed = true;
                break;
            }
            opened.push(new);
            if !self
                .tabs
                .get(new)
                .is_some_and(WindowTab::live_default_shell)
            {
                failed = true;
                break;
            }
        }

        if failed || !self.splits.apply_template(tab, shape, &opened) {
            if !opened.is_empty() {
                close_opened(self, &opened);
                self.tabs.select(active);
                self.splits.focus(active);
                self.refresh_panes(previous, window, cx);
            }
            return false;
        }

        self.tabs.select(tab);
        self.splits.focus(tab);
        self.refresh_panes(previous, window, cx);
        true
    }
}

fn close_opened(prototype: &mut Prototype, opened: &[usize]) {
    for id in opened.iter().rev() {
        if let Some(closed) = prototype.tabs.close(*id)
            && let Some(binding) = closed.binding
            && let Err(error) = prototype.factory.sessions.close_tab(binding.tab_id())
        {
            eprintln!("Failed to close terminal tab: {error}");
        }
    }
}

#[cfg(test)]
mod tests;
