// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it } from "vitest"
import { Terminal } from "@xterm/xterm"

import {
  createScrollbackMemory,
  releaseClearedScrollback,
  scrollbackForSetting,
} from "@/components/TerminalTab/scrollbackMemory"
import { startingScrollbackLines, UNLIMITED_SCROLLBACK_INITIAL } from "@/lib/scrollback"

let term: Terminal | null = null

function setup(setting: number) {
  const container = document.createElement("div")
  document.body.appendChild(container)
  term = new Terminal({ cols: 20, rows: 5, scrollback: startingScrollbackLines(setting) })
  term.open(container)
  const memory = createScrollbackMemory(term, () => setting)
  const write = (data: string) =>
    new Promise<void>((resolve) => {
      const reserved = memory.reserve(data)
      term!.write(data, () => {
        memory.settle(reserved)
        resolve()
      })
    })
  return { term, memory, write }
}

/** Lines xterm's ring still references, live or not. */
function heldLines(t: Terminal): number {
  const lines = (t as unknown as { _core: { buffer: { lines: { _array: unknown[] } } } })._core
    .buffer.lines
  return lines._array.filter((line) => line !== undefined).length
}

function scrollRegion(t: Terminal, which: "normal" | "alt"): [number, number] {
  const buffer = (
    t as unknown as {
      _core: { buffers: Record<string, { scrollTop: number; scrollBottom: number }> }
    }
  )._core.buffers[which]
  return [buffer.scrollTop, buffer.scrollBottom]
}

const numbered = (from: number, count: number) =>
  Array.from({ length: count }, (_, i) => `line ${from + i}\r\n`).join("")

beforeAll(() => {
  window.matchMedia ||= () =>
    ({ matches: false, addListener() {}, removeListener() {} }) as unknown as MediaQueryList
  window.ResizeObserver ||= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
})

afterEach(() => {
  term?.dispose()
  term = null
  document.body.replaceChildren()
})

describe("createScrollbackMemory", () => {
  it("grows unlimited scrollback ahead of output so no line is trimmed", async () => {
    const { term, write } = setup(0)
    expect(term.options.scrollback).toBe(UNLIMITED_SCROLLBACK_INITIAL)

    // One chunk larger than the starting capacity.
    await write(numbered(0, UNLIMITED_SCROLLBACK_INITIAL * 3))

    expect(term.options.scrollback).toBeGreaterThan(UNLIMITED_SCROLLBACK_INITIAL * 3)
    expect(term.buffer.normal.getLine(0)?.translateToString(true)).toBe("line 0")
  })

  it("keeps a main screen scroll region while unlimited scrollback grows", async () => {
    const { term, write } = setup(0)
    // Like apt's progress bar: a region above a pinned bottom line.
    await write("\x1b[2;4r")
    expect(scrollRegion(term, "normal")).toEqual([1, 3])

    await write(numbered(0, UNLIMITED_SCROLLBACK_INITIAL))

    expect(term.options.scrollback).toBeGreaterThan(UNLIMITED_SCROLLBACK_INITIAL)
    expect(scrollRegion(term, "normal")).toEqual([1, 3])
  })

  it("keeps an alternate screen scroll region while unlimited scrollback grows", async () => {
    const { term, write } = setup(0)
    // Like vim scrolling inside its window.
    await write("\x1b[?1049h\x1b[1;3r")
    expect(scrollRegion(term, "alt")).toEqual([0, 2])

    await write(numbered(0, UNLIMITED_SCROLLBACK_INITIAL))

    expect(term.options.scrollback).toBeGreaterThan(UNLIMITED_SCROLLBACK_INITIAL)
    expect(scrollRegion(term, "alt")).toEqual([0, 2])
  })

  it("leaves a limited scrollback alone while writing", async () => {
    const { term, write } = setup(100)
    await write(numbered(0, 500))
    expect(term.options.scrollback).toBe(100)
  })

  it("lets go of the lines ED 3 erased", async () => {
    const { term, write } = setup(1000)
    await write(numbered(0, 800))
    expect(heldLines(term)).toBeGreaterThan(800)

    await write("\x1b[H\x1b[2J\x1b[3J")
    await new Promise((resolve) => setTimeout(resolve, 0))

    expect(heldLines(term)).toBeLessThanOrEqual(term.rows)
    expect(term.options.scrollback).toBe(1000)
  })

  it("keeps the scroll region set when ED 3 arrives", async () => {
    const { term, write } = setup(1000)
    await write(numbered(0, 50))
    await write("\x1b[2;4r\x1b[3J")
    await new Promise((resolve) => setTimeout(resolve, 0))

    expect(scrollRegion(term, "normal")).toEqual([1, 3])
  })

  it("shrinks grown unlimited scrollback back after ED 3", async () => {
    const { term, write } = setup(0)
    await write(numbered(0, UNLIMITED_SCROLLBACK_INITIAL * 2))
    expect(term.options.scrollback).toBeGreaterThan(UNLIMITED_SCROLLBACK_INITIAL)

    await write("\x1b[3J")
    await new Promise((resolve) => setTimeout(resolve, 0))

    expect(term.options.scrollback).toBe(UNLIMITED_SCROLLBACK_INITIAL)
  })
})

describe("releaseClearedScrollback", () => {
  it("lets go of the lines Terminal.clear dropped", async () => {
    const { term, write } = setup(1000)
    await write(numbered(0, 800))

    term.clear()
    releaseClearedScrollback(term, 1000)

    expect(heldLines(term)).toBeLessThanOrEqual(term.rows)
    expect(term.options.scrollback).toBe(1000)
  })
})

describe("scrollbackForSetting", () => {
  it("keeps the room unlimited scrollback has grown", () => {
    const { term } = setup(0)
    term.options.scrollback = 40_000
    expect(scrollbackForSetting(term, 0)).toBe(40_000)
    expect(scrollbackForSetting(term, 500)).toBe(500)
  })
})
