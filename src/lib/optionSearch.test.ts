import { describe, expect, it } from "vitest"

import { filterOptionGroups, type SearchableOptionGroup } from "@/lib/optionSearch"

describe("filterOptionGroups", () => {
  const groups: SearchableOptionGroup[] = [
    { label: null, options: [{ value: "", label: "Same as the app theme" }] },
    {
      label: "Presets",
      options: [
        { value: "default", label: "Default" },
        { value: "ocean", label: "Ocean" },
      ],
    },
    {
      label: "Library",
      options: [
        { value: "catalog:Red Planet", label: "Red Planet" },
        { value: "catalog:Tomorrow Night", label: "Tomorrow Night" },
      ],
    },
  ]

  it("keeps everything for an empty query", () => {
    expect(filterOptionGroups(groups, "  ")).toEqual(groups)
  })

  it("matches every word in any order, ignoring case", () => {
    expect(filterOptionGroups(groups, "night TOMORROW")).toEqual([
      {
        label: "Library",
        options: [{ value: "catalog:Tomorrow Night", label: "Tomorrow Night" }],
      },
    ])
  })

  it("drops groups left empty", () => {
    expect(filterOptionGroups(groups, "red").map((group) => group.label)).toEqual(["Library"])
    expect(filterOptionGroups(groups, "nothing like this")).toEqual([])
  })

  it("finds an option by its detail and keywords", () => {
    const hosts: SearchableOptionGroup[] = [
      {
        label: "prod",
        options: [
          { value: "a", label: "bastion", detail: "ops@10.0.0.1:22", keywords: ["prod"] },
          { value: "b", label: "web", detail: "deploy@10.0.0.2:22", keywords: ["prod"] },
        ],
      },
    ]
    expect(filterOptionGroups(hosts, "10.0.0.2")[0].options.map((o) => o.value)).toEqual(["b"])
    expect(filterOptionGroups(hosts, "prod ops")[0].options.map((o) => o.value)).toEqual(["a"])
  })
})
