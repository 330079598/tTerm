import type { Terminal } from "@xterm/xterm"

/** DECSTR: charsets, text attributes, modes, scroll region, cursor visibility. */
const SOFT_RESET = "\x1b[!p"

/** DECSTR leaves mouse reporting alone; turn off every tracking mode and encoding. */
const MOUSE_REPORTING_OFF = [9, 1000, 1001, 1002, 1003, 1005, 1006, 1015, 1016]
  .map((mode) => `\x1b[?${mode}l`)
  .join("")

const LEAVE_ALTERNATE_SCREEN = "\x1b[?1049l"

/**
 * Recovers a terminal left garbled by stray control sequences (`cat` on a
 * binary file switching to line-drawing glyphs, enabling mouse reporting or
 * the alternate screen) while keeping the screen and scrollback intact, so
 * unlike `term.reset()` (RIS) no history is lost.
 *
 * Goes through the write queue so it lands after any output still pending.
 */
export function resetTerminalState(term: Terminal) {
  // Leaving the normal screen "again" would restore the cursor saved by
  // DECSC, so only leave the alternate one.
  const leaveAlternate = term.buffer.active.type === "alternate" ? LEAVE_ALTERNATE_SCREEN : ""
  term.write(`${leaveAlternate}${SOFT_RESET}${MOUSE_REPORTING_OFF}`)
}
