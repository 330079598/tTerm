import { describe, expect, it } from "vitest"

import { profileMatchScore, profileSearchFields, rankProfiles } from "@/lib/profileSearch"
import { matchesQuery } from "@/lib/quickConnect"
import { DEFAULT_SERIAL_SETTINGS } from "@/lib/serial"
import type { SavedProfile } from "@/types/tab"

const profile = (overrides: Partial<SavedProfile> = {}): SavedProfile => ({
  id: "p1",
  name: "web-01",
  group_path: "Production",
  connection_type: "ssh",
  host: "10.0.0.5",
  port: 22,
  username: "root",
  remember_password: true,
  keepalive_interval_secs: 15,
  keepalive_count_max: 3,
  ...overrides,
})

describe("profileSearchFields", () => {
  it("finds a connection by words from different fields in any order", () => {
    const fields = profileSearchFields(profile())
    expect(matchesQuery("root web", fields)).toBe(true)
    expect(matchesQuery("prod 10.0.0", fields)).toBe(true)
    expect(matchesQuery("web admin", fields)).toBe(false)
  })

  it("finds a serial connection by its port", () => {
    const fields = profileSearchFields(
      profile({
        connection_type: "serial",
        host: undefined,
        username: undefined,
        serial: { ...DEFAULT_SERIAL_SETTINGS, port: "COM3" },
      })
    )
    expect(matchesQuery("com3", fields)).toBe(true)
  })
})

describe("profileMatchScore", () => {
  it("scores the name above the start of a name above the other fields", () => {
    expect(profileMatchScore(profile({ name: "web" }), "WEB")).toBe(4)
    expect(profileMatchScore(profile({ name: "web-01" }), "web")).toBe(3)
    expect(profileMatchScore(profile({ name: "prod-web-01" }), "web")).toBe(3)
    expect(profileMatchScore(profile({ name: "webhook-web" }), "hook web")).toBe(2)
    expect(profileMatchScore(profile({ name: "gateway", host: "10.0.0.5" }), "10.0")).toBe(2)
    expect(profileMatchScore(profile({ name: "gateway" }), "root")).toBe(1)
    expect(profileMatchScore(profile(), "  ")).toBe(0)
  })
})

describe("rankProfiles", () => {
  const ids = (profiles: SavedProfile[]) => profiles.map((item) => item.id)

  it("puts the best fit first, then the one used last, then the saved order", () => {
    const profiles = [
      profile({ id: "user", name: "gateway", username: "web" }),
      profile({ id: "older", name: "web-02" }),
      profile({ id: "newer", name: "web-03" }),
      profile({ id: "exact", name: "web" }),
      profile({ id: "unused", name: "web-04" }),
    ]
    const lastUsed = new Map([
      ["older", 1],
      ["newer", 2],
    ])
    expect(ids(rankProfiles(profiles, "web", lastUsed))).toEqual([
      "exact",
      "newer",
      "older",
      "unused",
      "user",
    ])
  })

  it("orders by last use alone with nothing typed", () => {
    const profiles = ["a", "b", "c"].map((id) => profile({ id }))
    expect(ids(rankProfiles(profiles, "", new Map([["c", 5]])))).toEqual(["c", "a", "b"])
  })
})
