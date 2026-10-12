import { useEffect } from "react"

/**
 * Marks the page while a workspace sash is resized or a group is dragged,
 * so the styles can show it and text is not selected along the way.
 */
export function useDockInteractionClasses(dockRootRef: React.RefObject<HTMLDivElement | null>) {
  useEffect(() => {
    const root = dockRootRef.current
    if (!root) {
      return
    }

    const stopResizing = () => {
      document.body.classList.remove(
        "workspace-sash-resizing",
        "workspace-sash-resizing-horizontal",
        "workspace-sash-resizing-vertical"
      )
    }

    const stopGroupDrag = () => {
      document.body.classList.remove("workspace-group-dragging")
    }

    const startResizing = (event: PointerEvent) => {
      if (event.button !== 0 || !(event.target instanceof Element)) {
        return
      }

      const sash = event.target.closest(".dv-sash")
      if (!sash || !root.contains(sash)) {
        return
      }

      const splitView = sash.closest(".dv-split-view-container")
      document.body.classList.add("workspace-sash-resizing")
      document.body.classList.toggle(
        "workspace-sash-resizing-horizontal",
        splitView?.classList.contains("dv-horizontal") ?? false
      )
      document.body.classList.toggle(
        "workspace-sash-resizing-vertical",
        splitView?.classList.contains("dv-vertical") ?? false
      )
      window.getSelection()?.removeAllRanges()
      event.preventDefault()
    }

    const startGroupDrag = (target: EventTarget | null) => {
      if (!(target instanceof Element)) {
        return
      }

      const dragHandle = target.closest(".dv-tab, .dv-draggable")
      if (!dragHandle || !root.contains(dragHandle)) {
        return
      }

      document.body.classList.add("workspace-group-dragging")
      window.getSelection()?.removeAllRanges()
    }

    const handleGroupPointerDown = (event: PointerEvent) => {
      if (event.button === 0) {
        startGroupDrag(event.target)
      }
    }

    const handleNativeDragStart = (event: DragEvent) => {
      startGroupDrag(event.target)
    }

    root.addEventListener("pointerdown", startResizing, true)
    root.addEventListener("pointerdown", handleGroupPointerDown, true)
    root.addEventListener("dragstart", handleNativeDragStart, true)
    document.addEventListener("pointerup", stopResizing)
    document.addEventListener("pointercancel", stopResizing)
    document.addEventListener("pointerup", stopGroupDrag)
    document.addEventListener("pointercancel", stopGroupDrag)
    document.addEventListener("dragend", stopGroupDrag)
    document.addEventListener("drop", stopGroupDrag)
    window.addEventListener("blur", stopResizing)
    window.addEventListener("blur", stopGroupDrag)

    return () => {
      root.removeEventListener("pointerdown", startResizing, true)
      root.removeEventListener("pointerdown", handleGroupPointerDown, true)
      root.removeEventListener("dragstart", handleNativeDragStart, true)
      document.removeEventListener("pointerup", stopResizing)
      document.removeEventListener("pointercancel", stopResizing)
      document.removeEventListener("pointerup", stopGroupDrag)
      document.removeEventListener("pointercancel", stopGroupDrag)
      document.removeEventListener("dragend", stopGroupDrag)
      document.removeEventListener("drop", stopGroupDrag)
      window.removeEventListener("blur", stopResizing)
      window.removeEventListener("blur", stopGroupDrag)
      stopResizing()
      stopGroupDrag()
    }
  }, [dockRootRef])
}
