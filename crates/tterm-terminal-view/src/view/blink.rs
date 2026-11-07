//! Cursor blink state.
//!
//! Blinking is the only timer in the view. It runs only while the view is
//! visible, the window is active, the terminal has focus, and the program
//! left the cursor visible and blinking. Every other idle state schedules no
//! frame at all.

use std::time::Duration;

/// How long the cursor stays in one phase.
pub const BLINK_INTERVAL: Duration = Duration::from_millis(530);

/// The blink phase and whether the timer is running.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlinkState {
    /// Whether the cursor is in its visible phase.
    pub on: bool,
    /// Whether a timer is running.
    pub running: bool,
    /// How many phase changes happened, for diagnostics.
    pub toggles: u64,
}

impl BlinkState {
    /// Whether the timer should run at all.
    pub fn should_run(
        visible: bool,
        focused: bool,
        window_active: bool,
        cursor_drawn: bool,
        cursor_blinks: bool,
    ) -> bool {
        visible && focused && window_active && cursor_drawn && cursor_blinks
    }

    /// Starts or stops the timer to match `should_run`.
    ///
    /// Returns whether a running timer should be cancelled; the caller owns
    /// the task handle because only the GPUI context can spawn it.
    pub fn set_running(&mut self, should_run: bool) -> bool {
        if should_run {
            if !self.running {
                self.running = true;
                self.on = true;
            }
            false
        } else {
            let was_running = self.running;
            self.running = false;
            self.on = true;
            was_running
        }
    }

    /// Flips the phase, which is what the timer callback does.
    pub fn toggle(&mut self) {
        if self.running {
            self.on = !self.on;
            self.toggles += 1;
        }
    }

    /// Shows the cursor again after output or input.
    pub fn reset(&mut self) {
        self.on = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_visible_focused_cursor_blinks() {
        assert!(BlinkState::should_run(true, true, true, true, true));
    }

    #[test]
    fn blur_hiding_and_animation_off_stop_the_timer() {
        assert!(!BlinkState::should_run(false, true, true, true, true));
        assert!(!BlinkState::should_run(true, false, true, true, true));
        assert!(!BlinkState::should_run(true, true, false, true, true));
        assert!(!BlinkState::should_run(true, true, true, false, true));
        assert!(!BlinkState::should_run(true, true, true, true, false));
    }

    #[test]
    fn stopping_reports_whether_a_task_was_running() {
        let mut blink = BlinkState::default();
        assert!(!blink.set_running(false));
        assert!(!blink.set_running(true));
        assert!(blink.running);
        assert!(blink.on);
        assert!(blink.set_running(false));
        assert!(!blink.running);
        assert!(blink.on);
    }

    #[test]
    fn toggling_only_happens_while_running() {
        let mut blink = BlinkState::default();
        blink.toggle();
        assert!(!blink.on);
        assert_eq!(blink.toggles, 0);
        blink.set_running(true);
        blink.toggle();
        assert!(!blink.on);
        assert_eq!(blink.toggles, 1);
        blink.reset();
        assert!(blink.on);
    }
}
