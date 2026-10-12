import React, { useCallback, useEffect } from "react"

import type {
  SftpDirectoryListing,
  SftpContextMenuState,
  SftpDialogState,
} from "@/components/SftpDrawer/types"
import { hasOpenShortcutBlockingLayer, useKeymap } from "@/contexts/KeymapContext"
import { isImeKeyEvent } from "@/lib/ime"

interface UseSftpSelectionModeOptions {
  visible: boolean
  isGlobalShortcutTarget: boolean
  dialogType: SftpDialogState["type"]
  filteredListing: SftpDirectoryListing | null
  isLoading: boolean
  error: string | null
  isSelectionMode: boolean
  setIsSelectionMode: React.Dispatch<React.SetStateAction<boolean>>
  setSelectedPaths: React.Dispatch<React.SetStateAction<string[]>>
  setActivePath: React.Dispatch<React.SetStateAction<string | null>>
  contextMenu: SftpContextMenuState | null
  setContextMenu: React.Dispatch<React.SetStateAction<SftpContextMenuState | null>>
  drawerRef: React.RefObject<HTMLDivElement | null>
}

/**
 * Selection mode: entering and leaving it, selecting every entry by
 * shortcut, and leaving it with Escape.
 */
export function useSftpSelectionMode({
  visible,
  isGlobalShortcutTarget,
  dialogType,
  filteredListing,
  isLoading,
  error,
  isSelectionMode,
  setIsSelectionMode,
  setSelectedPaths,
  setActivePath,
  contextMenu,
  setContextMenu,
  drawerRef,
}: UseSftpSelectionModeOptions) {
  const { registerHandler } = useKeymap()

  useEffect(() => {
    return registerHandler("sftp.selectAll", () => {
      if (!visible || !isGlobalShortcutTarget || dialogType !== "none") return false
      const entries = filteredListing?.entries ?? []
      if (entries.length === 0 || isLoading || error) {
        return false
      }

      setIsSelectionMode(true)
      setSelectedPaths(entries.map((entry) => entry.path))
      setActivePath((current) =>
        current && entries.some((entry) => entry.path === current) ? current : entries[0].path
      )
      setContextMenu(null)
    })
  }, [
    dialogType,
    error,
    filteredListing?.entries,
    isGlobalShortcutTarget,
    isLoading,
    registerHandler,
    visible,
    setActivePath,
    setContextMenu,
    setIsSelectionMode,
    setSelectedPaths,
  ])

  const enterSelectionMode = useCallback(() => {
    setIsSelectionMode(true)
  }, [setIsSelectionMode])

  const exitSelectionMode = useCallback(() => {
    setIsSelectionMode(false)
    setSelectedPaths([])
    setContextMenu(null)
  }, [setContextMenu, setIsSelectionMode, setSelectedPaths])

  // Escape leaves selection mode wherever focus is: WebKit does not focus
  // buttons on click and a drag can leave focus on the covered terminal, so a
  // drawer-level key handler only fires some of the time. Captured on window
  // so the terminal does not also receive it.
  useEffect(() => {
    if (!visible || !isGlobalShortcutTarget || !isSelectionMode) {
      return
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented || isImeKeyEvent(event)) {
        return
      }

      if (dialogType !== "none" || contextMenu !== null || hasOpenShortcutBlockingLayer(document)) {
        return
      }

      // The filter and path fields inside the drawer close themselves first.
      const target = event.target as HTMLElement | null
      if (
        target &&
        drawerRef.current?.contains(target) &&
        target.closest("input, textarea, [contenteditable='true']")
      ) {
        return
      }

      event.preventDefault()
      event.stopPropagation()
      exitSelectionMode()
    }

    window.addEventListener("keydown", handleKeyDown, true)
    return () => window.removeEventListener("keydown", handleKeyDown, true)
  }, [
    contextMenu,
    dialogType,
    exitSelectionMode,
    isGlobalShortcutTarget,
    isSelectionMode,
    visible,
    drawerRef,
  ])

  const toggleSelectionMode = useCallback(() => {
    if (isSelectionMode) {
      exitSelectionMode()
    } else {
      enterSelectionMode()
    }
  }, [enterSelectionMode, exitSelectionMode, isSelectionMode])

  return { enterSelectionMode, exitSelectionMode, toggleSelectionMode }
}
