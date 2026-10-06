import { describe, expect, it } from "vitest"

import { hasLegacyLocalTitle } from "@/lib/localTabTitle"

describe("hasLegacyLocalTitle", () => {
  it("recognizes the old default names of local tabs only", () => {
    expect(hasLegacyLocalTitle({ type: "terminal", title: "OS terminal" })).toBe(true)
    expect(hasLegacyLocalTitle({ type: "terminal", title: "OS terminal-3" })).toBe(true)
    expect(hasLegacyLocalTitle({ type: "terminal", title: "本地终端" })).toBe(true)
    expect(hasLegacyLocalTitle({ type: "terminal", title: "builds" })).toBe(false)
    expect(hasLegacyLocalTitle({ type: "ssh", title: "Terminal" })).toBe(false)
  })
})
