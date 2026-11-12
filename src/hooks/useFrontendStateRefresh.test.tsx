// @vitest-environment jsdom

import { act, renderHook } from "@testing-library/react"
import { invoke } from "@tauri-apps/api/core"
import { useCallback, useState } from "react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { useFrontendStateRefresh } from "@/hooks/useFrontendStateRefresh"
import {
  hydrateFrontendState,
  reloadFrontendState,
  resetFrontendStateCacheForTests,
} from "@/lib/frontendStateCache"
import { loadRecentProfiles } from "@/lib/profileRecents"
import { loadRecentQuickConnections } from "@/lib/quickConnect"
import { loadRecentCommands } from "@/lib/recentCommands"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

const mockedInvoke = vi.mocked(invoke)

describe("useFrontendStateRefresh", () => {
  beforeEach(() => {
    localStorage.clear()
    resetFrontendStateCacheForTests()
    mockedInvoke.mockReset()
  })

  it("rereads imported recents so the next save keeps them", async () => {
    hydrateFrontendState({})
    mockedInvoke.mockResolvedValue({
      recentCommands: [{ id: "imported", commandText: "ls", lastUsedAt: 1, useCount: 1 }],
      recentProfiles: [{ profileId: "imported", lastUsedAt: 2 }],
      recentQuickConnections: [
        {
          title: "imported",
          username: "root",
          host: "example.com",
          port: 22,
          lastUsedAt: 3,
        },
      ],
    })
    const { result } = renderHook(() => {
      const [commands, setCommands] = useState(() => loadRecentCommands(localStorage))
      const [profiles, setProfiles] = useState(() => loadRecentProfiles(localStorage))
      const [connections, setConnections] = useState(() => loadRecentQuickConnections(localStorage))
      const refresh = useCallback(() => {
        setCommands(loadRecentCommands(localStorage))
        setProfiles(loadRecentProfiles(localStorage))
        setConnections(loadRecentQuickConnections(localStorage))
      }, [])
      useFrontendStateRefresh(refresh)
      return { commands, profiles, connections }
    })

    await act(() => reloadFrontendState())

    expect(result.current.commands[0]?.id).toBe("imported")
    expect(result.current.profiles[0]?.profileId).toBe("imported")
    expect(result.current.connections[0]?.host).toBe("example.com")
  })
})
