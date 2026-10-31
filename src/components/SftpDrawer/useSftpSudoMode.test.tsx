// @vitest-environment jsdom

import { act, renderHook, waitFor } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import type { LoadSftpDirectory } from "@/components/SftpDrawer/types"
import { useReloadOnSudoSwitch, useSftpSudoMode } from "@/components/SftpDrawer/useSftpSudoMode"

const connection = { type: "ssh" as const, host: "web01", username: "alice" }

/** The drawer's two hooks together, with `loadDirectory` answering as told. */
function renderDrawer(loadDirectory: LoadSftpDirectory, currentPath: string | null = "/etc") {
  return renderHook(() => {
    const mode = useSftpSudoMode(connection)
    useReloadOnSudoSwitch({
      sudoMode: mode.sudoMode,
      setSudoMode: mode.setSudoMode,
      loadDirectory,
      currentPath,
    })
    return mode
  })
}

describe("useSftpSudoMode", () => {
  it("marks the drawer's requests for root only while sudo mode is on", () => {
    const { result } = renderDrawer(vi.fn(async () => undefined))
    expect(result.current.connection).toBe(connection)

    act(() => result.current.setSudoMode(true))
    expect(result.current.connection).toEqual({ ...connection, sftpSudo: true })

    act(() => result.current.setSudoMode(false))
    expect(result.current.connection).toBe(connection)
  })

  it("loads the folder in view again as root, and not when merely rendered", async () => {
    const loadDirectory = vi.fn<LoadSftpDirectory>(async () => undefined)
    const { result, rerender } = renderDrawer(loadDirectory)
    rerender()
    expect(loadDirectory).not.toHaveBeenCalled()

    act(() => result.current.setSudoMode(true))
    await waitFor(() => expect(loadDirectory).toHaveBeenCalledWith("/etc", { throwOnError: true }))
    expect(loadDirectory).toHaveBeenCalledTimes(1)
  })

  it("goes back to the user, keeping the listing, when sudo refuses", async () => {
    const loadDirectory = vi.fn<LoadSftpDirectory>(async () => {
      throw new Error("sudo did not accept the password")
    })
    const { result } = renderDrawer(loadDirectory)

    act(() => result.current.setSudoMode(true))
    await waitFor(() => expect(result.current.sudoMode).toBe(false))
    // The failed switch is not followed by a load as the user, which would
    // clear the error that says why.
    expect(loadDirectory).toHaveBeenCalledTimes(1)
  })

  it("leaves a folder only root could read for the first folder", async () => {
    const loadDirectory = vi.fn<LoadSftpDirectory>(async () => undefined)
    const { result } = renderDrawer(loadDirectory, "/root")
    act(() => result.current.setSudoMode(true))
    await waitFor(() => expect(loadDirectory).toHaveBeenCalledTimes(1))

    loadDirectory.mockImplementationOnce(async () => {
      throw new Error("Permission denied")
    })
    act(() => result.current.setSudoMode(false))
    await waitFor(() => expect(loadDirectory).toHaveBeenLastCalledWith(null))
    expect(loadDirectory).toHaveBeenNthCalledWith(2, "/root", { throwOnError: true })
  })
})
