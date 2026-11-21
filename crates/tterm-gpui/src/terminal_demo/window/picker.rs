//! Searchable overflow menu for sessions already owned by the workspace.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Div, Entity, MouseButton, Subscription, Window, div, px};
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_ui::components::input::{InputOptions, input};
use tterm_ui::tokens::gpui::hsla;

use super::{CommandSpec, Locale, Prototype, WindowTab};

pub(super) struct TabPicker {
    field: Entity<InputState>,
    _events: Subscription,
}

/// Only display and search fields are retained, never the connection credentials.
pub(super) struct ConnectionLabel {
    terms: String,
    host: String,
}

impl ConnectionLabel {
    fn matches(&self, number: usize, title: &str, normalized_query: &str) -> bool {
        format!("{number} {title} {}", self.terms)
            .to_lowercase()
            .contains(normalized_query)
    }

    pub(super) fn new(command: &CommandSpec, options: Option<&PtyConnectionOptions>) -> Self {
        let kind = match command {
            CommandSpec::SshProfile { .. } => "ssh",
            _ => "terminal",
        };
        let name = options
            .and_then(|o| o.profile_name.as_deref())
            .unwrap_or("");
        let user = options.and_then(|o| o.username.as_deref()).unwrap_or("");
        let host = options.and_then(|o| o.host.as_deref()).unwrap_or("");
        let port = options.and_then(|o| o.port);
        Self {
            terms: format!(
                "{kind} {host} {user} {name} {}",
                port.map(|p| p.to_string()).unwrap_or_default()
            ),
            host: match port {
                Some(port) if !host.is_empty() && port != 22 => format!("{host}:{port}"),
                _ => host.to_owned(),
            },
        }
    }
}

fn text(locale: &Locale, key: TranslationKey) -> String {
    locale.text(key).unwrap_or_else(|_| key.as_str().to_owned())
}

impl WindowTab {
    pub(super) fn display_title(&self, locale: &Locale) -> String {
        self.program_title
            .borrow()
            .clone()
            .unwrap_or_else(|| text(locale, self.fallback_key))
    }
}

struct Row {
    id: usize,
    number: usize,
    title: String,
    host: String,
}

impl Prototype {
    fn matching_rows(&self, query: &str, cx: &App) -> Vec<Row> {
        let query = query.trim().to_lowercase();
        self.tabs
            .ordered()
            .into_iter()
            .enumerate()
            .filter_map(|(index, (id, tab))| {
                let number = index + 1;
                let title = tab.display_title(self.ui.locale.read(cx));
                tab.search.matches(number, &title, &query).then(|| Row {
                    id,
                    number,
                    title,
                    host: tab.search.host.clone(),
                })
            })
            .collect()
    }

    pub(super) fn open_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| InputState::new(window, cx));
        let placeholder = text(self.ui.locale.read(cx), keys::TAB_SEARCH);
        field.update(cx, |state, cx| {
            state.set_placeholder(placeholder, window, cx);
            state.focus(window, cx);
        });
        let events = cx.subscribe_in(
            &field,
            window,
            |this, field, event, window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => {
                    if let Some(row) = this.matching_rows(&field.read(cx).value(), cx).first() {
                        this.picker = None;
                        this.select_tab(row.id, window, cx);
                    }
                }
                _ => {}
            },
        );
        self.picker = Some(TabPicker {
            field,
            _events: events,
        });
        cx.notify();
    }

    fn dismiss_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.picker = None;
        self.tabs
            .active()
            .view
            .read(cx)
            .focus_handle()
            .focus(window, cx);
        cx.notify();
    }

    pub(super) fn picker_panel(&self, window: &Window, cx: &mut Context<Self>) -> Option<Div> {
        let field = self.picker.as_ref()?.field.clone();
        let query = field.read(cx).value();
        let rows = self.matching_rows(&query, cx);
        let locale = self.ui.locale.read(cx);
        let theme = self.ui.theme;
        let colors = theme.colors;
        let mut results = div()
            .id("tab-picker-results")
            .overflow_y_scroll()
            .max_h((window.viewport_size().height - px(105.)).min(px(366.)));
        if rows.is_empty() {
            results = results.child(div().p_2().child(text(locale, keys::TAB_NO_MATCH)));
        }
        for row in rows {
            let id = row.id;
            results = results.child(
                div()
                    .id(("tab-picker-row", id))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .h(px(32.))
                    .rounded_sm()
                    .cursor_pointer()
                    .when(id == self.tabs.active_id(), |row| {
                        row.bg(hsla(colors.tab_active, 1.))
                    })
                    .hover(|style| style.bg(hsla(colors.tab_hover, 1.)))
                    .child(
                        div()
                            .text_color(hsla(colors.muted_foreground, 1.))
                            .child(row.number.to_string()),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(row.title))
                    .child(div().truncate().child(row.host))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.picker = None;
                        this.select_tab(id, window, cx);
                    })),
            );
        }
        let clear = text(locale, keys::TAB_CLEAR);
        let search = div()
            .flex()
            .items_center()
            .child(div().flex_1().min_w_0().child(input(
                &field,
                InputOptions {
                    theme: Some(theme),
                    ..Default::default()
                },
            )))
            .when(!query.is_empty(), |header| {
                header.child(
                    div()
                        .id("tab-picker-clear")
                        .aria_label(clear)
                        .px_2()
                        .cursor_pointer()
                        .child("×")
                        .on_click(cx.listener(move |_, _, window, cx| {
                            field.update(cx, |state, cx| {
                                state.set_value("", window, cx);
                                state.focus(window, cx);
                            });
                        })),
                )
            });
        let viewport = window.viewport_size().width;
        let width = (viewport * 0.7).min(px(320.));
        let left = (self.overflow.trigger_right() - width)
            .min(viewport - width - px(8.))
            .max(px(8.));
        let panel = div()
            .id("tab-picker-panel")
            .absolute()
            .top(px(40.))
            .left(left)
            .w(width)
            .p_1()
            .rounded_md()
            .border_1()
            .border_color(hsla(colors.border, 1.))
            .bg(hsla(colors.popover, 1.))
            .text_color(hsla(colors.popover_foreground, 1.))
            .text_sm()
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(search)
            .child(results);
        Some(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .capture_key_down(cx.listener(
                    |this, event: &gpui_kit::KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape" && !event.prefer_character_input {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.dismiss_picker(window, cx);
                        }
                    },
                ))
                .on_any_mouse_down(
                    cx.listener(|this, _, window, cx| this.dismiss_picker(window, cx)),
                )
                .child(panel),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_tab_position_title_and_public_connection_fields_but_not_credentials() {
        let mut options = CommandSpec::Shell.connection_options().unwrap();
        options.host = Some("Build.Example".into());
        options.port = Some(2222);
        options.username = Some("Builder".into());
        options.profile_name = Some("Nightly Jobs".into());
        options.password = Some("password-marker".into());
        options.private_key_passphrase = Some("passphrase-marker".into());
        options.private_key_path = Some("keypath-marker".into());
        let command = CommandSpec::SshProfile {
            selector: "saved".into(),
        };
        let label = ConnectionLabel::new(&command, Some(&options));
        assert_eq!(label.host, "Build.Example:2222");
        for query in [
            "",
            "13",
            "mixed title",
            "ssh",
            "build.example",
            "2222",
            "builder",
            "nightly jobs",
        ] {
            assert!(label.matches(13, "Mixed Title", query), "{query}");
        }
        for query in [
            "14",
            "password-marker",
            "passphrase-marker",
            "keypath-marker",
        ] {
            assert!(!label.matches(13, "Mixed Title", query), "{query}");
        }
    }
}
