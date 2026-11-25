//! SSH-specific fixed replies for a client which owns the terminal parser.
//!
//! Only a possible fixed query is held across reads. Display slices and fixed
//! replies are visited in stream order, so the caller can drain model replies
//! for each preceding slice before writing the fixed reply.

use std::borrow::Cow;

use super::{DA1_REPLY, DA2_REPLY, DEC_CPR_REPLY, MAX_DEFERRED_QUERY_BYTES, capability_reply};

/// One part of the SSH output stream, in the order in which it was received.
pub enum Part<'a> {
    /// Feed these bytes to the live terminal model.
    Display(&'a [u8]),
    /// Write these bytes to the session instead of feeding the query to the model.
    Reply(&'a [u8]),
}

/// How many continuation bytes a valid UTF-8 sequence still expects.
///
/// Strings carry this across reads. A `0x9c` byte is a raw ST only when it is
/// not the continuation of a multi-byte character; otherwise OSC titles and
/// DCS payloads written in UTF-8 would end in the middle of a character.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Utf8 {
    remaining: u8,
}

impl Utf8 {
    /// Advances over one string byte, starting a new sequence when needed.
    fn advance(&mut self, byte: u8) {
        if self.remaining > 0 {
            if (0x80..=0xbf).contains(&byte) {
                self.remaining -= 1;
                return;
            }
            // An invalid sequence ends before this byte, which may start one.
            self.remaining = 0;
        }
        self.remaining = match byte {
            0xc2..=0xdf => 1,
            0xe0..=0xef => 2,
            0xf0..=0xf4 => 3,
            _ => 0,
        };
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum State {
    #[default]
    Ground,
    Escape,
    Csi,
    Dcs,
    Capability,
    CapabilityEscape,
    String {
        bell_ends: bool,
        utf8: Utf8,
    },
    StringEscape {
        bell_ends: bool,
    },
}

/// The bounded, per-SSH-session framing state of the fixed-reply adapter.
///
/// `pending` never grows past the adapter's deferred-query bound. Display
/// bytes that do not belong to a possible query are forwarded immediately;
/// only an incomplete query tail waits for the next read.
#[derive(Default)]
pub struct OrderedQueries {
    state: State,
    pending: Vec<u8>,
    display: Vec<u8>,
}

impl OrderedQueries {
    /// Coalesces display bytes between fixed replies, so a screenful of SGR
    /// changes still takes the model lock only once. Plain text needs no copy.
    pub fn feed(&mut self, bytes: &[u8], mut visit: impl FnMut(Part<'_>)) {
        if self.state == State::Ground && self.pending.is_empty() && !bytes.contains(&0x1b) {
            if !bytes.is_empty() {
                visit(Part::Display(bytes));
            }
            return;
        }
        let mut display = std::mem::take(&mut self.display);
        self.scan(bytes, &mut |part| match part {
            Part::Display(bytes) => display.extend_from_slice(bytes),
            Part::Reply(bytes) => {
                if !display.is_empty() {
                    visit(Part::Display(&display));
                    display.clear();
                }
                visit(Part::Reply(bytes));
            }
        });
        if !display.is_empty() {
            visit(Part::Display(&display));
            display.clear();
        }
        self.display = display;
    }

    fn scan(&mut self, mut bytes: &[u8], visit: &mut impl FnMut(Part<'_>)) {
        while !bytes.is_empty() {
            match self.state {
                State::Ground => {
                    let end = bytes
                        .iter()
                        .position(|&byte| byte == 0x1b)
                        .unwrap_or(bytes.len());
                    if end > 0 {
                        visit(Part::Display(&bytes[..end]));
                        bytes = &bytes[end..];
                        continue;
                    }
                }
                State::String { bell_ends, utf8 } => {
                    let (end, utf8) = string_run(bytes, bell_ends, utf8);
                    self.state = State::String { bell_ends, utf8 };
                    if end > 0 {
                        visit(Part::Display(&bytes[..end]));
                        bytes = &bytes[end..];
                        continue;
                    }
                }
                _ => {}
            }
            self.push(bytes[0], visit);
            bytes = &bytes[1..];
        }
    }

    fn push(&mut self, byte: u8, visit: &mut impl FnMut(Part<'_>)) {
        use State::*;
        match self.state {
            String { bell_ends, .. } => {
                // Only a terminator reaches here; string_run consumed the rest.
                visit(Part::Display(std::slice::from_ref(&byte)));
                self.state = if byte == 0x1b {
                    StringEscape { bell_ends }
                } else {
                    Ground
                };
                return;
            }
            StringEscape { bell_ends } => {
                visit(Part::Display(std::slice::from_ref(&byte)));
                self.state =
                    if matches!(byte, b'\\' | 0x18 | 0x1a | 0x9c) || (bell_ends && byte == 0x07) {
                        Ground
                    } else if byte == 0x1b {
                        // Consecutive escape bytes stay payload until a final `\`.
                        StringEscape { bell_ends }
                    } else {
                        let mut utf8 = Utf8::default();
                        utf8.advance(byte);
                        String { bell_ends, utf8 }
                    };
                return;
            }
            _ => {}
        }
        if byte == 0x1b && self.state != Capability {
            self.flush(visit);
            self.pending.push(byte);
            self.state = Escape;
            return;
        }
        if self.state == CapabilityEscape {
            // The preceding ESC is still in `pending`; complete the capability
            // without growing past the deferred-query bound.
            match byte {
                b'\\' | 0x9c => {
                    let reply = capability_reply(&self.pending[4..self.pending.len() - 1]);
                    visit(Part::Reply(&reply));
                    self.pending.clear();
                    self.state = Ground;
                }
                _ => {
                    // The ESC was not a string terminator. Release the whole
                    // candidate and show the byte as ordinary output.
                    self.flush(visit);
                    self.pending.push(byte);
                    self.flush(visit);
                    self.state = Ground;
                }
            }
            return;
        }
        self.pending.push(byte);
        let mut reply = None;
        self.state = match self.state {
            Escape if byte == b'[' => Csi,
            Escape if byte == b'P' => Dcs,
            Escape if matches!(byte, b']' | b'X' | b'^' | b'_') => String {
                bell_ends: byte == b']',
                utf8: Utf8::default(),
            },
            Csi if (0x40..=0x7e).contains(&byte) => {
                reply = fixed_csi(&self.pending).map(Cow::Borrowed);
                Ground
            }
            Csi if (0x20..=0x3f).contains(&byte) && self.pending.len() < 32 => Csi,
            Dcs if self.pending == b"\x1bP+" => Dcs,
            Dcs if self.pending == b"\x1bP+q" => Capability,
            Dcs if byte == 0x9c => Ground,
            Dcs if !matches!(byte, 0x18 | 0x1a) => {
                let mut utf8 = Utf8::default();
                utf8.advance(byte);
                String {
                    bell_ends: false,
                    utf8,
                }
            }
            Capability if byte == 0x1b => CapabilityEscape,
            Capability if byte == 0x9c => {
                reply = Some(capability_reply(&self.pending[4..self.pending.len() - 1]));
                Ground
            }
            Capability if matches!(byte, 0x18 | 0x1a) => Ground,
            Capability if self.pending.len() < MAX_DEFERRED_QUERY_BYTES => Capability,
            Capability => {
                let mut utf8 = Utf8::default();
                utf8.advance(byte);
                String {
                    bell_ends: false,
                    utf8,
                }
            }
            _ => Ground,
        };
        if let Some(reply) = reply {
            visit(Part::Reply(&reply));
            self.pending.clear();
        } else if matches!(self.state, Ground | String { .. }) {
            self.flush(visit);
        }
    }

    fn flush(&mut self, visit: &mut impl FnMut(Part<'_>)) {
        if !self.pending.is_empty() {
            visit(Part::Display(&self.pending));
            self.pending.clear();
        }
    }
}

/// Returns the bytes of a control string up to its first terminator.
fn string_run(bytes: &[u8], bell_ends: bool, mut utf8: Utf8) -> (usize, Utf8) {
    let mut end = 0;
    while end < bytes.len() {
        let byte = bytes[end];
        if byte == 0x1b
            || matches!(byte, 0x18 | 0x1a)
            || (bell_ends && byte == 0x07)
            || (byte == 0x9c && utf8.remaining == 0)
        {
            break;
        }
        utf8.advance(byte);
        end += 1;
    }
    (end, utf8)
}

fn fixed_csi(bytes: &[u8]) -> Option<&'static [u8]> {
    match bytes {
        b"\x1b[c" | b"\x1b[?c" => Some(DA1_REPLY),
        b"\x1b[?6n" => Some(DEC_CPR_REPLY),
        _ => bytes
            .strip_prefix(b"\x1b[>")?
            .strip_suffix(b"c")?
            .iter()
            .all(|byte| byte.is_ascii_digit() || *byte == b';')
            .then_some(DA2_REPLY),
    }
}

#[cfg(test)]
mod tests;
