import { describe, expect, it } from "vitest"

import {
  addRecentProfile,
  loadRecentProfiles,
  PROFILE_RECENTS_LIMIT,
  PROFILE_RECENTS_STORAGE_KEY,
  removeRecentProfile,
} from "@/lib/profileRecents"

describe("profile recents", () => {
  it("moves a profile used again to the front", () => {
    let recents = addRecentProfile([], "a", 1)
    recents = addRecentProfile(recents, "b", 2)
    recents = addRecentProfile(recents, "a", 3)
    expect(recents).toEqual([
      { profileId: "a", lastUsedAt: 3 },
      { profileId: "b", lastUsedAt: 2 },
    ])
  })

  it("keeps only the newest ones", () => {
    let recents = addRecentProfile([], "first", 0)
    for (let index = 1; index <= PROFILE_RECENTS_LIMIT; index += 1) {
      recents = addRecentProfile(recents, `p${index}`, index)
    }
    expect(recents).toHaveLength(PROFILE_RECENTS_LIMIT)
    expect(recents.some((recent) => recent.profileId === "first")).toBe(false)
  })

  it("forgets a profile", () => {
    const recents = addRecentProfile(addRecentProfile([], "a", 1), "b", 2)
    expect(removeRecentProfile(recents, "a")).toEqual([{ profileId: "b", lastUsedAt: 2 }])
  })

  it("loads only well-formed entries", () => {
    const stored = JSON.stringify([
      { profileId: "a", lastUsedAt: 1 },
      { profileId: 2, lastUsedAt: 1 },
      null,
    ])
    const storage = {
      getItem: (key: string) => (key === PROFILE_RECENTS_STORAGE_KEY ? stored : null),
    }
    expect(loadRecentProfiles(storage)).toEqual([{ profileId: "a", lastUsedAt: 1 }])
    expect(loadRecentProfiles({ getItem: () => "{not json" })).toEqual([])
  })
})
