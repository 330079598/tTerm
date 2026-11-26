//! The local-shells list rows and the editor panel.

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Div, Entity, FontWeight, Role, Stateful, div, px};
use tterm_core::config::local_shells::LocalShell;
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{ArgumentValue, TranslationRequest};
use tterm_core::workspace::local_shells::types::{ShellEntry, ShellEntryKind};
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::icons::{
    ARROW_DOWN, ARROW_UP, EYE, EYE_OFF, PLUS, TRIANGLE_ALERT, X, icon,
};
use tterm_ui::components::input::{InputOptions, input};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use super::list_actions::{can_hide, can_move_custom};
use super::path_status::EditorStatus;
use super::{
    ARGS_INPUT_ID, CANCEL_ID, COPY_PREFIX, DEFAULT_PREFIX, DETECTED_PREFIX, EDIT_PREFIX, EDITOR_ID,
    EDITOR_PADDING, ERROR_ID, HIDDEN_PREFIX, LocalShellsSetting, MISSING_ID, MISSING_PREFIX,
    MOVE_DOWN_PREFIX, MOVE_UP_PREFIX, NAME_INPUT_ID, PATH_INPUT_ID, REMOVE_PREFIX, ROW_PAD_Y,
    ROW_PREFIX, RowLabels, RowSlot, SAVE_ID, SECTION_GAP, SET_DEFAULT_PREFIX, VISIBILITY_PREFIX,
};

/// What one row's action buttons need besides their translated labels.
struct RowActions<'a> {
    slot: RowSlot,
    entry: &'a ShellEntry,
    is_default: bool,
    state: ButtonState,
    custom: Option<&'a LocalShell>,
}

impl LocalShellsSetting {
    /// The editor's answer for the path it holds, as the line under the field.
    fn status_line(&self, cx: &App) -> Option<Div> {
        let theme = self.theme;
        match self.paths.editor_status().clone() {
            EditorStatus::Idle => None,
            EditorStatus::Checking => Some(status_text(
                theme,
                self.text(cx, keys::LOCAL_SHELLS_CHECKING),
            )),
            EditorStatus::Found(program) => {
                let arguments = [("program", ArgumentValue::Text(&program))];
                let request =
                    TranslationRequest::new(keys::LOCAL_SHELLS_STARTS).arguments(&arguments);
                Some(status_text(theme, self.translated(cx, request)))
            }
            EditorStatus::NotFound => Some(status_alert(
                theme,
                self.text(cx, keys::LOCAL_SHELLS_NOT_FOUND),
            )),
        }
    }

    /// The name, its detail and the badges that mark a row's state.
    fn row_label_line(
        &self,
        cx: &App,
        slot: RowSlot,
        entry: &ShellEntry,
        is_default: bool,
        missing: bool,
        labels: &RowLabels<'_>,
    ) -> Div {
        let mut label_line = div().flex().items_center().gap(px(6.0)).child(
            div()
                .min_w(px(0.0))
                .truncate()
                .text_size(px(13.0))
                .font_weight(FontWeight(500.0))
                .child(entry.label.clone()),
        );
        if is_default {
            label_line = label_line.child(list_badge(
                self.theme,
                labels.default_badge,
                format!("{DEFAULT_PREFIX}{}", slot.entry),
            ));
        }
        if entry.kind == ShellEntryKind::Detected {
            label_line = label_line.child(list_badge(
                self.theme,
                labels.detected_badge,
                format!("{DETECTED_PREFIX}{}", slot.entry),
            ));
        }
        if entry.hidden {
            label_line = label_line.child(list_badge(
                self.theme,
                labels.hidden_badge,
                format!("{HIDDEN_PREFIX}{}", slot.entry),
            ));
        }
        if missing {
            label_line = label_line.child(missing_badge(
                self.theme,
                self.text(cx, keys::LOCAL_SHELLS_MISSING),
                format!("{MISSING_PREFIX}{}", slot.custom.unwrap_or_default()),
            ));
        }
        label_line
    }

