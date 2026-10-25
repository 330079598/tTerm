import { describe, expect, it } from "vitest"

import {
  filterProfiles,
  groupProfiles,
  loadProfileSort,
  UNGROUPED_KEY,
} from "@/components/ProfilesPanel/profileListing"
import type { SavedProfile } from "@/types/tab"

const profile = (overrides: Partial<SavedProfile> = {}): SavedProfile => ({
  id: "p1",
  name: "web-01",
  group: "",
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
  it("matches every word in any field, the connection type too", () => {
    const profiles = [profile(), profile({ id: "p2", name: "com", connection_type: "serial" })]
    expect(ids(filterProfiles(profiles, "root web"))).toEqual(["p1"])
    expect(ids(filterProfiles(profiles, "serial"))).toEqual(["p2"])
    expect(ids(filterProfiles(profiles, ""))).toEqual(["p1", "p2"])
  })
})

describe("groupProfiles", () => {
  it("sorts groups by name, puts the ungrouped last and keeps the saved order", () => {
    const grouped = groupProfiles([
      profile({ id: "a" }),
      profile({ id: "b", group: "Test" }),
      profile({ id: "c", group: "Prod" }),
      profile({ id: "d", group: "Test" }),
    ])
    expect(grouped.map(([group, items]) => [group, ids(items)])).toEqual([
      ["Prod", ["c"]],
      ["Test", ["b", "d"]],
      [UNGROUPED_KEY, ["a"]],
    ])
  })
})

describe("groupProfiles while searching", () => {
  it("puts the group holding the best fit first, and the best fit first in it", () => {
    const grouped = groupProfiles(
      [
        profile({ id: "a", group: "Alpha", name: "gateway", username: "web" }),
        profile({ id: "b", group: "Zulu", name: "db", username: "web" }),
        profile({ id: "c", group: "Zulu", name: "web" }),
      ],
      "web"
    )
    expect(grouped.map(([group, items]) => [group, ids(items)])).toEqual([
      ["Zulu", ["c", "b"]],
      ["Alpha", ["a"]],
    ])
  })
})

describe("loadProfileSort", () => {
  it("falls back to the grouped list for anything unknown", () => {
    expect(loadProfileSort({ getItem: () => "recent" })).toBe("recent")
    expect(loadProfileSort({ getItem: () => "sideways" })).toBe("default")
    expect(
      loadProfileSort({
        getItem: () => {
          throw new Error("blocked")
        },
      })
    ).toBe("default")
  })
})
