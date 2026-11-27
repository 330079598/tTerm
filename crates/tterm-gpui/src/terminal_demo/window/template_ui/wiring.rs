//! Connects the layout picker to the workspace and its title bar.

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, Stateful, Window};
use tterm_ui::components::icons::LAYOUT_TEMPLATE;

use super::super::Prototype;
use super::{OPEN_BUTTON_ID, TemplateTarget};
use crate::app_window::chrome::actions::action_button;

impl Prototype {
    /// Opens the picker for the active tab.
    pub(in super::super) fn open_layout_templates(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.layout_template_target(self.tabs.active_id()) else {
            return;
        };
        if self.tunnel_page_is_open() {
            self.close_tunnel_page(window, cx);
        }
        self.ui
            .template_ui
            .update(cx, |template_ui, cx| template_ui.open(target, window, cx));
    }

    /// Keeps the picker attached to the same tab and closes it when that tab
    /// disappears.
    pub(in super::super) fn sync_layout_template_target(&mut self, cx: &mut Context<Self>) {
        let tab = self.ui.template_ui.read(cx).target_tab();
        let target = tab.and_then(|tab| self.layout_template_target(tab));
        self.ui
            .template_ui
            .update(cx, |template_ui, cx| template_ui.sync_target(target, cx));
    }

    fn layout_template_target(&self, tab: usize) -> Option<TemplateTarget> {
        self.tabs.get(tab)?;
        Some(TemplateTarget {
            tab,
            root: self.splits.root(tab)?.clone(),
            can_copy: self.template_can_add_panes(tab),
        })
    }

    /// The title-bar entry that opens the picker.
    pub(in super::super) fn layout_template_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = self.ui.template_ui.read(cx).open_button_label(cx);
        action_button(OPEN_BUTTON_ID, label, LAYOUT_TEMPLATE, self.ui.theme)
            .on_click(cx.listener(|this, _, window, cx| this.open_layout_templates(window, cx)))
    }
}
