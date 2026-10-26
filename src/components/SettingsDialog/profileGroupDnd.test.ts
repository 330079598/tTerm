import { describe, expect, it } from "vitest"

import {
  getGroupIdFromDragId,
  getGroupKeyFromDropId,
  getProfileDropIdFromDndId,
  getProfileIdFromDndId,
  groupBodyDropId,
  groupDragId,
  groupDropId,
  isMoveUnchanged,
  moveProfilesInList,
  profileDragId,
  profileDropId,
  ROOT_DROP_ID,
} from "@/components/SettingsDialog/profileGroupDnd"
import type { SavedProfile } from "@/types/tab"

describe("drag and drop ids", () => {
  it("read back the profile or group they were made from, colons and all", () => {
    expect(getProfileIdFromDndId(profileDragId("p1"))).toBe("p1")
    expect(getProfileDropIdFromDndId(profileDropId("p1"))).toBe("p1")
    expect(getProfileDropIdFromDndId(profileDragId("p1"))).toBeNull()

    expect(getGroupKeyFromDropId(groupDropId("ops:eu"))).toBe("ops:eu")
    expect(getGroupKeyFromDropId(groupBodyDropId("ops:eu"))).toBe("ops:eu")
    expect(getGroupKeyFromDropId(groupDragId("ops:eu"))).toBeNull()
    expect(getGroupIdFromDragId(groupDragId("ops:eu"))).toBe("ops:eu")
    expect(getGroupIdFromDragId(profileDragId("p1"))).toBeNull()
  })

  it("tell the top-level drop zone apart from any group", () => {
    expect(getGroupKeyFromDropId(ROOT_DROP_ID)).toBeNull()
    expect(getProfileIdFromDndId(ROOT_DROP_ID)).toBeNull()
    expect(getGroupKeyFromDropId(null)).toBeNull()
  })
})

const profile = (id: string, groupId?: string): SavedProfile => ({
  id,
  name: id,
  group_id: groupId,
  connection_type: "ssh",
  remember_password: false,
  keepalive_interval_secs: 15,
  keepalive_count_max: 3,
})

const filing = (profiles: SavedProfile[]) => profiles.map((item) => [item.id, item.group_id])

describe("moveProfilesInList", () => {
  it("files the profile in the group before the target, or last", () => {
    const profiles = [profile("a"), profile("b", "g"), profile("c", "g")]
    expect(filing(moveProfilesInList(profiles, ["a"], "g", "c"))).toEqual([
      ["b", "g"],
      ["a", "g"],
      ["c", "g"],
    ])
    expect(filing(moveProfilesInList(profiles, ["b"], undefined, null))).toEqual([
      ["a", undefined],
      ["c", "g"],
      ["b", undefined],
    ])
  })

  it("keeps a batch in the order it was listed, whatever order it was picked in", () => {
    const profiles = [profile("a"), profile("b", "g"), profile("c"), profile("d", "g")]
    expect(filing(moveProfilesInList(profiles, ["c", "a"], "g", "d"))).toEqual([
      ["b", "g"],
      ["a", "g"],
      ["c", "g"],
      ["d", "g"],
    ])
  })

  it("leaves the list alone when none of the profiles exist", () => {
    const profiles = [profile("a")]
    expect(moveProfilesInList(profiles, ["missing"], "g", null)).toBe(profiles)
  })
})

describe("isMoveUnchanged", () => {
  const profiles = [profile("a", "g"), profile("b", "g"), profile("x"), profile("c", "g")]
  const unchanged = (ids: string[], groupId: string | undefined, targetId: string | null) =>
    isMoveUnchanged(profiles, moveProfilesInList(profiles, ids, groupId, targetId), ids, groupId)

  it("holds for a drop that leaves the group's order as it was", () => {
    expect(unchanged(["a"], "g", "b")).toBe(true)
    expect(unchanged(["a", "b"], "g", "c")).toBe(true)
    // Moving the group's last profile after the others moves it past "x",
    // which the page doesn't show.
    expect(unchanged(["c"], "g", null)).toBe(true)
  })

  it("fails for a reorder or a profile from another group", () => {
    expect(unchanged(["a"], "g", "c")).toBe(false)
    expect(unchanged(["a", "x"], "g", "c")).toBe(false)
    expect(unchanged(["a"], undefined, null)).toBe(false)
  })
})