    /// The row's action buttons, in the order the row renders them.
    fn row_actions(
        &self,
        cx: &mut Context<Self>,
        row: &RowActions<'_>,
        labels: &RowLabels<'_>,
    ) -> Div {
        let mut actions = div().flex().items_center().gap(px(2.0)).flex_shrink_0();
        if row.entry.kind == ShellEntryKind::Detected {
            actions = self.detected_actions(actions, cx, row, labels);
        }
        if row.slot.custom.is_some() && row.custom.is_some() {
            actions = self.custom_actions(actions, cx, row, labels);
        }
        if !row.is_default {
            actions = self.default_action(actions, cx, row, labels);
        }
        actions
    }

    /// Copies a detected shell into the user's own list, and hides it there.
    fn detected_actions(
        &self,
        actions: Div,
        cx: &mut Context<Self>,
        row: &RowActions<'_>,
        labels: &RowLabels<'_>,
    ) -> Div {
        let entry = row.entry;
        let busy = row.state.disabled;
        let copy_selector = format!("{COPY_PREFIX}{}", row.slot.entry);
        let copy_label = labels.copy.to_string();
        let copied = entry.clone();
        let visibility_selector = format!("{VISIBILITY_PREFIX}{}", row.slot.entry);
        let (visibility_label, visibility_icon, key, hidden) = if entry.hidden {
            (labels.show.to_string(), EYE, entry.key.clone(), false)
        } else {
            (labels.hide.to_string(), EYE_OFF, entry.key.clone(), true)
        };
        let visibility_state = ButtonState {
            disabled: busy || (hidden && !can_hide(row.is_default, entry.hidden)),
            ..ButtonState::default()
        };
        actions
            .child(
                icon_action(self.theme, copy_selector, copy_label, PLUS, row.state).on_click(
                    cx.listener(move |this, _, window, cx| this.copy_shell(&copied, window, cx)),
                ),
            )
            .child(
                icon_action(
                    self.theme,
                    visibility_selector,
                    visibility_label,
                    visibility_icon,
                    visibility_state,
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.set_hidden(key.clone(), hidden, cx)),
                ),
            )
    }

    /// Moves, edits and removes one of the user's own shells.
    fn custom_actions(
        &self,
        actions: Div,
        cx: &mut Context<Self>,
        row: &RowActions<'_>,
        labels: &RowLabels<'_>,
    ) -> Div {
        let busy = row.state.disabled;
        let custom_index = row.slot.custom.unwrap_or_default();
        let stable_id = row.custom.map(|shell| shell.id.clone());
        let up_selector = format!("{MOVE_UP_PREFIX}{custom_index}");
        let up_id = stable_id.clone();
        let up_state = ButtonState {
            disabled: busy || !can_move_custom(custom_index, row.slot.custom_count, -1),
            ..ButtonState::default()
        };
        let down_selector = format!("{MOVE_DOWN_PREFIX}{custom_index}");
        let down_id = stable_id.clone();
        let down_state = ButtonState {
            disabled: busy || !can_move_custom(custom_index, row.slot.custom_count, 1),
            ..ButtonState::default()
        };
        let edit_selector = format!("{EDIT_PREFIX}{custom_index}");
        let id = stable_id.clone();
        let remove_selector = format!("{REMOVE_PREFIX}{custom_index}");
        let remove_id = stable_id;
        actions
            .child(
                icon_action(
                    self.theme,
                    up_selector,
                    labels.move_up.to_string(),
                    ARROW_UP,
                    up_state,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(id) = up_id.clone() {
                        this.move_shell(id, -1, cx);
                    }
                })),
            )
            .child(
                icon_action(
                    self.theme,
                    down_selector,
                    labels.move_down.to_string(),
                    ARROW_DOWN,
                    down_state,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(id) = down_id.clone() {
                        this.move_shell(id, 1, cx);
                    }
                })),
            )
            .child(
                button(
                    edit_selector.clone(),
                    labels.edit.to_string(),
                    Variant::Ghost,
                    ButtonSize::Xs,
                    row.state,
                    self.theme,
                )
                .debug_selector(move || edit_selector.clone())
                .role(Role::Button)
                .aria_label(labels.edit.to_string())
                .on_click(cx.listener(move |this, _, window, cx| {
                    if this.saving || this.list_saving {
                        return;
                    }
                    // Resolve the shell again by its stable id so the editor
                    // opens the latest values, not a row captured at render.
                    let shell = id.as_deref().and_then(|id| {
                        this.config
                            .read(cx)
                            .config()
                            .local_shells
                            .iter()
                            .find(|shell| shell.id == id)
                            .cloned()
                    });
                    if let Some(shell) = shell {
                        this.start_edit(shell, window, cx);
                    }
                })),
            )
            .child(
                icon_action(
                    self.theme,
                    remove_selector,
                    labels.remove.to_string(),
                    X,
                    row.state,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(id) = remove_id.clone() {
                        this.remove_shell(id, cx);
                    }
                })),
            )
    }

    /// Makes a shell other than the default one the default.
    fn default_action(
        &self,
        actions: Div,
        cx: &mut Context<Self>,
        row: &RowActions<'_>,
        labels: &RowLabels<'_>,
    ) -> Div {
        let selector = format!("{SET_DEFAULT_PREFIX}{}", row.slot.entry);
        let choice = row.entry.choice.clone();
        actions.child(
            button(
                selector.clone(),
                labels.make_default.to_string(),
                Variant::Ghost,
                ButtonSize::Xs,
                row.state,
                self.theme,
            )
            .debug_selector(move || selector.clone())
            .role(Role::Button)
            .aria_label(labels.make_default.to_string())
            .on_click(cx.listener(move |this, _, _, cx| this.set_default(choice.clone(), cx))),
        )
    }

    pub(super) fn row(
        &self,
        cx: &mut Context<Self>,
        slot: RowSlot,
        entry: &ShellEntry,
        is_default: bool,
        editing: bool,
        labels: &RowLabels<'_>,
    ) -> Stateful<Div> {
        let colors = self.theme.colors;
        let custom = entry.choice.shell_id.as_deref().and_then(|id| {
            self.config
                .read(cx)
                .config()
                .local_shells
                .iter()
                .find(|shell| shell.id == id)
                .cloned()
        });
        let missing = custom
            .as_ref()
            .is_some_and(|shell| self.paths.saved_missing(&shell.path));
        let row_selector = format!("{ROW_PREFIX}{}", slot.entry);
        let label_line = self.row_label_line(cx, slot, entry, is_default, missing, labels);
        let actions = self.row_actions(
            cx,
            &RowActions {
                slot,
                entry,
                is_default,
                state: ButtonState {
                    disabled: self.saving || self.list_saving,
                    ..ButtonState::default()
                },
                custom: custom.as_ref(),
            },
            labels,
        );

        div()
            .id(row_selector.clone())
            .debug_selector(move || row_selector.clone())
            .flex()
            .flex_col()
            .py(px(ROW_PAD_Y))
            .border_b(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .when(editing, |row| row.bg(hsla(colors.primary, 0.05)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().min_w(px(0.0)).flex_1().child(label_line).when(
                        !entry.detail.is_empty(),
                        |column| {
                            column.child(
                                div()
                                    .truncate()
                                    .text_size(px(12.0))
                                    .text_color(hsla(colors.muted_foreground, 1.0))
                                    .child(entry.detail.clone()),
                            )
                        },
                    ))
                    .child(actions),
            )
    }

    pub(super) fn editor_panel(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(draft) = &self.editor else {
            return div().id("settings-local-shells-no-editor");
        };
        let theme = self.theme;
        let program = self.text(cx, keys::LOCAL_SHELLS_PROGRAM);
        let name = self.text(cx, keys::LOCAL_SHELLS_NAME);
        let args = self.text(cx, keys::LOCAL_SHELLS_ARGS);
        let save = self.text(cx, keys::LOCAL_SHELLS_SAVE);
        let cancel = self.text(cx, keys::LOCAL_SHELLS_CANCEL);
        let path_input = draft.path.clone();
        let name_input = draft.name.clone();
        let args_input = draft.args.clone();
        let status_line = self.status_line(cx);
        let error = self.error.clone();
        let state = ButtonState {
            disabled: self.saving,
            ..ButtonState::default()
        };
        let mut panel = div()
            .id(EDITOR_ID)
            .debug_selector(|| EDITOR_ID.to_string())
            .mt(px(SECTION_GAP))
            .mb(px(SECTION_GAP))
            .p(px(EDITOR_PADDING))
            .rounded(px(6.0))
            .border(px(1.0))
            .border_color(hsla(theme.colors.border, 1.0))
            .bg(hsla(theme.colors.muted, 0.4))
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(field(theme, program, PATH_INPUT_ID, &path_input))
                    .when_some(status_line, |wrapper, line| wrapper.child(line)),
            )
            .child(
                div()
                    .flex()
                    .gap(px(SECTION_GAP))
                    .child(div().min_w(px(0.0)).flex_1().child(field(
                        theme,
                        name,
                        NAME_INPUT_ID,
                        &name_input,
                    )))
                    .child(div().min_w(px(0.0)).flex_1().child(field(
                        theme,
                        args,
                        ARGS_INPUT_ID,
                        &args_input,
                    ))),
            );
        if let Some(error) = error {
            panel = panel.child(
                div()
                    .id(ERROR_ID)
                    .debug_selector(|| ERROR_ID.to_string())
                    .text_size(px(12.0))
                    .text_color(hsla(theme.colors.destructive, 1.0))
                    .child(error),
            );
        }
        panel.child(
            div()
                .flex()
                .justify_end()
                .gap(px(SECTION_GAP))
                .child(
                    button(
                        CANCEL_ID,
                        cancel,
                        Variant::Ghost,
                        ButtonSize::Sm,
                        ButtonState::default(),
                        theme,
                    )
                    .debug_selector(|| CANCEL_ID.to_string())
                    .role(Role::Button)
                    .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                )
                .child(
                    button(
                        SAVE_ID,
                        save.clone(),
                        Variant::Default,
                        ButtonSize::Sm,
                        state,
                        theme,
                    )
                    .debug_selector(|| SAVE_ID.to_string())
                    .role(Role::Button)
                    .aria_label(save)
                    .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                ),
        )
    }
}

