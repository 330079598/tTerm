// @vitest-environment jsdom

import { cleanup, renderHook, waitFor } from "@testing-library/react"
import type { Terminal } from "@xterm/xterm"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { useTerminalFileDrop } from "@/components/TerminalTab/useTerminalFileDrop"
import type { TerminalTabProps } from "@/components/TerminalTab/types"

type DragDropEvent = {
  payload: { type: string; paths?: string[]; position?: { x: number; y: number } }
}

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

vi.mock("@/lib/uiPlatform", () => ({ getDetectedPlatform: () => "macos" }))

function drop(paths: string[]) {
  for (const handler of dropHandlers) {
    handler({ payload: { type: "drop", paths, position: { x: 10, y: 10 } } })
  }
}

function renderTerminal(connection: TerminalTabProps["connection"], isActive = true) {
  const surface = document.createElement("div")
  const inner = document.createElement("div")
  surface.append(inner)
  document.body.append(surface)
  const term = { focus: vi.fn(), paste: vi.fn() }
  renderHook(() =>
    useTerminalFileDrop({
      connection,
      isActive,
      shellName: "zsh",
      termRef: { current: term as unknown as Terminal },
      surfaceRef: { current: surface },
    })
  )
  return { term, inner }
}

describe("useTerminalFileDrop", () => {
  beforeEach(() => {
    document.elementFromPoint = vi.fn()
  })

  afterEach(() => {
    cleanup()
    dropHandlers.clear()
    document.body.replaceChildren()
  })

  it("types the dropped paths into a local terminal under the cursor", async () => {
    const { term, inner } = renderTerminal({ type: "terminal" })
    vi.mocked(document.elementFromPoint).mockReturnValue(inner)
    await waitFor(() => expect(dropHandlers.size).toBe(1))

    drop(["/Users/me/My File.txt", "/tmp/a"])

    expect(term.paste).toHaveBeenCalledWith("/Users/me/My\\ File.txt /tmp/a ")
    expect(term.focus).toHaveBeenCalled()
  })

  it("ignores a drop on another pane", async () => {
    const { term } = renderTerminal({ type: "terminal" })
    vi.mocked(document.elementFromPoint).mockReturnValue(document.body)
    await waitFor(() => expect(dropHandlers.size).toBe(1))

    drop(["/tmp/a"])

    expect(term.paste).not.toHaveBeenCalled()
  })

  it("leaves drops on SSH tabs and hidden tabs alone", async () => {
    renderTerminal({ type: "ssh" })
    renderTerminal({ type: "terminal" }, false)
    await Promise.resolve()

    expect(dropHandlers.size).toBe(0)
  })
})
