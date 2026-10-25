import { useCallback, useEffect, useRef, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useTranslation } from "react-i18next"

import { isDragOutActive } from "@/components/SftpDrawer/dragOut"
import type { SftpDirectoryListing } from "@/components/SftpDrawer/types"
import { useStableRef } from "@/hooks/useStableRef"

interface UseSftpDragDropParams {
  drawerRef: React.RefObject<HTMLDivElement | null>
  isGlobalShortcutTarget: boolean
  listing: SftpDirectoryListing | null
  setError: React.Dispatch<React.SetStateAction<string | null>>
  uploadPaths: (paths: string[]) => Promise<void>
  visible: boolean
}

interface UseSftpDragDropReturn {
  handleDragEnter: (event: React.DragEvent<HTMLDivElement>) => void
  handleDragLeave: (event: React.DragEvent<HTMLDivElement>) => void
  handleDragOver: (event: React.DragEvent<HTMLDivElement>) => void
  handleDrop: (event: React.DragEvent<HTMLDivElement>) => Promise<void>
  isDragActive: boolean
}

const DROP_DEDUP_WINDOW_MS = 750
const PANEL_SELECTOR = "[data-workspace-panel-id]"

/** The workspace panel under a point of a window drag, given in physical pixels. */
function panelAt(position: { x: number; y: number }): Element | null {
  const ratio = window.devicePixelRatio || 1
  return (
    document.elementFromPoint(position.x / ratio, position.y / ratio)?.closest(PANEL_SELECTOR) ??
    null
  )
}

export function useSftpDragDrop({
  drawerRef,
  isGlobalShortcutTarget,
  listing,
  setError,
  uploadPaths,
  visible,
}: UseSftpDragDropParams): UseSftpDragDropReturn {
  const { t } = useTranslation()
  const [isDragActive, setIsDragActive] = useState(false)
  const dragCounterRef = useRef(0)
  const lastDropRef = useRef<{ signature: string; timestamp: number } | null>(null)

  const handleUploadPaths = useCallback(
    async (paths: string[]) => {
      if (!listing) {
        setError(t("sftp.errors.notReady", { defaultValue: "SFTP not ready" }))
        return
      }

      const validPaths = paths.filter((path) => typeof path === "string" && path.length > 0)
      if (validPaths.length === 0) {
        setError(
          t("sftp.errors.noValidFiles", {
            defaultValue: "No valid files to upload",
          })
        )
        return
      }

      const signature = [...validPaths].sort().join("\0")
      const now = Date.now()
      const lastDrop = lastDropRef.current
      if (
        lastDrop &&
        lastDrop.signature === signature &&
        now - lastDrop.timestamp < DROP_DEDUP_WINDOW_MS
      ) {
        console.log("Skipping duplicate drop upload", validPaths)
        return
      }
      lastDropRef.current = { signature, timestamp: now }

      await uploadPaths(validPaths)
    },
    [listing, setError, t, uploadPaths]
  )

  const isShortcutTargetRef = useStableRef(isGlobalShortcutTarget)
  const handleUploadPathsRef = useStableRef(handleUploadPaths)

  useEffect(() => {
    if (!visible) {
      return
    }

    // The OS delivers a drop to the window, not to a tab. Background tabs
    // stay mounted with their drawer open, and split panes show several
    // drawers at once, so only the drawer in the pane under the cursor takes
    // it. Focus can't decide: a file dragged in from another window, or
    // another monitor, leaves it on whichever pane was clicked last. A drop
    // outside every pane, such as on the tab bar, goes to the focused one.
    const ownsPoint = (position: { x: number; y: number }) => {
      const panel = panelAt(position)
      if (!panel) return isShortcutTargetRef.current
      return panel === drawerRef.current?.closest(PANEL_SELECTOR)
    }

    // Registering takes several round trips, so the effect can be cleaned
    // up before it resolves; the listener must then be stopped as soon as it
    // arrives, or it outlives the drawer state it was made for and keeps
    // uploading drops into that tab's directory.
    let disposed = false
    let unlisten: (() => void) | undefined
    getCurrentWindow()
      .onDragDropEvent((event) => {
        if (isDragOutActive()) {
          return
        }

        const { payload } = event
        if (payload.type === "enter" || payload.type === "over") {
          setIsDragActive(ownsPoint(payload.position))
        } else if (payload.type === "leave") {
          setIsDragActive(false)
        } else if (payload.type === "drop") {
          dragCounterRef.current = 0
          setIsDragActive(false)
          if (payload.paths.length > 0 && ownsPoint(payload.position)) {
            void handleUploadPathsRef.current(payload.paths)
          }
        }
      })
      .then((stop) => {
        if (disposed) stop()
        else unlisten = stop
      })
      .catch(console.error)

    return () => {
      disposed = true
      dragCounterRef.current = 0
      setIsDragActive(false)
      unlisten?.()
    }
  }, [drawerRef, handleUploadPathsRef, isShortcutTargetRef, visible])

  const handleDragEnter = useCallback((event: React.DragEvent<HTMLDivElement>) => {
    event.preventDefault()
    event.stopPropagation()
    dragCounterRef.current += 1
    setIsDragActive(true)
  }, [])

  const handleDragLeave = useCallback((event: React.DragEvent<HTMLDivElement>) => {
    event.preventDefault()
    event.stopPropagation()
    dragCounterRef.current = Math.max(0, dragCounterRef.current - 1)
    if (dragCounterRef.current === 0) {
      setIsDragActive(false)
    }
  }, [])

  const handleDragOver = useCallback((event: React.DragEvent<HTMLDivElement>) => {
    event.preventDefault()
    event.stopPropagation()
  }, [])

  const handleDrop = useCallback(
    async (event: React.DragEvent<HTMLDivElement>) => {
      event.preventDefault()
      event.stopPropagation()
      dragCounterRef.current = 0
      setIsDragActive(false)

      const droppedPaths = Array.from(event.dataTransfer.files)
        .map((file) => file.path)
        .filter((path): path is string => typeof path === "string" && path.length > 0)

      await handleUploadPaths(droppedPaths)
    },
    [handleUploadPaths]
  )

  return {
    handleDragEnter,
    handleDragLeave,
    handleDragOver,
    handleDrop,
    isDragActive: visible && isDragActive,
  }
}
