// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest"
import { Terminal } from "@xterm/xterm"

import {
  ACTIVATION_SETTLE_MS,
  installImeFocusRepair,
  REPAIR_DELAY_MS,
  REPAIR_QUIET_MS,
} from "@/components/TerminalTab/imeFocusRepair"

interface TestCore {
  coreService: { decPrivateModes: { sendFocus: boolean } }
}

let term: Terminal | null = null
let repair: { dispose: () => void } | null = null

function setup() {
  vi.useFakeTimers()
  const container = document.createElement("div")
  document.body.appendChild(container)
  term = new Terminal({ cols: 80, rows: 24 })
  term.open(container)
  const repairNativeFocus = vi.fn(() => Promise.resolve())
  repair = installImeFocusRepair(term, repairNativeFocus)
  const textarea = term.textarea!
  textarea.focus()
  return { term, textarea, repairNativeFocus }
}

const insertText = (textarea: HTMLTextAreaElement, data: string) =>
  textarea.dispatchEvent(new InputEvent("input", { inputType: "insertText", data }))

const activateWindow = () => window.dispatchEvent(new FocusEvent("focus"))

describe("installImeFocusRepair", () => {
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
    repair?.dispose()
    term?.dispose()
    repair = null
    term = null
    document.body.replaceChildren()
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  it("repairs once the window activation has settled", () => {
    const { repairNativeFocus } = setup()
    activateWindow()
    vi.advanceTimersByTime(ACTIVATION_SETTLE_MS - 1)
    expect(repairNativeFocus).not.toHaveBeenCalled()
    vi.advanceTimersByTime(1)
    expect(repairNativeFocus).toHaveBeenCalledTimes(1)
  })

  it("repairs when an ideograph arrives outside a composition", () => {
    const { textarea, repairNativeFocus } = setup()
    insertText(textarea, "等")
    vi.advanceTimersByTime(REPAIR_DELAY_MS)
    expect(repairNativeFocus).toHaveBeenCalledTimes(1)
  })

  it("ignores plain characters and composed input", () => {
    const { textarea, repairNativeFocus } = setup()
    insertText(textarea, "d")
    insertText(textarea, "，")
    textarea.dispatchEvent(new CompositionEvent("compositionstart"))
    insertText(textarea, "等")
    vi.advanceTimersByTime(REPAIR_DELAY_MS)
    expect(repairNativeFocus).not.toHaveBeenCalled()
  })

  it("leaves an unfocused terminal alone", () => {
    const { textarea, repairNativeFocus } = setup()
    textarea.blur()
    activateWindow()
    vi.advanceTimersByTime(ACTIVATION_SETTLE_MS)
    expect(repairNativeFocus).not.toHaveBeenCalled()
  })

  it("ignores the window focus its own repair causes", () => {
    const { textarea, repairNativeFocus } = setup()
    insertText(textarea, "等")
    vi.advanceTimersByTime(REPAIR_DELAY_MS)
    activateWindow()
    vi.advanceTimersByTime(ACTIVATION_SETTLE_MS)
    expect(repairNativeFocus).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(REPAIR_QUIET_MS)
    activateWindow()
    vi.advanceTimersByTime(ACTIVATION_SETTLE_MS)
    expect(repairNativeFocus).toHaveBeenCalledTimes(2)
  })

  it("holds focus reports back from the app during the repair", () => {
    const { term, textarea } = setup()
    const modes = (term as unknown as { _core: TestCore })._core.coreService.decPrivateModes
    modes.sendFocus = true
    const sent: string[] = []
    term.onData((data) => sent.push(data))
    insertText(textarea, "等")
    vi.advanceTimersByTime(REPAIR_DELAY_MS)
    sent.length = 0
    textarea.blur()
    textarea.focus()
    expect(sent).toEqual([])
    vi.advanceTimersByTime(REPAIR_QUIET_MS)
    expect(modes.sendFocus).toBe(true)
  })

  it("stops repairing once disposed", () => {
    const { textarea, repairNativeFocus } = setup()
    repair!.dispose()
    repair = null
    activateWindow()
    insertText(textarea, "等")
    vi.advanceTimersByTime(ACTIVATION_SETTLE_MS)
    expect(repairNativeFocus).not.toHaveBeenCalled()
  })
})
