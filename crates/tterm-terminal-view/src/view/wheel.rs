//! Wheel event handling for the terminal view.

use super::TerminalView;
use crate::input::wheel::{Wheel, WheelAction, wheel_action};
use gpui_kit::{Context, ScrollEvent, Window};
use tterm_terminal::terminal::Scroll;

impl TerminalView {
    pub(super) fn on_wheel(
        &mut self,
        event: &ScrollEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(data) = self.last_frame.as_ref() else {
            return;
        };
        let lines = self.scroll.lines(event.delta, data.metrics.cell_height);
        if lines == 0 {
            return;
        }
        let action = wheel_action(
            data.snapshot.modes(),
            event.modifiers.shift,
            Wheel { lines, columns: 0 },
        );
        match action {
            WheelAction::Scroll(lines) => {
                self.terminal.scroll(Scroll::Delta(lines));
                self.invalidate(cx);
            }
            WheelAction::Alternate(arrows) => {
                self.scroll.reset();
                if let Some(input) = &self.input {
                    for arrow in arrows {
                        input(arrow.sequence());
                    }
                }
            }
            WheelAction::Report(buttons) => {
                self.scroll.reset();
                if let Some((point, _)) = self.pointer_cell(event.position) {
                    for button in buttons {
                        self.report_button(
                            point,
                            button,
                            crate::input::mouse::ButtonState::Pressed,
                            event.modifiers,
                            cx,
                        );
                    }
                }
            }
            WheelAction::None => self.scroll.reset(),
        }
    }
}
