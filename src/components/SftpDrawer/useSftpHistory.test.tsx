// @vitest-environment jsdom

import { useCallback, useRef, useState } from "react"
import { createPortal } from "react-dom"
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import type { LoadSftpDirectory } from "@/components/SftpDrawer/types"
import { useSftpHistory, useSftpHistoryNavigation } from "@/components/SftpDrawer/useSftpHistory"

const { keymapHandlers } = vi.hoisted(() => ({
  keymapHandlers: new Map<string, () => void | boolean>(),
}))

vi.mock("@/contexts/KeymapContext", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/contexts/KeymapContext")>()),
  useKeymap: () => ({
    registerHandler: (actionId: string, handler: () => void | boolean) => {
      keymapHandlers.set(actionId, handler)
      return () => keymapHandlers.delete(actionId)
    },
  }),
}))

/** Remote directories and their parents; anything else fails to list. */
const REMOTE_PARENTS: Record<string, string | null> = {
  "/": null,
  "/home": "/",
  "/home/a": "/home",
  "/home/b": "/home",
}

interface HarnessProps {
  contextMenuOpen?: boolean
  dialogOpen?: boolean
  isGlobalShortcutTarget?: boolean
  listDirectory: (path: string) => Promise<{ currentPath: string; parentPath: string | null }>
  visible?: boolean
}

/** Mirrors how the drawer wires `loadDirectory` into the two history hooks. */
function Harness({
  contextMenuOpen = false,
  dialogOpen = false,
  isGlobalShortcutTarget = true,
  listDirectory,
  visible = true,
}: HarnessProps) {
  const drawerRef = useRef<HTMLDivElement>(null)
  const [listing, setListing] = useState<{ currentPath: string; parentPath: string | null }>()
  const [isLoading, setIsLoading] = useState(false)
  const { history, recordFailed, recordLoaded } = useSftpHistory()

  const loadDirectory = useCallback<LoadSftpDirectory>(
    async (path, options) => {
      setIsLoading(true)
      try {
        const nextListing = await listDirectory(path ?? "/home")
        setListing(nextListing)
        recordLoaded(nextListing.currentPath, options?.historyIndex)
      } catch {
        recordFailed(options?.historyIndex)
      } finally {
        setIsLoading(false)
      }
    },
    [listDirectory, recordFailed, recordLoaded]
  )

  const navigation = useSftpHistoryNavigation({
    contextMenuOpen,
    dialogOpen,
    drawerRef,
    history,
    isGlobalShortcutTarget,
    isLoading,
    loadDirectory,
    parentPath: listing?.parentPath ?? null,
    visible,
  })

  return (
    <div
      ref={drawerRef}
      data-testid="drawer"
      onMouseDown={navigation.handleSideButton}
      onMouseUp={navigation.handleSideButton}
    >
      <span data-testid="path">{listing?.currentPath ?? ""}</span>
      <span data-testid="can-back">{String(navigation.canGoBack)}</span>
      <span data-testid="can-forward">{String(navigation.canGoForward)}</span>
      <button type="button" data-testid="open-home" onClick={() => void loadDirectory(null)} />
      <button type="button" data-testid="open-a" onClick={() => void loadDirectory("/home/a")} />
      <button type="button" data-testid="open-b" onClick={() => void loadDirectory("/home/b")} />
      {createPortal(<div data-testid="portaled" />, document.body)}
    </div>
  )
}

function createRemote(missing = new Set<string>()) {
  return vi.fn(async (path: string) => {
    if (missing.has(path) || !(path in REMOTE_PARENTS)) {
      throw new Error(`No such file: ${path}`)
    }
    return { currentPath: path, parentPath: REMOTE_PARENTS[path] }
  })
}

async function renderHarness(props: HarnessProps) {
  const view = render(<Harness {...props} />)
  // The drawer lists its home directory first, then the user opens /home/a and /home/b.
  await act(async () => {
    fireEvent.click(screen.getByTestId("open-home"))
  })
  await act(async () => {
    fireEvent.click(screen.getByTestId("open-a"))
  })
  await act(async () => {
    fireEvent.click(screen.getByTestId("open-b"))
  })
  return view
}

async function runShortcut(actionId: string) {
  let result: void | boolean = undefined
  await act(async () => {
    result = keymapHandlers.get(actionId)?.()
  })
  return result
}

async function sideClick(target: Element, button: 3 | 4) {
  const down = new MouseEvent("mousedown", { bubbles: true, cancelable: true, button })
  const up = new MouseEvent("mouseup", { bubbles: true, cancelable: true, button })
  await act(async () => {
    target.dispatchEvent(down)
  })
  await act(async () => {
    target.dispatchEvent(up)
  })
  return { down, up }
}

const currentPath = () => screen.getByTestId("path").textContent

