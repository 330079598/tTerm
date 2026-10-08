// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest"
import { Terminal } from "@xterm/xterm"

import { installImeEarlyInputFix } from "@/components/TerminalTab/imeEarlyInput"

let term: Terminal | null = null
let fix: { dispose: () => void } | null = null

function setup({ install = true } = {}) {
  vi.useFakeTimers()
  const container = document.createElement("div")
  document.body.appendChild(container)
  term = new Terminal({ cols: 80, rows: 24 })
  term.open(container)
  if (install) fix = installImeEarlyInputFix(term)
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
const COMMA = { key: ",", code: "Comma" }
const PERIOD = { key: ".", code: "Period" }

// The order WKWebView reports: the IME's input lands before the keystroke's keydown.
function pressWebKit(textarea: HTMLTextAreaElement, data: string, init: KeyboardEventInit) {
  imeInsert(textarea, data)
  key(textarea, "keydown", init, 229)
  vi.advanceTimersByTime(0)
}

function typeWebKit(textarea: HTMLTextAreaElement, data: string) {
  pressWebKit(textarea, data, BANG)
  key(textarea, "keyup", BANG, 49)
}

describe("installImeEarlyInputFix", () => {
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

  it("sends punctuation typed before the previous key is released", () => {
    const { textarea, sent } = setup()
    pressWebKit(textarea, "，", COMMA)
    pressWebKit(textarea, "。", PERIOD)
    key(textarea, "keyup", COMMA, 188)
    key(textarea, "keyup", PERIOD, 190)
    expect(sent).toEqual(["，", "。"])
  })

  it("sends punctuation typed while Caps Lock is on", () => {
    const { textarea, sent } = setup()
    // macOS sends no keyup until Caps Lock is turned off again.
    key(textarea, "keydown", { key: "CapsLock", code: "CapsLock" }, 20)
    pressWebKit(textarea, "，", COMMA)
    expect(sent).toEqual(["，"])
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
