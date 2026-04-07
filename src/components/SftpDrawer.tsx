import "@/components/SftpDrawer.css"
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { createPortal } from "react-dom"
import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog"
import {
  ArrowUpFromLine,
  ChevronRight,
  File,
  Folder,
  FolderPlus,
  RefreshCcw,
  X,
  AlertCircle,
  Loader2,
} from "lucide-react"
import { useTranslation } from "react-i18next"
import type { Tab, TransferTask } from "@/types/tab"
import { ContextMenu } from "@/components/ContextMenu"
import { TransferManager } from "@/components/TransferManager"
import { Button } from "@/components/ui/button"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { cn } from "@/lib/utils"

interface SftpDrawerProps {
  tabId: string
  visible: boolean
  connection?: Tab["connection"]
  onClose: () => void
}

interface SftpDirectoryEntry {
  name: string
  path: string
  isDir: boolean
  isSymlink: boolean
  size?: number
  modifiedAt?: number
  permissions?: string
  owner?: string
  group?: string
}

interface SftpDirectoryListing {
  currentPath: string
  parentPath?: string | null
  entries: SftpDirectoryEntry[]
}

function joinRemotePath(basePath: string, name: string): string {
  if (basePath === "/") {
    return `/${name}`
  }
  return `${basePath}/${name}`
}

