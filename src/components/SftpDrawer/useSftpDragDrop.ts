import { useCallback, useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useTranslation } from "react-i18next"

import type { SftpDirectoryListing } from "@/components/SftpDrawer/types"

interface UseSftpDragDropParams {
  listing: SftpDirectoryListing | null
  loadDirectory: (path?: string | null) => Promise<void>
  setError: React.Dispatch<React.SetStateAction<string | null>>
  uploadFiles: (files: File[]) => Promise<void>
  visible: boolean
}

interface UseSftpDragDropReturn {
  handleDragEnter: (event: React.DragEvent<HTMLDivElement>) => void
  handleDragLeave: (event: React.DragEvent<HTMLDivElement>) => void
  handleDragOver: (event: React.DragEvent<HTMLDivElement>) => void
  handleDrop: (event: React.DragEvent<HTMLDivElement>) => Promise<void>
  isDragActive: boolean
}

export function useSftpDragDrop({
  listing,
  loadDirectory,
  setError,
  uploadFiles,
  visible,
}: UseSftpDragDropParams): UseSftpDragDropReturn {
  const { t } = useTranslation()
  const [isDragActive, setIsDragActive] = useState(false)
  const dragCounterRef = useRef(0)

  useEffect(() => {
    if (visible && !listing) {
      void loadDirectory(null)
    }
  }, [listing, loadDirectory, visible])

  useEffect(() => {
    if (!visible) {
      return
    }

    let unlisten: (() => void) | undefined

    const setupDragDropListener = async () => {
      const appWindow = getCurrentWindow()

      unlisten = await appWindow.onDragDropEvent((event) => {
        console.log("Drag drop event:", event)

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
          console.log("Dropped paths:", paths)

          if (paths && paths.length > 0) {
            void (async () => {
              const filesPromises = paths.map(async (path) => {
                const fileName = path.split(/[\\/]/).pop() || "file"
                let fileSize = 0
                try {
                  fileSize = await invoke<number>("get_file_size", { localPath: path })
                } catch (invokeError) {
                  console.warn("Failed to get file size for", path, invokeError)
                }

                const fileObj = {
                  name: fileName,
                  path,
                  size: fileSize,
                  type: "application/octet-stream",
                } as File & { path: string }
                return fileObj
              })
              const files = await Promise.all(filesPromises)
              console.log("Uploading files:", files)
              void uploadFiles(files)
            })()
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
  }, [uploadFiles, visible])

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

      if (!listing) {
        setError(t("sftp.errors.notReady", { defaultValue: "SFTP not ready" }))
        return
      }

      const droppedFiles = Array.from(event.dataTransfer.files).filter(
        (file) => typeof file.path === "string" && file.path.length > 0
      )

      if (droppedFiles.length === 0) {
        setError(
          t("sftp.errors.noValidFiles", {
            defaultValue: "No valid files to upload",
          })
        )
        return
      }

      await uploadFiles(droppedFiles)
    },
    [listing, setError, t, uploadFiles]
  )

  return {
    handleDragEnter,
    handleDragLeave,
    handleDragOver,
    handleDrop,
    isDragActive: visible && isDragActive,
  }
}
