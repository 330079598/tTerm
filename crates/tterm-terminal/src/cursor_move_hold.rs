//! Holds a cursor-move chunk until the drawing that follows it.
//!
//! Spinners redraw a line in several writes. Holding the move avoids showing
//! the cursor at column zero between them. The caller owns the timer and the
//! terminal parser; this module owns only the buffering rules.

use std::sync::LazyLock;

use regress::Regex;

use crate::js_text::{from_utf16_units_lossy, utf16_units};

/// The longest a held chunk waits for the drawing after it.
pub const HOLD_MS: f64 = 50.0;

/// After a keystroke, output is the answer to it and is not held.
pub const INPUT_GRACE_MS: f64 = 150.0;

const ESC: char = '\u{001B}';

static SEQUENCE_AFTER_ESC: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^\[(?:\?25[hl]|([0-9;]*)([A-Za-z]))(\r*)$").ok());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Effect {
    LineStart,
    Vertical,
    None,
    Other,
}

fn effect_of(params: &str, final_byte: char) -> Effect {
    let mut parts = params.split(';');
    let first = parts.next().unwrap_or_default();
    let second = parts.next().unwrap_or_default();
    match final_byte {
        'E' | 'F' => Effect::LineStart,
        'G' => {
            if number_or_one(first) <= 1.0 {
                Effect::LineStart
            } else {
                Effect::Other
            }
        }
        'H' | 'f' => {
            if number_or_one(second) <= 1.0 {
                Effect::LineStart
            } else {
                Effect::Other
            }
        }
        'A' | 'B' | 'd' => Effect::Vertical,
        'J' | 'K' => Effect::None,
        _ => Effect::Other,
    }
}

fn number_or_one(text: &str) -> f64 {
    if text.is_empty() {
        return 1.0;
    }
    text.parse::<f64>().unwrap_or(f64::INFINITY)
}

/// Whether a whole chunk draws nothing and leaves the cursor at a line start,
/// or does not move it.
pub fn is_line_redraw_lead(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let mut pieces = text.split(ESC);
    let lead = pieces.next().unwrap_or_default();
    if !lead.chars().all(|character| character == '\r') {
        return false;
    }
    let mut effects = Vec::new();
    if !lead.is_empty() {
        effects.push(Effect::LineStart);
    }
    for piece in pieces {
        let units = utf16_units(piece);
        let Some(found) = SEQUENCE_AFTER_ESC
            .as_ref()
            .and_then(|pattern| pattern.find_from_ucs2(&units, 0).next())
        else {
            return false;
        };
        let params = found
            .group(1)
            .map(|range| from_utf16_units_lossy(&units[range]))
            .unwrap_or_default();
        let final_byte = found
            .group(2)
            .map(|range| from_utf16_units_lossy(&units[range]))
            .and_then(|value| value.chars().next());
        effects.push(match final_byte {
            Some(final_byte) => effect_of(&params, final_byte),
            None => Effect::None,
        });
        if found.group(3).is_some_and(|range| !range.is_empty()) {
            effects.push(Effect::LineStart);
        }
    }
    if effects.contains(&Effect::Other) {
        return false;
    }
    !effects.contains(&Effect::Vertical) || effects.contains(&Effect::LineStart)
}

/// One write the caller should hand to the terminal parser.
pub struct HeldWrite {
    /// The bytes to write.
    pub text: String,
    /// Callbacks to run once the terminal has parsed the write.
    pub callbacks: Vec<Box<dyn FnOnce()>>,
}

struct HeldChunks {
    text: String,
    callbacks: Vec<Box<dyn FnOnce()>>,
}

/// Buffers cursor-move chunks until the drawing that follows them.
pub struct CursorMoveHold {
    held: Option<HeldChunks>,
    deadline: Option<f64>,
    last_input_at: f64,
}

impl Default for CursorMoveHold {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorMoveHold {
    /// Creates a hold with no pending chunk and no input seen.
    pub fn new() -> Self {
        Self {
            held: None,
            deadline: None,
            last_input_at: f64::NEG_INFINITY,
        }
    }

    /// Notes input at `now_ms`, so its echo is not delayed.
    pub fn note_input(&mut self, now_ms: f64) {
        self.last_input_at = now_ms;
    }

    /// The clock value at which [`Self::flush_due`] should run.
    pub fn next_deadline(&self) -> Option<f64> {
        self.deadline
    }

    /// Writes a chunk now or holds it if it leads a line redraw.
    pub fn write(
        &mut self,
        text: &str,
        callback: Option<Box<dyn FnOnce()>>,
        now_ms: f64,
    ) -> Option<HeldWrite> {
        let callbacks = callback.into_iter().collect();
        if is_line_redraw_lead(text) && now_ms - self.last_input_at >= INPUT_GRACE_MS {
            match &mut self.held {
                Some(held) => {
                    held.text.push_str(text);
                    held.callbacks.extend(callbacks);
                }
                None => {
                    self.held = Some(HeldChunks {
                        text: text.to_owned(),
                        callbacks,
                    });
                    self.deadline = Some(now_ms + HOLD_MS);
                }
            }
            return None;
        }
        let previous = self.take_held();
        match previous {
            None => Some(HeldWrite {
                text: text.to_owned(),
                callbacks,
            }),
            Some(mut previous) => {
                previous.text.push_str(text);
                previous.callbacks.extend(callbacks);
                Some(previous)
            }
        }
    }

    /// Writes the held chunk, if any, before the caller writes directly.
    pub fn flush(&mut self) -> Option<HeldWrite> {
        self.take_held()
    }

    /// Writes the held chunk once its deadline arrives.
    pub fn flush_due(&mut self, now_ms: f64) -> Option<HeldWrite> {
        if self.deadline.is_some_and(|deadline| now_ms >= deadline) {
            self.take_held()
        } else {
            None
        }
    }

    /// Drops any held chunk.
    pub fn dispose(&mut self) {
        self.take_held();
    }

    fn take_held(&mut self) -> Option<HeldWrite> {
        self.deadline = None;
        self.held.take().map(|held| HeldWrite {
            text: held.text,
            callbacks: held.callbacks,
        })
    }
}

#[cfg(test)]
mod tests;
