import { describe, expect, it } from "vitest"

import { nextTypedLength } from "@/lib/passwordFeedback"

describe("nextTypedLength", () => {
  it("counts each typed character, pasted ones included", () => {
    expect(nextTypedLength(0, "s")).toBe(1)
    expect(nextTypedLength(1, "ecret")).toBe(6)
    // One character, however many UTF-16 units it takes.
    expect(nextTypedLength(0, "密码🔑")).toBe(3)
  })

  it("erases with Backspace and clears with Ctrl+U", () => {
    expect(nextTypedLength(3, "\x7f")).toBe(2)
    expect(nextTypedLength(3, "\b")).toBe(2)
    expect(nextTypedLength(0, "\x7f")).toBe(0)
    expect(nextTypedLength(5, "\x15ab")).toBe(2)
  })

  it("ignores other control characters", () => {
    expect(nextTypedLength(2, "\t\x04")).toBe(2)
  })

  it("ends at Enter or Ctrl+C", () => {
    expect(nextTypedLength(4, "\r")).toBeNull()
    expect(nextTypedLength(4, "pw\n")).toBeNull()
    expect(nextTypedLength(4, "\x03")).toBeNull()
  })
})
