import "@/components/SftpDrawer.css"
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react"
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
  const dragCounterRef = useRef(0)

  const addTransfer = useCallback(
    (transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">) => {
      const newTransfer: TransferTask = {
        ...transfer,
        id: `${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
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

  const cancelTransfer = useCallback(
    (id: string) => {
      updateTransfer(id, { status: "cancelled", endTime: Date.now() })
    },
    [updateTransfer]
  )

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

      // 清除之前的错误消息
      setError(null)

      // 并发上传所有文件，而不是串行
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

        try {
          const startTime = Date.now()
          await invoke("sftp_upload_file", {
            tabId,
            connection,
            localPath,
            remotePath: joinRemotePath(listing.currentPath, fileName),
          })

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

          console.log("Upload completed:", fileName)
          return { success: true, fileName }
        } catch (error) {
          console.error("Upload failed:", fileName, error)
          updateTransfer(transferId, {
            status: "failed",
            error: String(error),
            endTime: Date.now(),
          })
          return { success: false, fileName, error: String(error) }
        }
      })

      // 等待所有上传完成
      const results = await Promise.allSettled(uploadPromises)

      // 统计结果
      const succeeded = results.filter((r) => r.status === "fulfilled" && r.value.success).length
      const failed = results.filter(
        (r) => r.status === "rejected" || (r.status === "fulfilled" && !r.value.success)
      ).length

      console.log(`Upload summary: ${succeeded} succeeded, ${failed} failed`)

      // 刷新目录列表
      console.log("Refreshing directory listing")
      await loadDirectory(listing.currentPath)

      // 显示成功消息
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

    // 监听 Tauri 文件拖放事件
    let unlisten: (() => void) | undefined

    const setupDragDropListener = async () => {
      const appWindow = getCurrentWindow()

      unlisten = await appWindow.onDragDropEvent((event) => {
        console.log("Drag drop event:", event)

        // 只在 drawer 可见时处理拖放事件
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
            // 将路径转换为带有 path 属性的对象
            const files = paths.map((path) => {
              const fileName = path.split(/[\\/]/).pop() || "file"
              // 创建一个模拟的 File 对象，带有 path 属性
              const fileObj = {
                name: fileName,
                path: path,
                size: 0,
                type: "application/octet-stream",
              } as File & { path: string }
              return fileObj
            })
            console.log("Uploading files:", files)
            void uploadFiles(files)
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
        // 双击文件时下载
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
    const directoryName = window.prompt(
      t("sftp.prompts.newFolder", { defaultValue: "New folder name" }),
      ""
    )
    if (!directoryName || !listing) return

    const trimmedName = directoryName.trim()
    if (!trimmedName) return

    void runAndRefresh(() =>
      invoke("sftp_create_directory", {
        tabId,
        connection,
        path: joinRemotePath(listing.currentPath, trimmedName),
      })
    )
  }, [connection, listing, runAndRefresh, t, tabId])

  const handleRename = useCallback(() => {
    const entry = contextMenuEntry || selectedEntry
    if (!entry || !listing) return

    const nextName = window.prompt(
      t("sftp.prompts.rename", { defaultValue: "Rename entry" }),
      entry.name
    )
    if (!nextName) return

    const trimmedName = nextName.trim()
    if (!trimmedName || trimmedName === entry.name) return

    void runAndRefresh(() =>
      invoke("sftp_rename_entry", {
        tabId,
        connection,
        oldPath: entry.path,
        newPath: joinRemotePath(listing.currentPath, trimmedName),
      })
    )
  }, [connection, contextMenuEntry, listing, runAndRefresh, selectedEntry, t, tabId])

  const handleDelete = useCallback(() => {
    const entry = contextMenuEntry || selectedEntry
    if (!entry) return

    const confirmed = window.confirm(
      t("sftp.prompts.delete", {
        defaultValue: `Delete ${entry.name}?`,
        name: entry.name,
      })
    )
    if (!confirmed) return

    void runAndRefresh(() =>
      invoke("sftp_delete_entry", {
        tabId,
        connection,
        path: entry.path,
        isDir: entry.isDir,
      })
    )
  }, [connection, contextMenuEntry, runAndRefresh, selectedEntry, t, tabId])

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

    // 并发上传所有文件
    const uploadPromises = paths.map(async (path) => {
      const fileName = path.split(/[\\/]/).pop() ?? "file"

      const transferId = addTransfer({
        direction: "upload",
        localPath: path,
        remotePath: joinRemotePath(listing.currentPath, fileName),
        fileName,
        fileSize: 0,
        speed: 0,
      })

      updateTransfer(transferId, { status: "transferring" })

      try {
        const startTime = Date.now()
        await invoke("sftp_upload_file", {
          tabId,
          connection,
          localPath: path,
          remotePath: joinRemotePath(listing.currentPath, fileName),
        })

        const duration = Date.now() - startTime
        const speed = duration > 0 ? (1024 / duration) * 1000 : 0

        updateTransfer(transferId, {
          status: "completed",
          transferred: 1024,
          fileSize: 1024,
          endTime: Date.now(),
          speed,
        })

        return { success: true, fileName }
      } catch (error) {
        updateTransfer(transferId, {
          status: "failed",
          error: String(error),
          endTime: Date.now(),
        })
        return { success: false, fileName, error: String(error) }
      }
    })

    // 等待所有上传完成
    await Promise.allSettled(uploadPromises)

    // 刷新目录
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
        {/* 拖拽覆盖层 */}
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

      {contextMenu && contextMenuEntry && (
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
        />
      )}
    </div>
  )
}
