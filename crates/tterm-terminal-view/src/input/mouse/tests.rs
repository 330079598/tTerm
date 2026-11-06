//! Tests for the mouse report encoders.
//!
//! The expected bytes are Alacritty's `normal_mouse_report`,
//! `sgr_mouse_report` and `mouse_moved` in `input/mod.rs`.

use tterm_terminal::grid::GridPoint;
use tterm_terminal::mode::Modes;

use super::ButtonAction;
use super::ButtonState;
use super::Motion;
use super::MouseButton;
use super::MouseModifiers;
use super::button_action;
use super::encode_button;
use super::encode_motion;
use super::motion;
use super::reports_mouse;

fn point(line: i32, column: usize) -> GridPoint {
    GridPoint::new(line, column)
}

fn click() -> Modes {
    Modes::MOUSE_REPORT_CLICK
}

fn drag() -> Modes {
    Modes::MOUSE_DRAG
}

fn any() -> Modes {
    Modes::MOUSE_MOTION
}

#[test]
fn mouse_reports_need_a_reporting_mode() {
    assert!(!reports_mouse(Modes::NONE));
    assert!(reports_mouse(click()));
    assert!(reports_mouse(drag()));
    assert!(reports_mouse(any()));
    // Vi mode takes the pointer for its own cursor.
    assert!(!reports_mouse(click().union(Modes::VI)));
    assert_eq!(
        encode_button(
            point(0, 0),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            Modes::NONE
        ),
        None
    );
}

#[test]
fn normal_click_encoding() {
    let modes = click();
    assert_eq!(
        encode_button(
            point(0, 0),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            modes
        )
        .as_deref(),
        Some(b"\x1b[M !!".as_slice())
    );
    assert_eq!(
        encode_button(
            point(5, 10),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            modes
        )
        .as_deref(),
        Some(b"\x1b[M +&".as_slice())
    );
    // Shift 4 + Control 16.
    assert_eq!(
        encode_button(
            point(0, 0),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers {
                shift: true,
                alt: false,
                control: true,
            },
            modes
        )
        .as_deref(),
        Some(b"\x1b[M4!!".as_slice())
    );
    // The normal encoding has one release code, 3, for every button.
    assert_eq!(
        encode_button(
            point(0, 0),
            MouseButton::Right,
            ButtonState::Released,
            MouseModifiers::default(),
            modes
        )
        .as_deref(),
        Some(b"\x1b[M#!!".as_slice())
    );
}

#[test]
fn sgr_click_encoding() {
    let modes = click().union(Modes::SGR_MOUSE);
    assert_eq!(
        encode_button(
            point(5, 10),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            modes
        )
        .as_deref(),
        Some(b"\x1b[<0;11;6M".as_slice())
    );
    assert_eq!(
        encode_button(
            point(5, 10),
            MouseButton::Right,
            ButtonState::Released,
            MouseModifiers {
                control: true,
                ..MouseModifiers::default()
            },
            modes
        )
        .as_deref(),
        Some(b"\x1b[<18;11;6m".as_slice())
    );
    // SGR has no coordinate limit.
    assert_eq!(
        encode_button(
            point(10, 5000),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            modes
        )
        .as_deref(),
        Some(b"\x1b[<0;5001;11M".as_slice())
    );
}

#[test]
fn normal_encoding_stops_at_223() {
    let modes = click();
    let last = encode_button(
        point(0, 222),
        MouseButton::Left,
        ButtonState::Pressed,
        MouseModifiers::default(),
        modes,
    );
    assert_eq!(last.as_deref(), Some(b"\x1b[M \xff!".as_slice()));
    assert_eq!(
        encode_button(
            point(0, 223),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            modes
        ),
        None
    );
    assert_eq!(
        encode_button(
            point(223, 0),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            modes
        ),
        None
    );
    // SGR still reports the same point.
    assert_eq!(
        encode_button(
            point(223, 223),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            modes.union(Modes::SGR_MOUSE)
        )
        .as_deref(),
        Some(b"\x1b[<0;224;224M".as_slice())
    );
}

