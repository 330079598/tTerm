import { describe, expect, it } from "vitest"

import {
  getGroupIdFromDragId,
  getGroupKeyFromDropId,
  getProfileDropIdFromDndId,
  getProfileIdFromDndId,
  groupBodyDropId,
  groupDragId,
  groupDropId,
  moveProfileInList,
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

describe("moveProfileInList", () => {
  const profile = (id: string, groupId?: string): SavedProfile => ({
    id,
    name: id,
    group_id: groupId,
    connection_type: "ssh",
    remember_password: false,
    keepalive_interval_secs: 15,
    keepalive_count_max: 3,
  })

  it("files the profile in the group before the target, or last", () => {
    const profiles = [profile("a"), profile("b", "g"), profile("c", "g")]
    const before = moveProfileInList(profiles, "a", "g", "c")
    expect(before.map((item) => [item.id, item.group_id])).toEqual([
      ["b", "g"],
      ["a", "g"],
      ["c", "g"],
    ])
    const last = moveProfileInList(profiles, "b", undefined, null)
    expect(last.map((item) => [item.id, item.group_id])).toEqual([
      ["a", undefined],
      ["c", "g"],
      ["b", undefined],
    ])
  })
})
