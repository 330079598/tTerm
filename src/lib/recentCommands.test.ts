import { describe, expect, it } from "vitest"

import { addRecentCommand, createCommandDraft, suggestCommandName } from "@/lib/recentCommands"

describe("recent commands", () => {
  it("deduplicates the same command within a profile", () => {
    const first = addRecentCommand([], {
      commandText: "  git status ",
      profileId: "one",
      executedAt: 10,
    })
    const second = addRecentCommand(first, {
      commandText: "git status",
      profileId: "one",
      executedAt: 20,
    })

    expect(second).toHaveLength(1)
    expect(second[0]).toMatchObject({ commandText: "git status", lastUsedAt: 20, useCount: 2 })
  })

  it("keeps identical commands from different profiles separate", () => {
    const commands = addRecentCommand(
      addRecentCommand([], { commandText: "uptime", profileId: "one", executedAt: 10 }),
      { commandText: "uptime", profileId: "two", executedAt: 20 }
    )
    expect(commands).toHaveLength(2)
  })

  it("creates a favorite draft scoped to the current profile", () => {
    expect(createCommandDraft("  docker ps  ", { id: "p1", name: "Production" })).toEqual({
      name: "docker ps",
      commandText: "docker ps",
      description: "",
      tags: [],
      scopeType: "profile",
      scopeId: "p1",
      isFavorite: true,
    })
  })

  it("truncates generated names", () => {
    expect(suggestCommandName("x".repeat(60))).toHaveLength(48)
  })
})
