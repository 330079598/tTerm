//! Routing GPUI keyboard events into the existing terminal encoder.

use gpui_kit::{Context, KeyDownEvent, KeyUpEvent, Keystroke, Window};
use tterm_terminal::mode::Modes;
use tterm_terminal::terminal::Scroll;

use super::TerminalView;
use crate::input::focus::focus_report;
use crate::input::keyboard::{KeyPress, KeyState, encode};

impl TerminalView {
    pub(super) fn input_modes(&self) -> Modes {
        self.last_frame
            .as_ref()
            .map_or(Modes::NONE, |frame| frame.snapshot.modes())
    }

    pub(super) fn send_bytes(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        if bytes.is_empty() {
            return;
        }
        if let Some(input) = &self.input {
            input(bytes);
        }
        self.blink.reset();
        self.blink_task.take();
        self.invalidate(cx);
    }

    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.prefer_character_input {
            return;
        }
        if self.clipboard_key(&event.keystroke, cx)
            || self.encode_key(&event.keystroke, KeyState::Pressed, event.is_held, cx)
        {
            window.prevent_default();
            cx.stop_propagation();
        }
    }

    pub(super) fn on_key_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.encode_key(&event.keystroke, KeyState::Released, false, cx) {
            window.prevent_default();
            cx.stop_propagation();
        }
    }

    fn encode_key(
        &mut self,
        keystroke: &Keystroke,
        state: KeyState,
        repeat: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut press = KeyPress::new(keystroke);
        press.state = state;
        press.repeat = repeat;
        let Some(bytes) = encode(&press, self.input_modes(), self.keyboard) else {
            return false;
        };
        if state == KeyState::Pressed && !press.is_modifier() {
            self.terminal.clear_selection();
            self.terminal.scroll(Scroll::Bottom);
        }
        self.send_bytes(&bytes, cx);
        true
    }

    pub(super) fn focus_changed(
        &mut self,
        focused: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input_focused != focused {
            self.input_focused = focused;
            if let Some(bytes) = focus_report(self.input_modes(), focused) {
                self.send_bytes(bytes, cx);
            }
        }
        if !focused {
            self.stop_blink();
        }
        window.invalidate_character_coordinates();
        self.invalidate(cx);
    }

    pub(super) fn observe_input_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.input_observed {
            return;
        }
        self.input_observed = true;
        cx.on_focus(&self.focus_handle, window, |view, window, cx| {
            view.focus_changed(window.is_window_active(), window, cx);
        })
        .detach();
        cx.on_blur(&self.focus_handle, window, |view, window, cx| {
            view.focus_changed(false, window, cx);
        })
        .detach();
        cx.observe_window_activation(window, |view, window, cx| {
            let focused = window.is_window_active() && view.focus_handle.is_focused(window);
            view.focus_changed(focused, window, cx);
        })
        .detach();
    }
}
