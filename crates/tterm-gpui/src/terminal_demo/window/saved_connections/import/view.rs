//! The SSH config import dialog.

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, Entity, FontWeight, MouseButton, Role, Stateful, div, px};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{ArgumentValue, TranslationRequest};
use tterm_core::profiles::types::{SshConfigImportHost, SshConfigImportProxy};
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::input::{InputOptions, input};
use tterm_ui::tokens::color::Hsl;
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use super::super::SavedConnectionsUi;
use super::{
    IMPORT_CANCEL_ID, IMPORT_CHOOSE_DIRECTORY_ID, IMPORT_CHOOSE_FILE_ID, IMPORT_CONFIRM_ID,
    IMPORT_COUNT_ID, IMPORT_DIALOG_ID, IMPORT_EMPTY_ID, IMPORT_ERROR_ID, IMPORT_FORWARDS_ID,
    IMPORT_GROUP_ID, IMPORT_LIST_ID, IMPORT_LOADING_ID, IMPORT_OVERWRITE_ID, IMPORT_PREVIEW_ID,
    IMPORT_ROW_PREFIX, IMPORT_SELECT_ALL_ID, IMPORT_SOURCE_ID, IMPORT_WARNINGS_ID, ImportDialog,
    ImportSourceKind,
};

const DIALOG_WIDTH: f32 = 680.0;
const DIALOG_PADDING: f32 = 24.0;
const DIALOG_RADIUS: f32 = 8.0;
const LIST_MAX_HEIGHT: f32 = 320.0;
const CHECK_SIZE: f32 = 14.0;

