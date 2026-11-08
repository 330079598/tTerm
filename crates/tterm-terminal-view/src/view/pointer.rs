//! Pointer routing and selection use the frame's grid geometry.

use super::TerminalView;
use super::scrollbar::ScrollbarGeometry;
use crate::input::mouse::{self, ButtonAction, ButtonState};
use gpui_kit::{
    Context, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    Window,
};
use tterm_terminal::{
    grid::GridPoint,
    selection::{SelectionKind, Side},
    terminal::Scroll,
};

#[derive(Default)]
pub(super) struct PointerState {
    held: Option<mouse::MouseButton>,
    selecting: bool,
    reported_press: bool,
    last_cell: Option<GridPoint>,
}

impl TerminalView {
    pub(super) fn pointer_cell(&self, position: Point<Pixels>) -> Option<(GridPoint, Side)> {
        let frame = self.last_frame.as_ref()?;
        let origin = self.input_origin?;
        let x = (position.x - origin.x).as_f32() / frame.metrics.cell_width;
        let y = (position.y - origin.y).as_f32() / frame.metrics.cell_height;
        let column = x.floor().clamp(0.0, (frame.grid().columns - 1) as f32) as usize;
        let row = y.floor().clamp(0.0, (frame.grid().screen_lines - 1) as f32) as i32;
        let side = if x.fract() < 0.5 {
            Side::Left
        } else {
            Side::Right
        };
        Some((
            GridPoint::new(row - frame.snapshot.display_offset() as i32, column),
            side,
        ))
    }

    pub(super) fn on_pointer_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.position.x.as_f32()
            >= self
                .input_origin
                .map(|origin| origin.x.as_f32())
                .unwrap_or(0.0)
                + self
                    .last_frame
                    .as_ref()
                    .map(|frame| frame.metrics.cell_width * frame.grid().columns as f32)
                    .unwrap_or(0.0)
        {
            if let Some(frame) = self.last_frame.as_ref()
                && frame.snapshot.history_size() > 0
            {
                let track = frame.snapshot.grid().screen_lines as f32 * frame.metrics.cell_height;
                let geometry = ScrollbarGeometry::new(
                    track,
                    frame.snapshot.grid().screen_lines,
                    frame.snapshot.history_size(),
                    frame.snapshot.display_offset(),
                );
                let y = (event.position.y - self.input_origin.unwrap().y).as_f32();
                let target = geometry.offset_at(y, frame.snapshot.history_size());
                let current = frame.snapshot.display_offset();
                self.terminal
                    .scroll(Scroll::Delta(target as i32 - current as i32));
                self.scrollbar_drag = true;
                cx.stop_propagation();
                return;
            }
        }
        let Some(button) = protocol_button(event.button) else {
            return;
        };
        let Some((point, side)) = self.pointer_cell(event.position) else {
            return;
        };
        window.focus(&self.focus_handle, cx);
        self.pointer.held = Some(button);
        self.pointer.last_cell = Some(point);
        let action = mouse::button_action(self.input_modes(), event.modifiers.shift);
        self.pointer.selecting = action == ButtonAction::Local
            && matches!(button, mouse::MouseButton::Left | mouse::MouseButton::Right);
        self.pointer.reported_press = action == ButtonAction::Report;
        if action == ButtonAction::Report {
            self.report_button(point, button, ButtonState::Pressed, event.modifiers, cx);
        } else if button == mouse::MouseButton::Left {
            let kind = match event.click_count {
                2 => SelectionKind::Semantic,
                3 => SelectionKind::Lines,
                _ if event.modifiers.control => SelectionKind::Block,
                _ => SelectionKind::Simple,
            };
            self.terminal.select(kind, point, side);
            self.invalidate(cx);
        } else if self.pointer.selecting {
            self.terminal.extend_selection(point, side);
            self.invalidate(cx);
        }
        cx.stop_propagation();
    }

    pub(super) fn on_pointer_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.scrollbar_drag {
            if let Some(frame) = self.last_frame.as_ref()
                && let Some(origin) = self.input_origin
            {
                let track = frame.snapshot.grid().screen_lines as f32 * frame.metrics.cell_height;
                let geometry = ScrollbarGeometry::new(
                    track,
                    frame.snapshot.grid().screen_lines,
                    frame.snapshot.history_size(),
                    frame.snapshot.display_offset(),
                );
                let y = (event.position.y - origin.y).as_f32();
                let target = geometry.offset_at(y, frame.snapshot.history_size());
                let current = frame.snapshot.display_offset();
                if target != current {
                    self.terminal
                        .scroll(Scroll::Delta(target as i32 - current as i32));
                    self.invalidate(cx);
                }
            }
            return;
        }
        // A release outside the native window may not be delivered. Recover
        // from the platform's button state when the pointer returns instead.
        if event.pressed_button.is_none() && self.pointer.held.is_some() {
            self.finish_pointer(event.position, event.modifiers, cx);
        }
        let Some((point, side)) = self.pointer_cell(event.position) else {
            return;
        };
        let held = self.pointer.held;
        let motion = if self.pointer.selecting {
            None
        } else {
            mouse::motion(self.input_modes(), held, event.modifiers.shift)
        };
        if let Some(motion) = motion {
            if self.pointer.last_cell != Some(point)
                && let Some(bytes) =
                    mouse::encode_motion(point, motion, event.modifiers.into(), self.input_modes())
            {
                self.send_bytes(&bytes, cx);
            }
        } else if self.pointer.selecting
            || (event.modifiers.shift
                && matches!(
                    held,
                    Some(mouse::MouseButton::Left | mouse::MouseButton::Right)
                ))
        {
            self.terminal.extend_selection(point, side);
            self.invalidate(cx);
        }
        self.pointer.last_cell = Some(point);
    }

    pub(super) fn on_pointer_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.scrollbar_drag {
            self.scrollbar_drag = false;
            cx.stop_propagation();
            return;
        }
        let Some(button) = protocol_button(event.button) else {
            return;
        };
        if self.pointer.held != Some(button) {
            return;
        }
        self.finish_pointer(event.position, event.modifiers, cx);
        cx.stop_propagation();
    }

    fn finish_pointer(
        &mut self,
        position: Point<Pixels>,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let Some(button) = self.pointer.held else {
            return;
        };
        if let Some((point, side)) = self.pointer_cell(position) {
            if self.pointer.selecting {
                self.terminal.extend_selection(point, side);
                self.invalidate(cx);
            } else if self.pointer.reported_press {
                self.report_button(point, button, ButtonState::Released, modifiers, cx);
            }
        }
        self.pointer = PointerState::default();
    }

    pub(super) fn report_button(
        &mut self,
        point: GridPoint,
        button: mouse::MouseButton,
        state: ButtonState,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        if let Some(bytes) =
            mouse::encode_button(point, button, state, modifiers.into(), self.input_modes())
        {
            self.send_bytes(&bytes, cx);
        }
    }
}

fn protocol_button(button: MouseButton) -> Option<mouse::MouseButton> {
    match button {
        MouseButton::Left => Some(mouse::MouseButton::Left),
        MouseButton::Middle => Some(mouse::MouseButton::Middle),
        MouseButton::Right => Some(mouse::MouseButton::Right),
        _ => None,
    }
}
