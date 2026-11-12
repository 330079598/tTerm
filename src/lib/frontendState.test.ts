// @vitest-environment jsdom

import { invoke } from "@tauri-apps/api/core"
import { beforeEach, describe, expect, it, vi } from "vitest"

import {
  cachedFrontendState,
  isFrontendStateHydrated,
  onFrontendStateWriteError,
  reloadFrontendState,
  resetFrontendStateCacheForTests,
  takePendingFrontendStateStartupError,
  writeFrontendStateValue,
} from "@/lib/frontendStateCache"
import { initializeFrontendState, readLegacyFrontendState } from "@/lib/frontendStateMigration"
import {
  COMMAND_COMPOSER_LINE_BY_LINE_STORAGE_KEY,
  COMMAND_COMPOSER_OPEN_STORAGE_KEY,
} from "@/lib/commandComposer"
import { loadRecentProfiles, PROFILE_RECENTS_STORAGE_KEY } from "@/lib/profileRecents"
import { loadRecentQuickConnections, QUICK_CONNECT_RECENTS_STORAGE_KEY } from "@/lib/quickConnect"
import {
  loadRecentCommands,
  RECENT_COMMANDS_STORAGE_KEY,
  saveRecentCommands,
} from "@/lib/recentCommands"
import { loadSerialSendEcho, SERIAL_SEND_ECHO_STORAGE_KEY } from "@/lib/serialSendStore"
import { loadSftpColumnWeights } from "@/components/SftpDrawer/useSftpColumnWidths"
import {
  loadSftpViewPreferences,
  SFTP_COLUMN_WIDTH_STORAGE_KEY,
  SFTP_VIEW_STORAGE_KEY,
} from "@/components/SftpDrawer/sftpView"
import {
  loadReplayViewPreferences,
  REPLAY_VIEW_STORAGE_KEY,
} from "@/components/LogsPanel/replayView"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

const mockedInvoke = vi.mocked(invoke)

