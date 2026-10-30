import React, { useCallback, useEffect, useRef } from "react"

import {
  dragSash,
  getSplitAt,
  minimumExtent,
  type PaneMinimum,
  type SashBox,
} from "@/lib/paneLayout"
import type { TabGroup } from "@/lib/tabGroups"

interface SashDrag {
  pointerId: number
  /** The divider being dragged, which shows its grip until the drag ends. */
  element: HTMLElement
  sash: SashBox
  start: number
  sizes: number[]
  minimums: number[]
}

interface WorkspaceSashesProps {
  group: TabGroup
  sashes: SashBox[]
  gap: number
  minimum: PaneMinimum
  resizeSplit: (groupId: string, path: number[], sizes: number[]) => void
}

const RESIZING_CLASSES = [
  "workspace-sash-resizing",
  "workspace-sash-resizing-horizontal",
  "workspace-sash-resizing-vertical",
]

/** The dividers between the panes of the tab in sight, dragged to share the room out again. */
export function WorkspaceSashes({
  group,
  sashes,
  gap,
  minimum,
  resizeSplit,
}: WorkspaceSashesProps) {
  const dragRef = useRef<SashDrag | null>(null)

  const stopResizing = useCallback(() => {
    dragRef.current?.element.classList.remove("dragging")
    dragRef.current = null
    document.body.classList.remove(...RESIZING_CLASSES)
  }, [])

  useEffect(() => stopResizing, [stopResizing])

  const handlePointerDown = (event: React.PointerEvent<HTMLDivElement>, sash: SashBox) => {
    const split = getSplitAt(group.root, sash.path)
    if (event.button !== 0 || !split) return
    event.preventDefault()
    event.currentTarget.setPointerCapture(event.pointerId)
    window.getSelection()?.removeAllRanges()
    event.currentTarget.classList.add("dragging")
    dragRef.current = {
      pointerId: event.pointerId,
      element: event.currentTarget,
      sash,
      start: sash.orientation === "horizontal" ? event.clientX : event.clientY,
      sizes: split.sizes,
      minimums: split.children.map((child) =>
        minimumExtent(child, split.orientation, minimum, gap)
      ),
    }
    document.body.classList.add(
      "workspace-sash-resizing",
      `workspace-sash-resizing-${sash.orientation}`
    )
  }

  const handlePointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current
    if (!drag || drag.pointerId !== event.pointerId) return
    const position = drag.sash.orientation === "horizontal" ? event.clientX : event.clientY
    resizeSplit(
      group.id,
      drag.sash.path,
      dragSash(drag.sizes, drag.sash.index, position - drag.start, drag.sash.room, drag.minimums)
    )
  }

  return (
    <>
      {sashes.map((sash) => (
        <div
          key={`${sash.path.join(".")}:${sash.index}`}
          className={`workspace-sash ${sash.orientation}`}
          style={{ left: sash.left, top: sash.top, width: sash.width, height: sash.height }}
          role="separator"
          aria-orientation={sash.orientation === "horizontal" ? "vertical" : "horizontal"}
          onPointerDown={(event) => handlePointerDown(event, sash)}
          onPointerMove={handlePointerMove}
          onPointerUp={stopResizing}
          onPointerCancel={stopResizing}
          onLostPointerCapture={stopResizing}
        />
      ))}
    </>
  )
}
