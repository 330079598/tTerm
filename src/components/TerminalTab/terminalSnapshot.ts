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

/**
 * The text that shows `content` again above a new session, closed by a dim
 * line with `banner`; nothing when the terminal had shown nothing.
 */
export function restoredContentText(content: string, banner: string): string {
  if (!content.trim()) return ""
  return `${content}\x1b[0m\r\n\x1b[2m── ${banner} ──\x1b[0m\r\n`
}
