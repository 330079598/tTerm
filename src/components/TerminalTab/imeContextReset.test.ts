// @vitest-environment jsdom
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest"
import { Terminal } from "@xterm/xterm"

import { installImeContextReset, isControlKey } from "@/components/TerminalTab/imeContextReset"

let term: Terminal | null = null
let reset: { dispose: () => void } | null = null

// What the textarea holds after "是的" was committed with the Pinyin IME.
const COMMITTED = " 是的"

function setup({ install = true } = {}) {
  vi.useFakeTimers()
  const container = document.createElement("div")
  document.body.appendChild(container)
  term = new Terminal({ cols: 80, rows: 24 })
  term.open(container)
  if (install) reset = installImeContextReset(term)
  const textarea = term.textarea!
  textarea.focus()
  textarea.value = COMMITTED
  const sent: string[] = []
  term.onData((data) => sent.push(data))
  return { textarea, sent }
}

function keydown(textarea: HTMLTextAreaElement, init: KeyboardEventInit, keyCode: number) {
  const event = new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init })
  Object.defineProperty(event, "keyCode", { value: keyCode })
  textarea.dispatchEvent(event)
}

const CTRL_J = { key: "j", code: "KeyJ", ctrlKey: true }

describe("installImeContextReset", () => {
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
    reset?.dispose()
    term?.dispose()
    reset = null
    term = null
    document.body.replaceChildren()
    vi.useRealTimers()
  })

  it("keeps the committed text after Ctrl+J without the fix", () => {
    const { textarea, sent } = setup({ install: false })
    keydown(textarea, CTRL_J, 74)
    expect(sent).toEqual(["\n"])
    expect(textarea.value).toBe(COMMITTED)
  })

  it("empties the textarea when Ctrl+J starts a new line", () => {
    const { textarea, sent } = setup()
    keydown(textarea, CTRL_J, 74)
    expect(sent).toEqual(["\n"])
    expect(textarea.value).toBe("")
  })

  it("empties the textarea on arrow keys and Backspace", () => {
    const { textarea, sent } = setup()
    keydown(textarea, { key: "ArrowLeft", code: "ArrowLeft" }, 37)
    expect(textarea.value).toBe("")
    textarea.value = COMMITTED
    keydown(textarea, { key: "Backspace", code: "Backspace" }, 8)
    expect(textarea.value).toBe("")
    expect(sent).toEqual(["\x1b[D", "\x7f"])
  })

  it("keeps the context for printable keys", () => {
    const { textarea, sent } = setup()
    keydown(textarea, { key: "a", code: "KeyA" }, 65)
    expect(sent).toEqual(["a"])
    expect(textarea.value).toBe(COMMITTED)
  })

  it("leaves a pending IME diff alone instead of sending DEL", () => {
    const { textarea, sent } = setup()
    keydown(textarea, { key: ".", code: "Period" }, 229)
    keydown(textarea, CTRL_J, 74)
    vi.advanceTimersByTime(0)
    expect(sent).toEqual(["\n"])
    expect(textarea.value).toBe(COMMITTED)
  })
})

describe("isControlKey", () => {
  it("tells control output from printable text", () => {
    expect(isControlKey("\n")).toBe(true)
    expect(isControlKey("\x1b[A")).toBe(true)
    expect(isControlKey("\x7f")).toBe(true)
    expect(isControlKey("a")).toBe(false)
    expect(isControlKey("是")).toBe(false)
  })
})
