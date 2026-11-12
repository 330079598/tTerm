// @vitest-environment jsdom

import { invoke } from "@tauri-apps/api/core"
import { beforeEach, describe, expect, it, vi } from "vitest"

import {
  cachedFrontendState,
  onFrontendStateWriteError,
  resetFrontendStateCacheForTests,
  writeFrontendStateValue,
} from "@/lib/frontendStateCache"
import { initializeFrontendState, readLegacyFrontendState } from "@/lib/frontendStateMigration"
import { COMMAND_COMPOSER_OPEN_STORAGE_KEY } from "@/lib/commandComposer"
import { PROFILE_RECENTS_STORAGE_KEY } from "@/lib/profileRecents"
import { QUICK_CONNECT_RECENTS_STORAGE_KEY } from "@/lib/quickConnect"
import { loadRecentCommands, RECENT_COMMANDS_STORAGE_KEY } from "@/lib/recentCommands"
import { SERIAL_SEND_ECHO_STORAGE_KEY } from "@/lib/serialSendStore"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

const mockedInvoke = vi.mocked(invoke)

describe("frontend state storage", () => {
  beforeEach(() => {
    localStorage.clear()
    resetFrontendStateCacheForTests()
    mockedInvoke.mockReset()
  })

  it("hands the localStorage values over on the first start", async () => {
    const commands = [{ id: "c1", commandText: "ls", lastUsedAt: 1, useCount: 1 }]
    localStorage.setItem(RECENT_COMMANDS_STORAGE_KEY, JSON.stringify(commands))
    localStorage.setItem(COMMAND_COMPOSER_OPEN_STORAGE_KEY, "1")
    mockedInvoke.mockResolvedValue({
      recentCommands: commands,
      commandComposerOpen: true,
    })

    await initializeFrontendState()

    expect(mockedInvoke).toHaveBeenCalledWith("migrate_frontend_state", {
      legacy: {
        recentCommands: JSON.stringify(commands),
        commandComposerOpen: "true",
      },
    })
    expect(cachedFrontendState("recentCommands")).toEqual(commands)
    expect(cachedFrontendState("commandComposerOpen")).toBe(true)
  })

  it("uses the backend values when the migration already ran", async () => {
    localStorage.setItem(
      RECENT_COMMANDS_STORAGE_KEY,
      JSON.stringify([{ id: "old", commandText: "old", lastUsedAt: 1, useCount: 1 }])
    )
    const fromBackend = [{ id: "new", commandText: "new", lastUsedAt: 2, useCount: 1 }]
    mockedInvoke.mockResolvedValue({ recentCommands: fromBackend })

    await initializeFrontendState()

    expect(loadRecentCommands(localStorage)).toEqual(fromBackend)
  })

  it("skips damaged localStorage values without losing the others", () => {
    localStorage.setItem(RECENT_COMMANDS_STORAGE_KEY, "{not json")
    localStorage.setItem(PROFILE_RECENTS_STORAGE_KEY, JSON.stringify({ profileId: "wrong shape" }))
    localStorage.setItem(
      QUICK_CONNECT_RECENTS_STORAGE_KEY,
      JSON.stringify([
        { title: "host", username: "root", host: "example.com", port: 22, lastUsedAt: 1 },
      ])
    )
    localStorage.setItem(SERIAL_SEND_ECHO_STORAGE_KEY, JSON.stringify([1, "port:COM1"]))

    expect(readLegacyFrontendState(localStorage)).toEqual({
      recentQuickConnections: JSON.stringify([
        { title: "host", username: "root", host: "example.com", port: 22, lastUsedAt: 1 },
      ]),
      serialSendEcho: JSON.stringify(["port:COM1"]),
    })
  })

  it("starts in defaults when the backend cannot be read", async () => {
    localStorage.setItem(
      RECENT_COMMANDS_STORAGE_KEY,
      JSON.stringify([{ id: "legacy", commandText: "ls", lastUsedAt: 1, useCount: 1 }])
    )
    mockedInvoke.mockRejectedValue(new Error("database is locked"))

    await expect(initializeFrontendState()).resolves.toBeUndefined()
    expect(loadRecentCommands(localStorage)).toEqual([])
  })

  it("reports a failed write to the user", async () => {
    mockedInvoke.mockResolvedValueOnce({})
    await initializeFrontendState()
    mockedInvoke.mockRejectedValueOnce(new Error("disk is full"))
    const listener = vi.fn()
    const unsubscribe = onFrontendStateWriteError(listener)

    writeFrontendStateValue("recentCommands", [])

    await vi.waitFor(() => expect(listener).toHaveBeenCalledWith("disk is full"))
    unsubscribe()
  })
})
