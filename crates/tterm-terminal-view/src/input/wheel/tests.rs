//! Tests for the wheel routing.
//!
//! The order is Alacritty's `scroll_terminal`: reports first, then the
//! alternate-screen arrows, then a local scroll.

use tterm_terminal::mode::Modes;

use super::AlternateArrow;
use super::Wheel;
use super::WheelAction;
use super::wheel_action;
use crate::input::mouse::MouseButton;

fn wheel(lines: i32, columns: i32) -> Wheel {
    Wheel { lines, columns }
}

#[test]
fn a_program_that_reports_the_mouse_gets_wheel_reports() {
    let modes = Modes::MOUSE_REPORT_CLICK;
    assert_eq!(
        wheel_action(modes, false, wheel(2, 0)),
        WheelAction::Report(vec![MouseButton::WheelUp, MouseButton::WheelUp])
    );
    assert_eq!(
        wheel_action(modes, false, wheel(-1, 0)),
        WheelAction::Report(vec![MouseButton::WheelDown])
    );
    assert_eq!(
        wheel_action(modes, false, wheel(0, 1)),
        WheelAction::Report(vec![MouseButton::WheelLeft])
    );
    assert_eq!(
        wheel_action(modes, false, wheel(0, -2)),
        WheelAction::Report(vec![MouseButton::WheelRight, MouseButton::WheelRight])
    );
}

#[test]
fn shift_forces_a_local_scroll() {
    // Shift is the escape hatch from a program that owns the mouse.
    assert_eq!(
        wheel_action(Modes::MOUSE_REPORT_CLICK, true, wheel(3, 0)),
        WheelAction::Scroll(3)
    );
    let alternate = Modes::ALT_SCREEN.union(Modes::ALTERNATE_SCROLL);
    assert_eq!(
        wheel_action(alternate, true, wheel(3, 0)),
        WheelAction::Scroll(3)
    );
}

#[test]
fn alternate_screen_scroll_sends_arrows() {
    let modes = Modes::ALT_SCREEN.union(Modes::ALTERNATE_SCROLL);
    assert_eq!(
        wheel_action(modes, false, wheel(2, 0)),
        WheelAction::Alternate(vec![AlternateArrow::Up, AlternateArrow::Up])
    );
    assert_eq!(
        wheel_action(modes, false, wheel(-1, 0)),
        WheelAction::Alternate(vec![AlternateArrow::Down])
    );
    assert_eq!(
        wheel_action(modes, false, wheel(0, 1)),
        WheelAction::Alternate(vec![AlternateArrow::Left])
    );
    assert_eq!(
        wheel_action(modes, false, wheel(0, -1)),
        WheelAction::Alternate(vec![AlternateArrow::Right])
    );
    assert_eq!(AlternateArrow::Up.sequence(), b"\x1bOA");
    assert_eq!(AlternateArrow::Right.sequence(), b"\x1bOC");
}

#[test]
fn a_plain_screen_scrolls_locally() {
    assert_eq!(
        wheel_action(Modes::NONE, false, wheel(-2, 0)),
        WheelAction::Scroll(-2)
    );
    // Mode 1007 without the alternate screen does not change the wheel.
    assert_eq!(
        wheel_action(Modes::ALTERNATE_SCROLL, false, wheel(1, 0)),
        WheelAction::Scroll(1)
    );
    // Horizontal wheel movements only matter to a reporting program.
    assert_eq!(
        wheel_action(Modes::NONE, false, wheel(0, 3)),
        WheelAction::None
    );
}

#[test]
fn no_movement_does_nothing() {
    assert_eq!(
        wheel_action(Modes::MOUSE_REPORT_CLICK, false, wheel(0, 0)),
        WheelAction::None
    );
}