impl SavedConnectionsUi {
    /// The import dialog replaces the picker body inside the same modal layer.
    pub(in crate::terminal_demo::window::saved_connections) fn import_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let Some(dialog) = self.import.as_ref() else {
            return div().id(IMPORT_DIALOG_ID);
        };
        let theme = self.theme;
        let colors = theme.colors;
        let title = self.text(cx, keys::SSH_CONFIG_IMPORT_TITLE);
        let description = self.text(cx, keys::SSH_CONFIG_IMPORT_DESCRIPTION);
        let mut body = div()
            .id(IMPORT_DIALOG_ID)
            .debug_selector(|| IMPORT_DIALOG_ID.to_string())
            .role(Role::Dialog)
            .aria_label(title.clone())
            .aria_description(description.clone())
            .w(px(DIALOG_WIDTH))
            .max_h(px(720.0))
            .p(px(DIALOG_PADDING))
            .flex()
            .flex_col()
            .gap(px(14.0))
            .overflow_y_scroll()
            .rounded(px(DIALOG_RADIUS))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .bg(hsla(colors.background, 1.0))
            .text_color(hsla(colors.foreground, 1.0))
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(FontWeight(600.0))
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(description),
            )
            .child(self.import_paths(cx, dialog))
            .child(self.import_toolbar(cx, dialog));
        if let Some(error) = &dialog.error {
            body = body.child(status_line(
                IMPORT_ERROR_ID,
                error.clone(),
                colors.destructive,
                Role::Status,
            ));
        } else if let Some(preview) = &dialog.preview
            && !preview.warnings.is_empty()
        {
            body = body.child(status_line(
                IMPORT_WARNINGS_ID,
                preview.warnings.join(" "),
                colors.warning,
                Role::Status,
            ));
        }
        body.child(self.import_list(cx, dialog))
            .child(self.import_footer(cx, dialog))
    }

    fn import_paths(&self, cx: &mut Context<Self>, dialog: &ImportDialog) -> Div {
        let theme = self.theme;
        let colors = theme.colors;
        let source_label = self.text(cx, keys::SSH_CONFIG_IMPORT_SOURCE);
        let group_label = self.text(cx, keys::SSH_CONFIG_IMPORT_GROUP);
        let preview_label = self.text(cx, keys::SSH_CONFIG_IMPORT_PREVIEW);
        let choose_file_label = self.text(cx, keys::SSH_CONFIG_IMPORT_CHOOSE_SOURCE_FILE);
        let choose_directory_label = self.text(cx, keys::SSH_CONFIG_IMPORT_CHOOSE_SOURCE_DIRECTORY);
        let preview_disabled = dialog.loading || dialog.importing;
        let choose_disabled = preview_disabled;
        let preview_button = button(
            IMPORT_PREVIEW_ID,
            preview_label.clone(),
            Variant::Outline,
            ButtonSize::Sm,
            ButtonState {
                disabled: preview_disabled,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| IMPORT_PREVIEW_ID.to_string())
        .role(Role::Button)
        .aria_label(preview_label)
        .when(!preview_disabled, |preview| {
            preview.on_click(cx.listener(|this, _, window, cx| {
                this.start_import_preview(window, cx);
            }))
        });
        let choose_file_button = button(
            IMPORT_CHOOSE_FILE_ID,
            choose_file_label.clone(),
            Variant::Outline,
            ButtonSize::Sm,
            ButtonState {
                disabled: choose_disabled,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| IMPORT_CHOOSE_FILE_ID.to_string())
        .role(Role::Button)
        .aria_label(choose_file_label)
        .when(!choose_disabled, |button| {
            button.on_click(cx.listener(|this, _, window, cx| {
                this.choose_import_source(ImportSourceKind::File, window, cx);
            }))
        });
        let choose_directory_button = button(
            IMPORT_CHOOSE_DIRECTORY_ID,
            choose_directory_label.clone(),
            Variant::Outline,
            ButtonSize::Sm,
            ButtonState {
                disabled: choose_disabled,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| IMPORT_CHOOSE_DIRECTORY_ID.to_string())
        .role(Role::Button)
        .aria_label(choose_directory_label)
        .when(!choose_disabled, |button| {
            button.on_click(cx.listener(|this, _, window, cx| {
                this.choose_import_source(ImportSourceKind::Directory, window, cx);
            }))
        });
        div()
            .flex()
            .gap(px(12.0))
            .items_start()
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(labeled_input(
                        IMPORT_SOURCE_ID,
                        source_label,
                        &dialog.source,
                        theme,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(8.0))
                            .child(choose_file_button)
                            .child(choose_directory_button)
                            .child(preview_button),
                    ),
            )
            .child(div().min_w_0().flex_1().child(labeled_input(
                IMPORT_GROUP_ID,
                group_label,
                &dialog.group,
                theme,
            )))
            .when(dialog.loading, |row| {
                row.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(hsla(colors.muted_foreground, 1.0))
                        .child(self.text(cx, keys::SSH_CONFIG_IMPORT_LOADING)),
                )
            })
    }

    fn import_toolbar(&self, cx: &mut Context<Self>, dialog: &ImportDialog) -> Div {
        let theme = self.theme;
        let colors = theme.colors;
        let total = dialog.selectable_count();
        let selected = dialog.selected.len();
        let select_label = if dialog.all_selected() {
            self.text(cx, keys::SSH_CONFIG_IMPORT_CLEAR_SELECTION)
        } else {
            self.text(cx, keys::SSH_CONFIG_IMPORT_SELECT_ALL)
        };
        let toggle_disabled = dialog.loading || dialog.importing || total == 0;
        let select_button = button(
            IMPORT_SELECT_ALL_ID,
            select_label.clone(),
            Variant::Ghost,
            ButtonSize::Sm,
            ButtonState {
                disabled: toggle_disabled,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| IMPORT_SELECT_ALL_ID.to_string())
        .role(Role::Button)
        .aria_label(select_label)
        .when(!toggle_disabled, |button| {
            button.on_click(cx.listener(|this, _, _, cx| this.toggle_import_all(cx)))
        });
        let arguments = [
            ("selected", ArgumentValue::Number(selected as f64)),
            ("total", ArgumentValue::Number(total as f64)),
        ];
        let count_label = self.import_label(
            TranslationRequest::new(keys::SSH_CONFIG_IMPORT_SELECTED_COUNT).arguments(&arguments),
            cx,
        );
        let forwards = dialog.forward_count();
        let forwards_arguments = [("count", ArgumentValue::Number(forwards as f64))];
        let forwards_label = self.import_label(
            TranslationRequest::new(keys::SSH_CONFIG_IMPORT_IMPORT_FORWARDS)
                .arguments(&forwards_arguments),
            cx,
        );
        let forwards_disabled = dialog.loading || dialog.importing || forwards == 0;
        let forwards_on = dialog.import_forwards && forwards > 0;
        let forwards_button = button(
            IMPORT_FORWARDS_ID,
            forwards_label,
            if forwards_on {
                Variant::Default
            } else {
                Variant::Outline
            },
            ButtonSize::Sm,
            ButtonState {
                disabled: forwards_disabled,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| IMPORT_FORWARDS_ID.to_string())
        .role(Role::Button)
        .aria_selected(forwards_on)
        .when(!forwards_disabled, |button| {
            button.on_click(cx.listener(|this, _, _, cx| this.toggle_import_forwards(cx)))
        });
        let overwrite_label = self.text(cx, keys::SSH_CONFIG_IMPORT_OVERWRITE_EXISTING);
        let overwrite_disabled = dialog.loading || dialog.importing;
        let overwrite_button = button(
            IMPORT_OVERWRITE_ID,
            overwrite_label,
            if dialog.overwrite {
                Variant::Default
            } else {
                Variant::Outline
            },
            ButtonSize::Sm,
            ButtonState {
                disabled: overwrite_disabled,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| IMPORT_OVERWRITE_ID.to_string())
        .role(Role::Button)
        .aria_selected(dialog.overwrite)
        .when(!overwrite_disabled, |button| {
            button.on_click(cx.listener(|this, _, _, cx| this.toggle_import_overwrite(cx)))
        });
        div()
            .flex()
            .items_center()
            .justify_between()
            .flex_wrap()
            .gap(px(8.0))
            .py(px(4.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .id(IMPORT_COUNT_ID)
                            .debug_selector(|| IMPORT_COUNT_ID.to_string())
                            .text_size(px(13.0))
                            .text_color(hsla(colors.muted_foreground, 1.0))
                            .child(count_label),
                    )
                    .child(select_button),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(forwards_button)
                    .child(overwrite_button),
            )
    }

    fn import_list(&self, cx: &mut Context<Self>, dialog: &ImportDialog) -> Stateful<Div> {
        let colors = self.theme.colors;
        let mut list = div()
            .id(IMPORT_LIST_ID)
            .debug_selector(|| IMPORT_LIST_ID.to_string())
            .flex()
            .flex_col()
            .max_h(px(LIST_MAX_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&dialog.scroll)
            .rounded(px(6.0))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0));
        if dialog.loading {
            list = list.child(status_line(
                IMPORT_LOADING_ID,
                self.text(cx, keys::SSH_CONFIG_IMPORT_LOADING),
                colors.muted_foreground,
                Role::Status,
            ));
            return list;
        }
        let Some(preview) = &dialog.preview else {
            return list;
        };
        if preview.hosts.is_empty() {
            list = list.child(status_line(
                IMPORT_EMPTY_ID,
                self.text(cx, keys::SSH_CONFIG_IMPORT_EMPTY),
                colors.muted_foreground,
                Role::Status,
            ));
            return list;
        }
        for (index, host) in preview.hosts.iter().enumerate() {
            list = list.child(self.import_row(cx, dialog, index, host));
        }
        list
    }

    fn import_row(
        &self,
        cx: &mut Context<Self>,
        dialog: &ImportDialog,
        index: usize,
        host: &SshConfigImportHost,
    ) -> Stateful<Div> {
        let theme = self.theme;
        let colors = theme.colors;
        let selected = dialog.selected.contains(&host.host_pattern);
        let disabled = host.skipped || dialog.loading || dialog.importing;
        let arguments = [("name", ArgumentValue::Text(&host.name))];
        let aria_label = self.import_label(
            TranslationRequest::new(keys::SSH_CONFIG_IMPORT_SELECT_HOST).arguments(&arguments),
            cx,
        );
        let selector = format!("{IMPORT_ROW_PREFIX}{index}");
        let debug_selector = selector.clone();
        let pattern = host.host_pattern.clone();
        div()
            .id(selector)
            .debug_selector(move || debug_selector.clone())
            .role(Role::Button)
            .aria_label(aria_label)
            .aria_selected(selected)
            .flex()
            .items_start()
            .gap(px(10.0))
            .px(px(12.0))
            .py(px(10.0))
            .border_b(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .when(!disabled, |row| {
                row.cursor_pointer()
                    .hover(|style| style.bg(hsla(colors.accent, 1.0)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_import_host(&pattern, cx);
                    }))
            })
            .child(selection_box(selected, host.skipped, theme))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .child(self.import_row_heading(cx, host, selected))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(hsla(colors.muted_foreground, 1.0))
                            .child(dialog.endpoint(host)),
                    )
                    .when(!host.jump_hosts.is_empty(), |column| {
                        let request = TranslationRequest::new(keys::JUMP_HOST_VIA_COUNT)
                            .count(host.jump_hosts.len() as f64);
                        column.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(hsla(colors.muted_foreground, 1.0))
                                .child(self.import_label(request, cx)),
                        )
                    })
                    .when(!host.forwards.is_empty(), |column| {
                        let request =
                            TranslationRequest::new(keys::SSH_CONFIG_IMPORT_FORWARD_COUNT)
                                .count(host.forwards.len() as f64);
                        column.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(hsla(colors.muted_foreground, 1.0))
                                .child(self.import_label(request, cx)),
                        )
                    })
                    .when_some(host.proxy.as_ref(), |column, proxy| {
                        column.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(hsla(colors.muted_foreground, 1.0))
                                .child(proxy_line(proxy)),
                        )
                    })
                    .when_some(host.skip_reason.clone(), |column, reason| {
                        column.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(hsla(colors.muted_foreground, 1.0))
                                .child(reason),
                        )
                    })
                    .when(!host.warnings.is_empty(), |column| {
                        column.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(hsla(colors.warning, 1.0))
                                .child(host.warnings.join(" ")),
                        )
                    })
                    .when(!host.unsupported_options.is_empty(), |column| {
                        let arguments = [(
                            "options",
                            ArgumentValue::Text(&host.unsupported_options.join(", ")),
                        )];
                        column.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(hsla(colors.muted_foreground, 1.0))
                                .child(
                                    self.import_label(
                                        TranslationRequest::new(
                                            keys::SSH_CONFIG_IMPORT_UNSUPPORTED_OPTIONS,
                                        )
                                        .arguments(&arguments),
                                        cx,
                                    ),
                                ),
                        )
                    }),
            )
    }

    /// The host's name and the badges that mark its import state.
    fn import_row_heading(
        &self,
        cx: &Context<Self>,
        host: &SshConfigImportHost,
        selected: bool,
    ) -> Div {
        let colors = self.theme.colors;
        let warning =
            !host.skipped && (!host.warnings.is_empty() || !host.unsupported_options.is_empty());
        let mut heading = div().flex().items_center().flex_wrap().gap(px(6.0)).child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight(500.0))
                .text_color(hsla(
                    if host.skipped {
                        colors.muted_foreground
                    } else {
                        colors.foreground
                    },
                    1.0,
                ))
                .child(host.name.clone()),
        );
        if host.existing_profile_id.is_some() {
            heading = heading.child(badge(
                self.text(cx, keys::SSH_CONFIG_IMPORT_EXISTS),
                colors.muted_foreground,
            ));
        }
        heading = if host.skipped {
            heading.child(badge(
                self.text(cx, keys::SSH_CONFIG_IMPORT_SKIPPED),
                colors.muted_foreground,
            ))
        } else if warning {
            heading.child(badge(
                self.text(cx, keys::SSH_CONFIG_IMPORT_WARNING),
                colors.warning,
            ))
        } else if selected {
            heading.child(badge(
                self.text(cx, keys::SSH_CONFIG_IMPORT_READY),
                colors.success,
            ))
        } else {
            heading
        };
        heading
    }

    fn import_footer(&self, cx: &mut Context<Self>, dialog: &ImportDialog) -> Div {
        let theme = self.theme;
        let cancel = self.text(cx, keys::COMMON_CANCEL);
        let confirm = if dialog.importing {
            self.text(cx, keys::SSH_CONFIG_IMPORT_IMPORTING)
        } else {
            self.text(cx, keys::SSH_CONFIG_IMPORT_IMPORT_SELECTED)
        };
        let cancel_disabled = dialog.importing;
        let confirm_disabled = dialog.loading || dialog.importing || dialog.selected.is_empty();
        let cancel_button = button(
            IMPORT_CANCEL_ID,
            cancel.clone(),
            Variant::Outline,
            ButtonSize::Default,
            ButtonState {
                disabled: cancel_disabled,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| IMPORT_CANCEL_ID.to_string())
        .role(Role::Button)
        .aria_label(cancel)
        .when(!cancel_disabled, |button| {
            button.on_click(cx.listener(|this, _, window, cx| this.cancel_import(window, cx)))
        });
        let confirm_button = button(
            IMPORT_CONFIRM_ID,
            confirm.clone(),
            Variant::Default,
            ButtonSize::Default,
            ButtonState {
                disabled: confirm_disabled,
                ..ButtonState::default()
            },
            theme,
        )
        .debug_selector(|| IMPORT_CONFIRM_ID.to_string())
        .role(Role::Button)
        .aria_label(confirm)
        .when(!confirm_disabled, |button| {
            button.on_click(cx.listener(|this, _, window, cx| this.submit_import(window, cx)))
        });
        div()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(8.0))
            .child(cancel_button)
            .child(confirm_button)
    }
}