fn field(theme: Theme, label: String, id: &'static str, state: &Entity<InputState>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight(500.0))
                .child(label),
        )
        .child(
            input(
                state,
                InputOptions {
                    theme: Some(theme),
                    ..InputOptions::default()
                },
            )
            .id(id)
            .debug_selector(move || id.to_string()),
        )
}

/// A short label beside a row's name, such as the default or detected mark.
fn list_badge(theme: Theme, label: &str, selector: String) -> Stateful<Div> {
    div()
        .id(selector.clone())
        .debug_selector(move || selector.clone())
        .aria_label(label.to_string())
        .px(px(6.0))
        .py(px(1.0))
        .rounded(px(4.0))
        .border(px(1.0))
        .border_color(hsla(theme.colors.border, 1.0))
        .text_size(px(11.0))
        .line_height(px(14.0))
        .text_color(hsla(theme.colors.muted_foreground, 1.0))
        .child(label.to_string())
}

/// A compact row action whose icon carries the action and whose accessible
/// label names it.
fn icon_action(
    theme: Theme,
    selector: String,
    label: String,
    data: &'static [u8],
    state: ButtonState,
) -> Stateful<Div> {
    let color = hsla(theme.colors.foreground, 1.0);
    button(
        selector.clone(),
        "",
        Variant::Ghost,
        ButtonSize::IconXs,
        state,
        theme,
    )
    .debug_selector(move || selector.clone())
    .role(Role::Button)
    .aria_label(label)
    .child(icon(data, color, 14.0))
}

/// A saved shell's path is known to lead nowhere here.
fn missing_badge(theme: Theme, label: String, selector: String) -> Stateful<Div> {
    let color = hsla(theme.colors.destructive, 1.0);
    div()
        .id(selector)
        .debug_selector(|| MISSING_ID.to_string())
        .flex()
        .items_center()
        .gap(px(4.0))
        .text_size(px(12.0))
        .text_color(color)
        .child(icon(TRIANGLE_ALERT, color, 12.0))
        .child(label)
}

fn status_text(theme: Theme, label: String) -> Div {
    div()
        .text_size(px(12.0))
        .line_height(px(16.0))
        .text_color(hsla(theme.colors.muted_foreground, 1.0))
        .child(label)
}

fn status_alert(theme: Theme, label: String) -> Div {
    let color = hsla(theme.colors.destructive, 1.0);
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .text_size(px(12.0))
        .line_height(px(16.0))
        .text_color(color)
        .child(icon(TRIANGLE_ALERT, color, 12.0))
        .child(label)
}
