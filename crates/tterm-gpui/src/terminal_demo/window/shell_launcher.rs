//! Opens a fresh session from a shell selected in the tab strip.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::prelude::*;
use gpui_kit::{Context, Window};
use tterm_core::workspace::local_shells::connections::local_shell_connection;
use tterm_core::workspace::local_shells::saved::own_shell_choice;
use tterm_core::workspace::local_shells::types::LocalShellChoice;
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
                let hidden = config.read(cx).config().hidden_shells.clone();
                let entries = {
                    let state = config.read(cx);
                    let locale = locale.read(cx);
                    crate::local_shells_setting::shell_entries(state, locale)
                };
                for entry in entries {
                    if entry.hidden || hidden.contains(&entry.key) {
                        continue;
                    }
                    let owner = owner.clone();
                    let choice = entry.choice;
                    let title = entry.label.clone();
                    menu = menu.item(PopupMenuItem::new(entry.label).on_click(
                        move |_, window, app| {
                            let _ = owner.update(app, |workspace, cx| {
                                workspace.open_shell_choice(
                                    choice.clone(),
                                    Some(title.clone()),
                                    window,
                                    cx,
                                );
                            });
                        },
                    ));
                }
                menu
            })
    }

    /// Opens the shell the user picked in the launcher, rather than the
    /// configured default. A shell of the user's own follows its latest saved
    /// path and arguments, and is not started if it was removed meanwhile.
    pub(super) fn open_shell_choice(
        &mut self,
        choice: LocalShellChoice,
        title: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(choice) = self.current_shell_choice(choice, cx) else {
            return;
        };
        if self.tunnel_page_is_open() {
            self.close_tunnel_page(window, cx);
        }
        if !self.close_settings(window, cx) {
            return;
        }
        let saved_id = self.next_saved_id();
        let mut tab = Tab::new(saved_id, title.unwrap_or_default(), TabType::Terminal);
        tab.set_connection(Some(local_shell_connection(&choice, None)));
        let launch = match session_workspace::launch::local_options_for_choice(&choice) {
            Ok(options) => TabLaunch::selected_shell(tab, options),
            Err(reason) => TabLaunch::unsupported(tab, reason),
        };
        let previous = self.visible_ids();
        self.tabs
            .push(|id| self.factory.open_launch(id, launch, &self.ui.locale, cx));
        self.splits.open(self.tabs.active_id());
        self.refresh_panes(previous, window, cx);
        self.mark_workspace_dirty(cx);
    }

    fn current_shell_choice(
        &self,
        choice: LocalShellChoice,
        cx: &Context<Self>,
    ) -> Option<LocalShellChoice> {
        if choice.shell != "custom" {
            return Some(choice);
        }
        let Some(id) = choice.shell_id.as_deref().filter(|id| !id.is_empty()) else {
            return Some(choice);
        };
        self.factory
            .config
            .read(cx)
            .config()
            .local_shells
            .iter()
            .find(|shell| shell.id == id)
            .map(own_shell_choice)
    }
}
