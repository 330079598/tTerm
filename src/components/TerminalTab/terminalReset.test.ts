// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest"
import { Terminal } from "@xterm/xterm"

import { resetTerminalState } from "@/components/TerminalTab/terminalReset"

describe("resetTerminalState", () => {
  let term: Terminal | null = null

  afterEach(() => {
    term?.dispose()
    term = null
  })

  const create = () => {
    term = new Terminal({ cols: 40, rows: 5, scrollback: 100, allowProposedApi: true })
    return term
  }
  const write = (target: Terminal, data: string) =>
    new Promise<void>((resolve) => target.write(data, resolve))
  /** Resolves once everything queued so far, the reset included, is parsed. */
  const flush = (target: Terminal) => write(target, "")
  const line = (target: Terminal, y: number) =>
    target.buffer.active.getLine(y)?.translateToString(true) ?? ""

  it("switches back from the line-drawing charset", async () => {
    const target = create()
    await write(target, "\x1b(0lqk\r\n")
    expect(line(target, 0)).toBe("┌─┐")

    resetTerminalState(target)
    await write(target, "lqk")
    expect(line(target, 1)).toBe("lqk")
  })

  it("undoes a shift out to G1", async () => {
    const target = create()
    await write(target, "\x1b)0\x0eabc\r\n")
    resetTerminalState(target)
    await write(target, "abc")
    expect(line(target, 1)).toBe("abc")
  })

  it("keeps the screen and scrollback", async () => {
    const target = create()
    for (let index = 1; index <= 20; index++) {
      await write(target, `line ${index}\r\n`)
    }
    resetTerminalState(target)
    await flush(target)

    expect(target.buffer.active.length).toBe(21)
    expect(line(target, 0)).toBe("line 1")
    expect(line(target, 19)).toBe("line 20")
  })

  it("turns off mouse reporting and colors", async () => {
    const target = create()
    await write(target, "\x1b[?1003h\x1b[?1006h\x1b[31;1m")
    expect(target.modes.mouseTrackingMode).toBe("any")

    resetTerminalState(target)
    await write(target, "x")
    expect(target.modes.mouseTrackingMode).toBe("none")
    const cell = target.buffer.active.getLine(0)?.getCell(0)
    expect(cell?.isFgDefault()).toBe(true)
    expect(cell?.isBold()).toBe(0)
  })

  it("leaves the alternate screen without moving the normal screen's cursor", async () => {
    const target = create()
    await write(target, "shell output\r\n$ ")
    await write(target, "\x1b[?1049h")
    expect(target.buffer.active.type).toBe("alternate")

    resetTerminalState(target)
    await flush(target)
    expect(target.buffer.active.type).toBe("normal")
    expect(line(target, 0)).toBe("shell output")
    expect(target.buffer.active.cursorY).toBe(1)
  })

  it("does not move the cursor on the normal screen", async () => {
    const target = create()
    await write(target, "\x1b7one\r\ntwo\r\nthree")
    resetTerminalState(target)
    await flush(target)
    expect(target.buffer.active.cursorY).toBe(2)
    expect(target.buffer.active.cursorX).toBe(5)
  })
})
