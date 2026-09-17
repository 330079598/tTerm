// @vitest-environment jsdom

import { cleanup, renderHook, waitFor } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { useSftpDragDrop } from "@/components/SftpDrawer/useSftpDragDrop"

type DragDropEvent = { payload: { type: string; paths?: string[] } }

const { dropHandlers } = vi.hoisted(() => ({
  dropHandlers: new Set<(event: DragDropEvent) => void>(),
}))

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onDragDropEvent: (handler: (event: DragDropEvent) => void) => {
      dropHandlers.add(handler)
      return Promise.resolve(() => dropHandlers.delete(handler))
    },
  }),
}))

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (_key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? "",
  }),
}))

function dropFile(path: string) {
  for (const handler of dropHandlers) {
    handler({ payload: { type: "drop", paths: [path] } })
  }
}

function renderDrawer(overrides: { isGlobalShortcutTarget: boolean; currentPath: string }) {
  const uploadPaths = vi.fn().mockResolvedValue(undefined)

  renderHook(() =>
    useSftpDragDrop({
      isGlobalShortcutTarget: overrides.isGlobalShortcutTarget,
      listing: { currentPath: overrides.currentPath, entries: [] },
      loadDirectory: vi.fn().mockResolvedValue(undefined),
      setError: vi.fn(),
      uploadPaths,
      visible: true,
    })
  )

  return uploadPaths
}

describe("useSftpDragDrop", () => {
  afterEach(() => {
    cleanup()
    dropHandlers.clear()
    vi.clearAllMocks()
  })

  it("uploads a dropped file from the focused tab's drawer", async () => {
    const uploadPaths = renderDrawer({
      isGlobalShortcutTarget: true,
      currentPath: "/home/stone/code",
    })
    await waitFor(() => expect(dropHandlers.size).toBe(1))

    dropFile("/local/Zed-aarch64.dmg")

    await waitFor(() => expect(uploadPaths).toHaveBeenCalledWith(["/local/Zed-aarch64.dmg"]))
  })

  // A drop is delivered to the window, and background tabs stay mounted with
  // their drawer open, so an unfocused drawer must ignore it: otherwise one
  // drop uploads the file to every open drawer's own current directory.
  it("ignores a window drop in an unfocused tab's drawer", async () => {
    const focused = renderDrawer({ isGlobalShortcutTarget: true, currentPath: "/home/stone/code" })
    const background = renderDrawer({ isGlobalShortcutTarget: false, currentPath: "/data/tafp" })
    await waitFor(() => expect(dropHandlers.size).toBe(1))

    dropFile("/local/Zed-aarch64.dmg")

    await waitFor(() => expect(focused).toHaveBeenCalledWith(["/local/Zed-aarch64.dmg"]))
    expect(background).not.toHaveBeenCalled()
  })
})
