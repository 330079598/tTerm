// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react"
import { describe, expect, it } from "vitest"

import { useCapsLock } from "@/hooks/useCapsLock"

function pressKey(target: EventTarget, type: "keydown" | "keyup", capsLock: boolean) {
  const event = new KeyboardEvent(type, { key: "a", bubbles: true, modifierCapsLock: capsLock })
  act(() => {
    target.dispatchEvent(event)
  })
}

describe("useCapsLock", () => {
  it("follows the Caps Lock state of key events", () => {
    const { result } = renderHook(() => useCapsLock())
    expect(result.current).toBe(false)

    pressKey(window, "keydown", true)
    expect(result.current).toBe(true)

    pressKey(window, "keyup", false)
    expect(result.current).toBe(false)
  })

  it("sees keys whose handler stops them from bubbling", () => {
    const { result } = renderHook(() => useCapsLock())
    const input = document.createElement("textarea")
    input.addEventListener("keydown", (event) => event.stopPropagation())
    document.body.append(input)

    pressKey(input, "keydown", true)
    expect(result.current).toBe(true)

    pressKey(input, "keydown", false)
    input.remove()
  })
})
