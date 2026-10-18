use super::SerialView;
use crate::terminal::encoding::{OutputDecoder, TerminalEncoding};
use crate::terminal::reader::{FilteredOutput, OutputFilter};
use std::fmt::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Bytes on one line of the hex view.
const HEX_BYTES_PER_LINE: usize = 16;
/// A pause this long between bytes starts a new line of the hex view, so each
/// frame a device sends begins a line of its own.
const HEX_FRAME_GAP: Duration = Duration::from_millis(50);

/// Turns what a serial device sends into what its tab shows: text in the
/// device's charset decoded to UTF-8, or every byte in hex. The session log
/// records the text either way.
pub(super) struct SerialOutput {
    decoder: Option<OutputDecoder>,
    view: Arc<SerialView>,
    hex: HexDump,
}

impl SerialOutput {
    pub(super) fn new(encoding: TerminalEncoding, view: Arc<SerialView>) -> Self {
        Self {
            decoder: encoding.output_decoder(),
            view,
            hex: HexDump::default(),
        }
    }

    pub(super) fn filter_at(&mut self, output: Vec<u8>, now: Instant) -> FilteredOutput {
        // The hex view shows the bytes the device sent, not their decoding.
        let display = self.view.hex().then(|| self.hex.dump(&output, now));
        let text = match self.decoder.as_mut() {
            Some(decoder) => decoder.decode(&output),
            None => output,
        };
        match display.or_else(|| self.hex.leave(&text)) {
            Some(display) => FilteredOutput {
                display,
                log: Some(text),
            },
            None => FilteredOutput {
                display: text,
                log: None,
            },
        }
    }
}

impl OutputFilter for SerialOutput {
    fn filter(&mut self, output: Vec<u8>) -> FilteredOutput {
        self.filter_at(output, Instant::now())
    }
}

/// Where the hex view is in its current line.
#[derive(Default)]
pub(super) struct HexDump {
    showing: bool,
    column: usize,
    last_byte_at: Option<Instant>,
    /// The text shown before the view turned on ended its line.
    text_mid_line: bool,
}

impl HexDump {
    /// `bytes` as hex, `0D 0A ` and so on, starting new lines as they fill
    /// and after a pause.
    pub(super) fn dump(&mut self, bytes: &[u8], now: Instant) -> Vec<u8> {
        let mut out = String::with_capacity(bytes.len() * 3 + 4);
        let paused = self
            .last_byte_at
            .is_some_and(|last| now.duration_since(last) >= HEX_FRAME_GAP);
        if !self.showing {
            self.showing = true;
            if self.text_mid_line {
                out.push_str("\r\n");
            }
            self.column = 0;
        } else if paused && self.column > 0 {
            out.push_str("\r\n");
            self.column = 0;
        }
        for byte in bytes {
            if self.column == HEX_BYTES_PER_LINE {
                out.push_str("\r\n");
                self.column = 0;
            }
            let _ = write!(out, "{byte:02X} ");
            self.column += 1;
        }
        if !bytes.is_empty() {
            self.last_byte_at = Some(now);
        }
        out.into_bytes()
    }

    /// Text shown after the hex view: on a line of its own when the view had
    /// one open. `None` when the text shows as it is.
    pub(super) fn leave(&mut self, text: &[u8]) -> Option<Vec<u8>> {
        let mid_hex_line = self.showing && self.column > 0;
        self.showing = false;
        self.column = 0;
        if let Some(&last) = text.last() {
            self.text_mid_line = last != b'\n';
        }
        mid_hex_line.then(|| [b"\r\n".as_slice(), text].concat())
    }
}
