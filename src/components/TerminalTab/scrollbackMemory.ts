import type { IDisposable, Terminal } from "@xterm/xterm"

import {
  countLineFeeds,
  grownScrollbackCapacity,
  startingScrollbackLines,
  isUnlimitedScrollback,
  UNLIMITED_SCROLLBACK_INITIAL,
} from "@/lib/scrollback"

interface ScrollRegion {
  scrollTop: number
  scrollBottom: number
}

/**
 * Changes the scrollback without disturbing running programs. xterm applies
 * it through a same-size resize of both buffers, which also resets their
 * scroll regions (DECSTBM) to the full screen; the size is unchanged, so the
 * regions programs like vim or apt's progress bar set stay valid and are put
 * back.
 */
function setScrollback(term: Terminal, lines: number): void {
  const buffers = (
    term as unknown as { _core?: { buffers?: { normal: ScrollRegion; alt: ScrollRegion } } }
  )._core?.buffers
  const regions = buffers
    ? [buffers.normal, buffers.alt].map((buffer) => ({
        buffer,
        top: buffer.scrollTop,
        bottom: buffer.scrollBottom,
      }))
    : []
  term.options.scrollback = lines
  for (const { buffer, top, bottom } of regions) {
    buffer.scrollTop = top
    buffer.scrollBottom = bottom
  }
}

/**
 * Scrollback for a changed setting. Unlimited keeps whatever room it has
 * already grown, so switching to it never trims history.
 */
export function scrollbackForSetting(term: Terminal, setting: number | undefined): number {
  if (!isUnlimitedScrollback(setting)) return startingScrollbackLines(setting)
  return Math.max(term.options.scrollback ?? 0, UNLIMITED_SCROLLBACK_INITIAL)
}

/**
 * Lets xterm collect lines dropped by a clear. Clearing the scrollback (ED 3
 * or `Terminal.clear`) only moves the start of xterm's line ring, so the
 * dropped lines stay referenced until the ring wraps, which for a large
 * scrollback is never. Changing the scrollback reallocates the ring with just
 * the live lines; unlimited scrollback also shrinks back to what it holds.
 */
export function releaseClearedScrollback(
  term: Terminal,
  setting: number | undefined,
  pendingLines = 0
): void {
  const capacity = term.options.scrollback ?? 0
  const target = isUnlimitedScrollback(setting)
    ? (grownScrollbackCapacity(
        UNLIMITED_SCROLLBACK_INITIAL,
        term.buffer.normal.length + pendingLines
      ) ?? UNLIMITED_SCROLLBACK_INITIAL)
    : capacity
  if (target !== capacity) {
    setScrollback(term, target)
    return
  }
  setScrollback(term, capacity + 1)
  setScrollback(term, capacity)
}

/**
 * Sizes xterm's line ring to what the terminal holds: unlimited scrollback
 * grows ahead of each output chunk so no line is trimmed before there is room
 * for it, and the ring is reallocated after the scrollback is cleared.
 */
export function createScrollbackMemory(term: Terminal, setting: () => number | undefined) {
  let pendingLines = 0
  let releaseTimer: ReturnType<typeof setTimeout> | null = null

  const release = () => {
    releaseTimer = null
    releaseClearedScrollback(term, setting(), pendingLines)
  }

  const clearHandler: IDisposable = term.parser.registerCsiHandler({ final: "J" }, (params) => {
    // ED 3 erases the scrollback; release once xterm's own handler has run.
    if (params[0] === 3 && releaseTimer === null) releaseTimer = setTimeout(release, 0)
    return false
  })

  return {
    /** Makes room for `text` before it is written; pass the result to `settle`. */
    reserve(text: string): number {
      if (!isUnlimitedScrollback(setting())) return 0
      const lines = countLineFeeds(text)
      pendingLines += lines
      const capacity = term.options.scrollback ?? 0
      const next = grownScrollbackCapacity(capacity, term.buffer.normal.length + pendingLines)
      if (next !== null) setScrollback(term, next)
      return lines
    },
    /** Marks lines reserved for a write as parsed. */
    settle(lines: number) {
      pendingLines -= lines
    },
    dispose() {
      clearHandler.dispose()
      if (releaseTimer !== null) clearTimeout(releaseTimer)
    },
  }
}
