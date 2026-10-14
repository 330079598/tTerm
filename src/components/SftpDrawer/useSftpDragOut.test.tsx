// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react"
import type React from "react"
import { afterEach, describe, expect, it, vi } from "vitest"

import type { DragOutResult } from "@/components/SftpDrawer/dragOut"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"
import { useSftpDragOut } from "@/components/SftpDrawer/useSftpDragOut"

const { startDragOut, supported } = vi.hoisted(() => ({
  startDragOut: vi.fn<(entries: SftpDirectoryEntry[]) => Promise<DragOutResult>>(),
  supported: { current: true },
}))

vi.mock("@/components/SftpDrawer/dragOut", () => ({
  startDragOut,
  supportsDragOut: () => supported.current,
}))

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

const file: SftpDirectoryEntry = {
  isDir: false,
  isSymlink: false,
  name: "a.log",
  path: "/remote/a.log",
  size: 10,
}

function renderDragOut() {
  const downloadEntriesToFolder = vi.fn().mockResolvedValue(undefined)
  const setError = vi.fn()
  const { result } = renderHook(() => useSftpDragOut({ downloadEntriesToFolder, setError }))
  return { downloadEntriesToFolder, result, setError }
}

function press(dragOut: NonNullable<ReturnType<typeof useSftpDragOut>>) {
  act(() => {
    dragOut.beginPress({ clientX: 10, clientY: 10 } as React.MouseEvent, [file])
  })
}

function moveTo(clientX: number, clientY: number) {
  act(() => {
    window.dispatchEvent(new MouseEvent("mousemove", { clientX, clientY }))
  })
}

describe("useSftpDragOut", () => {
  afterEach(() => {
    cleanup()
    supported.current = true
    vi.clearAllMocks()
  })

  it("downloads into the folder the rows were dropped on", async () => {
    startDragOut.mockResolvedValue({ kind: "dropped", directory: "/Users/me/Desktop" })
    const { downloadEntriesToFolder, result } = renderDragOut()

    press(result.current!)
    moveTo(30, 12)

    await waitFor(() =>
      expect(downloadEntriesToFolder).toHaveBeenCalledWith([file], "/Users/me/Desktop")
    )
    expect(startDragOut).toHaveBeenCalledTimes(1)
  })

  it("leaves a press that does not move as a click", () => {
    const { result } = renderDragOut()

    press(result.current!)
    moveTo(12, 11)
    act(() => {
      window.dispatchEvent(new MouseEvent("mouseup"))
    })
    moveTo(80, 80)

    expect(startDragOut).not.toHaveBeenCalled()
    expect(result.current!.startedRef.current).toBe(false)
  })

  it("explains a drop that landed on something other than a folder", async () => {
    startDragOut.mockResolvedValue({ kind: "unknownTarget" })
    const { downloadEntriesToFolder, result, setError } = renderDragOut()

    press(result.current!)
    moveTo(40, 40)

    await waitFor(() => expect(setError).toHaveBeenCalledWith("sftp.errors.dragOutUnknownTarget"))
    expect(downloadEntriesToFolder).not.toHaveBeenCalled()
  })

  it("is not offered where the platform cannot name the drop folder", () => {
    supported.current = false
    const { result } = renderDragOut()

    expect(result.current).toBeNull()
  })
})
