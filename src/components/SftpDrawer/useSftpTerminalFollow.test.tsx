// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react"
import { useCallback, useState } from "react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import type { LoadSftpDirectory, SftpDirectoryListing } from "@/components/SftpDrawer/types"
import { useSftpTerminalFollow } from "@/components/SftpDrawer/useSftpTerminalFollow"
import { setTabRemoteCwd } from "@/lib/remoteCwd"
import type { SftpStartDirectory } from "@/lib/sftpStartDirectory"

const mocks = vi.hoisted(() => ({
  config: { sftp_start_directory: "last" as SftpStartDirectory },
  loaded: [] as Array<string | null>,
  unlistable: new Set<string>(),
}))

vi.mock("@/contexts/ConfigContext", () => ({
  useConfig: () => ({ config: mocks.config }),
}))

const TAB = "follow-tab"

interface HarnessProps {
  visible: boolean
  sftpStartDirectory?: SftpStartDirectory
}

/** The drawer's listing state around the hook, as `useSftpDirectoryLoader` keeps it. */
function useHarness({ visible, sftpStartDirectory }: HarnessProps) {
  const [listing, setListing] = useState<SftpDirectoryListing | null>(null)
  const loadDirectory = useCallback<LoadSftpDirectory>(async (path, options) => {
    mocks.loaded.push(path ?? null)
    if (path && mocks.unlistable.has(path)) {
      if (options?.throwOnError) throw new Error("Permission denied")
      return
    }
    setListing({ currentPath: path ?? "/last", entries: [] })
  }, [])
  const follow = useSftpTerminalFollow({
    tabId: TAB,
    connection: { type: "ssh", sftpStartDirectory },
    visible,
    listing,
    isLoading: false,
    loadDirectory,
  })
  return { ...follow, listing }
}

function render(props: HarnessProps) {
  return renderHook((current: HarnessProps) => useHarness(current), { initialProps: props })
}

beforeEach(() => {
  mocks.config.sftp_start_directory = "last"
  mocks.loaded = []
  mocks.unlistable.clear()
})

afterEach(() => {
  cleanup()
  setTabRemoteCwd(TAB, null)
})

describe("useSftpTerminalFollow", () => {
  it("opens at the last directory and stays put by default", async () => {
    setTabRemoteCwd(TAB, "/var/log")
    const { result } = render({ visible: true })
    await waitFor(() => expect(result.current.listing?.currentPath).toBe("/last"))
    expect(result.current.follows).toBe(false)

    act(() => setTabRemoteCwd(TAB, "/etc"))
    expect(mocks.loaded).toEqual([null])
  })

  it("opens at the terminal's directory and follows each cd", async () => {
    mocks.config.sftp_start_directory = "terminal"
    setTabRemoteCwd(TAB, "/var/log")
    const { result } = render({ visible: true })
    await waitFor(() => expect(result.current.listing?.currentPath).toBe("/var/log"))
    expect(result.current.follows).toBe(true)

    act(() => setTabRemoteCwd(TAB, "/etc"))
    await waitFor(() => expect(result.current.listing?.currentPath).toBe("/etc"))
  })

  it("lets the connection choose over the setting", async () => {
    mocks.config.sftp_start_directory = "terminal"
    setTabRemoteCwd(TAB, "/var/log")
    const { result } = render({ visible: true, sftpStartDirectory: "home" })
    await waitFor(() => expect(result.current.listing?.currentPath).toBe("."))
    expect(result.current.follows).toBe(false)
  })

  it("catches up with the terminal once the hidden drawer opens again", async () => {
    mocks.config.sftp_start_directory = "terminal"
    setTabRemoteCwd(TAB, "/var/log")
    const { result, rerender } = render({ visible: true })
    await waitFor(() => expect(result.current.listing?.currentPath).toBe("/var/log"))

    rerender({ visible: false })
    act(() => setTabRemoteCwd(TAB, "/tmp"))
    act(() => setTabRemoteCwd(TAB, "/srv"))
    expect(mocks.loaded).toEqual(["/var/log"])

    rerender({ visible: true })
    await waitFor(() => expect(result.current.listing?.currentPath).toBe("/srv"))
    expect(mocks.loaded).toEqual(["/var/log", "/srv"])
  })

  it("opens at home when the terminal's directory cannot be listed", async () => {
    mocks.config.sftp_start_directory = "terminal"
    mocks.unlistable.add("/root")
    setTabRemoteCwd(TAB, "/root")
    const { result } = render({ visible: true })
    await waitFor(() => expect(result.current.listing?.currentPath).toBe("."))
    expect(mocks.loaded).toEqual(["/root", "."])
  })

  it("does not retry a directory it could not follow to", async () => {
    mocks.config.sftp_start_directory = "terminal"
    setTabRemoteCwd(TAB, "/var/log")
    const { result } = render({ visible: true })
    await waitFor(() => expect(result.current.listing?.currentPath).toBe("/var/log"))

    mocks.unlistable.add("/root")
    act(() => setTabRemoteCwd(TAB, "/root"))
    await waitFor(() => expect(mocks.loaded).toEqual(["/var/log", "/root"]))
    expect(result.current.listing?.currentPath).toBe("/var/log")
  })

  it("goes to the terminal's directory when following is turned on", async () => {
    setTabRemoteCwd(TAB, "/var/log")
    const { result } = render({ visible: true })
    await waitFor(() => expect(result.current.listing?.currentPath).toBe("/last"))

    act(() => result.current.toggleFollow())
    expect(result.current.follows).toBe(true)
    await waitFor(() => expect(result.current.listing?.currentPath).toBe("/var/log"))

    act(() => result.current.toggleFollow())
    act(() => setTabRemoteCwd(TAB, "/etc"))
    expect(result.current.listing?.currentPath).toBe("/var/log")
  })
})
