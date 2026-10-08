// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest"
import { Terminal } from "@xterm/xterm"

import { installImeModifierInputFix } from "@/components/TerminalTab/imeModifierInput"

let term: Terminal | null = null
let fix: { dispose: () => void } | null = null

function setup({ install = true } = {}) {
  vi.useFakeTimers()
  const container = document.createElement("div")
  document.body.appendChild(container)
  term = new Terminal({ cols: 80, rows: 24 })
  term.open(container)
  if (install) fix = installImeModifierInputFix(term)
  const textarea = term.textarea!
  textarea.focus()
  const sent: string[] = []
  term.onData((data) => sent.push(data))
  return { textarea, sent }
}

function key(
  textarea: HTMLTextAreaElement,
  type: "keydown" | "keyup",
  init: KeyboardEventInit,
  keyCode: number
) {
  const event = new KeyboardEvent(type, { bubbles: true, cancelable: true, ...init })
  Object.defineProperty(event, "keyCode", { value: keyCode })
  textarea.dispatchEvent(event)
}

// Trusted input events are composed, which is what makes xterm consult _keyDownSeen.
function imeInsert(textarea: HTMLTextAreaElement, data: string) {
  textarea.value += data
  textarea.dispatchEvent(
    new InputEvent("input", { bubbles: true, composed: true, inputType: "insertText", data })
  )
}

const SHIFT = { key: "Shift", code: "ShiftLeft", shiftKey: true }
const BANG = { key: "!", code: "Digit1", shiftKey: true }

// The order WKWebView reports: the IME's input lands before the keystroke's keydown.
function typeWebKit(textarea: HTMLTextAreaElement, data: string) {
  imeInsert(textarea, data)
  key(textarea, "keydown", BANG, 229)
  vi.advanceTimersByTime(0)
  key(textarea, "keyup", BANG, 49)
}

describe("installImeModifierInputFix", () => {
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
    fix?.dispose()
    term?.dispose()
    fix = null
    term = null
    document.body.replaceChildren()
    vi.useRealTimers()
  })

  it("drops the first shifted IME punctuation without the fix", () => {
    const { textarea, sent } = setup({ install: false })
    key(textarea, "keydown", SHIFT, 16)
    typeWebKit(textarea, "！")
    expect(sent).toEqual([])
  })

  it("sends every shifted IME punctuation once, in WebKit's order", () => {
    const { textarea, sent } = setup()
    key(textarea, "keydown", SHIFT, 16)
    typeWebKit(textarea, "！")
    typeWebKit(textarea, "！")
    key(textarea, "keyup", { key: "Shift", code: "ShiftLeft" }, 16)
    expect(sent).toEqual(["！", "！"])
  })

  it("leaves Chromium's keydown-first order to xterm's own diff", () => {
    const { textarea, sent } = setup()
    key(textarea, "keydown", SHIFT, 16)
    key(textarea, "keydown", BANG, 229)
    imeInsert(textarea, "！")
    vi.advanceTimersByTime(0)
    key(textarea, "keyup", BANG, 49)
    expect(sent).toEqual(["！"])
  })

  it("stops once disposed", () => {
    const { textarea, sent } = setup()
    fix!.dispose()
    fix = null
    key(textarea, "keydown", SHIFT, 16)
    typeWebKit(textarea, "！")
    expect(sent).toEqual([])
  })
})