/// The square that shows whether a host row is chosen.
fn selection_box(selected: bool, skipped: bool, theme: Theme) -> Div {
    let colors = theme.colors;
    let border = if skipped {
        colors.border
    } else {
        colors.primary
    };
    let mut mark = div()
        .flex_none()
        .mt(px(1.0))
        .size(px(CHECK_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.0))
        .border(px(1.0))
        .border_color(hsla(border, 1.0));
    if selected {
        mark = mark
            .bg(hsla(colors.primary, 1.0))
            .text_size(px(10.0))
            .text_color(hsla(colors.primary_foreground, 1.0))
            .child("✓");
    }
    mark
}

fn badge(label: String, color: Hsl) -> Div {
    div()
        .px(px(6.0))
        .py(px(1.0))
        .rounded(px(4.0))
        .border(px(1.0))
        .border_color(hsla(color, 1.0))
        .text_size(px(10.0))
        .text_color(hsla(color, 1.0))
        .child(label)
}

fn proxy_line(proxy: &SshConfigImportProxy) -> String {
    match proxy {
        SshConfigImportProxy::Direct => "ProxyCommand none".to_string(),
        SshConfigImportProxy::Server { kind, host, port } => {
            format!("{} {host}:{port}", kind.label())
        }
        SshConfigImportProxy::Command { command } => format!("$ {command}"),
    }
}

fn labeled_input(id: &'static str, label: String, state: &Entity<InputState>, theme: Theme) -> Div {
    let aria_label = label.clone();
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight(500.0))
                .child(label),
        )
        .child(
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .aria_label(aria_label)
                .child(input(
                    state,
                    InputOptions {
                        theme: Some(theme),
                        ..InputOptions::default()
                    },
                )),
        )
}

fn status_line(id: &'static str, message: String, color: Hsl, role: Role) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .role(role)
        .aria_label(message.clone())
        .py(px(4.0))
        .text_size(px(13.0))
        .text_color(hsla(color, 1.0))
        .child(message)
}
