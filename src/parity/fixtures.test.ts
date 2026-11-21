import { fixtures } from "@/parity/fixtures"
import { parityScreenIds } from "@/parity/screens"
import manifest from "@manifest"
import { describe, expect, it } from "vitest"

/**
 * `screens` is also the GPUI comparison contract. States whose GPUI view is
 * not written yet stay in `referenceScreens` so the reference can be
 * captured without breaking `compare`.
 */
const manifestScreens = [...manifest.screens, ...(manifest.referenceScreens ?? [])]

describe("parity screen manifest", () => {
  it("uses ids that the React registry knows", () => {
    expect(manifestScreens.map((screen) => screen.id).sort()).toEqual(parityScreenIds)
  })

  it("has a fixture for every registered screen", () => {
    for (const screen of manifestScreens) {
      expect(fixtures[screen.id], `missing fixture for ${screen.id}`).toBeDefined()
    }
  })

  it("does not register a screen the manifest forgot", () => {
    const manifestIds = new Set(manifestScreens.map((screen) => screen.id))
    expect(parityScreenIds.filter((id) => !manifestIds.has(id))).toEqual([])
  })

  it("does not list a screen on both sides of the GPUI contract", () => {
    const comparedIds = new Set(manifest.screens.map((screen) => screen.id))
    for (const screen of manifest.referenceScreens ?? []) {
      expect(comparedIds.has(screen.id), `${screen.id} is both compared and reference-only`).toBe(
        false
      )
    }
  })
})
