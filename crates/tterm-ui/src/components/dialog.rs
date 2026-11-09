//! Locally themed modal primitives and app-dialog compositions.
pub mod presentation;
#[cfg(test)]
mod tests;

use crate::tokens::theme::Theme;
use gpui_kit::component::input::InputState;
use gpui_kit::{Context, Entity, FocusHandle, Focusable, KeyDownEvent, MouseDownEvent, Window};
use presentation::PartObserver;
use std::rc::Rc;

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

/// Text and variant supplied by the caller, using its shared i18n catalogue.
#[derive(Clone, Debug)]
pub struct DialogOptions {
    pub kind: DialogKind,
    pub title: String,
    pub description: Option<String>,
    pub label: Option<String>,
    pub destructive: bool,
    pub cancel_text: String,
    pub confirm_text: String,
}
impl DialogOptions {
    pub fn confirm(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            kind: DialogKind::Confirm,
            title: title.into(),
            description: Some(description.into()),
            label: None,
            destructive: false,
            cancel_text: String::new(),
            confirm_text: String::new(),
        }
    }
    pub fn prompt(title: impl Into<String>, label: impl Into<String>) -> Self {
        let mut options = Self::confirm(title, "");
        options.kind = DialogKind::Prompt;
        options.description = None;
        let label = label.into();
        options.label = (!label.is_empty()).then_some(label);
        options
    }
}

/// Preventable pointer event, equivalent to React's onInteractOutside event.
pub struct InteractOutsideEvent<'a> {
    pub original_event: &'a MouseDownEvent,
    default_prevented: bool,
}
impl InteractOutsideEvent<'_> {
    pub fn prevent_default(&mut self) {
        self.default_prevented = true;
    }
    pub fn default_prevented(&self) -> bool {
        self.default_prevented
    }
}
type OutsideHandler = Rc<dyn Fn(&mut InteractOutsideEvent<'_>)>;

/// Owns modal visibility, editing state and the focus boundary.
pub struct Dialog {
    pub options: DialogOptions,
    pub theme: Theme,
    open: bool,
    focus: FocusHandle,
    controls: [FocusHandle; 3],
    restore_focus: Option<FocusHandle>,
    input: Option<Entity<InputState>>,
    initialized_focus: bool,
    outside: Option<OutsideHandler>,
    observer: Option<PartObserver>,
    pub last_action: Option<DialogAction>,
}
impl Dialog {
    pub fn new(
        options: DialogOptions,
        theme: Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            options,
            theme,
            open: true,
            focus: cx.focus_handle(),
            controls: std::array::from_fn(|_| cx.focus_handle()),
            restore_focus: window.focused(cx),
            input: None,
            initialized_focus: false,
            outside: None,
            observer: None,
            last_action: None,
        }
    }
    pub fn with_input(mut self, input: Entity<InputState>) -> Self {
        self.input = Some(input);
        self
    }
    /// Observes actual prepainted part bounds with the style used to draw them.
    pub fn observe_parts(mut self, observer: PartObserver) -> Self {
        self.observer = Some(observer);
        self
    }
    pub fn on_interact_outside(
        mut self,
        handler: impl Fn(&mut InteractOutsideEvent<'_>) + 'static,
    ) -> Self {
        self.outside = Some(Rc::new(handler));
        self
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
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
        self.restore_focus = window.focused(cx);
        self.open = true;
        self.initialized_focus = false;
        cx.notify();
    }
    fn outside(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let mut event = InteractOutsideEvent {
            original_event: event,
            default_prevented: false,
        };
        if let Some(handler) = &self.outside {
            handler(&mut event);
        }
        if !event.default_prevented {
            self.close(None, window, cx);
        }
    }
    fn focus_order(&self, cx: &Context<Self>) -> Vec<FocusHandle> {
        self.input
            .iter()
            .map(|state| state.read(cx).focus_handle(cx))
            .chain(self.controls.iter().cloned())
            .collect()
    }
    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => {
                window.prevent_default();
                cx.stop_propagation();
                self.close(None, window, cx);
            }
            "tab" => {
                let order = self.focus_order(cx);
                let index = order.iter().position(|focus| focus.is_focused(window));
                let next = if event.keystroke.modifiers.shift {
                    index.map_or(order.len() - 1, |i| (i + order.len() - 1) % order.len())
                } else {
                    index.map_or(0, |i| (i + 1) % order.len())
                };
                order[next].focus(window, cx);
                window.prevent_default();
                cx.stop_propagation();
                cx.notify();
            }
            "enter" | "space" if self.controls.iter().any(|focus| focus.is_focused(window)) => {
                let action = if self.controls[0].is_focused(window) {
                    Some(DialogAction::Cancel)
                } else if self.controls[1].is_focused(window) {
                    Some(DialogAction::Confirm)
                } else {
                    None
                };
                window.prevent_default();
                cx.stop_propagation();
                self.close(action, window, cx);
            }
            "enter" if self.options.kind == DialogKind::Prompt => {
                self.close(Some(DialogAction::Confirm), window, cx)
            }
            _ => {}
        }
    }
}
