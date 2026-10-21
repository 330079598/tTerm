// @vitest-environment jsdom
import { Terminal, type IDecorationOptions } from "@xterm/xterm"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { KeywordHighlighter } from "@/components/TerminalTab/keywordHighlighter"
import type { CompiledHighlightRule } from "@/lib/keywordHighlight/match"

const RULES: CompiledHighlightRule[] = [{ regex: /error/gi, foreground: "#cc0000" }]

const write = (term: Terminal, data: string) =>
  new Promise<void>((resolve) => term.write(data, resolve))

/** The terminal, its highlighter, and the decorations alive at any moment. */
function setup(rows = 5) {
  const term = new Terminal({ cols: 20, rows, scrollback: 100, allowProposedApi: true })
  const live = new Set<IDecorationOptions>()
  const register = term.registerDecoration.bind(term)
  vi.spyOn(term, "registerDecoration").mockImplementation((options) => {
    const decoration = register(options)
    if (!decoration) return decoration
    live.add(options)
    decoration.onDispose(() => live.delete(options))
    return decoration
  })
  const highlighter = new KeywordHighlighter()
  term.loadAddon(highlighter)
  const decorated = () =>
    [...live].map((options) => ({
      line: options.marker.line,
      x: options.x,
      width: options.width,
    }))
  return { term, highlighter, decorated }
}

function frame() {
  vi.advanceTimersToNextFrame()
}

describe("KeywordHighlighter", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] })
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it("decorates matches once their output is parsed", async () => {
    const { term, highlighter, decorated } = setup()
    highlighter.setRules(RULES, false)
    await write(term, "ok\r\nan error here")
    frame()
    expect(decorated()).toEqual([{ line: 1, x: 3, width: 5 }])
  })

  it("keeps a line's decorations while its text stays the same", async () => {
    const { term, highlighter } = setup()
    highlighter.setRules(RULES, false)
    await write(term, "error\r\n")
    frame()
    const calls = vi.mocked(term.registerDecoration).mock.calls.length
    await write(term, "more\r\n")
    frame()
    expect(vi.mocked(term.registerDecoration).mock.calls.length).toBe(calls)
  })

  it("drops decorations of lines far above the screen", async () => {
    const { term, highlighter, decorated } = setup(3)
    highlighter.setRules(RULES, false)
    await write(term, "error\r\n")
    frame()
    expect(decorated()).toHaveLength(1)
    await write(term, "line\r\n".repeat(10))
    frame()
    expect(decorated()).toEqual([])
  })

  it("leaves full-screen programs alone unless asked", async () => {
    const { term, highlighter, decorated } = setup()
    highlighter.setRules(RULES, false)
    await write(term, "\x1b[?1049herror")
    frame()
    expect(decorated()).toEqual([])
    highlighter.setRules(RULES, true)
    frame()
    expect(decorated()).toHaveLength(1)
  })

  it("waits for a hidden terminal to show and clears when the rules go", async () => {
    const { term, highlighter, decorated } = setup()
    highlighter.setRules(RULES, false)
    highlighter.setVisible(false)
    await write(term, "error")
    frame()
    expect(decorated()).toEqual([])
    highlighter.setVisible(true)
    frame()
    expect(decorated()).toHaveLength(1)
    highlighter.setRules([], false)
    frame()
    expect(decorated()).toEqual([])
  })

  it("removes everything with the terminal", async () => {
    const { term, highlighter, decorated } = setup()
    highlighter.setRules(RULES, false)
    await write(term, "error")
    frame()
    term.dispose()
    expect(decorated()).toEqual([])
  })
})
