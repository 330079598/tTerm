import { describe, expect, it } from "vitest"

import {
  filterThemeGroups,
  type ThemePickerGroup,
} from "@/components/ConnectionDialog/connectionDialogUtils"

describe("filterThemeGroups", () => {
  const groups: ThemePickerGroup[] = [
    { label: null, themes: [{ id: "", label: "Same as the app theme" }] },
    {
      label: "Presets",
      themes: [
        { id: "default", label: "Default" },
        { id: "ocean", label: "Ocean" },
      ],
    },
    {
      label: "Library",
      themes: [
        { id: "catalog:Red Planet", label: "Red Planet" },
        { id: "catalog:Tomorrow Night", label: "Tomorrow Night" },
      ],
    },
  ]

  it("keeps everything for an empty query", () => {
    expect(filterThemeGroups(groups, "  ")).toEqual(groups)
  })

  it("matches every word in any order, ignoring case", () => {
    expect(filterThemeGroups(groups, "night TOMORROW")).toEqual([
      { label: "Library", themes: [{ id: "catalog:Tomorrow Night", label: "Tomorrow Night" }] },
    ])
  })

  it("drops groups left empty", () => {
    expect(filterThemeGroups(groups, "red").map((group) => group.label)).toEqual(["Library"])
    expect(filterThemeGroups(groups, "nothing like this")).toEqual([])
  })
})
