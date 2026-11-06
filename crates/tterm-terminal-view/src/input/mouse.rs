// Derived from Alacritty's `alacritty/src/input/mod.rs`, commit
// 94e7c8874e526b1e67b349d9ba30ddf81669119e (v0.17.0), Apache-2.0.
// SPDX-License-Identifier: Apache-2.0
//
// Changes: mouse reports are pure functions over a grid point and the
// terminal modes; winit's `MouseButton`, `ElementState` and pixel-to-cell
// conversion stay in the view. The bytes are Alacritty's `mouse_report`,
// `normal_mouse_report` and `sgr_mouse_report`. The Shift escape that
// `mouse_input`, `mouse_moved` and `on_mouse_release` apply before reporting
// is a separate routing decision, `button_action` and `motion`.

//! Mouse report encoding.
//!
//! The view turns a pointer position into a [`GridPoint`] and calls these
//! functions; the encoding itself is independent of GPUI. Coordinates are
//! grid coordinates, where `line` is the absolute line and a negative line is
//! in the scrollback, which the protocols cannot address.
//!
//! The view routes a press, release or drag through [`button_action`] and
//! [`motion`] first: with Shift held the event belongs to the local selection
//! and never reaches the encoders.

use gpui_kit::Modifiers;
use tterm_terminal::grid::GridPoint;
use tterm_terminal::mode::Modes;

/// The buttons and wheels a terminal can report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    /// The primary button.
    Left,
    /// The middle button.
    Middle,
    /// The secondary button.
    Right,
    /// Wheel towards the top.
    WheelUp,
    /// Wheel towards the bottom.
    WheelDown,
    /// Wheel to the left.
    WheelLeft,
    /// Wheel to the right.
    WheelRight,
}

impl MouseButton {
    /// The low button number of the X10 protocol, before modifiers.
    const fn code(self) -> u8 {
        match self {
            Self::Left => 0,
            Self::Middle => 1,
            Self::Right => 2,
            Self::WheelUp => 64,
            Self::WheelDown => 65,
            Self::WheelLeft => 66,
            Self::WheelRight => 67,
        }
    }
}

/// Whether a report is a press or a release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonState {
    /// The button went down.
    Pressed,
    /// The button came up.
    Released,
}

/// The modifiers a mouse report can carry.
///
/// Shift, Alt and Control have protocol bits; the platform and function
/// modifiers do not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseModifiers {
    /// The Shift key.
    pub shift: bool,
    /// The Alt or Option key.
    pub alt: bool,
    /// The Control key.
    pub control: bool,
}

impl MouseModifiers {
    /// The protocol's modifier value: Shift 4, Alt 8, Control 16.
    const fn value(self) -> u8 {
        (self.shift as u8) * 4 + (self.alt as u8) * 8 + (self.control as u8) * 16
    }
}

impl From<Modifiers> for MouseModifiers {
    fn from(modifiers: Modifiers) -> Self {
        Self {
            shift: modifiers.shift,
            alt: modifiers.alt,
            control: modifiers.control,
        }
    }
}

/// A motion report: dragging a held button, or hovering with no button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    /// A button is held; the report uses that button's drag code.
    Drag(MouseButton),
    /// No button is held; only mode 1003 reports it.
    Hover,
}

/// Whether the terminal wants mouse reports at all.
///
/// This is Alacritty's `mouse_mode`: any of 1000, 1002 and 1003, except in Vi
/// mode where the pointer moves the Vi cursor instead.
pub fn reports_mouse(modes: Modes) -> bool {
    modes.intersects(Modes::MOUSE_MODE) && !modes.contains(Modes::VI)
}

/// What the view does with a mouse button press or release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonAction {
    /// Send the report to the program.
    Report,
    /// Keep the event local: selection, click handling and hint activation.
    Local,
}

/// Decides whether a button press or release is reported.
///
/// Shift is the local escape: Alacritty's `on_mouse_press` and
/// `on_mouse_release` report only when Shift is not held, so the view can
/// extend a selection over a program that owns the mouse. The encoder itself
/// stays a pure protocol function; the view calls this first and only then
/// [`encode_button`].
pub fn button_action(modes: Modes, shift: bool) -> ButtonAction {
    if !shift && reports_mouse(modes) {
        ButtonAction::Report
    } else {
        ButtonAction::Local
    }
}

