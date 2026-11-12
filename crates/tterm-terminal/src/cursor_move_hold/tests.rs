use std::cell::RefCell;
use std::rc::Rc;

use super::{CursorMoveHold, HOLD_MS, HeldWrite, INPUT_GRACE_MS, is_line_redraw_lead};

const TO_COLUMN_ZERO: &str = "\x1b[1G";
const ERASE_LINE: &str = "\x1b[0K";
const SPINNER_FRAME: &str = "⠹ Setting up project";
const CONPTY_RETURN_FRAME: &str = "\x1b[?25l\x1b[H\x1b[?25h";
const ANIMATION_FRAME: &str = "\x1b[3;3H⠹ Working";
const BACK_TO_INPUT: &str = "\x1b[30;12H";

struct Harness {
    hold: CursorMoveHold,
    clock: f64,
    writes: Vec<String>,
    parsed: Rc<RefCell<Vec<String>>>,
}

impl Harness {
    fn new() -> Self {
        Self {
            hold: CursorMoveHold::new(),
            clock: 1000.0,
            writes: Vec::new(),
            parsed: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn write(&mut self, text: &str, callback: Option<Box<dyn FnOnce()>>) {
        if let Some(write) = self.hold.write(text, callback, self.clock) {
            self.deliver(write);
        }
    }

    fn write_label(&mut self, text: &str, label: &'static str) {
        let parsed = Rc::clone(&self.parsed);
        self.write(
            text,
            Some(Box::new(move || {
                parsed.borrow_mut().push(label.to_string());
            })),
        );
    }

    fn flush(&mut self) {
        if let Some(write) = self.hold.flush() {
            self.deliver(write);
        }
    }

    fn advance_to(&mut self, clock: f64) {
        self.clock = clock;
        if let Some(write) = self.hold.flush_due(self.clock) {
            self.deliver(write);
        }
    }

    fn deliver(&mut self, write: HeldWrite) {
        self.writes.push(write.text);
        for callback in write.callbacks {
            callback();
        }
    }
}

#[test]
fn matches_a_return_to_a_lines_start_an_erase_or_a_cursor_toggle() {
    assert!(is_line_redraw_lead(TO_COLUMN_ZERO));
    assert!(is_line_redraw_lead(ERASE_LINE));
    assert!(is_line_redraw_lead(CONPTY_RETURN_FRAME));
    assert!(is_line_redraw_lead("\x1b[2K\x1b[1A\x1b[2K\x1b[G"));
    assert!(is_line_redraw_lead("\x1b[12;1H\r"));
    assert!(is_line_redraw_lead("\x1b[3F"));
    assert!(is_line_redraw_lead("\r"));
    assert!(is_line_redraw_lead("\x1b[?25l"));
}

#[test]
fn does_not_match_a_move_that_ends_in_another_column() {
    assert!(!is_line_redraw_lead(BACK_TO_INPUT));
    assert!(!is_line_redraw_lead("\x1b[?25l\x1b[30;12H\x1b[?25h"));
    assert!(!is_line_redraw_lead("\x1b[5G"));
    assert!(!is_line_redraw_lead("\x1b[1G\x1b[3C"));
    assert!(!is_line_redraw_lead("\r\x08"));
    assert!(!is_line_redraw_lead("\x1b[2A"));
}

#[test]
fn does_not_match_a_chunk_that_draws_scrolls_or_styles() {
    assert!(!is_line_redraw_lead(""));
    assert!(!is_line_redraw_lead(&format!(
        "{ERASE_LINE}{SPINNER_FRAME}"
    )));
    assert!(!is_line_redraw_lead("\x1b[1Gx"));
    assert!(!is_line_redraw_lead("\r\n"));
    assert!(!is_line_redraw_lead("\x1b[0m"));
    assert!(!is_line_redraw_lead("\x1b[?1049h"));
    assert!(!is_line_redraw_lead("\x1b]0;title\x07"));
}

#[test]
fn writes_the_return_and_erase_with_the_frame_that_follows_them() {
    let mut harness = Harness::new();
    harness.write_label(TO_COLUMN_ZERO, "return");
    harness.write_label(ERASE_LINE, "erase");
    assert!(harness.writes.is_empty());

    harness.write_label(SPINNER_FRAME, "frame");
    assert_eq!(
        harness.writes,
        vec![format!("{TO_COLUMN_ZERO}{ERASE_LINE}{SPINNER_FRAME}")]
    );
    assert_eq!(*harness.parsed.borrow(), vec!["return", "erase", "frame"]);
}

#[test]
fn holds_an_older_conptys_return_frame_the_same_way() {
    let mut harness = Harness::new();
    harness.write(CONPTY_RETURN_FRAME, None);
    harness.write(SPINNER_FRAME, None);
    assert_eq!(
        harness.writes,
        vec![format!("{CONPTY_RETURN_FRAME}{SPINNER_FRAME}")]
    );
}

#[test]
fn writes_a_move_back_to_an_input_box_at_once() {
    let mut harness = Harness::new();
    harness.write(ANIMATION_FRAME, None);
    harness.write(BACK_TO_INPUT, None);
    assert_eq!(harness.writes, vec![ANIMATION_FRAME, BACK_TO_INPUT]);
}

#[test]
fn writes_what_it_holds_once_hold_ms_passes_from_the_first_held_chunk() {
    let mut harness = Harness::new();
    harness.write(TO_COLUMN_ZERO, None);
    harness.clock += HOLD_MS - 1.0;
    harness.write(ERASE_LINE, None);
    assert!(harness.writes.is_empty());
    harness.advance_to(harness.clock + 1.0);
    assert_eq!(
        harness.writes,
        vec![format!("{TO_COLUMN_ZERO}{ERASE_LINE}")]
    );
}

#[test]
fn writes_other_output_at_once() {
    let mut harness = Harness::new();
    harness.write("hello\r\n", None);
    assert_eq!(harness.writes, vec!["hello\r\n"]);
}

#[test]
fn does_not_hold_the_answer_to_a_keystroke() {
    let mut harness = Harness::new();
    harness.hold.note_input(harness.clock);
    harness.clock += INPUT_GRACE_MS - 1.0;
    harness.write(TO_COLUMN_ZERO, None);
    assert_eq!(harness.writes, vec![TO_COLUMN_ZERO]);

    harness.clock += 1.0;
    harness.write(TO_COLUMN_ZERO, None);
    assert_eq!(harness.writes, vec![TO_COLUMN_ZERO]);
}

#[test]
fn keeps_a_held_chunk_ahead_of_a_keystrokes_answer() {
    let mut harness = Harness::new();
    harness.write(TO_COLUMN_ZERO, None);
    harness.hold.note_input(harness.clock);
    harness.write(ERASE_LINE, None);
    assert_eq!(
        harness.writes,
        vec![format!("{TO_COLUMN_ZERO}{ERASE_LINE}")]
    );
}

#[test]
fn writes_what_it_holds_when_flushed_and_not_again_after() {
    let mut harness = Harness::new();
    harness.write(TO_COLUMN_ZERO, None);
    harness.flush();
    assert_eq!(harness.writes, vec![TO_COLUMN_ZERO]);
    harness.advance_to(harness.clock + HOLD_MS);
    assert_eq!(harness.writes, vec![TO_COLUMN_ZERO]);
}

#[test]
fn drops_what_it_holds_once_disposed() {
    let mut harness = Harness::new();
    harness.write(TO_COLUMN_ZERO, None);
    harness.hold.dispose();
    harness.advance_to(harness.clock + HOLD_MS);
    assert!(harness.writes.is_empty());
}

#[test]
fn oracle_matches_edge_case_moves() {
    assert!(!is_line_redraw_lead("\x1b[999999999999999999999999G"));
    assert!(is_line_redraw_lead("\x1b[1;0H"));
    assert!(is_line_redraw_lead("\x1b[1;1;9H"));
    assert!(!is_line_redraw_lead("\u{3000}"));
    assert!(!is_line_redraw_lead("\x1b[1G\n"));
}