describe("frontend state storage", () => {
  beforeEach(() => {
    localStorage.clear()
    resetFrontendStateCacheForTests()
    mockedInvoke.mockReset()
    vi.restoreAllMocks()
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
    expect(localStorage.getItem(COMMAND_COMPOSER_OPEN_STORAGE_KEY)).toBe("1")
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
    expect(localStorage.getItem(RECENT_COMMANDS_STORAGE_KEY)).toBe(JSON.stringify(fromBackend))
  })

  it("acknowledges the values mirrored at startup", async () => {
    const commands = [{ id: "c1", commandText: "ls", lastUsedAt: 1, useCount: 1 }]
    const profiles = [{ profileId: "p1", lastUsedAt: 2 }]
    mockedInvoke.mockResolvedValue({
      recentCommands: commands,
      recentProfiles: profiles,
    })

    await initializeFrontendState()

    expect(mockedInvoke).toHaveBeenCalledWith("acknowledge_frontend_state_sync", {
      key: "recentCommands",
      value: commands,
    })
    expect(mockedInvoke).toHaveBeenCalledWith("acknowledge_frontend_state_sync", {
      key: "recentProfiles",
      value: profiles,
    })
    expect(
      mockedInvoke.mock.calls.filter(([command]) => command === "acknowledge_frontend_state_sync")
    ).toHaveLength(2)
  })

  it("mirrors every successful backend write in the format 0.8.x reads", async () => {
    mockedInvoke.mockResolvedValue({})
    await initializeFrontendState()
    mockedInvoke.mockClear()
    const commands = [{ id: "c2", commandText: "pwd", lastUsedAt: 2, useCount: 1 }]
    const quickConnections = [
      {
        title: "host",
        username: "root",
        host: "example.com",
        port: 22,
        lastUsedAt: 3,
      },
    ]
    const profiles = [{ profileId: "p1", lastUsedAt: 4 }]
    const widths = [300, 170, 100, 100, 126, 148]
    const sftpView = {
      sort: { column: "size" as const, direction: "desc" as const },
      showHidden: false,
    }
    const echo = ["port:COM1"]
    const replayView = { fontSize: 16, fitPanel: true }

    writeFrontendStateValue("recentCommands", commands)
    writeFrontendStateValue("recentQuickConnections", quickConnections)
    writeFrontendStateValue("recentProfiles", profiles)
    writeFrontendStateValue("sftpColumnWidths", widths)
    writeFrontendStateValue("sftpView", sftpView)
    writeFrontendStateValue("serialSendEcho", echo)
    writeFrontendStateValue("commandComposerOpen", true)
    writeFrontendStateValue("commandComposerLineByLine", false)
    writeFrontendStateValue("replayView", replayView)

    await vi.waitFor(() => expect(mockedInvoke).toHaveBeenCalledTimes(18))
    expect(localStorage.getItem(RECENT_COMMANDS_STORAGE_KEY)).toBe(JSON.stringify(commands))
    expect(localStorage.getItem(QUICK_CONNECT_RECENTS_STORAGE_KEY)).toBe(
      JSON.stringify(quickConnections)
    )
    expect(localStorage.getItem(PROFILE_RECENTS_STORAGE_KEY)).toBe(JSON.stringify(profiles))
    expect(localStorage.getItem(SFTP_COLUMN_WIDTH_STORAGE_KEY)).toBe(JSON.stringify(widths))
    expect(localStorage.getItem(SFTP_VIEW_STORAGE_KEY)).toBe(JSON.stringify(sftpView))
    expect(localStorage.getItem(SERIAL_SEND_ECHO_STORAGE_KEY)).toBe(JSON.stringify(echo))
    expect(localStorage.getItem(COMMAND_COMPOSER_OPEN_STORAGE_KEY)).toBe("1")
    expect(localStorage.getItem(COMMAND_COMPOSER_LINE_BY_LINE_STORAGE_KEY)).toBe("0")
    expect(localStorage.getItem(REPLAY_VIEW_STORAGE_KEY)).toBe(JSON.stringify(replayView))

    // Drop the memory cache so these are the 0.8.x localStorage readers.
    resetFrontendStateCacheForTests()
    expect(loadRecentCommands(localStorage)).toEqual(commands)
    expect(loadRecentQuickConnections(localStorage)).toEqual(quickConnections)
    expect(loadRecentProfiles(localStorage)).toEqual(profiles)
    expect(loadSftpColumnWeights(localStorage)).toEqual(widths)
    expect(loadSftpViewPreferences(localStorage)).toEqual(sftpView)
    expect(loadSerialSendEcho("port:COM1")).toBe(false)
    expect(loadReplayViewPreferences(localStorage)).toEqual(replayView)
    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "write_frontend_state", {
      key: "recentCommands",
      value: commands,
    })
    expect(mockedInvoke).toHaveBeenNthCalledWith(2, "acknowledge_frontend_state_sync", {
      key: "recentCommands",
      value: commands,
    })
  })

  it("does not record the localStorage copy when the old key cannot be written", async () => {
    mockedInvoke.mockResolvedValue({})
    await initializeFrontendState()
    mockedInvoke.mockClear()
    const setItem = Storage.prototype.setItem
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(function (
      this: Storage,
      key: string,
      value: string
    ) {
      if (key === RECENT_COMMANDS_STORAGE_KEY) throw new Error("quota exceeded")
      setItem.call(this, key, value)
    })

    writeFrontendStateValue("recentCommands", [])

    await vi.waitFor(() =>
      expect(mockedInvoke).toHaveBeenCalledWith("write_frontend_state", {
        key: "recentCommands",
        value: [],
      })
    )
    expect(mockedInvoke).not.toHaveBeenCalledWith(
      "acknowledge_frontend_state_sync",
      expect.anything()
    )
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

  it("keeps the whole session on localStorage when the backend cannot be read", async () => {
    const legacy = [{ id: "legacy", commandText: "ls", lastUsedAt: 1, useCount: 1 }]
    localStorage.setItem(RECENT_COMMANDS_STORAGE_KEY, JSON.stringify(legacy))
    mockedInvoke.mockRejectedValueOnce(new Error("database is locked"))

    await expect(initializeFrontendState()).resolves.toBeUndefined()
    expect(isFrontendStateHydrated()).toBe(false)
    expect(takePendingFrontendStateStartupError()).toBe("database is locked")
    expect(loadRecentCommands(localStorage)).toEqual(legacy)

    const updated = [{ id: "written", commandText: "pwd", lastUsedAt: 2, useCount: 1 }]
    saveRecentCommands(localStorage, updated)
    expect(JSON.parse(localStorage.getItem(RECENT_COMMANDS_STORAGE_KEY) ?? "null")).toEqual(updated)
    expect(mockedInvoke).toHaveBeenCalledTimes(1)

    mockedInvoke.mockResolvedValueOnce({ recentCommands: updated })
    await initializeFrontendState()
    expect(mockedInvoke).toHaveBeenCalledTimes(3)
    expect(cachedFrontendState("recentCommands")).toEqual(updated)
  })

  it("keeps local recents when a themes-only backup is imported in a degraded session", async () => {
    const commands = [{ id: "local", commandText: "ls", lastUsedAt: 1, useCount: 1 }]
    localStorage.setItem(RECENT_COMMANDS_STORAGE_KEY, JSON.stringify(commands))
    mockedInvoke.mockRejectedValueOnce(new Error("database is locked"))
    await initializeFrontendState()
    expect(isFrontendStateHydrated()).toBe(false)
    mockedInvoke.mockClear()
    mockedInvoke.mockResolvedValue({ recentCommands: commands })

    await reloadFrontendState({ customThemes: [{ id: "imported-theme" }] })

    expect(mockedInvoke).toHaveBeenCalledWith("migrate_frontend_state", {
      legacy: { recentCommands: JSON.stringify(commands) },
    })
    expect(mockedInvoke).not.toHaveBeenCalledWith("load_frontend_state")
    expect(isFrontendStateHydrated()).toBe(true)
    expect(loadRecentCommands(localStorage)).toEqual(commands)
    expect(localStorage.getItem(RECENT_COMMANDS_STORAGE_KEY)).toBe(JSON.stringify(commands))
  })

  it("keeps imported recents and untouched local keys in a degraded session", async () => {
    const localCommands = [{ id: "local", commandText: "ls", lastUsedAt: 1, useCount: 1 }]
    const importedCommands = [{ id: "imported", commandText: "pwd", lastUsedAt: 2, useCount: 3 }]
    const connections = [
      {
        title: "local-host",
        username: "root",
        host: "example.com",
        port: 22,
        lastUsedAt: 4,
      },
    ]
    localStorage.setItem(RECENT_COMMANDS_STORAGE_KEY, JSON.stringify(localCommands))
    localStorage.setItem(QUICK_CONNECT_RECENTS_STORAGE_KEY, JSON.stringify(connections))
    mockedInvoke.mockRejectedValueOnce(new Error("database is locked"))
    await initializeFrontendState()
    mockedInvoke.mockClear()
    mockedInvoke.mockResolvedValue({
      recentCommands: importedCommands,
      recentQuickConnections: connections,
    })

    await reloadFrontendState({ recentCommands: importedCommands })

    expect(mockedInvoke).toHaveBeenCalledWith("migrate_frontend_state", {
      legacy: {
        recentCommands: JSON.stringify(importedCommands),
        recentQuickConnections: JSON.stringify(connections),
      },
    })
    expect(loadRecentCommands(localStorage)).toEqual(importedCommands)
    expect(loadRecentQuickConnections(localStorage)).toEqual(connections)
    expect(localStorage.getItem(QUICK_CONNECT_RECENTS_STORAGE_KEY)).toBe(
      JSON.stringify(connections)
    )
  })

  it("stays degraded when the import recovery migration still fails", async () => {
    const imported = [{ id: "imported", commandText: "pwd", lastUsedAt: 2, useCount: 3 }]
    mockedInvoke.mockRejectedValueOnce(new Error("database is locked"))
    await initializeFrontendState()
    mockedInvoke.mockRejectedValueOnce(new Error("still locked"))

    await expect(reloadFrontendState({ recentCommands: imported })).rejects.toThrow("still locked")

    expect(isFrontendStateHydrated()).toBe(false)
    expect(JSON.parse(localStorage.getItem(RECENT_COMMANDS_STORAGE_KEY) ?? "null")).toEqual(
      imported
    )
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

  it("reports repeated write failures once until a write succeeds again", async () => {
    let writes = 0
    mockedInvoke.mockImplementation(async (command) => {
      if (command === "write_frontend_state") {
        writes += 1
        if (writes !== 3) throw new Error(`disk is full ${writes}`)
      }
      return {}
    })
    await initializeFrontendState()
    const listener = vi.fn()
    const unsubscribe = onFrontendStateWriteError(listener)

    writeFrontendStateValue("recentCommands", [])
    writeFrontendStateValue("recentProfiles", [])

    await vi.waitFor(() => expect(writes).toBe(2))
    expect(listener).toHaveBeenCalledTimes(1)
    expect(listener).toHaveBeenCalledWith("disk is full 1")

    writeFrontendStateValue("recentCommands", [])
    writeFrontendStateValue("recentProfiles", [])

    await vi.waitFor(() => expect(writes).toBe(4))
    expect(listener).toHaveBeenCalledTimes(2)
    expect(listener).toHaveBeenLastCalledWith("disk is full 4")
    unsubscribe()
  })
})