#[test]
fn utf8_encoding_uses_two_bytes_from_95() {
    let modes = click().union(Modes::UTF8_MOUSE);
    assert_eq!(
        encode_button(
            point(0, 94),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            modes
        )
        .as_deref(),
        Some(b"\x1b[M \x7f!".as_slice())
    );
    // 32 + 1 + 95 = 128: the two-byte encoding of 128.
    assert_eq!(
        encode_button(
            point(0, 95),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            modes
        )
        .as_deref(),
        Some(b"\x1b[M \xc2\x80!".as_slice())
    );
    assert_eq!(
        encode_button(
            point(95, 0),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            modes
        )
        .as_deref(),
        Some(b"\x1b[M !\xc2\x80".as_slice())
    );
    assert_eq!(
        encode_button(
            point(0, 2014),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            modes
        )
        .as_deref(),
        Some(b"\x1b[M \xdf\xbf!".as_slice())
    );
    assert_eq!(
        encode_button(
            point(0, 2015),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            modes
        ),
        None
    );
}

#[test]
fn scrollback_is_not_reportable() {
    assert_eq!(
        encode_button(
            point(-1, 0),
            MouseButton::Left,
            ButtonState::Pressed,
            MouseModifiers::default(),
            click()
        ),
        None
    );
}

#[test]
fn wheel_reports() {
    assert_eq!(
        encode_button(
            point(0, 0),
            MouseButton::WheelUp,
            ButtonState::Pressed,
            MouseModifiers::default(),
            click()
        )
        .as_deref(),
        Some(b"\x1b[M`!!".as_slice())
    );
    assert_eq!(
        encode_button(
            point(0, 0),
            MouseButton::WheelDown,
            ButtonState::Pressed,
            MouseModifiers::default(),
            click().union(Modes::SGR_MOUSE)
        )
        .as_deref(),
        Some(b"\x1b[<65;1;1M".as_slice())
    );
}

#[test]
fn shift_escapes_to_the_local_selection() {
    // Alacritty's on_mouse_press and on_mouse_release report only without
    // Shift; with Shift the view extends the selection instead.
    assert_eq!(button_action(click(), false), ButtonAction::Report);
    assert_eq!(button_action(click(), true), ButtonAction::Local);
    assert_eq!(button_action(Modes::NONE, false), ButtonAction::Local);
    assert_eq!(
        button_action(click().union(Modes::VI), true),
        ButtonAction::Local
    );
}

#[test]
fn motion_decision_follows_the_mode() {
    // Mode 1002: cell motion, only while a button is held.
    assert_eq!(
        motion(drag(), Some(MouseButton::Left), false),
        Some(Motion::Drag(MouseButton::Left))
    );
    assert_eq!(motion(drag(), None, false), None);
    // Mode 1003: every motion; no button is the hover code 35.
    assert_eq!(
        motion(any(), Some(MouseButton::Right), false),
        Some(Motion::Drag(MouseButton::Right))
    );
    assert_eq!(motion(any(), None, false), Some(Motion::Hover));
    // Mode 1000 does not report motion.
    assert_eq!(motion(click(), Some(MouseButton::Left), false), None);
    assert_eq!(motion(Modes::NONE, Some(MouseButton::Left), false), None);
}

#[test]
fn motion_keeps_the_shift_escape_of_a_drag() {
    // mouse_moved sends a left or right drag to the local selection when
    // Shift is held, so no report is built.
    assert_eq!(motion(any(), Some(MouseButton::Left), true), None);
    assert_eq!(motion(any(), Some(MouseButton::Right), true), None);
    // Alacritty's selection branch covers only the left and right buttons, so
    // a middle-button drag is still reported, Shift bit included.
    assert_eq!(
        motion(any(), Some(MouseButton::Middle), true),
        Some(Motion::Drag(MouseButton::Middle))
    );
    // A hover has no selection to extend; mode 1003 reports it either way.
    assert_eq!(motion(any(), None, true), Some(Motion::Hover));
}

#[test]
fn motion_encoding() {
    let modes = any().union(Modes::SGR_MOUSE);
    assert_eq!(
        encode_motion(
            point(2, 3),
            Motion::Drag(MouseButton::Left),
            MouseModifiers::default(),
            modes
        )
        .as_deref(),
        Some(b"\x1b[<32;4;3M".as_slice())
    );
    assert_eq!(
        encode_motion(point(2, 3), Motion::Hover, MouseModifiers::default(), modes).as_deref(),
        Some(b"\x1b[<35;4;3M".as_slice())
    );
    assert_eq!(
        encode_motion(
            point(2, 3),
            Motion::Drag(MouseButton::Left),
            MouseModifiers::default(),
            any()
        )
        .as_deref(),
        Some(b"\x1b[M@$#".as_slice())
    );
}
