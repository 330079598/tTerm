//! Opens a fresh session from a shell selected in the tab strip.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::prelude::*;
use gpui_kit::{Context, Window};
use tterm_core::i18n::keys;
use tterm_core::workspace::local_shells::connections::local_shell_connection;
use tterm_core::workspace::local_shells::entries::build_shell_entries;
use tterm_core::workspace::local_shells::saved::own_shell_choice;
use tterm_core::workspace::local_shells::types::ShellConfig;
use tterm_core::workspace::tab::{Tab, TabType};

use super::{Prototype, TabLaunch, session_workspace};

impl Prototype {
    pub(super) fn shell_launcher(
        &self,
        label: String,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let owner = cx.weak_entity();
        let config = self.factory.config.clone();
        let locale = self.ui.locale.clone();
        Button::new("new-session-tab")
            .label("+")
            .ghost()
            .compact()
            .tooltip(label.clone())
            .accessibility_label(label)
            .dropdown_menu(move |mut menu, _, cx| {
                let snapshot = config.read(cx).config();
                let auto = locale
                    .read(cx)
                    .text(keys::SHELL_LAUNCH_AUTO)
                    .unwrap_or_else(|_| "Auto".into());
                for entry in
                    build_shell_entries(&[], &ShellConfig::from(snapshot), |_, _| auto.clone())
                {
                    if entry.hidden || snapshot.hidden_shells.contains(&entry.key) {
                        continue;
                    }
                    let owner = owner.clone();
                    let id = entry.choice.shell_id;
                    menu = menu.item(PopupMenuItem::new(entry.label).on_click(
                        move |_, window, app| {
                            let _ = owner.update(app, |workspace, cx| match &id {
                                Some(id) => workspace.open_configured_shell(id, window, cx),
                                None => workspace.new_tab(window, cx),
                            });
                        },
                    ));
                }
                menu
            })
    }

    pub(super) fn open_configured_shell(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let saved_id = self.next_saved_id();
        let config = self.factory.config.read(cx).config();
        let Some(shell) = config.local_shells.iter().find(|s| s.id == id) else {
            return;
        };
        let mut tab = Tab::new(saved_id, &shell.name, TabType::Terminal);
        tab.set_connection(Some(local_shell_connection(&own_shell_choice(shell), None)));
        let launch = match session_workspace::launch::local_options(&tab, config) {
            Ok(options) => TabLaunch::selected_shell(tab, options),
            Err(reason) => TabLaunch::unsupported(tab, reason),
        };
        if !self.close_settings(window, cx) {
            return;
        }
        if self.tunnel_page_is_open() {
            self.close_tunnel_page(window, cx);
        }
        let previous = self.visible_ids();
        self.tabs
            .push(|id| self.factory.open_launch(id, launch, &self.ui.locale, cx));
        self.splits.open(self.tabs.active_id());
        self.refresh_panes(previous, window, cx);
        self.mark_workspace_dirty(cx);
    }
}
