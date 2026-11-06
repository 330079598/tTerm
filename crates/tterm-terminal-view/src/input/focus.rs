// Derived from Alacritty's `alacritty/src/input/mod.rs` (`on_focus_change`),
// commit 94e7c8874e526b1e67b349d9ba30ddf81669119e (v0.17.0), Apache-2.0.
// SPDX-License-Identifier: Apache-2.0
//
// Changes: returns the bytes instead of writing them to the PTY.

//! Focus in and focus out reports (mode 1004).

use tterm_terminal::mode::Modes;

/// The report to send when the window gains or loses focus.
///
/// `None` means the program did not ask for focus reports. The bytes are
/// Alacritty's `on_focus_change`: `CSI I` for focus and `CSI O` for blur.
pub fn focus_report(modes: Modes, focused: bool) -> Option<&'static [u8]> {
    modes.contains(Modes::FOCUS_IN_OUT).then_some(if focused {
        b"\x1b[I".as_slice()
    } else {
        b"\x1b[O".as_slice()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_reports_only_when_asked_for() {
        assert_eq!(focus_report(Modes::NONE, true), None);
        assert_eq!(
            focus_report(Modes::FOCUS_IN_OUT, true),
            Some(b"\x1b[I".as_slice())
        );
        assert_eq!(
            focus_report(Modes::FOCUS_IN_OUT, false),
            Some(b"\x1b[O".as_slice())
        );
        // Unrelated modes do not enable it.
        assert_eq!(focus_report(Modes::BRACKETED_PASTE, true), None);
    }
}
