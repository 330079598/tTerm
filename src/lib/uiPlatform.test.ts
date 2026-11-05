// @vitest-environment jsdom

import { afterEach, describe, expect, it } from "vitest"

import { applyUiScalePercent } from "@/lib/uiPlatform"
import tokens from "@/styles/tokens.json"

const root = () => document.documentElement

afterEach(() => {
  root().removeAttribute("style")
})

describe("interface scale", () => {
  it("writes every type-scale step from tokens.json", () => {
    applyUiScalePercent(100)

    for (const [name, size] of Object.entries(tokens.fontSizes)) {
      expect(root().style.getPropertyValue(`--text-${name}`)).toBe(`${size.rem}rem`)
      expect(root().style.getPropertyValue(`--text-${name}--line-height`)).toBe(
        String(size.lineHeight)
      )
    }
    expect(root().style.getPropertyValue("--ui-font-scale")).toBe("1")
  })

  it("scales the font size but leaves the line height alone", () => {
    applyUiScalePercent(125)

    expect(root().style.getPropertyValue("--ui-font-scale")).toBe("1.25")
    expect(root().style.getPropertyValue("--text-xs")).toBe("0.9375rem")
    expect(root().style.getPropertyValue("--text-base")).toBe("1.25rem")
    expect(root().style.getPropertyValue("--text-xs--line-height")).toBe("1.15")
  })
})
