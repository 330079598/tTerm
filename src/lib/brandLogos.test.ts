import { describe, expect, it } from "vitest"

import { BRAND_LOGO_IDS, brandLogoColor, brandLogoName, isBrandLogoId } from "@/lib/brandLogos"

describe("brandLogos", () => {
  it("names each logo and gives it a color a tab icon can keep", () => {
    for (const id of BRAND_LOGO_IDS) {
      expect(brandLogoName(id)).not.toBe("")
      expect(brandLogoColor(id)).toMatch(/^(#[0-9a-f]{6})?$/)
    }
  })

  it("knows its own logos only", () => {
    expect(isBrandLogoId("docker")).toBe(true)
    expect(isBrandLogoId("fish")).toBe(false)
    expect(isBrandLogoId(undefined)).toBe(false)
  })
})
