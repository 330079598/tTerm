//! Editable search over local shells and display-safe SSH summaries.
//!
//! With nothing typed the panel starts like the launcher menu: the configured
//! shell first, then the other visible shells, a new connection, and the saved
//! connections used lately. A query narrows every one of those groups instead
//! of rebuilding the menu for each keystroke.

use std::collections::HashSet;

use gpui_kit::component::input::Position;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Context, Entity, KeyDownEvent, MouseButton, Subscription, WeakEntity, Window, div, px,
};
use tterm_core::connections::launcher_query::{LauncherProfile, query_saved_profiles};
use tterm_core::connections::profile_recents::RecentProfile;
use tterm_core::connections::quick_connect::{
    QuickConnectEndpoint, QuickConnectTarget, QuickConnection, SshConfigHost,
    draft_profile_from_input, format_endpoint, parse_quick_connect_input,
    quick_connection_from_config, resolve_quick_connect_target,
};
#[cfg(not(test))]
use tterm_core::connections::quick_launch::launcher_config_host;
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{ArgumentValue, TranslationRequest};
use tterm_core::profiles::launch::SavedSshProfileSummary;
use tterm_core::profiles::types::SavedProfile;
use tterm_core::workspace::local_shells::types::LocalShellChoice;
use tterm_ui::components::input::{InputOptions, input};
use tterm_ui::tokens::gpui::hsla;

use super::super::picker::TabPicker;
use super::{Prototype, ShellChoiceRow, ordered_shell_rows, recent};

/// How many saved connections a query lists before the rest is cut off.
const PROFILE_MATCH_LIMIT: usize = 8;
/// How many `~/.ssh/config` aliases a query lists.
const CONFIG_MATCH_LIMIT: usize = 6;

struct Catalog {
    profiles: Vec<SavedSshProfileSummary>,
    recents: Vec<RecentProfile>,
    config_hosts: Vec<SshConfigHost>,
}

#[derive(Clone)]
enum RowAction {
    Shell {
        choice: LocalShellChoice,
        title: String,
    },
    Profile(String),
    QuickConnect(QuickConnection),
    AddUser {
        text: String,
        caret: usize,
    },
    NewConnection,
}

struct Row {
    label: String,
    detail: String,
    checked: bool,
    action: RowAction,
}

pub(in super::super) struct LauncherSearch {
    locale: Entity<super::Locale>,
    owner: WeakEntity<Prototype>,
    field: Entity<InputState>,
    shells: Vec<ShellChoiceRow>,
    new_connection_label: String,
    new_connection_detail: String,
    catalog: Option<Result<Catalog, String>>,
    selected: usize,
    _events: Subscription,
}

