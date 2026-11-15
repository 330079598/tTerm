// Derived from Alacritty's `alacritty/src/input/mod.rs` (`on_mouse_press`,
// `on_left_click`) and `alacritty/src/event.rs` (`ClickState`), commit
// 94e7c8874e526b1e67b349d9ba30ddf81669119e (v0.17.0), Apache-2.0.
// SPDX-License-Identifier: Apache-2.0
//
// Changes: the click counter is its own type and only decides which
// selection kind the view asks the model for; starting the selection and
// moving the Vi cursor stay in the view.

//! Click counting and the selection kind a click starts.
//!
//! Alacritty counts clicks within 400 ms on the same button: one is a simple
//! selection, two select a word, three select a line, and a fourth starts
//! over at one. Control turns the first click into a block selection and
//! suppresses the word and line selections. The selection itself is
//! Alacritty's, reached through `tterm_terminal::Terminal::select`.

use std::time::Duration;
use std::time::Instant;

use tterm_terminal::selection::SelectionKind;

/// The interval in which a second click counts as a double click.
///
/// Alacritty's `CLICK_THRESHOLD`: 400 ms.
pub const CLICK_THRESHOLD: Duration = Duration::from_millis(400);

/// How many clicks one press represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickCount {
    /// The first click, or a click after the interval.
    Single,
    /// Two clicks inside the interval.
    Double,
    /// Three clicks inside the interval.
    Triple,
}

/// The buttons that can start a selection.
///
/// Alacritty ignores the browser back and forward buttons for selection; the
/// view does not pass them here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickButton {
    /// The primary button.
    Left,
    /// The middle button.
    Middle,
    /// The secondary button.
    Right,
}

/// Counts consecutive clicks of the same button.
#[derive(Clone, Copy, Debug)]
pub struct ClickTracker {
    last_click: Option<Instant>,
    last_button: Option<ClickButton>,
    count: ClickCount,
}

impl Default for ClickTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ClickTracker {
    /// A tracker that has not seen a click.
    pub const fn new() -> Self {
        Self {
            last_click: None,
            last_button: None,
            count: ClickCount::Single,
        }
    }

    /// Records a press and returns how many clicks it represents.
    ///
    /// A different button or a press outside [`CLICK_THRESHOLD`] starts a new
    /// count. A fourth click inside the interval also starts over: Alacritty's
    /// `TripleClick` falls through to `Click`, not to a fourth kind.
    pub fn press(&mut self, button: ClickButton, now: Instant) -> ClickCount {
        let within_interval = self
            .last_click
            .is_some_and(|last| now.duration_since(last) < CLICK_THRESHOLD);
        self.count = if self.last_button != Some(button) || !within_interval {
            ClickCount::Single
        } else {
            match self.count {
                ClickCount::Single => ClickCount::Double,
                ClickCount::Double => ClickCount::Triple,
                ClickCount::Triple => ClickCount::Single,
            }
        };
        self.last_click = Some(now);
        self.last_button = Some(button);
        self.count
    }

    /// Forgets the previous click.
    ///
    /// Alacritty does this when a press is reported to the program instead of
    /// selecting locally, so the next local click is not counted with it.
    pub fn reset(&mut self) {
        self.last_click = None;
        self.last_button = None;
        self.count = ClickCount::Single;
    }

    /// The count of the last press.
    pub fn count(&self) -> ClickCount {
        self.count
    }
}

/// The selection a click starts, or `None` when it starts none.
///
/// A single click starts a simple selection, or a block selection when the
/// block modifier is held. Double and triple clicks select a word and a line;
/// the block modifier only affects the single click. That matches the Tauri
/// client, where Alt+drag starts a column selection but a double click still
/// selects the word under the pointer.
pub fn selection_kind(count: ClickCount, block: bool) -> Option<SelectionKind> {
    match count {
        ClickCount::Single if block => Some(SelectionKind::Block),
        ClickCount::Single => Some(SelectionKind::Simple),
        ClickCount::Double => Some(SelectionKind::Semantic),
        ClickCount::Triple => Some(SelectionKind::Lines),
    }
}

#[cfg(test)]
mod tests;