/// Decides whether a pointer motion should be reported.
///
/// The view calls this only when the cell under the pointer changed, which is
/// Alacritty's deduplication: the protocol addresses cells, so two motions
/// inside one cell are one report. Shift with the left or right button held is
/// the local escape, matching `mouse_moved`: the view extends the selection
/// instead of reporting the drag. The middle button has no selection branch in
/// Alacritty, and a hover has no button, so those reports keep their Shift
/// bit.
pub fn motion(modes: Modes, held: Option<MouseButton>, shift: bool) -> Option<Motion> {
    if !reports_mouse(modes) {
        return None;
    }
    if !modes.intersects(Modes::MOUSE_MOTION.union(Modes::MOUSE_DRAG)) {
        return None;
    }
    match held {
        Some(MouseButton::Left | MouseButton::Right) if shift => None,
        Some(MouseButton::Left) => Some(Motion::Drag(MouseButton::Left)),
        Some(MouseButton::Middle) => Some(Motion::Drag(MouseButton::Middle)),
        Some(MouseButton::Right) => Some(Motion::Drag(MouseButton::Right)),
        // Mode 1003 is the all-motion mode. In 1002 a motion without a held
        // button is local movement, not a report.
        None if modes.contains(Modes::MOUSE_MOTION) => Some(Motion::Hover),
        _ => None,
    }
}

/// Encodes one button press or release.
///
/// `None` means the terminal does not want the report, the point is in the
/// scrollback, or the coordinates do not fit the selected encoding.
pub fn encode_button(
    point: GridPoint,
    button: MouseButton,
    state: ButtonState,
    modifiers: MouseModifiers,
    modes: Modes,
) -> Option<Vec<u8>> {
    if !reports_mouse(modes) {
        return None;
    }
    let released = state == ButtonState::Released;
    encode_code(point, button.code(), released, modifiers, modes)
}

/// Encodes one motion report.
pub fn encode_motion(
    point: GridPoint,
    motion: Motion,
    modifiers: MouseModifiers,
    modes: Modes,
) -> Option<Vec<u8>> {
    if !reports_mouse(modes) {
        return None;
    }
    let code = match motion {
        Motion::Drag(MouseButton::Left) => 32,
        Motion::Drag(MouseButton::Middle) => 33,
        Motion::Drag(MouseButton::Right) => 34,
        Motion::Drag(_) | Motion::Hover => 35,
    };
    encode_code(point, code, false, modifiers, modes)
}

/// The shared part of [`encode_button`] and [`encode_motion`].
fn encode_code(
    point: GridPoint,
    button: u8,
    released: bool,
    modifiers: MouseModifiers,
    modes: Modes,
) -> Option<Vec<u8>> {
    // The scrollback is not addressable: alacritty_terminal reports negative
    // absolute lines above the viewport.
    if point.line < 0 {
        return None;
    }
    let modifiers = modifiers.value();
    if modes.contains(Modes::SGR_MOUSE) {
        let terminator = if released { 'm' } else { 'M' };
        return Some(
            format!(
                "\x1b[<{};{};{}{terminator}",
                button + modifiers,
                point.column + 1,
                point.line + 1
            )
            .into_bytes(),
        );
    }

    // The normal encoding only has one release code (3), whatever the button.
    let button = if released {
        3 + modifiers
    } else {
        button + modifiers
    };
    let utf8 = modes.contains(Modes::UTF8_MOUSE);
    let max_point: usize = if utf8 { 2015 } else { 223 };
    if point.line >= max_point as i32 || point.column >= max_point {
        return None;
    }
    let mut message = vec![b'\x1b', b'[', b'M', 32 + button];
    if utf8 && point.column >= 95 {
        message.extend(mouse_position(point.column));
    } else {
        message.push(32 + 1 + point.column as u8);
    }
    if utf8 && point.line >= 95 {
        message.extend(mouse_position(point.line as usize));
    } else {
        message.push(32 + 1 + point.line as u8);
    }
    Some(message)
}

/// Encodes a coordinate of 95 or more in the UTF-8 mouse mode.
fn mouse_position(position: usize) -> [u8; 2] {
    let position = 32 + 1 + position;
    [(0xC0 + position / 64) as u8, (0x80 + (position & 63)) as u8]
}

#[cfg(test)]
mod tests;
