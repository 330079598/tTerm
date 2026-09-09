// @vitest-environment jsdom
import { describe, expect, it } from "vitest"

import {
  eventToChord,
  formatChord,
  isEditableTarget,
  normalizeEventCode,
  normalizeEventKey,
  parseChord,
  serializeChord,
} from "@/lib/keymap/chord"

describe("normalizeEventKey", () => {
  it("normalizes letters to lowercase", () => {
    expect(normalizeEventKey("P")).toBe("p")
    expect(normalizeEventKey("p")).toBe("p")
  })

  it("normalizes named keys", () => {
    expect(normalizeEventKey("Tab")).toBe("tab")
    expect(normalizeEventKey("PageDown")).toBe("pagedown")
    expect(normalizeEventKey("F5")).toBe("f5")
  })

  it("aliases keys that would break + serialization", () => {
    expect(normalizeEventKey(" ")).toBe("space")
    expect(normalizeEventKey("+")).toBe("plus")
  })

  it("keeps bindable punctuation", () => {
    expect(normalizeEventKey("\\")).toBe("\\")
    expect(normalizeEventKey("`")).toBe("`")
    expect(normalizeEventKey("]")).toBe("]")
  })

  it("rejects modifiers and unbindable keys", () => {
    expect(normalizeEventKey("Shift")).toBeNull()
    expect(normalizeEventKey("Control")).toBeNull()
    expect(normalizeEventKey("Meta")).toBeNull()
    expect(normalizeEventKey("Dead")).toBeNull()
    expect(normalizeEventKey("")).toBeNull()
  })
})

describe("normalizeEventCode", () => {
  it("maps physical letter, digit, and punctuation keys to chord tokens", () => {
    expect(normalizeEventCode("KeyT")).toBe("t")
    expect(normalizeEventCode("Digit1")).toBe("1")
    expect(normalizeEventCode("Backslash")).toBe("\\")
    expect(normalizeEventCode("PageDown")).toBe("pagedown")
  })
})

describe("serializeChord / parseChord", () => {
  it("round-trips chords in canonical order", () => {
    const serialized = serializeChord({ key: "p", mod: true, shift: true })
    expect(serialized).toBe("mod+shift+p")
    expect(parseChord(serialized!)).toEqual({ key: "p", mod: true, shift: true })
  })

  it("parses serialized chords", () => {
    expect(parseChord("mod+f")).toEqual({ key: "f", mod: true })
    expect(parseChord("alt+1")).toEqual({ key: "1", alt: true })
    expect(parseChord("mod+tab")).toEqual({ key: "tab", mod: true })
    expect(parseChord("ctrl+tab")).toEqual({ key: "tab", ctrl: true })
    expect(parseChord("mod+shift+\\")).toEqual({ key: "\\", mod: true, shift: true })
  })

  it("rejects malformed serializations", () => {
    expect(parseChord("")).toBeNull()
    expect(parseChord("mod+shift+bogus+key")).toBeNull()
    expect(parseChord("shift+")).toBeNull()
  })
})

describe("eventToChord", () => {
  const baseEvent = {
    key: "p",
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
  } as KeyboardEvent

  it("maps ctrl to mod on non-mac platforms", () => {
    const event = { ...baseEvent, ctrlKey: true } as KeyboardEvent
    expect(eventToChord(event, false)).toEqual({ key: "p", mod: true })
  })

  it("maps meta and ctrl to mod on mac", () => {
    const metaEvent = { ...baseEvent, metaKey: true } as KeyboardEvent
    const ctrlEvent = { ...baseEvent, ctrlKey: true } as KeyboardEvent
    expect(eventToChord(metaEvent, true)).toEqual({ key: "p", mod: true })
    expect(eventToChord(ctrlEvent, true)).toEqual({ key: "p", ctrl: true })
  })

  it("captures alt and shift", () => {
    const event = { ...baseEvent, ctrlKey: true, altKey: true, shiftKey: true } as KeyboardEvent
    expect(eventToChord(event, false)).toEqual({
      key: "p",
      mod: true,
      alt: true,
      shift: true,
    })
  })

  it("uses physical key codes across non-Latin layouts", () => {
    const russianEvent = { ...baseEvent, key: "е", code: "KeyT", ctrlKey: true } as KeyboardEvent
    const frenchDigitEvent = {
      ...baseEvent,
      key: "&",
      code: "Digit1",
      altKey: true,
    } as KeyboardEvent

    expect(eventToChord(russianEvent, false)).toEqual({ key: "t", mod: true })
    expect(eventToChord(frenchDigitEvent, false)).toEqual({ key: "1", alt: true })
  })

  it("keeps Command and physical Control distinct on macOS", () => {
    const commandEvent = {
      ...baseEvent,
      key: "е",
      code: "KeyT",
      metaKey: true,
    } as KeyboardEvent
    const controlEvent = {
      ...baseEvent,
      key: "е",
      code: "KeyT",
      ctrlKey: true,
    } as KeyboardEvent

    expect(eventToChord(commandEvent, true)).toEqual({ key: "t", mod: true })
    expect(eventToChord(controlEvent, true)).toEqual({ key: "t", ctrl: true })
  })

  it("does not dispatch IME composition or dead-key intermediates", () => {
    expect(
      eventToChord({ ...baseEvent, key: "Process", code: "KeyT" } as KeyboardEvent, false)
    ).toBeNull()
    expect(
      eventToChord({ ...baseEvent, key: "Dead", code: "Quote" } as KeyboardEvent, false)
    ).toBeNull()
    expect(
      eventToChord({ ...baseEvent, code: "KeyT", isComposing: true } as KeyboardEvent, false)
    ).toBeNull()
  })

  it("returns null for bare modifiers", () => {
    expect(eventToChord({ ...baseEvent, key: "Control" } as KeyboardEvent, false)).toBeNull()
  })
})

describe("formatChord", () => {
  it("renders mac style symbols", () => {
    expect(formatChord("mod+shift+p", true)).toBe("⌘⇧P")
    expect(formatChord("mod+tab", true)).toBe("⌘Tab")
    expect(formatChord("alt+1", true)).toBe("⌥1")
    expect(formatChord("ctrl+tab", true)).toBe("⌃Tab")
  })

  it("renders windows style labels", () => {
    expect(formatChord("mod+shift+p", false)).toBe("Ctrl+Shift+P")
    expect(formatChord("mod+tab", false)).toBe("Ctrl+Tab")
    expect(formatChord("alt+1", false)).toBe("Alt+1")
  })

  it("falls back to the raw serialization for unparseable chords", () => {
    expect(formatChord("nonsense+key", false)).toBe("nonsense+key")
  })
})

describe("isEditableTarget", () => {
  it("detects input, textarea and contenteditable", () => {
    const input = document.createElement("input")
    expect(isEditableTarget(input)).toBe(true)

    const textarea = document.createElement("textarea")
    expect(isEditableTarget(textarea)).toBe(true)

    const div = document.createElement("div")
    div.setAttribute("contenteditable", "true")
    expect(isEditableTarget(div)).toBe(true)
  })

  it("detects editable ancestors", () => {
    const wrapper = document.createElement("div")
    wrapper.setAttribute("contenteditable", "true")
    const child = document.createElement("span")
    wrapper.appendChild(child)
    expect(isEditableTarget(child)).toBe(true)
  })

  it("rejects plain elements", () => {
    expect(isEditableTarget(document.createElement("div"))).toBe(false)
    expect(isEditableTarget(null)).toBe(false)
  })
})
