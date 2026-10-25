import type { SerializeAddon } from "@xterm/addon-serialize"
import type { Terminal } from "@xterm/xterm"

/** The most lines of a terminal kept for the next launch. */
export const SNAPSHOT_MAX_LINES = 5000

/**
 * What the terminal showed, as escape sequences that draw it again: the main
 * screen and its scrollback down to the cursor's line, with their colors. A
 * full-screen program's screen is left out, and so are the terminal's modes,
 * which would otherwise carry mouse reporting or bracketed paste into the
 * next session. What a program drew below the cursor, such as a status bar,
 * is left out too, so that it never shows under the next session's prompt.
 */
export function serializeTerminal(term: Terminal, serializer: SerializeAddon): string {
  const buffer = term.buffer.normal
  const end = buffer.baseY + buffer.cursorY
  const start = Math.max(0, end - SNAPSHOT_MAX_LINES + 1)
  return serializer.serialize({ range: { start, end }, excludeModes: true, excludeAltBuffer: true })
}

/** `savedAt` as month, day and 24-hour time, such as `10-25 14:03`. */
export function formatSavedAt(savedAt: number): string {
  const date = new Date(savedAt)
  const pad = (value: number) => String(value).padStart(2, "0")
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}

/** Cells `char` takes: two for East Asian wide and fullwidth characters. */
function charCells(char: string): number {
  const code = char.codePointAt(0) ?? 0
  const wide =
    (code >= 0x1100 && code <= 0x115f) ||
    (code >= 0x2e80 && code <= 0xa4cf) ||
    (code >= 0xac00 && code <= 0xd7a3) ||
    (code >= 0xf900 && code <= 0xfaff) ||
    (code >= 0xfe30 && code <= 0xfe4f) ||
    (code >= 0xff00 && code <= 0xff60) ||
    (code >= 0xffe0 && code <= 0xffe6) ||
    (code >= 0x20000 && code <= 0x3fffd)
  return wide ? 2 : 1
}

function textCells(text: string): number {
  let cells = 0
  for (const char of text) cells += charCells(char)
  return cells
}

/** `banner` in the middle of a rule as wide as `cols`. */
export function bannerLine(banner: string, cols: number): string {
  const label = ` ${banner} `
  const fill = Math.max(4, cols - textCells(label))
  const left = Math.floor(fill / 2)
  return `${"─".repeat(left)}${label}${"─".repeat(fill - left)}`
}

/**
 * The text that shows `content` again above a new session, closed by a dim
 * rule across the `cols` wide terminal with `banner` in its middle; nothing
 * when the terminal had shown nothing.
 */
export function restoredContentText(content: string, banner: string, cols: number): string {
  if (!content.trim()) return ""
  return `${content}\x1b[0m\r\n\x1b[2m${bannerLine(banner, cols)}\x1b[0m\r\n`
}
