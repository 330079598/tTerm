import { fixtures } from "@/parity/fixtures"
import { parityScreenIds } from "@/parity/screens"
import manifest from "@manifest"
import { describe, expect, it } from "vitest"

describe("parity screen manifest", () => {
  it("uses ids that the React registry knows", () => {
    expect(manifest.screens.map((screen) => screen.id).sort()).toEqual(parityScreenIds)
  })

  it("has a fixture for every registered screen", () => {
    for (const screen of manifest.screens) {
      expect(fixtures[screen.id], `missing fixture for ${screen.id}`).toBeDefined()
    }
  })

  it("does not register a screen the manifest forgot", () => {
    const manifestIds = new Set(manifest.screens.map((screen) => screen.id))
    expect(parityScreenIds.filter((id) => !manifestIds.has(id))).toEqual([])
  })
})
