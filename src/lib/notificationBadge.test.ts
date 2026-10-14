import { describe, expect, it } from "vitest"

import { badgeLabel, badgeOverlaySize } from "@/lib/notificationBadge"

describe("badgeLabel", () => {
  it("shows a single digit and caps the rest", () => {
    expect(badgeLabel(1)).toBe("1")
    expect(badgeLabel(9)).toBe("9")
    expect(badgeLabel(10)).toBe("9+")
    expect(badgeLabel(250)).toBe("9+")
  })
})

describe("badgeOverlaySize", () => {
  it("follows the display scale and never drops below 16px", () => {
    expect(badgeOverlaySize(1)).toBe(16)
    expect(badgeOverlaySize(1.5)).toBe(24)
    expect(badgeOverlaySize(2)).toBe(32)
    expect(badgeOverlaySize(0.5)).toBe(16)
  })
})
