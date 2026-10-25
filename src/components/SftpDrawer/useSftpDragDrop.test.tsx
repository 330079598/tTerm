// @vitest-environment jsdom

import { cleanup, renderHook, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { useSftpDragDrop } from "@/components/SftpDrawer/useSftpDragDrop"

type DragDropEvent = {
  payload: { type: string; paths?: string[]; position?: { x: number; y: number } }
}
type Handler = (event: DragDropEvent) => void

const { dropHandlers, dragOutActive, registration } = vi.hoisted(() => ({
  dropHandlers: new Set<(event: DragDropEvent) => void>(),
  dragOutActive: { current: false },
  // While deferred, registrations wait here until the test lets them through,
  // as Tauri's do while their round trips are in flight.
  registration: { deferred: false, pending: [] as (() => void)[] },
}))

vi.mock("@/components/SftpDrawer/dragOut", () => ({
  isDragOutActive: () => dragOutActive.current,
}))

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onDragDropEvent: (handler: Handler) => {
      const register = () => {
        dropHandlers.add(handler)
        return () => dropHandlers.delete(handler)
      }
      if (!registration.deferred) return Promise.resolve(register())
      return new Promise((resolve) => registration.pending.push(() => resolve(register())))
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
    handler({ payload: { type: "drop", paths: [path], position: { x: 10, y: 10 } } })
  }
}

/** Mounts a drawer inside a workspace panel of its own, as each tab has. */
function renderDrawer(overrides: { isGlobalShortcutTarget: boolean; currentPath: string }) {
  const uploadPaths = vi.fn().mockResolvedValue(undefined)
  const panel = document.createElement("div")
  panel.dataset.workspacePanelId = overrides.currentPath
  const drawer = document.createElement("div")
  panel.append(drawer)
  document.body.append(panel)

  const { unmount } = renderHook(() =>
    useSftpDragDrop({
      drawerRef: { current: drawer },
      isGlobalShortcutTarget: overrides.isGlobalShortcutTarget,
      listing: { currentPath: overrides.currentPath, entries: [] },
      setError: vi.fn(),
      uploadPaths,
      visible: true,
    })
  )

  return { drawer, panel, unmount, uploadPaths }
}

describe("useSftpDragDrop", () => {
  beforeEach(() => {
    document.elementFromPoint = vi.fn().mockReturnValue(document.body)
  })

  afterEach(() => {
    cleanup()
    dropHandlers.clear()
    dragOutActive.current = false
    registration.deferred = false
    registration.pending = []
    document.body.replaceChildren()
    vi.clearAllMocks()
  })

  it("uploads a file dropped on its own pane", async () => {
    const { drawer, uploadPaths } = renderDrawer({
      isGlobalShortcutTarget: true,
      currentPath: "/home/stone/code",
    })
    vi.mocked(document.elementFromPoint).mockReturnValue(drawer)
    await waitFor(() => expect(dropHandlers.size).toBe(1))

    dropFile("/local/Zed-aarch64.dmg")

    await waitFor(() => expect(uploadPaths).toHaveBeenCalledWith(["/local/Zed-aarch64.dmg"]))
  })

  // Split panes show several drawers, and a file dragged in from another
  // window leaves the focus on the pane clicked last, so the pane under the
  // cursor must decide where the file goes.
  it("uploads to the pane under the cursor, not the focused one", async () => {
    const focused = renderDrawer({ isGlobalShortcutTarget: true, currentPath: "/home/ubuntu" })
    const target = renderDrawer({ isGlobalShortcutTarget: false, currentPath: "/root/k8s" })
    vi.mocked(document.elementFromPoint).mockReturnValue(target.panel)
    await waitFor(() => expect(dropHandlers.size).toBe(2))

    dropFile("/local/deploy.yaml")

    await waitFor(() => expect(target.uploadPaths).toHaveBeenCalledWith(["/local/deploy.yaml"]))
    expect(focused.uploadPaths).not.toHaveBeenCalled()
  })

  // Background tabs stay mounted with their drawer open, so a drop outside
  // every pane, such as on the tab bar, must reach only the focused drawer.
  it("gives a drop outside every pane to the focused drawer alone", async () => {
    const focused = renderDrawer({ isGlobalShortcutTarget: true, currentPath: "/home/stone/code" })
    const background = renderDrawer({ isGlobalShortcutTarget: false, currentPath: "/data/tafp" })
    await waitFor(() => expect(dropHandlers.size).toBe(2))

    dropFile("/local/Zed-aarch64.dmg")

    await waitFor(() =>
      expect(focused.uploadPaths).toHaveBeenCalledWith(["/local/Zed-aarch64.dmg"])
    )
    expect(background.uploadPaths).not.toHaveBeenCalled()
  })

  // A listener that arrived after its drawer let go of it would keep
  // uploading every drop into that drawer's directory, whichever tab is in
  // front.
  it("stops a listener that finishes registering after the drawer let go of it", async () => {
    registration.deferred = true
    const { drawer, unmount, uploadPaths } = renderDrawer({
      isGlobalShortcutTarget: true,
      currentPath: "/home/ubuntu",
    })
    vi.mocked(document.elementFromPoint).mockReturnValue(drawer)
    unmount()

    for (const resolve of registration.pending) resolve()
    await new Promise((resolve) => setTimeout(resolve, 0))
    dropFile("/local/deploy.yaml")

    expect(dropHandlers.size).toBe(0)
    expect(uploadPaths).not.toHaveBeenCalled()
  })

  // On Windows a drag out carries placeholder files, and letting go over
  // tTerm's own window would otherwise upload them.
  it("ignores a window drop while tTerm drags files out", async () => {
    const { drawer, uploadPaths } = renderDrawer({
      isGlobalShortcutTarget: true,
      currentPath: "/home/stone/code",
    })
    vi.mocked(document.elementFromPoint).mockReturnValue(drawer)
    await waitFor(() => expect(dropHandlers.size).toBe(1))
    dragOutActive.current = true

    dropFile("C:\\Temp\\tTerm-drag-1\\tTerm-drop-1-0.log")

    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(uploadPaths).not.toHaveBeenCalled()
  })
})
