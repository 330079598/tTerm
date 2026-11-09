//! Dialog primitives matching `src/components/ui/dialog.tsx`.
//!
//! The component keeps the overlay, content, header, footer and close button in
//! one stateful view so keyboard and pointer dismissal use the same state. The
//! caller supplies the title, description and buttons; this mirrors the
//! production `app-dialog` composition without coupling the primitive to the
//! application promise based hooks.

use super::button::{ButtonSize, ButtonState, Variant, button};
use super::icons::{X, icon};
use super::input::InputOptions;
use crate::tokens::gpui::hsla;
use crate::tokens::theme::Theme;
use gpui_kit::component::input::InputState;
use gpui_kit::{
    Context, ElementId, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, px,
};

pub const OVERLAY_SLOT: &str = "dialog-overlay";
pub const CONTENT_SLOT: &str = "dialog-content";
pub const HEADER_SLOT: &str = "dialog-header";
pub const FOOTER_SLOT: &str = "dialog-footer";
pub const TITLE_SLOT: &str = "dialog-title";
pub const DESCRIPTION_SLOT: &str = "dialog-description";
pub const CLOSE_SLOT: &str = "dialog-close";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    Confirm,
    Prompt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogAction {
    Cancel,
    Confirm,
}

#[derive(Clone, Debug)]
pub struct DialogOptions {
    pub kind: DialogKind,
    pub title: String,
    pub description: Option<String>,
    pub label: Option<String>,
    pub placeholder: Option<String>,
    pub destructive: bool,
}

impl DialogOptions {
    pub fn confirm(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            kind: DialogKind::Confirm,
            title: title.into(),
            description: Some(description.into()),
            label: None,
            placeholder: None,
            destructive: false,
        }
    }

    pub fn prompt(title: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            kind: DialogKind::Prompt,
            title: title.into(),
            description: None,
            label: Some(label.into()),
            placeholder: None,
            destructive: false,
        }
    }
}

pub struct Dialog {
    pub options: DialogOptions,
    pub theme: Theme,
    open: bool,
    focus: FocusHandle,
    restore_focus: Option<FocusHandle>,
    pub input: Option<Entity<InputState>>,
    pub last_action: Option<DialogAction>,
}

