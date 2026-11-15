//! Bounded scanning for shell-integration prompt marks in terminal output.
//!
//! The Tauri client asks xterm's current mouse mode from an OSC handler when
//! a prompt mark arrives. The native model does not expose parser handlers to
//! the session, so the parser splits the output stream at completed top-level
//! OSC 133/633 type A marks and asks the real model at each one.
//!
//! The scanner must never become a second, unbounded parser. It stores only a
//! short OSC code and a bounded payload, forgets the contents of every other
//! control string, and returns byte offsets rather than decoded strings. Its
//! state survives arbitrary chunk splits, including a split between `ESC` and
//! the `\\` that completes a string terminator.

use crate::command_mark::CommandMark;
use crate::command_marks::parse_command_mark;

/// The exact sequence the Tauri guard writes to close stale mouse reporting.
pub const MOUSE_REPORTING_OFF: &[u8] = b"\x1b[?9l\x1b[?1000l\x1b[?1001l\x1b[?1002l\x1b[?1003l\x1b[?1005l\x1b[?1006l\x1b[?1015l\x1b[?1016l";

/// The longest OSC payload prefix kept for prompt recognition.
///
/// A prompt mark is `A` or `A;...`; 64 bytes cover every shell integration
/// payload the application needs while keeping a hostile unfinished OSC
/// bounded.
const MAX_OSC_PAYLOAD: usize = 64;

