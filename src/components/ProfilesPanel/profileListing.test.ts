import { describe, expect, it } from "vitest"

import { filterProfiles } from "@/components/ProfilesPanel/profileListing"
import type { SavedProfile } from "@/types/tab"

const profile = (overrides: Partial<SavedProfile> = {}): SavedProfile => ({
  id: "p1",
  name: "web-01",
  connection_type: "ssh",
  host: "10.0.0.5",
  port: 22,
  username: "root",
  remember_password: true,
  keepalive_interval_secs: 15,
  keepalive_count_max: 3,
  ...overrides,
})

const ids = (profiles: SavedProfile[]) => profiles.map((item) => item.id)

describe("filterProfiles", () => {
  it("matches every word in any field, the connection type and group path too", () => {
    const profiles = [
      profile({ group_path: "prod / eu" }),
      profile({ id: "p2", name: "com", connection_type: "serial" }),
    ]
    expect(ids(filterProfiles(profiles, "root web"))).toEqual(["p1"])
    expect(ids(filterProfiles(profiles, "eu web"))).toEqual(["p1"])
    expect(ids(filterProfiles(profiles, "serial"))).toEqual(["p2"])
    expect(ids(filterProfiles(profiles, ""))).toEqual(["p1", "p2"])
  })
})
