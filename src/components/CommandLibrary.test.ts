// @vitest-environment jsdom

import React from "react"
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { invoke } from "@tauri-apps/api/core"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import {
  addCommandTags,
  collectCommandTags,
  CommandEditorDialog,
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

afterEach(cleanup)

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

describe("command variables", () => {
  const sshCommand: SavedCommand = {
    ...commands[0],
    id: "3",
    name: "SSH",
    commandText: "ssh {{user}}@{{host}} '{{end}}'",
    variables: [
      {
        name: "user",
        label: "Login",
        valueType: "text",
        defaultValue: "root",
        isRequired: true,
        position: 0,
      },
      { name: "host", label: "Host", valueType: "text", isRequired: true, position: 1 },
    ],
  }

  function mockLibrary(saved: SavedCommand[]) {
    mockedInvoke.mockImplementation(async (command, args) => {
      if (command === "list_saved_commands") return saved
      if (command === "list_command_tags") return []
      if (command === "record_saved_command_use") return undefined
      if (command === "save_saved_command") {
        return { ...commands[0], ...(args as { input: object }).input, id: "saved" }
      }
      throw new Error(`Unexpected command: ${command}`)
    })
  }

  it("asks for the values and inserts the filled-in command", async () => {
    mockLibrary([sshCommand])
    const onInsert = vi.fn(async () => true)
    render(
      React.createElement(CommandLibrary, {
        open: true,
        onOpenChange: vi.fn(),
        canInsert: true,
        onInsert,
        onInsertRecent: vi.fn(async () => true),
        recentCommands: [],
      })
    )

    fireEvent.click(await screen.findByRole("button", { name: "commandLibrary.insert" }))
    const host = await screen.findByLabelText("Host")
    expect(screen.getByLabelText("Login")).toHaveProperty("value", "root")
    const form = host.closest("form")!

    fireEvent.submit(form)
    expect(screen.getByRole("alert").textContent).toBe(
      "commandLibrary.variables.valueErrors.required"
    )
    expect(onInsert).not.toHaveBeenCalled()

    fireEvent.change(host, { target: { value: "example.com" } })
    fireEvent.submit(form)

    await waitFor(() =>
      expect(onInsert).toHaveBeenCalledWith(
        expect.objectContaining({ id: "3", commandText: "ssh root@example.com '{{end}}'" })
      )
    )
    expect(mockedInvoke).toHaveBeenCalledWith("record_saved_command_use", { id: "3" })
  })

  it("asks before inserting a command that wants confirmation", async () => {
    mockLibrary([{ ...commands[0], confirmBeforeRun: true }])
    const onInsert = vi.fn(async () => true)
    render(
      React.createElement(CommandLibrary, {
        open: true,
        onOpenChange: vi.fn(),
        canInsert: true,
        onInsert,
        onInsertRecent: vi.fn(async () => true),
        recentCommands: [],
      })
    )

    fireEvent.click(await screen.findByRole("button", { name: "commandLibrary.insert" }))
    await screen.findByText("commandLibrary.confirmInsertTitle")
    expect(onInsert).not.toHaveBeenCalled()

    const insertButtons = screen.getAllByRole("button", { name: "commandLibrary.insert" })
    fireEvent.click(insertButtons[insertButtons.length - 1])
    await waitFor(() =>
      expect(onInsert).toHaveBeenCalledWith(
        expect.objectContaining({ id: "1", commandText: "docker logs -f api" })
      )
    )
  })

  it("saves placeholders typed in the editor as variables", async () => {
    mockLibrary([])
    render(
      React.createElement(CommandEditorDialog, {
        open: true,
        command: null,
        availableTags: [],
        onOpenChange: vi.fn(),
        onSaved: vi.fn(),
      })
    )

    const name = screen.getByLabelText("commandLibrary.form.name")
    fireEvent.change(name, { target: { value: "Ping" } })
    fireEvent.change(screen.getByLabelText("commandLibrary.form.command"), {
      target: { value: "ping -c {{count}} {{host}}" },
    })
    expect(
      screen.getAllByRole("checkbox", { name: "commandLibrary.variables.askFor" })
    ).toHaveLength(2)

    fireEvent.change(screen.getAllByLabelText("commandLibrary.variables.type")[0], {
      target: { value: "number" },
    })
    fireEvent.change(screen.getAllByLabelText("commandLibrary.variables.defaultValue")[0], {
      target: { value: "4" },
    })
    fireEvent.submit(name.closest("form")!)

    await waitFor(() =>
      expect(mockedInvoke).toHaveBeenCalledWith("save_saved_command", {
        input: expect.objectContaining({
          commandText: "ping -c {{count}} {{host}}",
          confirmBeforeRun: false,
          variables: [
            expect.objectContaining({
              name: "count",
              label: "count",
              valueType: "number",
              defaultValue: "4",
              position: 0,
            }),
            expect.objectContaining({ name: "host", valueType: "text", position: 1 }),
          ],
        }),
      })
    )
  })

  it("keeps placeholders of a captured command literal until they are enabled", async () => {
    mockLibrary([])
    render(
      React.createElement(CommandEditorDialog, {
        open: true,
        command: null,
        draft: {
          name: "Pods",
          commandText: "kubectl get pods -o go-template='{{end}}'",
          description: "",
          tags: [],
          scopeType: "global",
          isFavorite: false,
        },
        availableTags: [],
        onOpenChange: vi.fn(),
        onSaved: vi.fn(),
      })
    )

    const toggle = screen.getByRole("checkbox", { name: "commandLibrary.variables.askFor" })
    expect(toggle.getAttribute("aria-checked")).toBe("false")
    fireEvent.submit(screen.getByLabelText("commandLibrary.form.name").closest("form")!)

    await waitFor(() =>
      expect(mockedInvoke).toHaveBeenCalledWith("save_saved_command", {
        input: expect.objectContaining({ variables: [] }),
      })
    )
  })
})
