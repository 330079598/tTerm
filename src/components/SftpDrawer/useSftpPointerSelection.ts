import { useCallback, useEffect, useRef, useState } from "react"

import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

interface UseSftpPointerSelectionParams {
  entries: SftpDirectoryEntry[]
  enterSelectionMode: () => void
  exitSelectionMode: () => void
  handleActivateEntry: (path: string) => void
  handleSelectRange: (anchorPath: string, currentPath: string) => void
}

interface UseSftpPointerSelectionReturn {
  /** Set once a press sweeps across rows, so the click ending it is ignored. */
  pointerMovedRef: React.RefObject<boolean>
  rowRefs: React.RefObject<Map<string, HTMLDivElement>>
  /** Starts a sweep from the empty space below the rows. */
  startBlankSweep: () => void
  /** Starts a sweep from the row at `path`. */
  startRowSweep: (path: string) => void
}

/** Selects the rows a held mouse button sweeps across. */
export function useSftpPointerSelection({
  entries,
  enterSelectionMode,
  exitSelectionMode,
  handleActivateEntry,
  handleSelectRange,
}: UseSftpPointerSelectionParams): UseSftpPointerSelectionReturn {
  const [isPointerSelecting, setIsPointerSelecting] = useState(false)
  const pointerAnchorRef = useRef<string | null>(null)
  const pointerMovedRef = useRef(false)
  // Set when a press starts on the empty space below the rows, so a plain
  // click there (no rows swept) can clear the selection on release.
  const pointerFromBlankRef = useRef(false)
  const rowRefs = useRef(new Map<string, HTMLDivElement>())

  const updateSelectionFromPointer = useCallback(
    (clientY: number) => {
      const hoveredEntry = entries.find((entry) => {
        const row = rowRefs.current.get(entry.path)
        if (!row) {
          return false
        }

        const rect = row.getBoundingClientRect()
        return clientY >= rect.top && clientY <= rect.bottom
      })

      if (!hoveredEntry) {
        return
      }

      // Sweeping a row starts a multi-selection from any mode, so no toolbar
      // toggle is needed first. A press that stays on one row is still a click.
      if (!pointerAnchorRef.current) {
        pointerAnchorRef.current = hoveredEntry.path
        pointerMovedRef.current = true
        enterSelectionMode()
        handleActivateEntry(hoveredEntry.path)
        handleSelectRange(hoveredEntry.path, hoveredEntry.path)
        return
      }

      if (pointerAnchorRef.current === hoveredEntry.path) {
        return
      }

      pointerMovedRef.current = true
      enterSelectionMode()
      handleSelectRange(pointerAnchorRef.current, hoveredEntry.path)
    },
    [enterSelectionMode, entries, handleActivateEntry, handleSelectRange]
  )

  useEffect(() => {
    if (!isPointerSelecting) {
      return
    }

    const handleMouseMove = (event: MouseEvent) => {
      event.preventDefault()
      updateSelectionFromPointer(event.clientY)
    }

    const stopSelection = () => {
      if (pointerFromBlankRef.current && !pointerMovedRef.current) {
        exitSelectionMode()
      }

      setIsPointerSelecting(false)
      pointerAnchorRef.current = null
      pointerFromBlankRef.current = false
      document.body.classList.remove("sftp-no-select")
      window.setTimeout(() => {
        pointerMovedRef.current = false
      }, 0)
    }

    document.body.classList.add("sftp-no-select")
    window.addEventListener("mousemove", handleMouseMove)
    window.addEventListener("mouseup", stopSelection)
    return () => {
      document.body.classList.remove("sftp-no-select")
      window.removeEventListener("mousemove", handleMouseMove)
      window.removeEventListener("mouseup", stopSelection)
    }
  }, [exitSelectionMode, isPointerSelecting, updateSelectionFromPointer])

  const startBlankSweep = useCallback(() => {
    pointerAnchorRef.current = null
    pointerFromBlankRef.current = true
    pointerMovedRef.current = false
    setIsPointerSelecting(true)
  }, [])

  const startRowSweep = useCallback((path: string) => {
    pointerAnchorRef.current = path
    pointerMovedRef.current = false
    pointerFromBlankRef.current = false
    setIsPointerSelecting(true)
  }, [])

  return { pointerMovedRef, rowRefs, startBlankSweep, startRowSweep }
}
