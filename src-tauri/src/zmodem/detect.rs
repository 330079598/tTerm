//! Scans an ordinary terminal byte stream for the fixed literal sequences
//! that mark the start of a ZMODEM session (`sz` announces itself with
//! `ZRQINIT`, `rz` with `ZRINIT`), tolerating the trigger being split
//! across separate read/chunk boundaries — the same "hold back an
//! uncertain tail" idiom `terminal/io_batcher.rs`'s `utf8_complete_prefix_len`
//! already uses for partial UTF-8 sequences.

use crate::zmodem::protocol::consts::{ZmodemDirection, TRIGGER_ZRINIT, TRIGGER_ZRQINIT};

const TRIGGERS: [(&[u8], ZmodemDirection); 2] = [
    (TRIGGER_ZRQINIT, ZmodemDirection::Receive),
    (TRIGGER_ZRINIT, ZmodemDirection::Send),
];

/// Longest trigger literal; a carry buffer never needs to hold more than
/// this many bytes minus one across calls.
const MAX_TRIGGER_LEN: usize = TRIGGER_ZRQINIT.len();

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ScanOutcome {
    /// Bytes confirmed *not* to be (part of) a trigger; safe to forward to
    /// the terminal immediately.
    pub passthrough: Vec<u8>,
    /// Set once a full trigger is found: the direction it implies, plus the
    /// trigger bytes onward — exactly what a freshly constructed
    /// `ZmodemEngine` should receive as its first `feed()` call.
    pub triggered: Option<(ZmodemDirection, Vec<u8>)>,
}

/// Feeds `chunk` into `carry` (bytes withheld from the previous call) and
/// scans for a trigger. `carry` must be reused across calls for the same
/// tab/channel and is left empty once a trigger fires.
pub fn scan(carry: &mut Vec<u8>, chunk: &[u8]) -> ScanOutcome {
    carry.extend_from_slice(chunk);

    for (needle, direction) in TRIGGERS {
        if let Some(pos) = find(carry, needle) {
            let remainder = carry.split_off(pos);
            let passthrough = std::mem::take(carry);
            return ScanOutcome {
                passthrough,
                triggered: Some((direction, remainder)),
            };
        }
    }

    let keep = longest_possible_prefix_tail_len(carry);
    let release_len = carry.len() - keep;
    let passthrough = carry.drain(..release_len).collect();
    ScanOutcome {
        passthrough,
        triggered: None,
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Longest suffix of `buf` that is itself a prefix of either trigger
/// literal, i.e. could still grow into a full match given more bytes.
fn longest_possible_prefix_tail_len(buf: &[u8]) -> usize {
    let max_check = (MAX_TRIGGER_LEN - 1).min(buf.len());
    for len in (1..=max_check).rev() {
        let tail = &buf[buf.len() - len..];
        if TRIGGERS.iter().any(|(needle, _)| needle.starts_with(tail)) {
            return len;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_output_passes_through_untouched() {
        let mut carry = Vec::new();
        let outcome = scan(&mut carry, b"user@host:~$ ls -la\r\n");
        assert_eq!(outcome.passthrough, b"user@host:~$ ls -la\r\n");
        assert!(outcome.triggered.is_none());
        assert!(carry.is_empty());
    }

    #[test]
    fn detects_zrqinit_trigger_in_one_chunk() {
        let mut carry = Vec::new();
        let mut chunk = b"rz\r".to_vec();
        chunk.extend_from_slice(TRIGGER_ZRQINIT);
        chunk.extend_from_slice(b"...rest of header...");
        let outcome = scan(&mut carry, &chunk);
        assert_eq!(outcome.passthrough, b"rz\r");
        let (direction, remainder) = outcome.triggered.expect("trigger should fire");
        assert_eq!(direction, ZmodemDirection::Receive);
        assert!(remainder.starts_with(TRIGGER_ZRQINIT));
    }

    #[test]
    fn detects_zrinit_trigger() {
        let mut carry = Vec::new();
        let outcome = scan(&mut carry, TRIGGER_ZRINIT);
        let (direction, _) = outcome.triggered.expect("trigger should fire");
        assert_eq!(direction, ZmodemDirection::Send);
    }

    #[test]
    fn trigger_split_across_every_possible_chunk_boundary_is_still_found() {
        let mut full = b"some prompt text ".to_vec();
        full.extend_from_slice(TRIGGER_ZRQINIT);
        full.extend_from_slice(b" trailing header bytes");

        for split in 0..full.len() {
            let mut carry = Vec::new();
            let (first, second) = full.split_at(split);
            let mut collected_passthrough = Vec::new();

            let outcome = scan(&mut carry, first);
            collected_passthrough.extend_from_slice(&outcome.passthrough);
            let triggered = if let Some(hit) = outcome.triggered {
                Some(hit)
            } else {
                let outcome2 = scan(&mut carry, second);
                collected_passthrough.extend_from_slice(&outcome2.passthrough);
                outcome2.triggered
            };

            let (direction, remainder) =
                triggered.unwrap_or_else(|| panic!("trigger not found for split at {split}"));
            assert_eq!(direction, ZmodemDirection::Receive);
            assert_eq!(collected_passthrough, b"some prompt text ");
            assert!(
                remainder.starts_with(TRIGGER_ZRQINIT),
                "remainder should start with the full trigger for split {split}"
            );
        }
    }

    #[test]
    fn one_byte_at_a_time_still_finds_the_trigger() {
        let mut full = b"prefix-".to_vec();
        full.extend_from_slice(TRIGGER_ZRINIT);
        full.extend_from_slice(b"-suffix");

        let mut carry = Vec::new();
        let mut collected_passthrough = Vec::new();
        let mut triggered = None;
        for &byte in &full {
            let outcome = scan(&mut carry, std::slice::from_ref(&byte));
            collected_passthrough.extend_from_slice(&outcome.passthrough);
            if outcome.triggered.is_some() {
                triggered = outcome.triggered;
                break;
            }
        }
        let (direction, remainder) = triggered.expect("trigger should eventually fire");
        assert_eq!(direction, ZmodemDirection::Send);
        assert_eq!(collected_passthrough, b"prefix-");
        assert!(remainder.starts_with(TRIGGER_ZRINIT));
    }

    #[test]
    fn near_miss_prefix_is_eventually_released_as_passthrough() {
        // "**\x18B0" followed by something that is NOT "0" or "1" (a byte
        // that can't extend into either trigger) must not wedge forever.
        let mut carry = Vec::new();
        let outcome = scan(&mut carry, b"**\x18B0");
        assert!(
            outcome.passthrough.is_empty(),
            "still a valid prefix, must be held back"
        );
        assert!(carry.len() == 5);

        let outcome = scan(&mut carry, b"9");
        assert_eq!(outcome.passthrough, b"**\x18B09");
        assert!(outcome.triggered.is_none());
        assert!(carry.is_empty());
    }
}
