import type { IDisposable, Terminal } from "@xterm/xterm"

/**
 * Keeps the cursor from flashing at the start of a line a program is redrawing.
 *
 * Spinners such as npm's redraw their line in several writes: back to column
 * 0, erase the line, draw the frame. The writes can reach the terminal as
 * separate chunks. The backend sends the first bytes after a quiet spell at
 * once, so an echo is not delayed, and an older ConPTY on a Windows host
 * flushes the return as a frame of its own. Each chunk is rendered, so the
 * cursor shows at column 0 between the frames. With a block cursor it covers
 * the spinner a dozen times a second, and each move restarts the blink.
 *
 * A chunk that leads such a redraw is held back until a chunk that draws
 * arrives and is written together with it, so xterm renders only where the
 * cursor ends up. The bytes and their order are unchanged; only the render is
 * delayed, by at most HOLD_MS. A move that ends elsewhere is not held: a TUI
 * that draws an animation and then puts the cursor back in its input box
 * sends that move last, and holding it would leave the cursor on the
 * animation. Shortly after a keystroke nothing is held, so the arrow keys and
 * line editing stay immediate.
 */

/** The longest a held chunk waits for the drawing after it. */
export const HOLD_MS = 50

/** After a keystroke, output is the answer to it and goes out at once. */
export const INPUT_GRACE_MS = 150

const ESC = "\x1b"
// What follows ESC in DECTCEM or in a CSI sequence with numeric parameters,
// and any carriage returns after it.
const SEQUENCE_AFTER_ESC = /^\[(?:\?25[hl]|([0-9;]*)([A-Za-z]))(\r*)$/

type Effect = "line start" | "vertical" | "none" | "other"

/** What a CSI sequence does to the cursor, if it draws nothing. */
function effectOf(params: string, final: string): Effect {
  const [first = "", second = ""] = params.split(";")
  switch (final) {
    case "E":
    case "F":
      return "line start"
    case "G":
      return Number(first || 1) <= 1 ? "line start" : "other"
    case "H":
    case "f":
      return Number(second || 1) <= 1 ? "line start" : "other"
    case "A":
    case "B":
    case "d":
      return "vertical"
    case "J":
    case "K":
      return "none"
    default:
      return "other"
  }
}

/**
 * Whether a whole chunk draws nothing and either leaves the cursor at the
 * start of a line, as a program does before it redraws that line, or does
 * not move it. It may also erase and show or hide the cursor. Moving only up
 * or down keeps the column, which may be an input box's, so it does not count.
 */
export function isLineRedrawLead(text: string): boolean {
  if (text.length === 0) return false
  const [lead, ...sequences] = text.split(ESC)
  if (!/^\r*$/.test(lead)) return false
  const effects: Effect[] = lead ? ["line start"] : []
  for (const part of sequences) {
    const match = SEQUENCE_AFTER_ESC.exec(part)
    if (!match) return false
    const [, params, final, returns] = match
    effects.push(final === undefined ? "none" : effectOf(params, final))
    if (returns) effects.push("line start")
  }
  if (effects.includes("other")) return false
  return !effects.includes("vertical") || effects.includes("line start")
}

interface HeldChunks {
  text: string
  callbacks: (() => void)[]
}

export interface CursorMoveHold extends IDisposable {
  /** Writes a chunk of session output, holding it if it leads a line redraw. */
  write: (text: string, onParsed?: () => void) => void
  /** Writes what is held now, before the caller writes to the terminal directly. */
  flush: () => void
}

export function createCursorMoveHold(
  term: Terminal,
  now: () => number = () => performance.now()
): CursorMoveHold {
  let held: HeldChunks | null = null
  let timer: ReturnType<typeof setTimeout> | null = null
  let lastInputAt = -Infinity

  const inputListener = term.onData(() => {
    lastInputAt = now()
  })

  const takeHeld = () => {
    if (timer !== null) clearTimeout(timer)
    timer = null
    const chunks = held
    held = null
    return chunks
  }

  const writeAll = (text: string, callbacks: (() => void)[]) => {
    term.write(text, () => callbacks.forEach((callback) => callback()))
  }

  const flush = () => {
    const chunks = takeHeld()
    if (chunks) writeAll(chunks.text, chunks.callbacks)
  }

  const write = (text: string, onParsed?: () => void) => {
    const callbacks = onParsed ? [onParsed] : []
    if (isLineRedrawLead(text) && now() - lastInputAt >= INPUT_GRACE_MS) {
      if (held) {
        held.text += text
        held.callbacks.push(...callbacks)
      } else {
        held = { text, callbacks }
        // Not restarted by later chunks, so nothing waits longer than HOLD_MS.
        timer = setTimeout(flush, HOLD_MS)
      }
      return
    }
    const previous = takeHeld()
    if (!previous) {
      term.write(text, onParsed)
      return
    }
    // One write, so xterm sees only where the cursor ends up. When that is
    // where it started, as with a spinner, the blink is not restarted either.
    writeAll(previous.text + text, [...previous.callbacks, ...callbacks])
  }

  return {
    write,
    flush,
    dispose: () => {
      takeHeld()
      inputListener.dispose()
    },
  }
}
