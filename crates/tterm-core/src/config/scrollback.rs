//! Scrollback limits, from `src/lib/scrollback.ts`.

use tterm_terminal::theme::js::js_round;

/// Default scrollback when config is missing or invalid.
pub const DEFAULT_SCROLLBACK_LINES: u32 = 10_000;

/// Stored config value `0` means unlimited.
///
/// xterm.js has no true unlimited mode, so the value maps to a very large buffer.
pub const UNLIMITED_SCROLLBACK_SENTINEL: u32 = 0;

/// xterm.js has no unbounded mode, so unlimited history uses this deliberate
/// cap instead: high enough to behave like unlimited history in normal use,
/// while a corrupt config cannot allocate without limit.
pub const UNLIMITED_SCROLLBACK_BUFFER: u32 = 10_000_000;

/// The largest explicit (non-unlimited) scrollback the settings UI accepts.
pub const MAX_EXPLICIT_SCROLLBACK_LINES: u32 = 10_000_000;

/// Room unlimited scrollback starts with.
///
/// xterm allocates its line ring at the full scrollback size, so starting at
/// [`UNLIMITED_SCROLLBACK_BUFFER`] would cost about 80 MB per terminal before
/// any output, and again on every resize that changes the row count.
pub const UNLIMITED_SCROLLBACK_INITIAL: u32 = 10_000;

/// A value for storage or the settings form; `0` is the unlimited sentinel.
pub fn normalize_scrollback_config(value: Option<f64>) -> u32 {
    let Some(value) = value.filter(|value| value.is_finite()) else {
        return DEFAULT_SCROLLBACK_LINES;
    };

    let rounded = js_round(value);
    if rounded == UNLIMITED_SCROLLBACK_SENTINEL as f64 {
        return UNLIMITED_SCROLLBACK_SENTINEL;
    }

    rounded.clamp(1.0, MAX_EXPLICIT_SCROLLBACK_LINES as f64) as u32
}

/// The scrollback buffer size handed to xterm; unlimited maps to
/// [`UNLIMITED_SCROLLBACK_BUFFER`].
pub fn resolve_scrollback_lines(value: Option<f64>) -> u32 {
    let normalized = normalize_scrollback_config(value);
    if normalized == UNLIMITED_SCROLLBACK_SENTINEL {
        return UNLIMITED_SCROLLBACK_BUFFER;
    }
    normalized
}

/// Whether the setting asks for unlimited scrollback.
pub fn is_unlimited_scrollback(value: Option<f64>) -> bool {
    normalize_scrollback_config(value) == UNLIMITED_SCROLLBACK_SENTINEL
}

/// The scrollback to hand xterm for a setting; unlimited starts small and
/// grows through [`grown_scrollback_capacity`].
pub fn starting_scrollback_lines(value: Option<f64>) -> u32 {
    if is_unlimited_scrollback(value) {
        UNLIMITED_SCROLLBACK_INITIAL
    } else {
        resolve_scrollback_lines(value)
    }
}

/// Line feeds xterm executes for `text` (LF, VT and FF all advance a line).
pub fn count_line_feeds(text: &str) -> usize {
    text.bytes()
        .filter(|byte| matches!(byte, 0x0a | 0x0b | 0x0c))
        .count()
}

/// The capacity unlimited scrollback grows to so `lines` fit with as many
/// again to spare, or `None` while the current capacity still leaves that room.
pub fn grown_scrollback_capacity(capacity: u32, lines: u32) -> Option<u32> {
    let wanted = u64::from(lines) * 2;
    if wanted <= u64::from(capacity) {
        return None;
    }

    let mut next = u64::from(capacity.max(UNLIMITED_SCROLLBACK_INITIAL));
    while next < wanted {
        next *= 2;
    }
    Some(next.min(u64::from(UNLIMITED_SCROLLBACK_BUFFER)) as u32)
}

#[cfg(test)]
mod tests;