impl Prototype {
    pub(super) fn open_launcher_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.weak_entity();
        let locale = self.ui.locale.clone();
        let placeholder = recent::text(&self.ui.locale, keys::QUICK_CONNECT_PLACEHOLDER, cx);
        let new_connection_label =
            recent::text(&self.ui.locale, keys::QUICK_CONNECT_NEW_CONNECTION, cx);
        let new_connection_detail = recent::text(
            &self.ui.locale,
            keys::QUICK_CONNECT_NEW_CONNECTION_DETAIL,
            cx,
        );
        let shells = ordered_shell_rows(self.factory.config.read(cx), self.ui.locale.read(cx));
        let panel = cx.new(|cx: &mut Context<LauncherSearch>| {
            let field = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            let events =
                cx.subscribe_in(&field, window, |this, _, event, window, cx| match event {
                    InputEvent::Change => {
                        this.selected = 0;
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => {
                        if let Some(row) = this.rows(cx).get(this.selected) {
                            this.finish(Some(row.action.clone()), window, cx);
                        }
                    }
                    _ => {}
                });
            field.update(cx, |field, cx| field.focus(window, cx));
            LauncherSearch {
                locale,
                owner,
                field,
                shells,
                new_connection_label,
                new_connection_detail,
                selected: 0,
                catalog: None,
                _events: events,
            }
        });
        #[cfg(not(test))]
        {
            let task = self
                .factory
                .sessions
                .services()
                .shared
                .runtime
                .spawn_blocking(|| {
                    let config_hosts =
                        tterm_core::profiles::parser::build_ssh_config_import_preview(None)
                            .map(|preview| {
                                preview
                                    .hosts
                                    .into_iter()
                                    .map(launcher_config_host)
                                    .collect()
                            })
                            .unwrap_or_default();
                    Ok(Catalog {
                        profiles: tterm_core::profiles::launch::load_saved_ssh_profile_summaries()?,
                        recents: tterm_core::connections::profile_recents::read_recent_profiles()?,
                        config_hosts,
                    })
                });
            let weak = panel.downgrade();
            cx.spawn(async move |_, cx| {
                let catalog = task
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|value| value);
                let _ = weak.update(cx, |panel, cx| {
                    panel.catalog = Some(catalog);
                    cx.notify();
                });
            })
            .detach();
        }
        #[cfg(test)]
        panel.update(cx, |panel, cx| {
            panel.catalog = Some(Ok(Catalog {
                profiles: recent::Fixture::profiles(cx),
                recents: recent::Fixture::recents(cx),
                config_hosts: Vec::new(),
            }))
        });
        self.picker = Some(TabPicker::Launcher(panel));
        cx.notify();
    }
}