/// The most digits accepted in an OSC code before it is treated as invalid.
const MAX_OSC_CODE_DIGITS: u8 = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ScanState {
    #[default]
    Ground,
    Escape,
    EscapeIntermediate,
    Csi,
    OscCode,
    OscPayload,
    OscEscape,
    ControlString,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Utf8State {
    remaining: u8,
    min: u8,
    max: u8,
}

/// Scans decoded output for completed OSC 133/633 prompt marks.
#[derive(Debug, Default)]
pub struct MousePromptScanner {
    state: ScanState,
    utf8: Utf8State,
    osc_code: u16,
    osc_digits: u8,
    osc_code_valid: bool,
    osc_payload: Vec<u8>,
    osc_payload_valid: bool,
    osc_first_field_done: bool,
}

impl MousePromptScanner {
    /// Scans one chunk and returns the offsets just past each completed
    /// top-level prompt terminator in this chunk.
    ///
    /// An offset is always after the complete BEL or ST terminator. A split
    /// between `ESC` and `\` is never reported, because that would leave the
    /// model half way through a string terminator.
    pub fn scan(&mut self, bytes: &[u8]) -> Vec<usize> {
        let mut offsets = Vec::new();
        for (index, &byte) in bytes.iter().enumerate() {
            match self.state {
                ScanState::Ground => self.ground(byte),
                ScanState::Escape => self.escape(byte),
                ScanState::EscapeIntermediate => self.escape_intermediate(byte),
                ScanState::Csi => self.csi(byte),
                ScanState::OscCode => self.osc_code(byte),
                ScanState::OscPayload => self.osc_payload(byte, index, &mut offsets),
                ScanState::OscEscape => {
                    self.osc_escape(byte, index, &mut offsets);
                }
                ScanState::ControlString => self.control_string(byte),
            }
        }
        offsets
    }

    fn ground(&mut self, byte: u8) {
        if self.utf8.remaining > 0 {
            if (self.utf8.min..=self.utf8.max).contains(&byte) {
                self.utf8.remaining -= 1;
                if self.utf8.remaining == 0 {
                    self.utf8 = Utf8State::default();
                } else {
                    self.utf8.min = 0x80;
                    self.utf8.max = 0xBF;
                }
                return;
            }
            self.utf8 = Utf8State::default();
        }
        if byte < 0x80 {
            match byte {
                0x1B => self.state = ScanState::Escape,
                // CAN and SUB execute in ground without changing state.
                0x18 | 0x1A => {}
                _ => {}
            }
            return;
        }
        if let Some(utf8) = utf8_lead(byte) {
            self.utf8 = utf8;
        }
        // The model's vte handler ignores raw C1 introducers in ground; only
        // two-byte ESC sequences start control strings. A byte in that range
        // here is invalid UTF-8, not a sequence introducer.
    }

    fn escape(&mut self, byte: u8) {
        match byte {
            0x18 | 0x1A => self.state = ScanState::Ground,
            0x1B => {}
            0x20..=0x2F => self.state = ScanState::EscapeIntermediate,
            b']' => self.start_osc(),
            b'[' => self.state = ScanState::Csi,
            b'P' | b'X' | b'^' | b'_' => self.state = ScanState::ControlString,
            _ => self.state = ScanState::Ground,
        }
    }

    fn escape_intermediate(&mut self, byte: u8) {
        match byte {
            0x18 | 0x1A => self.state = ScanState::Ground,
            0x1B => self.state = ScanState::Escape,
            0x20..=0x2F | 0x7F => {}
            0x30..=0x7E => self.state = ScanState::Ground,
            _ => {}
        }
    }

    fn csi(&mut self, byte: u8) {
        match byte {
            0x18 | 0x1A => self.state = ScanState::Ground,
            0x1B => self.state = ScanState::Escape,
            0x40..=0x7E => self.state = ScanState::Ground,
            _ => {}
        }
    }

    fn control_string(&mut self, byte: u8) {
        match byte {
            0x18 | 0x1A => self.state = ScanState::Ground,
            0x1B => self.state = ScanState::Escape,
            _ => {}
        }
    }

    fn start_osc(&mut self) {
        self.state = ScanState::OscCode;
        self.osc_code = 0;
        self.osc_digits = 0;
        self.osc_code_valid = true;
        self.osc_payload.clear();
        self.osc_payload_valid = true;
        self.osc_first_field_done = false;
    }

    fn osc_code(&mut self, byte: u8) {
        match byte {
            b'0'..=b'9' if self.osc_digits < MAX_OSC_CODE_DIGITS => {
                self.osc_code = self
                    .osc_code
                    .saturating_mul(10)
                    .saturating_add(u16::from(byte - b'0'));
                self.osc_digits += 1;
            }
            b'0'..=b'9' => {
                self.osc_code_valid = false;
                self.osc_digits = self.osc_digits.saturating_add(1);
            }
            b';' => self.state = ScanState::OscPayload,
            0x07 => self.state = ScanState::Ground,
            0x18 | 0x1A => self.state = ScanState::Ground,
            0x1B => self.state = ScanState::OscEscape,
            0x00..=0x17 | 0x19 | 0x1C..=0x1F => {}
            _ => self.state = ScanState::Ground,
        }
    }

    fn osc_payload(&mut self, byte: u8, index: usize, offsets: &mut Vec<usize>) {
        match byte {
            0x07 => self.finish_osc(index + 1, offsets),
            0x18 | 0x1A => {
                self.clear_osc();
                self.state = ScanState::Ground;
            }
            0x1B => self.state = ScanState::OscEscape,
            // The first OSC parameter is enough to recognize a prompt mark;
            // every later attribute is discarded without growing memory.
            b';' if !self.osc_first_field_done => self.osc_first_field_done = true,
            _ if self.osc_first_field_done => {}
            0x00..=0x06 | 0x08..=0x17 | 0x19 | 0x1C..=0x1F => {}
            _ if self.osc_payload_valid && self.osc_payload.len() < MAX_OSC_PAYLOAD => {
                self.osc_payload.push(byte);
            }
            _ => self.osc_payload_valid = false,
        }
    }

    fn osc_escape(&mut self, byte: u8, index: usize, offsets: &mut Vec<usize>) {
        match byte {
            b'\\' => {
                self.finish_osc(index + 1, offsets);
                self.state = ScanState::Ground;
            }
            0x18 | 0x1A => {
                self.finish_osc(index, offsets);
                self.state = ScanState::Ground;
            }
            // Another ESC restarts the escape sequence. The previous OSC ended
            // at the first ESC, so that is where its prompt offset is reported.
            0x1B => {
                self.finish_osc(index, offsets);
                self.state = ScanState::Escape;
            }
            // VTE ends an OSC when ESC arrives and then processes this byte in
            // Escape. The offset is after the ESC (never between it and a
            // backslash), and this byte is the next escape byte.
            _ => {
                self.finish_osc(index, offsets);
                self.state = ScanState::Escape;
                self.escape(byte);
            }
        }
    }

    fn finish_osc(&mut self, end: usize, offsets: &mut Vec<usize>) {
        let prompt = self.osc_code_valid
            && matches!(self.osc_code, 133 | 633)
            && self.osc_payload_valid
            && std::str::from_utf8(&self.osc_payload)
                .ok()
                .and_then(parse_command_mark)
                == Some(CommandMark::Prompt);
        self.clear_osc();
        self.state = ScanState::Ground;
        if prompt {
            offsets.push(end);
        }
    }

    fn clear_osc(&mut self) {
        self.osc_payload.clear();
        self.osc_payload_valid = false;
        self.osc_first_field_done = false;
        self.osc_code_valid = false;
    }
}

/// The UTF-8 lead-byte state for the first continuation of a sequence.
fn utf8_lead(byte: u8) -> Option<Utf8State> {
    let (remaining, min, max) = match byte {
        0xC2..=0xDF => (1, 0x80, 0xBF),
        0xE0 => (2, 0xA0, 0xBF),
        0xE1..=0xEC | 0xEE..=0xEF => (2, 0x80, 0xBF),
        0xED => (2, 0x80, 0x9F),
        0xF0 => (3, 0x90, 0xBF),
        0xF1..=0xF3 => (3, 0x80, 0xBF),
        0xF4 => (3, 0x80, 0x8F),
        _ => return None,
    };
    Some(Utf8State {
        remaining,
        min,
        max,
    })
}

#[cfg(test)]
mod tests;
