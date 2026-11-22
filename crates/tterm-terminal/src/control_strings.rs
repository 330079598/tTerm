//! Bounded framing for the control strings the output scanners share.
//!
//! This extracts only the escape-state machine from `MousePromptScanner`.
//! Prompt marks keep their first OSC field; OSC 52 keeps the whole payload so
//! it can be decoded without a second parser. Both buffers are bounded and an
//! overlong string is dropped whole at its terminator rather than treated as a
//! truncated prefix.

use crate::osc52::MAX_OSC52_PAYLOAD_BYTES;

/// The longest OSC first field kept for prompt recognition.
const MAX_FIRST_FIELD: usize = 64;

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

/// A completed top-level control string and the offsets of its bytes.
pub(crate) struct CompletedControlString<'a> {
    /// The OSC code when its digits were valid.
    pub(crate) code: Option<u16>,
    /// The bytes before the first `;`, bounded by [`MAX_FIRST_FIELD`].
    pub(crate) first_field: &'a [u8],
    /// Whether the first field fit before its bound.
    pub(crate) first_field_valid: bool,
    /// The whole payload after the code, kept only for OSC 52.
    pub(crate) payload: &'a [u8],
    /// Whether the payload fit before its bound.
    pub(crate) payload_valid: bool,
    /// The byte offset just past the terminator.
    pub(crate) end: usize,
}

/// Splits output at complete top-level OSC strings without pausing the model.
#[derive(Debug, Default)]
pub(crate) struct ControlStringFramer {
    payload_codes: &'static [u16],
    payload_limit: usize,
    state: ScanState,
    utf8: Utf8State,
    osc_code: u16,
    osc_digits: u8,
    osc_code_valid: bool,
    first_field: Vec<u8>,
    first_field_valid: bool,
    first_field_done: bool,
    payload: Vec<u8>,
    payload_valid: bool,
}

impl ControlStringFramer {
    /// Keeps OSC 52 payload bytes for the store scanner.
    pub(crate) fn with_osc52_payload(capture: bool) -> Self {
        const OSC52_CODES: [u16; 1] = [crate::osc52::OSC52_CODE];
        Self {
            payload_codes: if capture { &OSC52_CODES } else { &[] },
            payload_limit: MAX_OSC52_PAYLOAD_BYTES,
            ..Self::default()
        }
    }

    /// Keeps the payload of each listed OSC code up to `limit` bytes.
    pub(crate) fn with_payload_codes(codes: &'static [u16], limit: usize) -> Self {
        Self {
            payload_codes: codes,
            payload_limit: limit,
            ..Self::default()
        }
    }

    /// Scans one chunk and calls `on_complete` for each completed OSC.
    pub(crate) fn scan(
        &mut self,
        bytes: &[u8],
        mut on_complete: impl FnMut(CompletedControlString<'_>),
    ) {
        for (index, &byte) in bytes.iter().enumerate() {
            match self.state {
                ScanState::Ground => self.ground(byte),
                ScanState::Escape => self.escape(byte),
                ScanState::EscapeIntermediate => self.escape_intermediate(byte),
                ScanState::Csi => self.csi(byte),
                ScanState::OscCode => self.osc_code(byte),
                ScanState::OscPayload => self.osc_payload(byte, index, &mut on_complete),
                ScanState::OscEscape => self.osc_escape(byte, index, &mut on_complete),
                ScanState::ControlString => self.control_string(byte),
            }
        }
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
        // Raw C1 introducers in ground are invalid bytes, not OSC starts.
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
        self.first_field.clear();
        self.first_field_valid = true;
        self.first_field_done = false;
        self.payload.clear();
        self.payload_valid = true;
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

    fn osc_payload(
        &mut self,
        byte: u8,
        index: usize,
        on_complete: &mut impl FnMut(CompletedControlString<'_>),
    ) {
        match byte {
            0x07 => self.finish_osc(index + 1, on_complete),
            0x18 | 0x1A => {
                self.clear_osc();
                self.state = ScanState::Ground;
            }
            0x1B => self.state = ScanState::OscEscape,
            b';' if !self.first_field_done => {
                self.first_field_done = true;
                if self.captures_payload() {
                    self.push_payload(b';');
                }
            }
            _ if self.first_field_done => {
                if self.captures_payload() {
                    self.push_payload(byte);
                }
            }
            0x00..=0x06 | 0x08..=0x17 | 0x19 | 0x1C..=0x1F => {}
            _ => self.push_first_field(byte),
        }
    }

    fn osc_escape(
        &mut self,
        byte: u8,
        index: usize,
        on_complete: &mut impl FnMut(CompletedControlString<'_>),
    ) {
        match byte {
            b'\\' => {
                self.finish_osc(index + 1, on_complete);
                self.state = ScanState::Ground;
            }
            0x18 | 0x1A => {
                self.finish_osc(index, on_complete);
                self.state = ScanState::Ground;
            }
            0x1B => {
                self.finish_osc(index, on_complete);
                self.state = ScanState::Escape;
            }
            _ => {
                self.finish_osc(index, on_complete);
                self.state = ScanState::Escape;
                self.escape(byte);
            }
        }
    }

    fn finish_osc(&mut self, end: usize, on_complete: &mut impl FnMut(CompletedControlString<'_>)) {
        let mut first_field = std::mem::take(&mut self.first_field);
        let mut payload = std::mem::take(&mut self.payload);
        on_complete(CompletedControlString {
            code: self.osc_code_valid.then_some(self.osc_code),
            first_field: &first_field,
            first_field_valid: self.first_field_valid,
            payload: &payload,
            payload_valid: self.payload_valid,
            end,
        });
        first_field.clear();
        payload.clear();
        self.first_field = first_field;
        self.payload = payload;
        self.clear_osc();
        self.state = ScanState::Ground;
    }

    fn clear_osc(&mut self) {
        self.first_field.clear();
        self.payload.clear();
        self.first_field_valid = false;
        self.first_field_done = false;
        self.payload_valid = false;
        self.osc_code_valid = false;
    }

    fn captures_payload(&self) -> bool {
        self.osc_code_valid && self.payload_codes.contains(&self.osc_code)
    }

    fn push_first_field(&mut self, byte: u8) {
        if self.first_field_valid {
            if self.first_field.len() < MAX_FIRST_FIELD {
                self.first_field.push(byte);
            } else {
                self.first_field_valid = false;
            }
        }
        if self.captures_payload() {
            self.push_payload(byte);
        }
    }

    fn push_payload(&mut self, byte: u8) {
        if self.payload_valid {
            if self.payload.len() < self.payload_limit {
                self.payload.push(byte);
            } else {
                self.payload_valid = false;
            }
        }
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
