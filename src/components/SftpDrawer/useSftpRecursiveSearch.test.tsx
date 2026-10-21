// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import type {
  SftpDirectoryEntry,
  SftpDirectoryListing,
  SftpSearchEvent,
  SftpSearchSummary,
} from "@/components/SftpDrawer/types"
import {
  locationUnder,
  useSftpRecursiveSearch,
} from "@/components/SftpDrawer/useSftpRecursiveSearch"

const mocks = vi.hoisted(() => ({
  calls: [] as Array<{ command: string; args: Record<string, unknown> }>,
  // Resolves the pending search with its summary.
  finish: null as ((summary: unknown) => void) | null,
}))

vi.mock("@tauri-apps/api/core", () => {
  class Channel<T> {
    onmessage: (message: T) => void = () => {}
  }
  return {
    Channel,
    invoke: (command: string, args: Record<string, unknown>) => {
      mocks.calls.push({ command, args })
      if (command !== "sftp_search_entries") return Promise.resolve()
      return new Promise((resolve) => {
        mocks.finish = resolve
      })
    },
  }
})

function entry(path: string): SftpDirectoryEntry {
  return { name: path.split("/").pop() ?? path, path, isDir: false, isSymlink: false }
}

const LISTING: SftpDirectoryListing = { currentPath: "/srv", entries: [entry("/srv/top.txt")] }
const SUMMARY: SftpSearchSummary = {
  method: "find",
  found: 1,
  truncated: false,
  cancelled: false,
  skippedFolders: 0,
}

function render(searchQuery: string, listing: SftpDirectoryListing = LISTING) {
  return renderHook(
    (props: { searchQuery: string; listing: SftpDirectoryListing }) =>
      useSftpRecursiveSearch({ tabId: "tab", connection: undefined, ...props }),
    { initialProps: { searchQuery, listing } }
  )
}

function lastSearchChannel() {
  const call = [...mocks.calls].reverse().find((item) => item.command === "sftp_search_entries")
  return call?.args.onEvent as { onmessage: (event: SftpSearchEvent) => void }
}

beforeEach(() => {
  mocks.calls = []
  mocks.finish = null
})

afterEach(() => {
  cleanup()
})

describe("locationUnder", () => {
  it("gives the folder relative to the searched one", () => {
    expect(locationUnder("/srv", "/srv/a/b/c.txt")).toBe("a/b")
    expect(locationUnder("/srv", "/srv/c.txt")).toBe("")
    expect(locationUnder("/", "/etc/hosts")).toBe("etc")
  })
})

describe("useSftpRecursiveSearch", () => {
  it("shows streamed matches in place of the listing until done", async () => {
    const { result } = render("log")
    expect(result.current.shownListing).toBe(LISTING)

    act(() => void result.current.start(false, true))
    expect(mocks.calls[0].args).toMatchObject({
      request: { root: "/srv", query: "log", regex: false, includeHidden: true },
    })
    expect(result.current.search?.status).toBe("searching")
    expect(result.current.shownListing?.entries).toEqual([])

    act(() =>
      lastSearchChannel().onmessage({ kind: "matches", entries: [entry("/srv/a/app.log")] })
    )
    expect(result.current.shownListing?.currentPath).toBe("/srv")
    expect(result.current.shownListing?.entries).toEqual([
      { ...entry("/srv/a/app.log"), location: "a" },
    ])

    await act(async () => mocks.finish?.(SUMMARY))
    expect(result.current.search?.status).toBe("done")
    expect(result.current.search?.summary).toEqual(SUMMARY)
  })

  it("drops the results and stops the search once the query is cleared", () => {
    const { result, rerender } = render("log")
    act(() => void result.current.start(false, false))
    const searchId = mocks.calls[0].args.searchId

    rerender({ searchQuery: "", listing: LISTING })
    expect(result.current.search).toBeNull()
    expect(result.current.shownListing).toBe(LISTING)
    expect(mocks.calls).toContainEqual({
      command: "sftp_cancel_remote_operation",
      args: { operationId: searchId },
    })
  })

  it("drops the results of another folder", () => {
    const { result, rerender } = render("log")
    act(() => void result.current.start(false, false))
    rerender({ searchQuery: "log", listing: { currentPath: "/etc", entries: [] } })
    expect(result.current.search).toBeNull()
  })

  it("ignores events from a search that was replaced", () => {
    const { result } = render("log")
    act(() => void result.current.start(false, false))
    const first = lastSearchChannel()
    act(() => void result.current.start(true, false))

    act(() => first.onmessage({ kind: "matches", entries: [entry("/srv/old.log")] }))
    expect(result.current.shownListing?.entries).toEqual([])
  })
})
