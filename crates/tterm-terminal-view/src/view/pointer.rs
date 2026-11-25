//! Pointer routing and selection use the frame's grid geometry.

use super::TerminalView;
use super::layout::cell_at;
use super::scrollbar::ScrollbarGeometry;
use crate::input::click::{ClickCount, selection_kind};
use crate::input::mouse::{self, ButtonAction, ButtonState};
use gpui_kit::{
    Context, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    Window,
};
use tterm_terminal::{grid::GridPoint, selection::Side, terminal::Scroll};

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
        let content = self.content_bounds()?;
        let grid = frame.grid();
        let (row, column) = cell_at(content, origin, grid, &frame.metrics, position)?;
        let x = (position.x - origin.x).as_f32() / frame.metrics.cell_width;
        let side = if x.fract() < 0.5 {
            Side::Left
        } else {
            Side::Right
        };
        Some((
            GridPoint::new(row as i32 - frame.snapshot.display_offset() as i32, column),
            side,
        ))
    }

    /// The nearest cell while a press that started inside the grid is held.
    ///
    /// A release delivered outside the pane still has to pair the button it
    /// reported or finish the selection it began. This fallback is never used
    /// for a fresh pointer position, so padding still cannot become a cell or
    /// start a mouse report.
    fn pointer_cell_clamped(&self, position: Point<Pixels>) -> Option<(GridPoint, Side)> {
        self.pointer.held?;
        let frame = self.last_frame.as_ref()?;
        let origin = self.input_origin?;
        let grid = frame.grid();
        let x = (position.x - origin.x).as_f32() / frame.metrics.cell_width;
        let y = (position.y - origin.y).as_f32() / frame.metrics.cell_height;
        let column = x.floor().clamp(0.0, (grid.columns - 1) as f32) as usize;
        let row = y.floor().clamp(0.0, (grid.screen_lines - 1) as f32) as i32;
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
        if let Some(frame) = self.last_frame.as_ref()
            && let Some(content) = self.content_bounds()
            && content.size.width >= gpui_kit::px(10.0)
            && let Some(origin) = self.input_origin
            && event.position.x
                >= origin.x + gpui_kit::px(frame.grid().columns as f32 * frame.metrics.cell_width)
            && event.position.x < content.right()
            && event.position.y >= content.origin.y
            && event.position.y < content.bottom()
            && frame.snapshot.history_size() > 0
        {
            let track = content.size.height.as_f32();
            let geometry = ScrollbarGeometry::new(
                track,
                frame.snapshot.grid().screen_lines,
                frame.snapshot.history_size(),
                frame.snapshot.display_offset(),
            );
            let y = (event.position.y - content.origin.y).as_f32();
            let target = geometry.offset_at(y, frame.snapshot.history_size());
            let current = frame.snapshot.display_offset();
            self.terminal
                .scroll(Scroll::Delta(target as i32 - current as i32));
            self.scrollbar_drag = true;
            self.invalidate(cx);
            cx.stop_propagation();
            return;
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
        let option_forces =
            self.pointer_policy.macos_option_forces_selection && event.modifiers.alt;
        let force_local = event.modifiers.shift || option_forces;
        let block = !option_forces
            && (event.modifiers.control
                || (self.pointer_policy.alt_selects_block_outside_macos && event.modifiers.alt));
        let action = mouse::button_action(self.input_modes(), force_local);
        if action == ButtonAction::Local
            && button == mouse::MouseButton::Right
            && self.right_click_paste
            && !event.modifiers.shift
        {
            // Windows Terminal and similar clients paste on right click. A
            // reported right click still belongs to the program above, and
            // Shift keeps the context-menu gesture available for a later menu.
            self.pointer.held = None;
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                self.request_paste(&text, window, cx);
            }
            cx.stop_propagation();
            return;
        }
        self.pointer.selecting = action == ButtonAction::Local
            && matches!(button, mouse::MouseButton::Left | mouse::MouseButton::Right);
        self.pointer.reported_press = action == ButtonAction::Report;
        if action == ButtonAction::Report {
            self.report_button(point, button, ButtonState::Pressed, event.modifiers, cx);
        } else if button == mouse::MouseButton::Middle {
            if self.pointer_policy.middle_click_pastes {
                self.paste_primary(window, cx);
            }
        } else if button == mouse::MouseButton::Left {
            if event.modifiers.shift && self.terminal.has_selection() {
                self.terminal.extend_selection(point, side);
            } else {
                let count = match event.click_count {
                    2 => ClickCount::Double,
                    3 => ClickCount::Triple,
                    _ => ClickCount::Single,
                };
                if let Some(kind) = selection_kind(count, block) {
                    self.terminal.select(kind, point, side);
                }
            }
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
                && let Some(content) = self.content_bounds()
            {
                let geometry = ScrollbarGeometry::new(
                    content.size.height.as_f32(),
                    frame.snapshot.grid().screen_lines,
                    frame.snapshot.history_size(),
                    frame.snapshot.display_offset(),
                );
                let y = (event.position.y - content.origin.y).as_f32();
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
        let strict = self.pointer_cell(event.position);
        let Some((point, side)) = strict.or_else(|| self.pointer_cell_clamped(event.position))
        else {
            return;
        };
        let in_grid = strict.is_some();
        let held = self.pointer.held;
        let option_forces =
            self.pointer_policy.macos_option_forces_selection && event.modifiers.alt;
        let force_local = event.modifiers.shift || option_forces;
        let motion = if self.pointer.selecting {
            None
        } else {
            mouse::motion(self.input_modes(), held, force_local)
        };
        if let Some(motion) = motion {
            if in_grid
                && self.pointer.last_cell != Some(point)
                && let Some(bytes) =
                    mouse::encode_motion(point, motion, event.modifiers.into(), self.input_modes())
            {
                self.send_bytes(&bytes, cx);
            }
        } else if self.pointer.selecting
            || (force_local
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
        if let Some((point, side)) = self
            .pointer_cell(position)
            .or_else(|| self.pointer_cell_clamped(position))
        {
            if self.pointer.selecting {
                self.terminal.extend_selection(point, side);
                self.invalidate(cx);
            } else if self.pointer.reported_press {
                self.report_button(point, button, ButtonState::Released, modifiers, cx);
            }
        }
        if self.pointer.selecting {
            self.terminal.finish_selection();
            if button == mouse::MouseButton::Left {
                self.copy_selection_on_release(cx);
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

    /// Pastes the primary selection for a Linux middle click.
    fn paste_primary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(primary) = self.primary.clone() else {
            return;
        };
        let Some(text) = primary(cx) else {
            return;
        };
        self.request_paste(&text, window, cx);
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
