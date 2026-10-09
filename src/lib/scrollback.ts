/** Default scrollback when config is missing or invalid. */
export const DEFAULT_SCROLLBACK_LINES = 10_000

/**
 * Stored config value `0` means unlimited.
 * xterm.js has no true unlimited mode, so we map to a very large buffer.
 */
export const UNLIMITED_SCROLLBACK_SENTINEL = 0

/**
 * xterm.js has no unbounded mode. This is deliberately high enough to behave
 * like unlimited history in normal use while preventing a corrupt config from
 * allocating without limit.
 */
export const UNLIMITED_SCROLLBACK_BUFFER = 10_000_000

/** Max explicit (non-unlimited) scrollback the settings UI accepts. */
export const MAX_EXPLICIT_SCROLLBACK_LINES = 10_000_000

/**
 * Normalize a value for storage / settings form state.
 * `0` is preserved as the unlimited sentinel.
 */
export function normalizeScrollbackConfig(value: number | undefined): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return DEFAULT_SCROLLBACK_LINES
  }

  const rounded = Math.round(value)
  if (rounded === UNLIMITED_SCROLLBACK_SENTINEL) {
    return UNLIMITED_SCROLLBACK_SENTINEL
  }

  return Math.min(Math.max(rounded, 1), MAX_EXPLICIT_SCROLLBACK_LINES)
}

/**
 * Resolve the scrollback buffer size passed to xterm.
 * Unlimited (`0`) maps to {@link UNLIMITED_SCROLLBACK_BUFFER}.
 */
export function resolveScrollbackLines(value: number | undefined): number {
  const normalized = normalizeScrollbackConfig(value)
  if (normalized === UNLIMITED_SCROLLBACK_SENTINEL) {
    return UNLIMITED_SCROLLBACK_BUFFER
  }
  return normalized
}

export function isUnlimitedScrollback(value: number | undefined): boolean {
  return normalizeScrollbackConfig(value) === UNLIMITED_SCROLLBACK_SENTINEL
}

/**
 * Room unlimited scrollback starts with. xterm allocates its line ring at the
 * full scrollback size, so starting at {@link UNLIMITED_SCROLLBACK_BUFFER}
 * would cost ~80 MB per terminal before any output, and again on every resize
 * that changes the row count.
 */
export const UNLIMITED_SCROLLBACK_INITIAL = 10_000

/** Scrollback to hand xterm for a setting; unlimited starts small and grows. */
export function startingScrollbackLines(value: number | undefined): number {
  return isUnlimitedScrollback(value) ? UNLIMITED_SCROLLBACK_INITIAL : resolveScrollbackLines(value)
}

/** Line feeds xterm executes for `text` (LF, VT and FF all advance a line). */
export function countLineFeeds(text: string): number {
  let count = 0
  for (let i = 0; i < text.length; i++) {
    const code = text.charCodeAt(i)
    if (code === 0x0a || code === 0x0b || code === 0x0c) count++
  }
  return count
}

/**
 * Capacity unlimited scrollback grows to so `lines` fit with as many again to
 * spare, or `null` while the current capacity still leaves that room.
 */
export function grownScrollbackCapacity(capacity: number, lines: number): number | null {
  if (lines * 2 <= capacity) return null
  let next = Math.max(capacity, UNLIMITED_SCROLLBACK_INITIAL)
  while (next < lines * 2) next *= 2
  return Math.min(next, UNLIMITED_SCROLLBACK_BUFFER)
}
