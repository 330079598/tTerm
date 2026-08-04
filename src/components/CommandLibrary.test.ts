// @vitest-environment jsdom

import React from "react"
import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { invoke } from "@tauri-apps/api/core"
import { beforeEach, describe, expect, it, vi } from "vitest"

import {
  addCommandTags,
  collectCommandTags,
  CommandLibrary,
  filterSavedCommands,
  parseCommandTags,
  removeCommandTag,
} from "@/components/CommandLibrary"
import type { SavedCommand } from "@/types/command"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number }) =>
      options?.count === undefined ? key : `${key}:${options.count}`,
  }),
}))

const mockedInvoke = vi.mocked(invoke)

beforeEach(() => {
  mockedInvoke.mockReset()
  mockedInvoke.mockImplementation(async (command) => {
    if (command === "list_saved_commands") return []
    throw new Error(`Unexpected command: ${command}`)
  })
})

const commands: SavedCommand[] = [
  {
    id: "1",
    name: "Docker logs",
    commandText: "docker logs -f api",
    description: "Follow service output",
    scopeType: "global",
    shellType: "any",
    platform: "any",
    isFavorite: true,
    confirmBeforeRun: false,
    sortOrder: 0,
    useCount: 0,
    createdAt: 1,
    updatedAt: 1,
    tags: ["container"],
    variables: [],
  },
  {
    id: "2",
    name: "Disk usage",
    commandText: "df -h",
    description: "",
    scopeType: "global",
    shellType: "any",
    platform: "linux",
    isFavorite: false,
    confirmBeforeRun: false,
    sortOrder: 0,
    useCount: 0,
    createdAt: 2,
    updatedAt: 2,
    tags: ["system"],
    variables: [],
  },
]

describe("command library filtering", () => {
  it("searches names, command text, descriptions, and tags", () => {
    expect(filterSavedCommands(commands, "SERVICE", "all").map((item) => item.id)).toEqual(["1"])
    expect(filterSavedCommands(commands, "df -h", "all").map((item) => item.id)).toEqual(["2"])
    expect(filterSavedCommands(commands, "container", "all").map((item) => item.id)).toEqual(["1"])
  })

  it("filters favorites independently from search", () => {
    expect(filterSavedCommands(commands, "", "favorites").map((item) => item.id)).toEqual(["1"])
  })
})

describe("command tag parsing", () => {
  it("trims and deduplicates mixed separators", () => {
    expect(parseCommandTags(" Docker,logs，docker\n system ")).toEqual(["Docker", "logs", "system"])
  })

  it("adds new tags without duplicating existing tags", () => {
    expect(addCommandTags(["Docker"], "docker, logs")).toEqual(["Docker", "logs"])
  })

  it("removes tags case-insensitively", () => {
    expect(removeCommandTag(["Docker", "logs"], "DOCKER")).toEqual(["logs"])
  })

  it("collects unique sorted tags from saved commands", () => {
    const taggedCommands = [
      { ...commands[0], tags: ["system", "Docker"] },
      { ...commands[1], tags: ["docker", "backup"] },
    ]

    expect(collectCommandTags(taggedCommands)).toEqual(["backup", "Docker", "system"])
  })
})

describe("recent command favorites", () => {
  it("saves a recent command as a profile-scoped favorite in one click", async () => {
    mockedInvoke.mockImplementation(async (command) => {
      if (command === "list_saved_commands") return []
      if (command === "save_saved_command") {
        return {
          ...commands[0],
          id: "saved-recent",
          name: "kubectl get pods",
          commandText: "kubectl get pods",
          scopeType: "profile",
          scopeId: "production",
          isFavorite: true,
        }
      }
      throw new Error(`Unexpected command: ${command}`)
    })

    render(
      React.createElement(CommandLibrary, {
        open: true,
        onOpenChange: vi.fn(),
        canInsert: true,
        onInsert: vi.fn(async () => true),
        onInsertRecent: vi.fn(async () => true),
        recentCommands: [
          {
            id: "recent-1",
            commandText: "kubectl get pods",
            profileId: "production",
            profileName: "Production",
            lastUsedAt: 100,
            useCount: 2,
          },
        ],
      })
    )

    fireEvent.click(screen.getByRole("button", { name: "commandLibrary.filters.recent" }))
    const favoriteButtons = await screen.findAllByRole("button", {
      name: "commandLibrary.favorite",
    })
    fireEvent.click(favoriteButtons[0])

    await waitFor(() =>
      expect(mockedInvoke).toHaveBeenCalledWith("save_saved_command", {
        input: expect.objectContaining({
          commandText: "kubectl get pods",
          scopeType: "profile",
          scopeId: "production",
          isFavorite: true,
        }),
      })
    )
  })
})