impl LauncherSearch {
    fn rows(&self, cx: &App) -> Vec<Row> {
        let query = self.field.read(cx).value();
        let normalized = query.trim().to_lowercase();
        let catalog = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.as_ref().ok());
        let profiles = catalog
            .map(|catalog| catalog.profiles.as_slice())
            .unwrap_or(&[]);
        let recents = catalog
            .map(|catalog| catalog.recents.as_slice())
            .unwrap_or(&[]);
        let config_hosts = catalog
            .map(|catalog| catalog.config_hosts.as_slice())
            .unwrap_or(&[]);
        let mut rows = Vec::new();
        if !normalized.is_empty() {
            let mut resolved_alias = None;
            let mut add_user = None;
            if let Some(target) = parse_quick_connect_input(&query) {
                match resolve_quick_connect_target(&target, config_hosts) {
                    Some(connection) => {
                        resolved_alias = Some(target.host.clone());
                        rows.push(self.quick_row(cx, connection));
                    }
                    None => add_user = Some(self.add_user_row(cx, query.trim(), &target)),
                }
            }
            rows.extend(
                query_saved_profiles(&normalized, profiles, recents, PROFILE_MATCH_LIMIT)
                    .into_iter()
                    .map(profile_row),
            );
            rows.extend(
                config_hosts
                    .iter()
                    .filter(|host| !host.skipped)
                    .filter(|host| config_matches(&normalized, host))
                    .filter(|host| resolved_alias.as_deref() != Some(host.host_pattern.as_str()))
                    .take(CONFIG_MATCH_LIMIT)
                    .map(|host| match quick_connection_from_config(host) {
                        Some(connection) => self.quick_row(cx, connection),
                        None => self.add_user_row(
                            cx,
                            &host.host_pattern,
                            &QuickConnectTarget {
                                username: None,
                                host: host.host_pattern.clone(),
                                port: None,
                                jump_host: None,
                                private_key_path: None,
                            },
                        ),
                    }),
            );
            rows.extend(add_user);
        }
        rows.extend(
            self.shells
                .iter()
                .filter(|shell| shell_matches(&normalized, shell))
                .map(|shell| Row {
                    label: shell.label.clone(),
                    detail: shell.detail.clone(),
                    checked: shell.is_default,
                    action: RowAction::Shell {
                        choice: shell.choice.clone(),
                        title: shell.label.clone(),
                    },
                }),
        );
        rows.push(Row {
            label: self.new_connection_label.clone(),
            // The React launcher says the form is filled in once anything is
            // typed, even when the text is not a connection and opens blank.
            detail: if normalized.is_empty() {
                self.new_connection_detail.clone()
            } else {
                recent::text(
                    &self.locale,
                    keys::QUICK_CONNECT_NEW_CONNECTION_PREFILLED,
                    cx,
                )
            },
            checked: false,
            action: RowAction::NewConnection,
        });
        if normalized.is_empty() {
            let recent_ids = recents
                .iter()
                .map(|recent| recent.profile_id.as_str())
                .collect::<HashSet<_>>();
            let recent_profiles = query_saved_profiles("", profiles, recents, PROFILE_MATCH_LIMIT);
            rows.extend(
                recent_profiles
                    .into_iter()
                    .filter(|profile| recent_ids.contains(profile.id.as_str()))
                    .map(profile_row),
            );
        }
        rows
    }

    fn quick_row(&self, cx: &App, connection: QuickConnection) -> Row {
        let endpoint = format_endpoint(&QuickConnectEndpoint {
            username: Some(connection.username.clone()),
            host: connection.host.clone(),
            port: Some(connection.port),
        });
        Row {
            label: self.connect_label(cx, &connection.title),
            detail: endpoint,
            checked: false,
            action: RowAction::QuickConnect(connection),
        }
    }

    fn add_user_row(&self, cx: &App, text: &str, target: &QuickConnectTarget) -> Row {
        let endpoint = format_endpoint(&QuickConnectEndpoint {
            username: target.username.clone(),
            host: target.host.clone(),
            port: target.port,
        });
        let (text, caret) =
            if target.username.is_none() && (text.starts_with("ssh ") || text.starts_with('-')) {
                (format!("{text} -l "), text.chars().count() + 4)
            } else if target.username.is_none() {
                (format!("@{text}"), 0)
            } else {
                (text.to_owned(), text.chars().count())
            };
        let detail = recent::text(&self.locale, keys::QUICK_CONNECT_ADD_USER, cx);
        Row {
            label: self.connect_label(cx, &endpoint),
            detail,
            checked: false,
            action: RowAction::AddUser { text, caret },
        }
    }

    fn connect_label(&self, cx: &App, target: &str) -> String {
        let arguments = [("target", ArgumentValue::Text(target))];
        self.locale
            .read(cx)
            .translate(
                TranslationRequest::new(keys::QUICK_CONNECT_CONNECT_TO).arguments(&arguments),
            )
            .unwrap_or_else(|_| target.to_owned())
    }

    /// The profile core parses from exactly the text the quick-connect row
    /// reads, against the same `~/.ssh/config` entries, so the form opens
    /// filled in for a connection and blank for anything else.
    fn new_connection_draft(&self, cx: &App) -> Option<SavedProfile> {
        let config_hosts = self
            .catalog
            .as_ref()
            .and_then(|catalog| catalog.as_ref().ok())
            .map(|catalog| catalog.config_hosts.as_slice())
            .unwrap_or(&[]);
        draft_profile_from_input(self.field.read(cx).value().trim(), config_hosts)
    }
}

fn profile_row(profile: LauncherProfile) -> Row {
    Row {
        label: profile.name,
        detail: profile.endpoint,
        checked: false,
        action: RowAction::Profile(profile.id),
    }
}

fn config_matches(query: &str, host: &SshConfigHost) -> bool {
    [
        host.host_pattern.as_str(),
        host.host.as_deref().unwrap_or_default(),
        host.username.as_deref().unwrap_or_default(),
    ]
    .iter()
    .any(|field| field.to_lowercase().contains(query))
}

fn shell_matches(query: &str, shell: &ShellChoiceRow) -> bool {
    if query.is_empty() {
        return true;
    }
    [
        shell.label.as_str(),
        shell.detail.as_str(),
        "local terminal",
    ]
    .iter()
    .any(|field| field.to_lowercase().contains(query))
}

