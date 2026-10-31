import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import type { Terminal } from "@xterm/xterm"

import {
  createCursorMoveHold,
  HOLD_MS,
  INPUT_GRACE_MS,
  isLineRedrawLead,
  type CursorMoveHold,
} from "@/components/TerminalTab/cursorMoveHold"

// One spinner frame through the bundled ConPTY, which passes Node's cursorTo,
// clearLine and write through as three chunks.
const TO_COLUMN_ZERO = "\x1b[1G"
const ERASE_LINE = "\x1b[0K"
const SPINNER_FRAME = "⠹ Setting up project"
// The same return through an older ConPTY, flushed as a frame of its own.
const CONPTY_RETURN_FRAME = "\x1b[?25l\x1b[H\x1b[?25h"
// A TUI drawing an animation and then putting the cursor back in its input box.
const ANIMATION_FRAME = "\x1b[3;3H⠹ Working"
const BACK_TO_INPUT = "\x1b[30;12H"

let clock = 0
let writes: string[] = []
let typeKey: () => void = () => {}
let hold: CursorMoveHold

function fakeTerminal(): Terminal {
  return {
    write: (text: string, onParsed?: () => void) => {
      writes.push(text)
      onParsed?.()
    },
    onData: (listener: () => void) => {
      typeKey = listener
      return { dispose: () => {} }
    },
  } as unknown as Terminal
}

beforeEach(() => {
  vi.useFakeTimers()
  clock = 1000
  writes = []
  hold = createCursorMoveHold(fakeTerminal(), () => clock)
})

afterEach(() => {
  hold.dispose()
  vi.useRealTimers()
})

describe("isLineRedrawLead", () => {
  it("matches a return to a line's start, an erase or a cursor toggle", () => {
    expect(isLineRedrawLead(TO_COLUMN_ZERO)).toBe(true)
    expect(isLineRedrawLead(ERASE_LINE)).toBe(true)
    expect(isLineRedrawLead(CONPTY_RETURN_FRAME)).toBe(true)
    expect(isLineRedrawLead("\x1b[2K\x1b[1A\x1b[2K\x1b[G")).toBe(true)
    expect(isLineRedrawLead("\x1b[12;1H\r")).toBe(true)
    expect(isLineRedrawLead("\x1b[3F")).toBe(true)
    expect(isLineRedrawLead("\r")).toBe(true)
    expect(isLineRedrawLead("\x1b[?25l")).toBe(true)
  })

  it("does not match a move that ends in another column", () => {
    expect(isLineRedrawLead(BACK_TO_INPUT)).toBe(false)
    expect(isLineRedrawLead("\x1b[?25l\x1b[30;12H\x1b[?25h")).toBe(false)
    expect(isLineRedrawLead("\x1b[5G")).toBe(false)
    expect(isLineRedrawLead("\x1b[1G\x1b[3C")).toBe(false)
    expect(isLineRedrawLead("\r\b")).toBe(false)
    expect(isLineRedrawLead("\x1b[2A")).toBe(false)
  })

  it("does not match a chunk that draws, scrolls or styles", () => {
    expect(isLineRedrawLead("")).toBe(false)
    expect(isLineRedrawLead(`${ERASE_LINE}${SPINNER_FRAME}`)).toBe(false)
    expect(isLineRedrawLead("\x1b[1Gx")).toBe(false)
    expect(isLineRedrawLead("\r\n")).toBe(false)
    expect(isLineRedrawLead("\x1b[0m")).toBe(false)
    expect(isLineRedrawLead("\x1b[?1049h")).toBe(false)
    expect(isLineRedrawLead("\x1b]0;title\x07")).toBe(false)
  })
})

describe("createCursorMoveHold", () => {
  it("writes the return and erase together with the frame that follows them", () => {
    const parsed: string[] = []
    hold.write(TO_COLUMN_ZERO, () => parsed.push("return"))
    hold.write(ERASE_LINE, () => parsed.push("erase"))
    expect(writes).toEqual([])

    hold.write(SPINNER_FRAME, () => parsed.push("frame"))
    expect(writes).toEqual([TO_COLUMN_ZERO + ERASE_LINE + SPINNER_FRAME])
    expect(parsed).toEqual(["return", "erase", "frame"])
  })

  it("holds an older ConPTY's return frame the same way", () => {
    hold.write(CONPTY_RETURN_FRAME)
    hold.write(SPINNER_FRAME)
    expect(writes).toEqual([CONPTY_RETURN_FRAME + SPINNER_FRAME])
  })

  it("writes a move back to an input box at once", () => {
    hold.write(ANIMATION_FRAME)
    hold.write(BACK_TO_INPUT)
    expect(writes).toEqual([ANIMATION_FRAME, BACK_TO_INPUT])
  })

  it("writes what it holds once HOLD_MS passes from the first held chunk", () => {
    hold.write(TO_COLUMN_ZERO)
    vi.advanceTimersByTime(HOLD_MS - 1)
    hold.write(ERASE_LINE)
    expect(writes).toEqual([])
    vi.advanceTimersByTime(1)
    expect(writes).toEqual([TO_COLUMN_ZERO + ERASE_LINE])
  })

  it("writes other output at once", () => {
    hold.write("hello\r\n")
    expect(writes).toEqual(["hello\r\n"])
  })

  it("does not hold the answer to a keystroke", () => {
    typeKey()
    clock += INPUT_GRACE_MS - 1
    hold.write(TO_COLUMN_ZERO)
    expect(writes).toEqual([TO_COLUMN_ZERO])

    clock += 1
    hold.write(TO_COLUMN_ZERO)
    expect(writes).toEqual([TO_COLUMN_ZERO])
  })

  it("keeps a held chunk ahead of a keystroke's answer", () => {
    hold.write(TO_COLUMN_ZERO)
    typeKey()
    hold.write(ERASE_LINE)
    expect(writes).toEqual([TO_COLUMN_ZERO + ERASE_LINE])
  })

  it("writes what it holds when flushed and not again after", () => {
    hold.write(TO_COLUMN_ZERO)
    hold.flush()
    expect(writes).toEqual([TO_COLUMN_ZERO])
    vi.advanceTimersByTime(HOLD_MS)
    expect(writes).toEqual([TO_COLUMN_ZERO])
  })

  it("drops what it holds once disposed", () => {
    hold.write(TO_COLUMN_ZERO)
    hold.dispose()
    vi.advanceTimersByTime(HOLD_MS)
    expect(writes).toEqual([])
  })
})