impl Dialog {
    pub fn new(
        options: DialogOptions,
        theme: Theme,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            options,
            theme,
            open: true,
            focus: cx.focus_handle(),
            restore_focus: None,
            input: None,
            last_action: None,
        }
    }

    pub fn with_input(mut self, input: Entity<InputState>) -> Self {
        self.input = Some(input);
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    pub fn set_restore_focus(&mut self, focus: FocusHandle) {
        self.restore_focus = Some(focus);
    }

    pub fn close(
        &mut self,
        action: Option<DialogAction>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open = false;
        self.last_action = action;
        if let Some(focus) = &self.restore_focus {
            focus.focus(window, cx);
        }
        cx.notify();
    }

    pub fn reopen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = true;
        self.focus.focus(window, cx);
        cx.notify();
    }

    /// Returns the next focusable index, wrapping in either direction.
    pub fn cycle_focus(current: usize, count: usize, backwards: bool) -> usize {
        if count == 0 {
            return 0;
        }
        if backwards {
            (current + count - 1) % count
        } else {
            (current + 1) % count
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => {
                window.prevent_default();
                self.close(None, window, cx);
            }
            // GPUI's regular focus traversal already wraps focusable children;
            // keeping this hook on the content makes the contract explicit and
            // lets callers replace it with a custom focus list when needed.
            "tab" => {}
            _ => {}
        }
    }

    fn render_button(
        &self,
        id: &'static str,
        label: &'static str,
        variant: Variant,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        button(
            id,
            label,
            variant,
            ButtonSize::Default,
            ButtonState::default(),
            self.theme,
        )
    }

    fn render_content(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let colors = self.theme.colors;
        let mut content = div()
            .id(ElementId::Name(CONTENT_SLOT.into()))
            .relative()
            .w(px(448.0))
            .max_w(px(736.0))
            .p(px(24.0))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .bg(hsla(colors.background, 1.0))
            .shadow(vec![
                gpui_kit::BoxShadow::new(px(0.0), px(10.0), gpui_kit::rgba(0x00000026).into())
                    .blur_radius(px(12.0)),
            ])
            .flex()
            .flex_col()
            .gap(px(16.0))
            .track_focus(&self.focus)
            .focusable()
            .on_key_down(cx.listener(Self::handle_key))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default());

        let mut header = div()
            .id(ElementId::Name(HEADER_SLOT.into()))
            .flex()
            .flex_col()
            .gap(px(8.0));
        let title = div()
            .id(ElementId::Name(TITLE_SLOT.into()))
            .text_size(px(18.0))
            .line_height(px(18.0))
            .font_weight(gpui_kit::FontWeight(600.0))
            .text_color(hsla(colors.foreground, 1.0))
            .child(self.options.title.clone());
        header = header.child(title);
        if let Some(description) = &self.options.description {
            header = header.child(
                div()
                    .id(ElementId::Name(DESCRIPTION_SLOT.into()))
                    .text_size(px(14.0))
                    .line_height(px(20.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(description.clone()),
            );
        }
        content = content.child(header);

        if self.options.kind == DialogKind::Prompt {
            let mut field = div().flex().flex_col().gap(px(8.0));
            if let Some(label) = &self.options.label {
                field = field.child(
                    div()
                        .text_size(px(14.0))
                        .text_color(hsla(colors.foreground, 1.0))
                        .child(label.clone()),
                );
            }
            if let Some(state) = &self.input {
                field = field.child(super::input::input(
                    state,
                    InputOptions {
                        theme: Some(self.theme),
                        ..InputOptions::default()
                    },
                ));
            }
            content = content.child(field);
        }

        let cancel = self
            .render_button("dialog-cancel", "Cancel", Variant::Outline)
            .on_click(cx.listener(|this, _, window, cx| {
                this.close(Some(DialogAction::Cancel), window, cx)
            }));
        let confirm = self
            .render_button(
                "dialog-confirm",
                if self.options.destructive {
                    "Delete"
                } else if self.options.kind == DialogKind::Prompt {
                    "OK"
                } else {
                    "Confirm"
                },
                if self.options.destructive {
                    Variant::Destructive
                } else {
                    Variant::Default
                },
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.close(Some(DialogAction::Confirm), window, cx)
            }));
        let footer = div()
            .id(ElementId::Name(FOOTER_SLOT.into()))
            .flex()
            .justify_end()
            .gap(px(8.0))
            .child(cancel)
            .child(confirm);
        content
            .child(footer)
            .child(
                div()
                    .id(ElementId::Name(CLOSE_SLOT.into()))
                    .absolute()
                    .top(px(16.0))
                    .right(px(16.0))
                    .child(
                        button(
                            "dialog-close-button",
                            "",
                            Variant::Ghost,
                            ButtonSize::IconSm,
                            ButtonState::default(),
                            self.theme,
                        )
                        .child(icon(X, hsla(colors.muted_foreground, 1.0), 16.0))
                        .on_click(cx.listener(|this, _, window, cx| this.close(None, window, cx))),
                    ),
            )
            .into_any_element()
    }
}

impl Render for Dialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let background = hsla(self.theme.colors.background, 0.8);
        let content = self.render_content(cx);
        div()
            .id(ElementId::Name(OVERLAY_SLOT.into()))
            .size_full()
            .absolute()
            .inset_0()
            .bg(background)
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.close(None, window, cx)),
            )
            .child(content)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focus_cycle_wraps_in_both_directions() {
        assert_eq!(Dialog::cycle_focus(1, 2, false), 0);
        assert_eq!(Dialog::cycle_focus(0, 2, true), 1);
        assert_eq!(Dialog::cycle_focus(0, 0, false), 0);
    }
}
