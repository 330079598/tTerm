import { afterEach, describe, expect, it, vi } from "vitest"

import { getSpinnerStyle, setSpinnerStyle, subscribeSpinnerStyle } from "@/lib/spinnerStyle"

describe("spinnerStyle", () => {
  afterEach(() => setSpinnerStyle("ring"))

  it("starts with the default working style", () => {
    expect(getSpinnerStyle()).toBe("ring")
  })

  it("tells subscribers about a change, and only a change", () => {
    const listener = vi.fn()
    const unsubscribe = subscribeSpinnerStyle(listener)
    setSpinnerStyle("comet")
    setSpinnerStyle("comet")
    expect(getSpinnerStyle()).toBe("comet")
    expect(listener).toHaveBeenCalledTimes(1)

    unsubscribe()
    setSpinnerStyle("dots")
    expect(listener).toHaveBeenCalledTimes(1)
  })
})
