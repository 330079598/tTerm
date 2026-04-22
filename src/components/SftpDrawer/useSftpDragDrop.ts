import { useCallback, useEffect, useRef, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useTranslation } from "react-i18next"

import type { SftpDirectoryListing } from "@/components/SftpDrawer/types"

interface UseSftpDragDropParams {
  listing: SftpDirectoryListing | null
  loadDirectory: (path?: string | null) => Promise<void>
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

export function useSftpDragDrop({
  listing,
  loadDirectory,
  setError,
  uploadPaths,
  visible,
}: UseSftpDragDropParams): UseSftpDragDropReturn {
  const { t } = useTranslation()
  const [isDragActive, setIsDragActive] = useState(false)
  const dragCounterRef = useRef(0)
  const lastDropRef = useRef<{ signature: string; timestamp: number } | null>(null)

  useEffect(() => {
    if (visible && !listing) {
      void loadDirectory(null)
    }
  }, [listing, loadDirectory, visible])

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

  useEffect(() => {
    if (!visible) {
      return
    }

    let unlisten: (() => void) | undefined

    const setupDragDropListener = async () => {
      const appWindow = getCurrentWindow()

      unlisten = await appWindow.onDragDropEvent((event) => {
        if (!visible) {
          return
        }

        if (event.payload.type === "enter") {
          dragCounterRef.current += 1
          setIsDragActive(true)
        } else if (event.payload.type === "leave") {
          dragCounterRef.current = Math.max(0, dragCounterRef.current - 1)
          if (dragCounterRef.current === 0) {
            setIsDragActive(false)
          }
        } else if (event.payload.type === "drop") {
          dragCounterRef.current = 0
          setIsDragActive(false)

          const paths = event.payload.paths as string[]
          if (paths && paths.length > 0) {
            void handleUploadPaths(paths)
          }
        }
      })
    }

    void setupDragDropListener()

    return () => {
      dragCounterRef.current = 0
      setIsDragActive(false)
      if (unlisten) {
        unlisten()
      }
    }
  }, [handleUploadPaths, visible])

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
