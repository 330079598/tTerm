// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it } from "vitest"
import { Terminal } from "@xterm/xterm"

import {
  installImeCursorGuard,
  TRANSIENT_CURSOR_HIDE_MS,
} from "@/components/TerminalTab/imeCursorGuard"

const CELL_WIDTH = 10
const CELL_HEIGHT = 20

const HIDE_CURSOR = "\x1b[?25l"
const SHOW_CURSOR = "\x1b[?25h"
const moveTo = (row: number, col: number) => `\x1b[${row + 1};${col + 1}H`

interface TestCore {
  _renderService: object
  _compositionHelper: { updateCompositionElements: (dontRecurse?: boolean) => void }
}

let term: Terminal | null = null
let guard: { dispose: () => void } | null = null
let clock = 0

function setup() {
  const container = document.createElement("div")
  document.body.appendChild(container)
  term = new Terminal({ cols: 80, rows: 24, allowProposedApi: true })
  term.open(container)
  const core = (term as unknown as { _core: TestCore })._core
  Object.defineProperty(core._renderService, "dimensions", {
    configurable: true,
    value: { css: { cell: { width: CELL_WIDTH, height: CELL_HEIGHT } } },
  })
  clock = 0
  guard = installImeCursorGuard(term, () => clock)
  return { term, core, textarea: term.textarea! }
}

const write = (target: Terminal, data: string) =>
  new Promise<void>((resolve) => target.write(data, resolve))

const flushMicrotasks = () => new Promise<void>((resolve) => setTimeout(resolve, 0))

const cellOf = (textarea: HTMLTextAreaElement) => ({
  col: parseFloat(textarea.style.left) / CELL_WIDTH,
  row: parseFloat(textarea.style.top) / CELL_HEIGHT,
})

const startComposition = (textarea: HTMLTextAreaElement, data = "ni") => {
  textarea.dispatchEvent(new CompositionEvent("compositionstart"))
  textarea.dispatchEvent(new CompositionEvent("compositionupdate", { data }))
}

describe("installImeCursorGuard", () => {
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
    guard?.dispose()
    term?.dispose()
    guard = null
    term = null
    document.body.replaceChildren()
  })

  it("follows a visible cursor", async () => {
    const { term, textarea } = setup()
    await write(term, moveTo(5, 7))
    expect(cellOf(textarea)).toEqual({ col: 7, row: 5 })
  })

  it("stops following the cursor while it is hidden", async () => {
    const { term, textarea } = setup()
    await write(term, moveTo(5, 7))
    await write(term, HIDE_CURSOR + moveTo(20, 0) + "spinner")
    expect(cellOf(textarea)).toEqual({ col: 7, row: 5 })
  })

  it("re-syncs when the cursor is shown without moving", async () => {
    const { term, textarea } = setup()
    await write(term, moveTo(5, 7))
    await write(term, HIDE_CURSOR + moveTo(10, 2))
    await write(term, SHOW_CURSOR)
    await flushMicrotasks()
    expect(cellOf(textarea)).toEqual({ col: 2, row: 10 })
  })

  it("keeps a composition anchored while output moves the cursor", async () => {
    const { term, core, textarea } = setup()
    await write(term, moveTo(5, 7))
    startComposition(textarea)
    await write(term, moveTo(20, 0) + "streaming output")
    core._compositionHelper.updateCompositionElements()
    expect(cellOf(textarea)).toEqual({ col: 7, row: 5 })
  })

  it("anchors to the last visible cursor during a brief hide", async () => {
    const { term, textarea } = setup()
    await write(term, moveTo(5, 7))
    clock = 1000
    await write(term, HIDE_CURSOR + moveTo(20, 0))
    clock += TRANSIENT_CURSOR_HIDE_MS - 1
    startComposition(textarea)
    expect(cellOf(textarea)).toEqual({ col: 7, row: 5 })
  })

  it("anchors to the current cursor after a long hide", async () => {
    const { term, textarea } = setup()
    await write(term, moveTo(5, 7))
    clock = 1000
    await write(term, HIDE_CURSOR + moveTo(20, 3))
    clock += TRANSIENT_CURSOR_HIDE_MS
    startComposition(textarea)
    expect(cellOf(textarea)).toEqual({ col: 3, row: 20 })
  })

  it("restores xterm's default behaviour when disposed", async () => {
    const { term, textarea } = setup()
    guard!.dispose()
    guard = null
    await write(term, HIDE_CURSOR + moveTo(12, 4))
    expect(cellOf(textarea)).toEqual({ col: 4, row: 12 })
  })
})
