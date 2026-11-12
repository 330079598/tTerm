//! Rolling scan of decoded terminal output for the "[Connecting" line.

use crate::js_encoding::decode_utf8_text;
use crate::js_text::{utf16_len, utf16_tail};

/// The status line that marks the cold connection path.
pub const STATUS_CONNECTING: &str = "[Connecting";

/// The marker can span chunk boundaries, so a carry of marker-length-minus-one
/// UTF-16 units is kept between chunks.
const CONNECTING_CARRY_CHARS: usize = 10;

/// State carried between output chunks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalOutputScanState {
    /// The tail of the previous chunks.
    pub tail: String,
}

/// The result of scanning one chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalOutputScanResult {
    /// The state to pass to the next chunk.
    pub state: TerminalOutputScanState,
    /// Whether a cold-path "[Connecting" line was seen in this chunk.
    pub connecting: bool,
}

/// An empty scan state.
pub fn empty_output_scan_state() -> TerminalOutputScanState {
    TerminalOutputScanState::default()
}

/// Scans one decoded chunk. The marker must reflect the live stream, so only
/// the carry joins the chunk; historical output cannot flag it again.
pub fn scan_terminal_output(
    current: &TerminalOutputScanState,
    chunk: &str,
) -> TerminalOutputScanResult {
    let window = format!("{}{}", current.tail, chunk);
    TerminalOutputScanResult {
        state: TerminalOutputScanState {
            tail: utf16_tail(&window, CONNECTING_CARRY_CHARS),
        },
        connecting: window.contains(STATUS_CONNECTING),
    }
}

/// Decodes a binary channel payload into text.
///
/// The backend batches on UTF-8 boundaries, but malformed bytes still decode
/// lossily to U+FFFD instead of dropping the whole chunk. The frontend uses a
/// decoder without streaming state, so each call is independent; this function
/// has the same contract.
pub fn decode_output_chunk(payload: &[u8]) -> String {
    decode_utf8_text(payload)
}

/// The number of UTF-16 units that can be carried between chunks.
pub const fn connecting_carry_chars() -> usize {
    CONNECTING_CARRY_CHARS
}

/// The length of the connecting marker in UTF-16 units.
pub fn connecting_marker_len() -> usize {
    utf16_len(STATUS_CONNECTING)
}

#[cfg(test)]
mod tests;