impl LauncherSearch {
    fn finish(&mut self, action: Option<RowAction>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(RowAction::AddUser { text, caret }) = action {
            let utf16 = text
                .chars()
                .take(caret)
                .map(|character| character.len_utf16() as u32)
                .sum();
            self.field.update(cx, |field, cx| {
                field.set_value(text, window, cx);
                field.set_cursor_position(Position::new(0, utf16), window, cx);
            });
            self.selected = 0;
            cx.notify();
            return;
        }
        let new_connection = if matches!(action, Some(RowAction::NewConnection)) {
            self.new_connection_draft(cx)
        } else {
            None
        };
        let _ = self.owner.update(cx, |owner, cx| {
            owner.dismiss_picker(window, cx);
            match action {
                Some(RowAction::Shell { choice, title }) => {
                    owner.open_shell_choice(choice, Some(title), window, cx);
                }
                Some(RowAction::Profile(id)) => recent::confirm(owner, id, window, cx),
                Some(RowAction::QuickConnect(connection)) => {
                    owner.open_quick_connection(connection, window, cx);
                }
                Some(RowAction::NewConnection) => {
                    owner.open_new_saved_connection(new_connection, window, cx)
                }
                Some(RowAction::AddUser { .. }) | None => {}
            }
        });
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.prefer_character_input {
            return;
        }
        match event.keystroke.key.as_str() {
            "escape" => self.finish(None, window, cx),
            key @ ("up" | "down") => {
                let len = self.rows(cx).len();
                if len > 0 {
                    self.selected = (self.selected + if key == "up" { len - 1 } else { 1 }) % len;
                }
                cx.notify();
            }
            _ => return,
        }
        window.prevent_default();
        cx.stop_propagation();
    }
}

#[cfg(test)]
impl LauncherSearch {
    pub(in crate::terminal_demo::window) fn test_labels(&self, cx: &App) -> Vec<String> {
        self.rows(cx)
            .into_iter()
            .map(|row| format!("{} · {}", row.label, row.detail))
            .collect()
    }
}

impl Render for LauncherSearch {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(owner) = self.owner.upgrade() else {
            return div();
        };
        let ui = &owner.read(cx).ui;
        let theme = ui.theme;
        let locale = ui.locale.clone();
        let rows = self.rows(cx);
        let selected = self.selected.min(rows.len().saturating_sub(1));
        let mut list = div().id("launcher-search-results").flex().flex_col();
        if rows.is_empty() {
            let key = match &self.catalog {
                None => keys::SAVED_CONNECTIONS_LOADING,
                Some(Err(_)) => keys::SAVED_CONNECTIONS_LOAD_FAILED,
                _ => keys::PROFILES_SEARCH_EMPTY,
            };
            list = list.child(recent::text(&locale, key, cx));
        }
        for (index, row) in rows.into_iter().enumerate() {
            let label = row.label.clone();
            let detail = row.detail.clone();
            list = list.child(
                div()
                    .id(format!("launcher-row-{index}"))
                    .debug_selector(move || format!("launcher-row-{index}"))
                    .role(gpui_kit::Role::Button)
                    .aria_label(format!("{label} · {detail}"))
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .when(index == selected, |row| {
                        row.bg(hsla(theme.colors.tab_active, 1.))
                    })
                    .child(if row.checked { "✓" } else { " " })
                    .child(format!("{label} · {detail}"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.finish(Some(row.action.clone()), window, cx)
                    })),
            );
        }
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .justify_center()
            .items_start()
            .pt(px(40.))
            .capture_key_down(cx.listener(Self::key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.finish(None, window, cx)),
            )
            .child(
                div()
                    .id("launcher-search")
                    .w(px(520.))
                    .max_w_full()
                    .max_h(px(460.))
                    .p_2()
                    .rounded_md()
                    .shadow_lg()
                    .bg(hsla(theme.colors.popover, 1.))
                    .text_color(hsla(theme.colors.popover_foreground, 1.))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(input(
                        &self.field,
                        InputOptions {
                            theme: Some(theme),
                            ..Default::default()
                        },
                    ))
                    .child(list.overflow_y_scroll().max_h(px(390.))),
            )
    }
}