function formatBytes(value?: number): string {
  if (value == null) return "--"
  if (value < 1024) return `${value} B`

  const units = ["KB", "MB", "GB", "TB"]
  let size = value / 1024
  let index = 0
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024
    index += 1
  }
  return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[index]}`
}

function formatTimestamp(value?: number): string {
  if (!value) return "--"
  return new Intl.DateTimeFormat(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(value)
}

export const SftpDrawer: React.FC<SftpDrawerProps> = ({ tabId, visible, connection, onClose }) => {
  const { t } = useTranslation()
  const [listing, setListing] = useState<SftpDirectoryListing | null>(null)
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [successMessage, setSuccessMessage] = useState<string | null>(null)
  const [isDragActive, setIsDragActive] = useState(false)
  const [contextMenu, setContextMenu] = useState<{
    x: number
    y: number
    entryPath: string
  } | null>(null)
  const [transfers, setTransfers] = useState<TransferTask[]>([])
  const [deleteDialog, setDeleteDialog] = useState<{
    open: boolean
    entry: SftpDirectoryEntry | null
  }>({ open: false, entry: null })
  const [renameDialog, setRenameDialog] = useState<{
    open: boolean
    entry: SftpDirectoryEntry | null
    newName: string
  }>({ open: false, entry: null, newName: "" })
  const [createFolderDialog, setCreateFolderDialog] = useState<{
    open: boolean
    folderName: string
  }>({ open: false, folderName: "" })
  const dragCounterRef = useRef(0)
  const transfersRef = useRef<TransferTask[]>([])
  const lastProgressUpdateRef = useRef<Map<string, number>>(new Map())

  // Keep transfersRef in sync
  useEffect(() => {
    transfersRef.current = transfers
  }, [transfers])

  const addTransfer = useCallback(
    (transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">) => {
      const newTransfer: TransferTask = {
        ...transfer,
        id: `${Date.now()}-${Math.random().toString(36).substring(2, 11)}`,
        startTime: Date.now(),
        status: "pending",
        transferred: 0,
      }
      setTransfers((prev) => [newTransfer, ...prev])
      return newTransfer.id
    },
    []
  )

  const updateTransfer = useCallback((id: string, updates: Partial<TransferTask>) => {
    setTransfers((prev) => prev.map((t) => (t.id === id ? { ...t, ...updates } : t)))
  }, [])

  const cancelTransfer = useCallback(async (id: string) => {
    // Don't change status yet - let the backend error handler do it
    // This preserves the transferred bytes for resume

    // Call backend to cancel the upload
    try {
      await invoke("sftp_cancel_upload", { transferId: id })
    } catch (error) {
      console.warn("Failed to cancel transfer on backend:", error)
    }
  }, [])

  const removeTransfer = useCallback((id: string) => {
    setTransfers((prev) => prev.filter((t) => t.id !== id))
  }, [])

  const clearCompletedTransfers = useCallback(() => {
    setTransfers((prev) =>
      prev.filter((t) => t.status === "pending" || t.status === "transferring")
    )
  }, [])

  const loadDirectory = useCallback(
    async (path?: string | null) => {
      if (!connection) {
        setError(t("sftp.errors.missingConnection", { defaultValue: "SSH connection is missing" }))
        return
      }

      setIsLoading(true)
      setError(null)
      setSuccessMessage(null)
      try {
        const nextListing = await invoke<SftpDirectoryListing>("sftp_list_directory", {
          tabId,
          connection,
          path: path ?? undefined,
        })
        setListing(nextListing)
        setSelectedPath(null)
      } catch (invokeError) {
        setError(String(invokeError))
      } finally {
        setIsLoading(false)
      }
    },
    [connection, t, tabId]
  )

  const uploadFiles = useCallback(
    async (files: File[]) => {
      console.log("uploadFiles called with:", files)
      console.log("listing:", listing)

      if (!listing || files.length === 0) {
        console.log("Early return: listing or files empty")
        return
      }

      const validFiles = files.filter((file) => {
        console.log("Checking file:", file.name, "path:", file.path)
        if (!file.path) {
          console.warn("File does not have path property:", file.name)
          setError(
            t("sftp.errors.invalidFile", {
              defaultValue: `Cannot upload ${file.name}: file path not available`,
              name: file.name,
            })
          )
          return false
        }
        return true
      })

      console.log("Valid files:", validFiles)

      if (validFiles.length === 0) {
        setError(
          t("sftp.errors.noValidFiles", {
            defaultValue: "No valid files to upload",
          })
        )
        return
      }

      // Clear previous error messages
      setError(null)

      // Upload all files concurrently instead of serially
      const uploadPromises = validFiles.map(async (file) => {
        const fileName = file.name
        const localPath = file.path!

        console.log("Uploading file:", fileName, "from:", localPath)

        const transferId = addTransfer({
          direction: "upload",
          localPath,
          remotePath: joinRemotePath(listing.currentPath, fileName),
          fileName,
          fileSize: file.size || 0,
          speed: 0,
        })

        updateTransfer(transferId, { status: "transferring" })

        // Listen for progress events
        const appWindow = getCurrentWindow()
        const unlisten = await appWindow.listen<{
          localPath: string
          transferred: number
          total: number
          progress: number
        }>(`sftp-upload-progress-${tabId}`, (event) => {
          if (event.payload.localPath === localPath) {
            // Throttle: update progress at most once every 100ms
            const now = Date.now()
            const lastUpdate = lastProgressUpdateRef.current.get(transferId) || 0
            if (now - lastUpdate < 100 && event.payload.progress < 100) {
              return
            }
            lastProgressUpdateRef.current.set(transferId, now)

            const transfer = transfersRef.current.find((t) => t.id === transferId)
            const duration = now - (transfer?.startTime || now)
            const speed = duration > 0 ? (event.payload.transferred / duration) * 1000 : 0

            updateTransfer(transferId, {
              transferred: event.payload.transferred,
              speed,
            })
          }
        })

        try {
          const startTime = Date.now()
          await invoke("sftp_upload_file", {
            tabId,
            connection,
            localPath,
            remotePath: joinRemotePath(listing.currentPath, fileName),
            transferId,
          })

          unlisten()

          const duration = Date.now() - startTime
          const fileSize = file.size || 1024
          const speed = duration > 0 ? (fileSize / duration) * 1000 : 0

          updateTransfer(transferId, {
            status: "completed",
            transferred: fileSize,
            fileSize,
            endTime: Date.now(),
            speed,
          })

          // Clean up progress update record
          lastProgressUpdateRef.current.delete(transferId)

          console.log("Upload completed:", fileName)
          return { success: true, fileName }
        } catch (error) {
          unlisten()
          lastProgressUpdateRef.current.delete(transferId)

          const errorMessage = String(error)
          console.error("Upload error:", fileName, errorMessage)

          // Check if it was cancelled
          if (errorMessage.includes("cancelled")) {
            updateTransfer(transferId, {
              status: "cancelled",
              endTime: Date.now(),
            })
            return { success: false, fileName, cancelled: true }
          }

          updateTransfer(transferId, {
            status: "failed",
            error: errorMessage,
            endTime: Date.now(),
          })
          return { success: false, fileName, error: errorMessage }
        }
      })

      // Wait for all uploads to complete
      const results = await Promise.allSettled(uploadPromises)

      // Count results
      const succeeded = results.filter((r) => r.status === "fulfilled" && r.value.success).length
      const failed = results.filter(
        (r) => r.status === "rejected" || (r.status === "fulfilled" && !r.value.success)
      ).length

      console.log(`Upload summary: ${succeeded} succeeded, ${failed} failed`)

      // Refresh directory listing
      console.log("Refreshing directory listing")
      await loadDirectory(listing.currentPath)

      // Show success message
      if (succeeded > 0) {
        setSuccessMessage(
          t("sftp.uploadComplete", {
            defaultValue: `Uploaded ${succeeded} file(s)`,
            count: succeeded,
          })
        )
        setTimeout(() => setSuccessMessage(null), 3000)
      }
    },
    [addTransfer, connection, listing, loadDirectory, t, tabId, updateTransfer]
  )

  useEffect(() => {
    if (!visible) {
      dragCounterRef.current = 0
      setIsDragActive(false)
      return
    }
    if (!listing) {
      void loadDirectory(null)
    }

    // Listen for Tauri file drop events
    let unlisten: (() => void) | undefined

    const setupDragDropListener = async () => {
      const appWindow = getCurrentWindow()

      unlisten = await appWindow.onDragDropEvent((event) => {
        console.log("Drag drop event:", event)

        // Only handle drop events when drawer is visible
        if (!visible) return

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
            // Convert paths to objects with path property and get file sizes
            void (async () => {
              const filesPromises = paths.map(async (path) => {
                const fileName = path.split(/[\\/]/).pop() || "file"
                let fileSize = 0
                try {
                  fileSize = await invoke<number>("get_file_size", { localPath: path })
                } catch (error) {
                  console.warn("Failed to get file size for", path, error)
                }
                // Create a mock File object with path and size properties
                const fileObj = {
                  name: fileName,
                  path: path,
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
      if (unlisten) {
        unlisten()
      }
    }
  }, [listing, loadDirectory, visible, uploadFiles])

  const selectedEntry = useMemo(
    () => listing?.entries.find((entry) => entry.path === selectedPath) ?? null,
    [listing?.entries, selectedPath]
  )

  const contextMenuEntry = useMemo(
    () => listing?.entries.find((entry) => entry.path === contextMenu?.entryPath) ?? null,
    [listing?.entries, contextMenu?.entryPath]
  )

  const breadcrumbs = useMemo(() => {
    const currentPath = listing?.currentPath ?? "/"
    if (currentPath === "/") return [{ label: "/", path: "/" }]

    const parts = currentPath.split("/").filter(Boolean)
    let cursor = ""
    const items = [{ label: "/", path: "/" }]
    parts.forEach((part) => {
      cursor = `${cursor}/${part}`
      items.push({ label: part, path: cursor })
    })
    return items
  }, [listing?.currentPath])

  const runAndRefresh = useCallback(
    async (action: () => Promise<void>) => {
      setError(null)
      try {
        await action()
        await loadDirectory(listing?.currentPath ?? null)
      } catch (invokeError) {
        setError(String(invokeError))
      }
    },
    [listing?.currentPath, loadDirectory]
  )

  const handleOpenEntry = useCallback(
    async (entry: SftpDirectoryEntry) => {
      if (entry.isDir) {
        void loadDirectory(entry.path)
      } else {
        // Download file on double-click
        const targetPath = await saveFileDialog({
          title: t("sftp.actions.download", { defaultValue: "Download File" }),
          defaultPath: entry.name,
        })
        if (!targetPath) return

        const transferId = addTransfer({
          direction: "download",
          localPath: targetPath,
          remotePath: entry.path,
          fileName: entry.name,
          fileSize: entry.size || 0,
          speed: 0,
        })

        updateTransfer(transferId, { status: "transferring" })

        try {
          const startTime = Date.now()
          await invoke("sftp_download_file", {
            tabId,
            connection,
            remotePath: entry.path,
            localPath: targetPath,
          })

          const duration = Date.now() - startTime
          const speed = duration > 0 ? ((entry.size || 0) / duration) * 1000 : 0

          updateTransfer(transferId, {
            status: "completed",
            transferred: entry.size || 0,
            endTime: Date.now(),
            speed,
          })
        } catch (error) {
          updateTransfer(transferId, {
            status: "failed",
            error: String(error),
            endTime: Date.now(),
          })
        }
      }
    },
    [addTransfer, connection, loadDirectory, t, tabId, updateTransfer]
  )

  const handleCreateDirectory = useCallback(() => {
    setCreateFolderDialog({ open: true, folderName: "" })
  }, [])

  const handleCreateDirectoryConfirm = useCallback(() => {
    const trimmedName = createFolderDialog.folderName.trim()
    if (!trimmedName || !listing) return

    void runAndRefresh(() =>
      invoke("sftp_create_directory", {
        tabId,
        connection,
        path: joinRemotePath(listing.currentPath, trimmedName),
      })
    )
    setCreateFolderDialog({ open: false, folderName: "" })
  }, [connection, createFolderDialog.folderName, listing, runAndRefresh, tabId])

  const handleRename = useCallback(() => {
    const entry = contextMenuEntry || selectedEntry
    if (!entry || !listing) return

    setRenameDialog({ open: true, entry, newName: entry.name })
  }, [contextMenuEntry, listing, selectedEntry])

  const handleRenameConfirm = useCallback(() => {
    const { entry, newName } = renameDialog
    if (!entry || !listing) return

    const trimmedName = newName.trim()
    if (!trimmedName || trimmedName === entry.name) {
      setRenameDialog({ open: false, entry: null, newName: "" })
      return
    }

    void runAndRefresh(() =>
      invoke("sftp_rename_entry", {
        tabId,
        connection,
        oldPath: entry.path,
        newPath: joinRemotePath(listing.currentPath, trimmedName),
      })
    )
    setRenameDialog({ open: false, entry: null, newName: "" })
  }, [connection, listing, renameDialog, runAndRefresh, tabId])

  const handleDelete = useCallback(() => {
    const entry = contextMenuEntry || selectedEntry
    if (!entry) return

    setDeleteDialog({ open: true, entry })
  }, [contextMenuEntry, selectedEntry])

  const handleDeleteConfirm = useCallback(() => {
    const { entry } = deleteDialog
    if (!entry) return

    void runAndRefresh(() =>
      invoke("sftp_delete_entry", {
        tabId,
        connection,
        path: entry.path,
        isDir: entry.isDir,
      })
    )
    setDeleteDialog({ open: false, entry: null })
  }, [connection, deleteDialog, runAndRefresh, tabId])

  const handleDownload = useCallback(async () => {
    const entry = contextMenuEntry || selectedEntry
    if (!entry || entry.isDir) return

    const targetPath = await saveFileDialog({
      title: t("sftp.actions.download", { defaultValue: "Download File" }),
      defaultPath: entry.name,
    })
    if (!targetPath) return

    const transferId = addTransfer({
      direction: "download",
      localPath: targetPath,
      remotePath: entry.path,
      fileName: entry.name,
      fileSize: entry.size || 0,
      speed: 0,
    })

    updateTransfer(transferId, { status: "transferring" })

    try {
      const startTime = Date.now()
      await invoke("sftp_download_file", {
        tabId,
        connection,
        remotePath: entry.path,
        localPath: targetPath,
      })

      const duration = Date.now() - startTime
      const speed = duration > 0 ? ((entry.size || 0) / duration) * 1000 : 0

      updateTransfer(transferId, {
        status: "completed",
        transferred: entry.size || 0,
        endTime: Date.now(),
        speed,
      })
    } catch (error) {
      updateTransfer(transferId, {
        status: "failed",
        error: String(error),
        endTime: Date.now(),
      })
    }
  }, [addTransfer, connection, contextMenuEntry, selectedEntry, t, tabId, updateTransfer])

  const handleUploadDialog = useCallback(async () => {
    const selection = await openFileDialog({
      multiple: true,
      directory: false,
      title: t("sftp.actions.upload", { defaultValue: "Upload Files" }),
    })

    if (!selection || !listing) return

    const paths = Array.isArray(selection) ? selection : [selection]

    // Upload all files concurrently
    const uploadPromises = paths.map(async (path) => {
      const fileName = path.split(/[\\/]/).pop() ?? "file"

      // Get file size
      let fileSize = 0
      try {
        fileSize = await invoke<number>("get_file_size", { localPath: path })
      } catch (error) {
        console.warn("Failed to get file size:", error)
      }

      const transferId = addTransfer({
        direction: "upload",
        localPath: path,
        remotePath: joinRemotePath(listing.currentPath, fileName),
        fileName,
        fileSize,
        speed: 0,
      })

      updateTransfer(transferId, { status: "transferring" })

      // Listen for progress events
      const appWindow = getCurrentWindow()
      const unlisten = await appWindow.listen<{
        localPath: string
        transferred: number
        total: number
        progress: number
      }>(`sftp-upload-progress-${tabId}`, (event) => {
        if (event.payload.localPath === path) {
          // Throttle: update progress at most once every 100ms
          const now = Date.now()
          const lastUpdate = lastProgressUpdateRef.current.get(transferId) || 0
          if (now - lastUpdate < 100 && event.payload.progress < 100) {
            return
          }
          lastProgressUpdateRef.current.set(transferId, now)

          const transfer = transfersRef.current.find((t) => t.id === transferId)
          const duration = now - (transfer?.startTime || now)
          const speed = duration > 0 ? (event.payload.transferred / duration) * 1000 : 0

          updateTransfer(transferId, {
            transferred: event.payload.transferred,
            speed,
          })
        }
      })

      try {
        const startTime = Date.now()
        await invoke("sftp_upload_file", {
          tabId,
          connection,
          localPath: path,
          remotePath: joinRemotePath(listing.currentPath, fileName),
          transferId,
        })

        unlisten()

        const duration = Date.now() - startTime
        const speed = duration > 0 ? (fileSize / duration) * 1000 : 0

        updateTransfer(transferId, {
          status: "completed",
          transferred: fileSize,
          fileSize,
          endTime: Date.now(),
          speed,
        })

        // Clean up progress update record
        lastProgressUpdateRef.current.delete(transferId)

        return { success: true, fileName }
      } catch (error) {
        unlisten()
        lastProgressUpdateRef.current.delete(transferId)

        const errorMessage = String(error)

        // Check if it was cancelled
        if (errorMessage.includes("cancelled")) {
          updateTransfer(transferId, {
            status: "cancelled",
            endTime: Date.now(),
          })
          return { success: false, fileName, cancelled: true }
        }

        updateTransfer(transferId, {
          status: "failed",
          error: errorMessage,
          endTime: Date.now(),
        })
        return { success: false, fileName, error: errorMessage }
      }
    })

    // Wait for all uploads to complete
    await Promise.allSettled(uploadPromises)

    // Refresh directory
    await loadDirectory(listing.currentPath)
  }, [addTransfer, connection, listing, loadDirectory, t, tabId, updateTransfer])

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
    [listing, t, uploadFiles]
  )

  return (
    <div className={`sftp-drawer ${visible ? "is-open" : ""}`} aria-hidden={!visible}>
      <div className="sftp-drawer-header">
        <div className="sftp-header-left">
          <span className="sftp-drawer-eyebrow">SFTP</span>
          <div className="flex items-center gap-1">
            {breadcrumbs.map((item, index) => (
              <React.Fragment key={item.path}>
                {index > 0 && <ChevronRight className="text-muted-foreground size-3" />}
                <Button
                  variant="ghost"
                  size="xs"
                  onClick={() => void loadDirectory(item.path)}
                  disabled={isLoading}
                  className="h-6 px-2"
                >
                  {item.label}
                </Button>
              </React.Fragment>
            ))}
          </div>
        </div>
        <div className="sftp-header-actions">
          <TransferManager
            transfers={transfers}
            onCancel={cancelTransfer}
            onRemove={removeTransfer}
            onClearCompleted={clearCompletedTransfers}
          />
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={handleUploadDialog}
            disabled={!listing || isLoading}
            title={t("sftp.actions.upload", { defaultValue: "Upload" })}
          >
            <ArrowUpFromLine className="size-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={handleCreateDirectory}
            disabled={!listing || isLoading}
            title={t("sftp.actions.newFolder", { defaultValue: "New Folder" })}
          >
            <FolderPlus className="size-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => void loadDirectory(listing?.currentPath ?? null)}
            disabled={isLoading}
            title={t("sftp.actions.refresh", { defaultValue: "Refresh" })}
          >
            <RefreshCcw className={cn("size-4", isLoading && "animate-spin")} />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={onClose}
            title={t("sftp.actions.close", { defaultValue: "Close" })}
          >
            <X className="size-4" />
          </Button>
        </div>
      </div>

      {successMessage && (
        <Alert className="rounded-none border-x-0 border-t-0 bg-green-50 text-green-900 dark:bg-green-950/20 dark:text-green-400">
          <AlertDescription className="text-center font-medium">{successMessage}</AlertDescription>
        </Alert>
      )}

      {error && (
        <Alert className="bg-destructive/10 text-destructive rounded-none border-x-0 border-t-0">
          <AlertCircle className="size-4" />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <div
        className={cn("sftp-drawer-body", isDragActive && "drag-active")}
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
        onDragOver={handleDragOver}
        onDrop={handleDrop}
      >
        {/* Drag overlay */}
        {isDragActive && (
          <div className="sftp-drag-overlay">
            <div className="sftp-drag-hint">
              <ArrowUpFromLine className="sftp-drag-hint-icon" />
              <div className="text-center">
                <p className="sftp-drag-hint-title">
                  {t("sftp.dropFiles", { defaultValue: "Drop files to upload" })}
                </p>
                <p className="sftp-drag-hint-description">
                  {t("sftp.dropFilesHint", {
                    defaultValue: "Files will be uploaded to current directory",
                  })}
                </p>
              </div>
            </div>
          </div>
        )}

        <div className="sftp-table-shell">
          <div className="sftp-table-header">
            <span>{t("sftp.columns.name", { defaultValue: "Name" })}</span>
            <span>{t("sftp.columns.modified", { defaultValue: "Date Modified" })}</span>
            <span>{t("sftp.columns.size", { defaultValue: "Size" })}</span>
            <span>{t("sftp.columns.owner", { defaultValue: "Owner/Group" })}</span>
          </div>

          <ScrollArea className="flex-1">
            {isLoading && (
              <div className="text-muted-foreground flex min-h-[200px] flex-col items-center justify-center gap-3">
                <Loader2 className="size-6 animate-spin" />
                <span className="text-sm">{t("sftp.loading", { defaultValue: "Loading..." })}</span>
              </div>
            )}

            {!isLoading && error && (
              <div className="text-destructive flex min-h-[200px] flex-col items-center justify-center gap-3">
                <AlertCircle className="size-6" />
                <span className="text-sm">{error}</span>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void loadDirectory(listing?.currentPath ?? null)}
                  className="mt-2"
                >
                  <RefreshCcw className="size-4" />
                  {t("sftp.retry", { defaultValue: "Retry" })}
                </Button>
              </div>
            )}

            {!isLoading &&
              !error &&
              listing?.entries.map((entry) => {
                const isSelected = entry.path === selectedPath
                return (
                  <button
                    key={entry.path}
                    type="button"
                    className={cn("sftp-row", isSelected && "bg-accent/15")}
                    onClick={() => setSelectedPath(entry.path)}
                    onDoubleClick={() => handleOpenEntry(entry)}
                    onContextMenu={(e) => {
                      e.preventDefault()
                      setSelectedPath(entry.path)
                      setContextMenu({ x: e.clientX, y: e.clientY, entryPath: entry.path })
                    }}
                  >
                    <span className="sftp-cell sftp-name-cell">
                      {entry.isDir ? (
                        <Folder className="size-4 shrink-0 text-blue-500" />
                      ) : (
                        <File className="text-muted-foreground size-4 shrink-0" />
                      )}
                      <span className="truncate">{entry.name}</span>
                    </span>
                    <span className="sftp-cell">{formatTimestamp(entry.modifiedAt)}</span>
                    <span className="sftp-cell">
                      {entry.isDir ? "--" : formatBytes(entry.size)}
                    </span>
                    <span className="sftp-cell">
                      {entry.owner ?? "--"}
                      {entry.group ? ` / ${entry.group}` : ""}
                    </span>
                  </button>
                )
              })}

            {!isLoading && !error && listing?.entries.length === 0 && (
              <div className="text-muted-foreground flex min-h-[200px] flex-col items-center justify-center gap-3">
                <FolderPlus className="size-6" />
                <span className="text-sm">
                  {t("sftp.emptyDescription", { defaultValue: "This folder is empty" })}
                </span>
              </div>
            )}
          </ScrollArea>
        </div>
      </div>

      {contextMenu &&
        contextMenuEntry &&
        createPortal(
          <ContextMenu
            x={contextMenu.x}
            y={contextMenu.y}
            actions={[
              {
                label: t("sftp.actions.download", { defaultValue: "Download" }),
                action: "download",
                icon: "copy",
                disabled: contextMenuEntry.isDir,
              },
              {
                label: t("sftp.actions.rename", { defaultValue: "Rename" }),
                action: "rename",
                icon: "edit",
              },
              { separator: true, label: "", action: "" },
              {
                label: t("sftp.actions.delete", { defaultValue: "Delete" }),
                action: "delete",
                icon: "x",
              },
            ]}
            onAction={(action) => {
              if (action === "download") void handleDownload()
              else if (action === "rename") handleRename()
              else if (action === "delete") handleDelete()
            }}
            onClose={() => setContextMenu(null)}
          />,
          document.body
        )}

      {/* Delete Confirmation Dialog */}
      <Dialog
        open={deleteDialog.open}
        onOpenChange={(open) => !open && setDeleteDialog({ open: false, entry: null })}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {t("sftp.dialogs.deleteTitle", { defaultValue: "Delete Item" })}
            </DialogTitle>
            <DialogDescription>
              {t("sftp.dialogs.deleteDescription", {
                defaultValue: `Are you sure you want to delete "${deleteDialog.entry?.name}"? This action cannot be undone.`,
                name: deleteDialog.entry?.name,
              })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteDialog({ open: false, entry: null })}>
              {t("common.cancel", { defaultValue: "Cancel" })}
            </Button>
            <Button variant="destructive" onClick={handleDeleteConfirm}>
              {t("common.delete", { defaultValue: "Delete" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Rename Dialog */}
      <Dialog
        open={renameDialog.open}
        onOpenChange={(open) => !open && setRenameDialog({ open: false, entry: null, newName: "" })}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {t("sftp.dialogs.renameTitle", { defaultValue: "Rename Item" })}
            </DialogTitle>
            <DialogDescription>
              {t("sftp.dialogs.renameDescription", {
                defaultValue: "Enter a new name for the item",
              })}
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="rename-input">
                {t("sftp.dialogs.nameLabel", { defaultValue: "Name" })}
              </Label>
              <Input
                id="rename-input"
                value={renameDialog.newName}
                onChange={(e) => setRenameDialog({ ...renameDialog, newName: e.target.value })}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault()
                    handleRenameConfirm()
                  }
                }}
                autoFocus
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setRenameDialog({ open: false, entry: null, newName: "" })}
            >
              {t("common.cancel", { defaultValue: "Cancel" })}
            </Button>
            <Button onClick={handleRenameConfirm}>
              {t("common.rename", { defaultValue: "Rename" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Create Folder Dialog */}
      <Dialog
        open={createFolderDialog.open}
        onOpenChange={(open) => !open && setCreateFolderDialog({ open: false, folderName: "" })}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {t("sftp.dialogs.createFolderTitle", { defaultValue: "Create New Folder" })}
            </DialogTitle>
            <DialogDescription>
              {t("sftp.dialogs.createFolderDescription", {
                defaultValue: "Enter a name for the new folder",
              })}
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="folder-name-input">
                {t("sftp.dialogs.folderNameLabel", { defaultValue: "Folder Name" })}
              </Label>
              <Input
                id="folder-name-input"
                value={createFolderDialog.folderName}
                onChange={(e) =>
                  setCreateFolderDialog({ ...createFolderDialog, folderName: e.target.value })
                }
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault()
                    handleCreateDirectoryConfirm()
                  }
                }}
                autoFocus
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setCreateFolderDialog({ open: false, folderName: "" })}
            >
              {t("common.cancel", { defaultValue: "Cancel" })}
            </Button>
            <Button onClick={handleCreateDirectoryConfirm}>
              {t("common.create", { defaultValue: "Create" })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
