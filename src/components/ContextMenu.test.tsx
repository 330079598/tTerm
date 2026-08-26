// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { ContextMenu } from "@/components/ContextMenu"
import type { TabContextMenuAction } from "@/types/tab"

afterEach(cleanup)

describe("ContextMenu keyboard navigation", () => {
  const actions: TabContextMenuAction[] = [
    { label: "New tab", action: "new" },
    { separator: true, label: "", action: "" },
    { label: "Close tab", action: "close" },
    { label: "Close left", action: "close-left", disabled: true },
    { label: "Close right", action: "close-right" },
  ]

  it("focuses the correct item after a separator and skips disabled actions", () => {
    render(<ContextMenu x={10} y={10} actions={actions} onAction={vi.fn()} onClose={vi.fn()} />)

    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "New tab" }))

    fireEvent.keyDown(document, { key: "ArrowDown" })
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Close tab" }))

    fireEvent.keyDown(document, { key: "ArrowDown" })
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Close right" }))
  })

  it("executes the focused close action", () => {
    const onAction = vi.fn()
    render(<ContextMenu x={10} y={10} actions={actions} onAction={onAction} onClose={vi.fn()} />)

    fireEvent.keyDown(document, { key: "ArrowDown" })
    fireEvent.keyDown(screen.getByRole("menuitem", { name: "Close tab" }), { key: "Enter" })

    expect(onAction).toHaveBeenCalledOnce()
    expect(onAction).toHaveBeenCalledWith("close")
  })

  it("mounts the fixed-position surface under the document body", () => {
    render(<ContextMenu x={10} y={10} actions={actions} onAction={vi.fn()} onClose={vi.fn()} />)

    const menu = screen.getByRole("menu")
    expect(menu.parentElement?.parentElement).toBe(document.body)
  })
})
