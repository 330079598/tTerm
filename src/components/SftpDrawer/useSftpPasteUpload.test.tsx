// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { useSftpPasteUpload } from "@/components/SftpDrawer/useSftpPasteUpload"

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))

vi.mock("@tauri-apps/api/core", () => ({ invoke }))

const listing = { currentPath: "/remote", entries: [] }

function dispatchPasteShortcut(target: EventTarget = window) {
  const event = new KeyboardEvent("keydown", {
    bubbles: true,
    cancelable: true,
    key: "v",
    metaKey: true,
  })
  target.dispatchEvent(event)
  return event
}

describe("useSftpPasteUpload", () => {
  afterEach(() => {
    cleanup()
    vi.clearAllMocks()
  })

  it("intercepts Cmd+V synchronously and uploads clipboard file paths", async () => {
    invoke.mockResolvedValue(["/tmp/copied-file.txt"])
    const uploadPaths = vi.fn().mockResolvedValue(undefined)

    renderHook(() =>
      useSftpPasteUpload({
        enabled: true,
        listing,
        setError: vi.fn(),
        uploadPaths,
        visible: true,
      })
    )

    const event = dispatchPasteShortcut()

    expect(event.defaultPrevented).toBe(true)
    expect(invoke).toHaveBeenCalledWith("read_clipboard_file_paths")
    await waitFor(() => expect(uploadPaths).toHaveBeenCalledWith(["/tmp/copied-file.txt"]))
  })

  it("does not register the shortcut for an inactive panel", () => {
    renderHook(() =>
      useSftpPasteUpload({
        enabled: false,
        listing,
        setError: vi.fn(),
        uploadPaths: vi.fn(),
        visible: true,
      })
    )

    const event = dispatchPasteShortcut()

    expect(event.defaultPrevented).toBe(false)
    expect(invoke).not.toHaveBeenCalled()
  })

  it("leaves editable fields available for normal paste", () => {
    renderHook(() =>
      useSftpPasteUpload({
        enabled: true,
        listing,
        setError: vi.fn(),
        uploadPaths: vi.fn(),
        visible: true,
      })
    )
    const input = document.createElement("input")
    document.body.appendChild(input)

    const event = dispatchPasteShortcut(input)

    expect(event.defaultPrevented).toBe(false)
    expect(invoke).not.toHaveBeenCalled()
    input.remove()
  })

  it("shows backend clipboard errors", async () => {
    invoke.mockRejectedValue(new Error("clipboard unavailable"))
    const setError = vi.fn()

    renderHook(() =>
      useSftpPasteUpload({
        enabled: true,
        listing,
        setError,
        uploadPaths: vi.fn(),
        visible: true,
      })
    )

    act(() => {
      dispatchPasteShortcut()
    })

    await waitFor(() => expect(setError).toHaveBeenCalledWith("Error: clipboard unavailable"))
  })
})
