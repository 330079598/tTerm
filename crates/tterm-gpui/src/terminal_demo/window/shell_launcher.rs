//! Opens a fresh session from a shell selected in the tab strip.

pub(super) mod recent;
pub(super) mod search;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::prelude::*;
use gpui_kit::{Context, Window, px};
use tterm_core::i18n::keys;
use tterm_core::workspace::local_shells::connections::local_shell_connection;
use tterm_core::workspace::local_shells::default_shell::{default_shell_choice, default_shell_key};
use tterm_core::workspace::local_shells::saved::own_shell_choice;
use tterm_core::workspace::local_shells::types::{LocalShellChoice, ShellConfig};
use tterm_core::workspace::tab::{Tab, TabType};
use tterm_ui::components::icons::{PLUS, icon};
use tterm_ui::tokens::gpui::hsla;

use crate::config_state::ConfigState;
use crate::locale::Locale;

use super::{Prototype, TabLaunch, session_workspace};

/// The label of a configured default that no detected or saved entry names.
fn default_choice_label(choice: &LocalShellChoice, locale: &Locale) -> String {
    let path = choice.custom_path.as_deref().map(str::trim).unwrap_or("");
    if !path.is_empty() {
        return path.to_string();
    }
    locale
        .text(keys::SHELL_LAUNCH_CUSTOM)
        .unwrap_or_else(|_| keys::SHELL_LAUNCH_CUSTOM.as_str().to_string())
}

/// One local shell the launcher offers, labelled and ordered the same way in
/// the menu and the searchable panel.
#[derive(Clone)]
pub(super) struct ShellChoiceRow {
    pub label: String,
    pub detail: String,
    pub choice: LocalShellChoice,
    pub is_default: bool,
}

/// The configured default first, then every shell the settings show and the
/// user has not hidden. The default stays listed when hidden because the
/// configuration still makes it the valid default action.
pub(super) fn ordered_shell_rows(state: &ConfigState, locale: &Locale) -> Vec<ShellChoiceRow> {
    let shell_config = ShellConfig::from(state.config());
    let default_key = default_shell_key(&shell_config);
    let fallback_choice = default_shell_choice(&shell_config);
    let entries = crate::local_shells_setting::shell_entries(state, locale);
    let default_entry = entries.iter().find(|entry| entry.key == default_key);
    let (default_label, default_detail, default_choice) = match default_entry {
        Some(entry) if !entry.label.trim().is_empty() => (
            entry.label.clone(),
            entry.detail.clone(),
            entry.choice.clone(),
        ),
        Some(entry) => (
            default_choice_label(&entry.choice, locale),
            entry.detail.clone(),
            entry.choice.clone(),
        ),
        None => (
            default_choice_label(&fallback_choice, locale),
            String::new(),
            fallback_choice,
        ),
    };
    let mut rows = vec![ShellChoiceRow {
        label: default_label,
        detail: default_detail,
        choice: default_choice,
        is_default: true,
    }];
    rows.extend(
        entries
            .into_iter()
            .filter(|entry| entry.key != default_key && !entry.hidden)
            .map(|entry| ShellChoiceRow {
                label: entry.label,
                detail: entry.detail,
                choice: entry.choice,
                is_default: false,
            }),
    );
    rows
}

impl Prototype {
    pub(super) fn shell_launcher(
        &self,
        label: String,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let owner = cx.weak_entity();
        let config = self.factory.config.clone();
        let locale = self.ui.locale.clone();
        let services = self.factory.sessions.services();
        Button::new("new-session-tab")
            .ghost()
            .compact()
            // React draws a 28 by 28 .tab-action with a 16 px Lucide plus in
            // the muted foreground; the stock ghost label was the component
            // theme's dark text and measured larger than the reference.
            .w(px(28.))
            .h(px(28.))
            .tooltip(label.clone())
            .accessibility_label(label)
            .child(icon(
                PLUS,
                hsla(self.ui.theme.colors.muted_foreground, 1.0),
                16.0,
            ))
            .dropdown_menu(move |mut menu, window, cx| {
                let rows = {
                    let state = config.read(cx);
                    ordered_shell_rows(state, locale.read(cx))
                };
                for row in rows {
                    let owner = owner.clone();
                    let title = row.label.clone();
                    let item = PopupMenuItem::new(row.label);
                    let item = if row.is_default {
                        item.checked(true)
                    } else {
                        item
                    };
                    menu = menu.item(item.on_click(move |_, window, app| {
                        let _ = owner.update(app, |workspace, cx| {
                            workspace.open_shell_choice(
                                row.choice.clone(),
                                Some(title.clone()),
                                window,
                                cx,
                            );
                        });
                    }));
                }
                let search_owner = owner.clone();
                let label = recent::text(&locale, keys::PROFILES_SEARCH_PLACEHOLDER, cx);
                menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, app| {
                    let _ = search_owner.update(app, |workspace, cx| {
                        workspace.open_launcher_search(window, cx)
                    });
                }));
                recent::submenu(menu, &owner, &locale, &services, window, cx)
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