describe("useSftpHistoryNavigation", () => {
  afterEach(() => {
    cleanup()
    keymapHandlers.clear()
    document.querySelectorAll('[role="menu"]').forEach((menu) => menu.remove())
  })

  it("walks back and forward through visited directories with the shortcuts", async () => {
    const listDirectory = createRemote()
    await renderHarness({ listDirectory })
    expect(currentPath()).toBe("/home/b")

    expect(await runShortcut("sftp.back")).toBe(true)
    expect(listDirectory).toHaveBeenLastCalledWith("/home/a")
    expect(currentPath()).toBe("/home/a")
    expect(screen.getByTestId("can-forward").textContent).toBe("true")

    await runShortcut("sftp.forward")
    expect(currentPath()).toBe("/home/b")
    expect(screen.getByTestId("can-forward").textContent).toBe("false")
  })

  it("goes to the parent directory and records it as a new visit", async () => {
    const listDirectory = createRemote()
    await renderHarness({ listDirectory })

    expect(await runShortcut("sftp.up")).toBe(true)
    expect(currentPath()).toBe("/home")
    expect(screen.getByTestId("can-forward").textContent).toBe("false")

    await runShortcut("sftp.back")
    expect(currentPath()).toBe("/home/b")
  })

  it("drops a back target that no longer lists and skips it next time", async () => {
    const missing = new Set<string>()
    const listDirectory = createRemote(missing)
    await renderHarness({ listDirectory })
    missing.add("/home/a")

    await runShortcut("sftp.back")
    expect(listDirectory).toHaveBeenLastCalledWith("/home/a")
    expect(currentPath()).toBe("/home/b")

    await runShortcut("sftp.back")
    expect(listDirectory).toHaveBeenLastCalledWith("/home")
    expect(currentPath()).toBe("/home")
    expect(screen.getByTestId("can-back").textContent).toBe("false")
  })

  it.each([
    ["a dialog is open", { dialogOpen: true }],
    ["the drawer is hidden", { visible: false }],
    ["another panel owns the shortcuts", { isGlobalShortcutTarget: false }],
  ])("leaves the shortcuts to others while %s", async (_, props) => {
    const listDirectory = createRemote()
    await renderHarness({ listDirectory, ...props })
    listDirectory.mockClear()

    expect(await runShortcut("sftp.back")).toBe(false)
    expect(await runShortcut("sftp.up")).toBe(false)
    expect(listDirectory).not.toHaveBeenCalled()
  })

  it("navigates with the mouse side buttons and cancels the WebView navigation", async () => {
    const listDirectory = createRemote()
    await renderHarness({ listDirectory })

    const back = await sideClick(screen.getByTestId("path"), 3)
    expect(back.down.defaultPrevented).toBe(true)
    expect(back.up.defaultPrevented).toBe(true)
    expect(currentPath()).toBe("/home/a")

    await sideClick(screen.getByTestId("path"), 4)
    expect(currentPath()).toBe("/home/b")
  })

  it("ignores side buttons on portaled content", async () => {
    const listDirectory = createRemote()
    await renderHarness({ listDirectory })
    listDirectory.mockClear()

    const { up } = await sideClick(screen.getByTestId("portaled"), 3)
    expect(up.defaultPrevented).toBe(true)
    expect(listDirectory).not.toHaveBeenCalled()
  })

  it("does not navigate on a side click while the entry context menu is open", async () => {
    const listDirectory = createRemote()
    await renderHarness({ listDirectory, contextMenuOpen: true })
    listDirectory.mockClear()

    await sideClick(screen.getByTestId("path"), 3)
    expect(listDirectory).not.toHaveBeenCalled()
  })

  it("treats a side click that dismisses a menu as a dismissal only", async () => {
    const listDirectory = createRemote()
    await renderHarness({ listDirectory })
    listDirectory.mockClear()
    const menu = document.createElement("div")
    menu.setAttribute("role", "menu")
    document.body.appendChild(menu)
    // Like the real menus, it closes itself on the press.
    const closeMenu = () => menu.remove()
    document.addEventListener("mousedown", closeMenu)

    await sideClick(screen.getByTestId("path"), 3)
    document.removeEventListener("mousedown", closeMenu)
    expect(menu.isConnected).toBe(false)
    expect(listDirectory).not.toHaveBeenCalled()

    await sideClick(screen.getByTestId("path"), 3)
    expect(currentPath()).toBe("/home/a")
  })

  it("needs the press and the release on the drawer", async () => {
    const listDirectory = createRemote()
    await renderHarness({ listDirectory })
    listDirectory.mockClear()

    await act(async () => {
      screen
        .getByTestId("path")
        .dispatchEvent(new MouseEvent("mouseup", { bubbles: true, cancelable: true, button: 3 }))
    })
    expect(listDirectory).not.toHaveBeenCalled()
  })
})
