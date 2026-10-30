// @vitest-environment jsdom

import { cleanup, fireEvent, render } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { WorkspaceSashes } from "@/components/TTermApp/WorkspaceSashes"
import type { SashBox } from "@/lib/paneLayout"
import type { TabGroup } from "@/lib/tabGroups"

const group: TabGroup = {
  id: "group-1",
  activeTabId: "tab-1",
  root: {
    type: "split",
    orientation: "horizontal",
    sizes: [0.5, 0.5],
    children: [
      { type: "pane", tabId: "tab-1" },
      { type: "pane", tabId: "tab-2" },
    ],
  },
}

const sash: SashBox = {
  left: 400,
  top: 0,
  width: 5,
  height: 600,
  path: [],
  index: 0,
  orientation: "horizontal",
  room: 795,
}

// jsdom implements no pointer capture.
HTMLElement.prototype.setPointerCapture = () => {}

afterEach(cleanup)

describe("WorkspaceSashes", () => {
  it("marks the divider being dragged until the drag ends", () => {
    const resizeSplit = vi.fn()
    const { container } = render(
      <WorkspaceSashes
        group={group}
        sashes={[sash]}
        gap={5}
        minimum={{ width: 220, height: 140 }}
        resizeSplit={resizeSplit}
      />
    )
    const divider = container.querySelector(".workspace-sash") as HTMLElement

    fireEvent.pointerDown(divider, { button: 0, pointerId: 1, clientX: 402 })
    expect(divider.classList.contains("dragging")).toBe(true)
    expect(document.body.classList.contains("workspace-sash-resizing")).toBe(true)

    fireEvent.pointerUp(divider, { pointerId: 1 })
    expect(divider.classList.contains("dragging")).toBe(false)
    expect(document.body.classList.contains("workspace-sash-resizing")).toBe(false)
  })
})
