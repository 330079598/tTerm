// Derived from Alacritty's `alacritty/src/input/mod.rs` (`scroll_terminal`),
// commit 94e7c8874e526b1e67b349d9ba30ddf81669119e (v0.17.0), Apache-2.0.
// SPDX-License-Identifier: Apache-2.0
//
// Changes: pixel accumulation and the scrolling multiplier stay in the view;
// this module only turns an already-quantised wheel movement into reports,
// alternate-screen arrows or a local scroll.

//! What a wheel movement means in the current terminal modes.
//!
//! The order is Alacritty's `scroll_terminal`:
//!
//! 1. A program that asked for mouse reports gets one report per wheel line.
//! 2. On the alternate screen with alternate scroll (mode 1007), and not with
//!    Shift held, the wheel sends arrow keys instead.
//! 3. Otherwise the viewport scrolls locally.

use tterm_terminal::mode::Modes;

use super::mouse::MouseButton;

/// The arrow an alternate-screen wheel movement sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlternateArrow {
    /// Wheel up.
    Up,
    /// Wheel down.
    Down,
    /// Wheel left.
    Left,
    /// Wheel right.
    Right,
}

impl AlternateArrow {
    /// The arrow's SS3 sequence, as `scroll_terminal` writes it.
    pub fn sequence(self) -> &'static [u8] {
        match self {
            Self::Up => b"\x1bOA",
            Self::Down => b"\x1bOB",
            Self::Right => b"\x1bOC",
            Self::Left => b"\x1bOD",
        }
    }
}

/// One wheel movement after the view has converted pixels to lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wheel {
    /// Lines towards the top; negative values scroll towards the bottom.
    pub lines: i32,
    /// Columns towards the left; negative values scroll towards the right.
    pub columns: i32,
}

/// What the view should do with one wheel movement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WheelAction {
    /// Report each step to the program, in order.
    Report(Vec<MouseButton>),
    /// Send one arrow sequence per step, in order.
    Alternate(Vec<AlternateArrow>),
    /// Scroll the viewport locally; positive values go back into history.
    Scroll(i32),
    /// Nothing moved.
    None,
}

/// Decides what a wheel movement does.
///
/// `shift` is the escape hatch: with Shift held a program that owns the mouse
/// does not get the wheel, and the viewport scrolls instead, which keeps text
/// selection and history scrolling possible.
pub fn wheel_action(modes: Modes, shift: bool, wheel: Wheel) -> WheelAction {
    if wheel.lines == 0 && wheel.columns == 0 {
        return WheelAction::None;
    }
    if !shift && super::mouse::reports_mouse(modes) {
        let mut reports = Vec::new();
        reports.extend(repeat_wheel(
            wheel.lines,
            MouseButton::WheelUp,
            MouseButton::WheelDown,
        ));
        reports.extend(repeat_wheel(
            wheel.columns,
            MouseButton::WheelLeft,
            MouseButton::WheelRight,
        ));
        return WheelAction::Report(reports);
    }

    let alternate_scroll = modes.contains(Modes::ALT_SCREEN.union(Modes::ALTERNATE_SCROLL));
    if !shift && alternate_scroll {
        let mut arrows = Vec::new();
        arrows.extend(repeat_arrow(
            wheel.lines,
            AlternateArrow::Up,
            AlternateArrow::Down,
        ));
        arrows.extend(repeat_arrow(
            wheel.columns,
            AlternateArrow::Left,
            AlternateArrow::Right,
        ));
        return WheelAction::Alternate(arrows);
    }

    if wheel.lines == 0 {
        WheelAction::None
    } else {
        WheelAction::Scroll(wheel.lines)
    }
}

/// Repeats a wheel button for the magnitude of `steps`.
fn repeat_wheel(steps: i32, positive: MouseButton, negative: MouseButton) -> Vec<MouseButton> {
    let button = if steps > 0 { positive } else { negative };
    vec![button; steps.unsigned_abs() as usize]
}

/// Repeats an alternate-screen arrow for the magnitude of `steps`.
fn repeat_arrow(
    steps: i32,
    positive: AlternateArrow,
    negative: AlternateArrow,
) -> Vec<AlternateArrow> {
    let arrow = if steps > 0 { positive } else { negative };
    vec![arrow; steps.unsigned_abs() as usize]
}

#[cfg(test)]
mod tests;
