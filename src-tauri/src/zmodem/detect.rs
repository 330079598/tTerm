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

/// Longest trigger literal; the match window never needs to hold more than
/// this many bytes minus one across calls.
const MAX_TRIGGER_LEN: usize = TRIGGER_ZRQINIT.len();

/// Shortest trigger prefix worth holding back from the terminal. A tail of
/// just `*` or `**` is ordinary output (sudo-rs password feedback, globs)
/// far more often than a split header, and holding it would hide it until
/// the next output arrives. From `**` + ZDLE on it is almost surely ZMODEM.
const MIN_HELD_PREFIX_LEN: usize = 3;

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

/// Bytes carried between `scan` calls for one tab/channel.
#[derive(Debug, Default)]
pub struct DetectState {
    /// Tail that could still grow into a trigger.
    window: Vec<u8>,
    /// How many leading bytes of `window` were already forwarded (a short
    /// `*`/`**` tail kept only for matching).
    released: usize,
}

impl DetectState {
    #[cfg(test)]
    fn held_len(&self) -> usize {
        self.window.len() - self.released
    }
}

/// Feeds `chunk` into `state` and scans for a trigger. `state` must be reused
/// across calls for the same tab/channel and is left empty once a trigger
/// fires. A trigger may begin with bytes an earlier call already forwarded;
/// `remainder` still starts with the full trigger.
pub fn scan(state: &mut DetectState, chunk: &[u8]) -> ScanOutcome {
    state.window.extend_from_slice(chunk);
    let released = state.released;

    for (needle, direction) in TRIGGERS {
        if let Some(pos) = find(&state.window, needle) {
            let remainder = state.window.split_off(pos);
            let passthrough = state.window.get(released..).unwrap_or_default().to_vec();
            state.window.clear();
            state.released = 0;
            return ScanOutcome {
                passthrough,
                triggered: Some((direction, remainder)),
            };
        }
    }

    let keep = longest_possible_prefix_tail_len(&state.window);
    let held_start = if keep >= MIN_HELD_PREFIX_LEN {
        state.window.len() - keep
    } else {
        state.window.len()
    };
    let passthrough = state.window[released.min(held_start)..held_start].to_vec();
    let window_start = state.window.len() - keep;
    state.window.drain(..window_start);
    state.released = released.max(held_start).saturating_sub(window_start);
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
        let mut state = DetectState::default();
        let outcome = scan(&mut state, b"user@host:~$ ls -la\r\n");
        assert_eq!(outcome.passthrough, b"user@host:~$ ls -la\r\n");
        assert!(outcome.triggered.is_none());
        assert_eq!(state.held_len(), 0);
    }

    #[test]
    fn detects_zrqinit_trigger_in_one_chunk() {
        let mut state = DetectState::default();
        let mut chunk = b"rz\r".to_vec();
        chunk.extend_from_slice(TRIGGER_ZRQINIT);
        chunk.extend_from_slice(b"...rest of header...");
        let outcome = scan(&mut state, &chunk);
        assert_eq!(outcome.passthrough, b"rz\r");
        let (direction, remainder) = outcome.triggered.expect("trigger should fire");
        assert_eq!(direction, ZmodemDirection::Receive);
        assert!(remainder.starts_with(TRIGGER_ZRQINIT));
    }

    #[test]
    fn detects_zrinit_trigger() {
        let mut state = DetectState::default();
        let outcome = scan(&mut state, TRIGGER_ZRINIT);
        let (direction, _) = outcome.triggered.expect("trigger should fire");
        assert_eq!(direction, ZmodemDirection::Send);
    }

    #[test]
    fn trigger_split_across_every_possible_chunk_boundary_is_still_found() {
        let mut full = b"some prompt text ".to_vec();
        full.extend_from_slice(TRIGGER_ZRQINIT);
        full.extend_from_slice(b" trailing header bytes");

        for split in 0..full.len() {
            let mut state = DetectState::default();
            let (first, second) = full.split_at(split);
            let mut collected_passthrough = Vec::new();

            let outcome = scan(&mut state, first);
            collected_passthrough.extend_from_slice(&outcome.passthrough);
            let triggered = if let Some(hit) = outcome.triggered {
                Some(hit)
            } else {
                let outcome2 = scan(&mut state, second);
                collected_passthrough.extend_from_slice(&outcome2.passthrough);
                outcome2.triggered
            };

            let (direction, remainder) =
                triggered.unwrap_or_else(|| panic!("trigger not found for split at {split}"));
            assert_eq!(direction, ZmodemDirection::Receive);
            // A split right after `*` or `**` forwards those before the
            // trigger is known; nothing else of the trigger may leak.
            let leaked = collected_passthrough
                .strip_prefix(b"some prompt text ".as_slice())
                .unwrap_or_else(|| panic!("prompt text lost for split {split}"));
            assert!(
                leaked.len() < MIN_HELD_PREFIX_LEN && TRIGGER_ZRQINIT.starts_with(leaked),
                "split {split} leaked {leaked:?}"
            );
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

        let mut state = DetectState::default();
        let mut collected_passthrough = Vec::new();
        let mut triggered = None;
        for &byte in &full {
            let outcome = scan(&mut state, std::slice::from_ref(&byte));
            collected_passthrough.extend_from_slice(&outcome.passthrough);
            if outcome.triggered.is_some() {
                triggered = outcome.triggered;
                break;
            }
        }
        let (direction, remainder) = triggered.expect("trigger should eventually fire");
        assert_eq!(direction, ZmodemDirection::Send);
        assert_eq!(collected_passthrough, b"prefix-**");
        assert!(remainder.starts_with(TRIGGER_ZRINIT));
    }

    #[test]
    fn near_miss_prefix_is_eventually_released_as_passthrough() {
        // "**\x18B0" followed by something that is NOT "0" or "1" (a byte
        // that can't extend into either trigger) must not wedge forever.
        let mut state = DetectState::default();
        let outcome = scan(&mut state, b"**\x18B0");
        assert!(
            outcome.passthrough.is_empty(),
            "still a valid prefix, must be held back"
        );
        assert_eq!(state.held_len(), 5);

        let outcome = scan(&mut state, b"9");
        assert_eq!(outcome.passthrough, b"**\x18B09");
        assert!(outcome.triggered.is_none());
        assert_eq!(state.held_len(), 0);
    }

    #[test]
    fn password_feedback_stars_are_forwarded_immediately() {
        // sudo-rs echoes one `*` per key; each must reach the terminal now,
        // not when the next output happens to arrive.
        let mut state = DetectState::default();
        let mut shown = Vec::new();
        for echo in [
            b"[sudo: authenticate] Password: *".as_slice(),
            b"*",
            b"*",
            b"\x08 \x08",
        ] {
            let outcome = scan(&mut state, echo);
            assert!(outcome.triggered.is_none());
            assert_eq!(outcome.passthrough, echo, "held back part of {echo:?}");
            shown.extend_from_slice(&outcome.passthrough);
        }
        assert_eq!(shown, b"[sudo: authenticate] Password: ***\x08 \x08");
    }

    #[test]
    fn trigger_starting_with_already_forwarded_stars_is_found() {
        let mut state = DetectState::default();
        assert_eq!(scan(&mut state, b"x**").passthrough, b"x**");
        let outcome = scan(&mut state, &TRIGGER_ZRINIT[2..]);
        assert!(outcome.passthrough.is_empty());
        let (direction, remainder) = outcome.triggered.expect("trigger should fire");
        assert_eq!(direction, ZmodemDirection::Send);
        assert_eq!(remainder, TRIGGER_ZRINIT);
    }
}
