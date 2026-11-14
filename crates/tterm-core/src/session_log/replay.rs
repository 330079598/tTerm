//! Replay frames of a raw terminal log and the player's timing rules, from
//! `src/lib/terminalLogReplay.ts`.
//!
//! A frame is a kind byte and the microseconds since the log started (u64),
//! then for output a length (u32) and the bytes, for a size change columns and
//! rows (u16 each). Little-endian, as `load_terminal_log_recording` writes it.

const FRAME_OUTPUT: u8 = 0;
const FRAME_RESIZE: u8 = 1;

/// One frame of a recording: output bytes or a size change.
#[derive(Debug, Clone, PartialEq)]
pub enum ReplayFrame {
    /// Bytes the terminal wrote.
    Output {
        /// Milliseconds from the start of the recording.
        at_ms: f64,
        /// The bytes written.
        data: Vec<u8>,
    },
    /// The terminal changed size.
    Resize {
        /// Milliseconds from the start of the recording.
        at_ms: f64,
        /// The new column count.
        cols: u16,
        /// The new row count.
        rows: u16,
    },
}

/// The size a recording starts at when it has no size frame.
pub const DEFAULT_REPLAY_SIZE: ReplaySize = ReplaySize { cols: 80, rows: 24 };

/// A terminal's columns and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplaySize {
    /// The column count.
    pub cols: u16,
    /// The row count.
    pub rows: u16,
}

impl ReplayFrame {
    /// Milliseconds from the start of the recording.
    pub fn at_ms(&self) -> f64 {
        match self {
            ReplayFrame::Output { at_ms, .. } | ReplayFrame::Resize { at_ms, .. } => *at_ms,
        }
    }
}

/// Parses the frames of a raw recording, stopping at a truncated or unknown one.
pub fn parse_replay_frames(buffer: &[u8]) -> Vec<ReplayFrame> {
    let mut frames = Vec::new();
    let mut offset = 0;
    while offset + 9 <= buffer.len() {
        let kind = buffer[offset];
        let at_us = u64::from_le_bytes([
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
            buffer[offset + 4],
            buffer[offset + 5],
            buffer[offset + 6],
            buffer[offset + 7],
            buffer[offset + 8],
        ]);
        let at_ms = at_us as f64 / 1000.0;
        offset += 9;

        match kind {
            FRAME_OUTPUT => {
                if offset + 4 > buffer.len() {
                    break;
                }
                let length = u32::from_le_bytes([
                    buffer[offset],
                    buffer[offset + 1],
                    buffer[offset + 2],
                    buffer[offset + 3],
                ]) as usize;
                offset += 4;
                let Some(end) = offset.checked_add(length) else {
                    break;
                };
                if end > buffer.len() {
                    break;
                }
                frames.push(ReplayFrame::Output {
                    at_ms,
                    data: buffer[offset..end].to_vec(),
                });
                offset = end;
            }
            FRAME_RESIZE => {
                if offset + 4 > buffer.len() {
                    break;
                }
                frames.push(ReplayFrame::Resize {
                    at_ms,
                    cols: u16::from_le_bytes([buffer[offset], buffer[offset + 1]]),
                    rows: u16::from_le_bytes([buffer[offset + 2], buffer[offset + 3]]),
                });
                offset += 4;
            }
            _ => break,
        }
    }
    frames
}

/// The size the recording starts at: its first known size.
pub fn initial_replay_size(frames: &[ReplayFrame]) -> ReplaySize {
    for frame in frames {
        if let ReplayFrame::Resize { cols, rows, .. } = frame {
            return ReplaySize {
                cols: *cols,
                rows: *rows,
            };
        }
    }
    DEFAULT_REPLAY_SIZE
}

/// When each frame plays, in ms from the start.
///
/// With `idle_limit_ms`, a pause longer than that plays as that long, so a
/// recording left open overnight does not take overnight to watch.
pub fn build_replay_timeline(frames: &[ReplayFrame], idle_limit_ms: Option<f64>) -> Vec<f64> {
    let mut timeline = Vec::with_capacity(frames.len());
    let mut previous_at = frames.first().map_or(0.0, ReplayFrame::at_ms);
    let mut play_at = 0.0;
    for frame in frames {
        let at_ms = frame.at_ms();
        let gap = (at_ms - previous_at).max(0.0);
        play_at += idle_limit_ms.map_or(gap, |limit| gap.min(limit));
        previous_at = at_ms;
        timeline.push(play_at);
    }
    timeline
}

/// How long a timeline plays, in ms.
pub fn replay_duration(timeline: &[f64]) -> f64 {
    timeline.last().copied().unwrap_or(0.0)
}

/// How many frames have played by `time_ms`.
pub fn frames_played_by(timeline: &[f64], time_ms: f64) -> usize {
    let mut low = 0;
    let mut high = timeline.len();
    while low < high {
        let middle = (low + high) / 2;
        if timeline[middle] <= time_ms {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    low
}

/// Formats a playback time as `M:SS`, or `H:MM:SS` past an hour.
pub fn format_replay_time(ms: f64) -> String {
    let total_seconds = (ms / 1000.0).floor().max(0.0) as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// The smallest font a replay can be set or fitted to.
pub const MIN_REPLAY_FONT_SIZE: f64 = 6.0;
/// The largest font a replay can be set or fitted to.
pub const MAX_REPLAY_FONT_SIZE: f64 = 48.0;

/// Keeps a replay font size inside the range the player accepts.
pub fn clamp_replay_font_size(font_size: f64) -> f64 {
    font_size.clamp(MIN_REPLAY_FONT_SIZE, MAX_REPLAY_FONT_SIZE)
}

/// The font size at which `text` grows or shrinks to fill `available` along one
/// dimension.
///
/// Only the text scales with the font; `chrome`, the space beside it, stays
/// the same. Rounds down to a quarter point, so it errs on the side of fitting.
pub fn fit_replay_font_size(font_size: f64, text: f64, chrome: f64, available: f64) -> f64 {
    if text <= 0.0 {
        return font_size;
    }
    clamp_replay_font_size(((font_size * (available - chrome) / text) * 4.0).floor() / 4.0)
}

/// A width and height in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayBox {
    /// The width.
    pub width: f64,
    /// The height.
    pub height: f64,
}

/// The font a replay is drawn at, and whether the panel held the preferred
/// size back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayFontSize {
    /// The size to draw at.
    pub font_size: f64,
    /// The panel held the preferred size back.
    pub capped: bool,
}

/// The font a replay is drawn at, given how big its text is at `font_size`:
/// `preferred`, or with `fit` the largest size that fits the panel.
///
/// Either way the recording always fits the panel, since a panel scrolling
/// around the terminal's own scrollback leaves two scrollbars and half the screen
/// out of view. `capped` says the panel held the preferred size back.
pub fn replay_font_size(
    font_size: f64,
    preferred: f64,
    fit: bool,
    text: ReplayBox,
    chrome: ReplayBox,
    available: ReplayBox,
) -> ReplayFontSize {
    let largest = fit_replay_font_size(font_size, text.width, chrome.width, available.width).min(
        fit_replay_font_size(font_size, text.height, chrome.height, available.height),
    );
    let wanted = if fit {
        largest
    } else {
        clamp_replay_font_size(preferred)
    };
    ReplayFontSize {
        font_size: wanted.min(largest),
        capped: largest < wanted,
    }
}

#[cfg(test)]
mod tests;
