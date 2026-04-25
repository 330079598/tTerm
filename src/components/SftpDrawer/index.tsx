import "@/components/SftpDrawer.css"
import React, { useCallback, useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { AlertCircle } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { toast } from "@/hooks/use-toast"

import { SftpDrawerContent } from "@/components/SftpDrawer/SftpDrawerContent"
import { SftpDrawerHeader } from "@/components/SftpDrawer/SftpDrawerHeader"
import { SftpDialogs } from "@/components/SftpDrawer/SftpDialogs"
import { SftpEntryContextMenu } from "@/components/SftpDrawer/SftpEntryContextMenu"
import { useSftpDragDrop } from "@/components/SftpDrawer/useSftpDragDrop"
import { joinRemotePath } from "@/components/SftpDrawer/sftpDrawerUtils"
import type {
  SftpCommandDeleteDialogState,
  SftpContextMenuState,
  SftpCreateFolderDialogState,
  SftpDeleteDialogState,
  SftpDirectoryEntry,
  SftpDirectoryListing,
  SftpDrawerProps,
  SftpDeleteProgressState,
  SftpRenameDialogState,
} from "@/components/SftpDrawer/types"
import { useSftpTransfers } from "@/components/SftpDrawer/useSftpTransfers"

interface DeleteBatchStartResult {
  batchId: string
}

interface DeletePreviewResult {
  command: string
  shouldPromptForCommand: boolean
  totalDirectories: number
  totalEntries: number
  totalFiles: number
  totalTruncated: boolean
}

interface DeleteBatchStartEvent extends SftpDeleteProgressState {
  entries: string[]
}

interface DeleteBatchCompleteEvent extends SftpDeleteProgressState {
  cancelled: boolean
  error?: string
}

export const SftpDrawer: React.FC<SftpDrawerProps> = ({ tabId, visible, connection, onClose }) => {
  const { t } = useTranslation()
  const [listing, setListing] = useState<SftpDirectoryListing | null>(null)
  const [activePath, setActivePath] = useState<string | null>(null)
  const [selectedPaths, setSelectedPaths] = useState<string[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [isDeleting, setIsDeleting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [contextMenu, setContextMenu] = useState<SftpContextMenuState | null>(null)
  const [deleteDialog, setDeleteDialog] = useState<SftpDeleteDialogState>({
    open: false,
    entries: [],
  })
  const [commandDeleteDialog, setCommandDeleteDialog] = useState<SftpCommandDeleteDialogState>({
    command: "",
    entries: [],
    open: false,
    totalDirectories: 0,
    totalEntries: 0,
    totalFiles: 0,
    totalTruncated: false,
  })
  const [renameDialog, setRenameDialog] = useState<SftpRenameDialogState>({
    open: false,
    entry: null,
    newName: "",
  })
  const [createFolderDialog, setCreateFolderDialog] = useState<SftpCreateFolderDialogState>({
    open: false,
    folderName: "",
  })

  const loadDirectory = useCallback(
    async (path?: string | null) => {
      if (!connection) {
        setError(t("sftp.errors.missingConnection", { defaultValue: "SSH connection is missing" }))
        return
      }

      setIsLoading(true)
      setError(null)
      try {
        const nextListing = await invoke<SftpDirectoryListing>("sftp_list_directory", {
          tabId,
          connection,
          path: path ?? undefined,
        })
        setListing(nextListing)
        setActivePath(null)
        setSelectedPaths([])
      } catch (invokeError) {
        setError(String(invokeError))
      } finally {
        setIsLoading(false)
      }
    },
    [connection, t, tabId]
  )

  const {
    addTransfer,
    downloadEntry,
    handleOpenEntry,
    handleUploadDialog,
    handleUploadFolderDialog,
    transfersRef,
    updateTransfer,
    uploadPaths,
  } = useSftpTransfers({
    connection,
    listing,
    loadDirectory,
    setError,
    tabId,
  })

  const { handleDragEnter, handleDragLeave, handleDragOver, handleDrop, isDragActive } =
    useSftpDragDrop({
      listing,
      loadDirectory,
      setError,
      uploadPaths,
      visible,
    })

  useEffect(() => {
    const appWindow = getCurrentWindow()
    let disposed = false
    const unlisteners: Array<() => void> = []

    const setupListeners = async () => {
      const nextUnlisteners = await Promise.all([
        appWindow.listen<DeleteBatchStartEvent>(`sftp-delete-batch-start-${tabId}`, (event) => {
          const { payload } = event
          addTransfer(
            {
              direction: "delete",
              fileName:
                payload.entries.join(", ") ||
                t("sftp.deleteProgress.sftp", { defaultValue: "Deleting files" }),
              fileSize: payload.totalEntries,
              localPath: "",
              remotePath: payload.entries.join(", "),
              speed: 0,
            },
            payload.batchId
          )
          updateTransfer(payload.batchId, {
            status: "transferring",
            transferred: 0,
          })
        }),
        appWindow.listen<SftpDeleteProgressState>(`sftp-delete-progress-${tabId}`, (event) => {
          const payload = event.payload
          const transferred = payload.deletedDirectories + payload.deletedFiles
          const transfer = transfersRef.current.find((item) => item.id === payload.batchId)
          const now = Date.now()
          const duration = now - (transfer?.startTime || now)
          const speed = duration > 0 ? (transferred / duration) * 1000 : 0

          updateTransfer(payload.batchId, {
            fileSize: payload.totalEntries,
            remotePath: payload.currentPath || transfer?.remotePath || "",
            speed,
            status: "transferring",
            transferred,
          })
        }),
        appWindow.listen<DeleteBatchCompleteEvent>(
          `sftp-delete-batch-complete-${tabId}`,
          (event) => {
            const { payload } = event
            setIsDeleting(false)
            void loadDirectory(listing?.currentPath ?? null)

            if (payload.error || payload.failed > 0) {
              const message = payload.error ?? `Failed to delete ${payload.failed} item(s).`
              updateTransfer(payload.batchId, {
                endTime: Date.now(),
                error: message,
                fileSize: payload.totalEntries,
                status: "failed",
                transferred: payload.deletedDirectories + payload.deletedFiles,
              })
              setError(message)
              toast({
                variant: "destructive",
                title: t("sftp.messages.deleteFailure", {
                  defaultValue: "Failed to delete selected items.",
                }),
                description: message,
              })
              return
            }

            updateTransfer(payload.batchId, {
              endTime: Date.now(),
              fileSize: payload.totalEntries,
              speed: 0,
              status: "completed",
              transferred: payload.totalEntries,
            })

            toast({
              title: t("sftp.messages.deleteSuccess", {
                count: payload.deletedDirectories + payload.deletedFiles,
                defaultValue: `Deleted ${payload.deletedDirectories + payload.deletedFiles} item(s).`,
              }),
              description:
                payload.method === "command"
                  ? t("sftp.messages.deleteUsedCommand", {
                      defaultValue: "Used remote command delete for a large folder.",
                    })
                  : undefined,
            })
          }
        ),
      ])

      if (disposed) {
        nextUnlisteners.forEach((unlisten) => unlisten())
        return
      }

      unlisteners.push(...nextUnlisteners)
    }

    void setupListeners()

    return () => {
      disposed = true
      unlisteners.forEach((unlisten) => unlisten())
    }
  }, [addTransfer, listing?.currentPath, loadDirectory, t, tabId, transfersRef, updateTransfer])

  const entryMap = useMemo(
    () => new Map((listing?.entries ?? []).map((entry) => [entry.path, entry])),
    [listing?.entries]
  )

  const selectedEntries = useMemo(
    () =>
      selectedPaths
        .map((path) => entryMap.get(path))
        .filter((entry): entry is SftpDirectoryEntry => Boolean(entry)),
    [entryMap, selectedPaths]
  )

  const activeEntry = useMemo(
    () => (activePath ? (entryMap.get(activePath) ?? null) : null),
    [activePath, entryMap]
  )

  const contextMenuEntry = useMemo(
    () => listing?.entries.find((entry) => entry.path === contextMenu?.entryPath) ?? null,
    [listing?.entries, contextMenu?.entryPath]
  )

  const breadcrumbs = useMemo(() => {
    const currentPath = listing?.currentPath ?? "/"
    if (currentPath === "/") {
      return [{ label: "/", path: "/" }]
    }

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

  const handleActivateEntry = useCallback((path: string | null) => {
    setActivePath(path)
  }, [])

  const buildSelectionRange = useCallback(
    (startPath: string, endPath: string) => {
      const entries = listing?.entries ?? []
      const startIndex = entries.findIndex((entry) => entry.path === startPath)
      const endIndex = entries.findIndex((entry) => entry.path === endPath)

      if (startIndex === -1 || endIndex === -1) {
        return [endPath]
      }

      const [from, to] = startIndex <= endIndex ? [startIndex, endIndex] : [endIndex, startIndex]
      return entries.slice(from, to + 1).map((entry) => entry.path)
    },
    [listing?.entries]
  )

  const handleSelectRange = useCallback(
    (anchorPath: string, currentPath: string) => {
      const range = buildSelectionRange(anchorPath, currentPath)
      setSelectedPaths(range)
    },
    [buildSelectionRange]
  )

  const handleToggleEntrySelection = useCallback((path: string, checked: boolean) => {
    setSelectedPaths((current) => {
      if (checked) {
        setActivePath(path)
        return current.includes(path) ? current : [...current, path]
      }

      return current.filter((item) => item !== path)
    })
  }, [])

  const handleClearSelection = useCallback(() => {
    setSelectedPaths([])
  }, [])

  const handleCreateDirectory = useCallback(() => {
    setCreateFolderDialog({ open: true, folderName: "" })
  }, [])

  const handleCreateDirectoryConfirm = useCallback(() => {
    const trimmedName = createFolderDialog.folderName.trim()
    if (!trimmedName || !listing) {
      return
    }

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
    const entry = contextMenuEntry ?? activeEntry
    if (!entry || !listing) {
      return
    }

    setRenameDialog({ open: true, entry, newName: entry.name })
  }, [activeEntry, contextMenuEntry, listing])

  const handleRenameConfirm = useCallback(() => {
    const { entry, newName } = renameDialog
    if (!entry || !listing) {
      return
    }

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

  const handleDeleteSelection = useCallback(() => {
    if (selectedEntries.length === 0) {
      return
    }

    setDeleteDialog({ open: true, entries: selectedEntries })
  }, [selectedEntries])

  const handleDelete = useCallback(() => {
    const entries =
      contextMenuEntry &&
      selectedPaths.includes(contextMenuEntry.path) &&
      selectedEntries.length > 0
        ? selectedEntries
        : contextMenuEntry
          ? [contextMenuEntry]
          : activeEntry
            ? [activeEntry]
            : []
    if (entries.length === 0) {
      return
    }

    setDeleteDialog({ open: true, entries })
  }, [activeEntry, contextMenuEntry, selectedEntries, selectedPaths])

  const startDeleteBatch = useCallback(
    async (entries: SftpDirectoryEntry[], useCommandDelete: boolean, command?: string) => {
      setIsDeleting(true)
      setError(null)
      setDeleteDialog({ open: false, entries: [] })
      setCommandDeleteDialog({
        command: "",
        entries: [],
        open: false,
        totalDirectories: 0,
        totalEntries: 0,
        totalFiles: 0,
        totalTruncated: false,
      })
      setContextMenu(null)

      try {
        await invoke<DeleteBatchStartResult>("sftp_delete_entries", {
          tabId,
          connection,
          entries: entries.map((entry) => ({
            path: entry.path,
            name: entry.name,
            isDir: entry.isDir,
          })),
          options: {
            command,
            useCommandDelete,
          },
        })
      } catch (invokeError) {
        const message = String(invokeError)
        setIsDeleting(false)
        setError(message)
        toast({
          variant: "destructive",
          title: t("sftp.messages.deleteFailure", {
            defaultValue: "Failed to delete selected items.",
          }),
          description: message,
        })
      }
    },
    [connection, t, tabId]
  )

  const handleDeleteConfirm = useCallback(() => {
    const entries = deleteDialog.entries
    if (entries.length === 0) {
      return
    }

    void (async () => {
      setIsDeleting(true)
      setError(null)
      try {
        const preview = await invoke<DeletePreviewResult>("sftp_preview_delete_entries", {
          tabId,
          connection,
          entries: entries.map((entry) => ({
            path: entry.path,
            name: entry.name,
            isDir: entry.isDir,
          })),
        })

        setIsDeleting(false)
        if (preview.shouldPromptForCommand) {
          setDeleteDialog({ open: false, entries: [] })
          setCommandDeleteDialog({
            command: preview.command,
            entries,
            open: true,
            totalDirectories: preview.totalDirectories,
            totalEntries: preview.totalEntries,
            totalFiles: preview.totalFiles,
            totalTruncated: preview.totalTruncated,
          })
          return
        }

        await startDeleteBatch(entries, false)
      } catch (invokeError) {
        const message = String(invokeError)
        setIsDeleting(false)
        setError(message)
        toast({
          variant: "destructive",
          title: t("sftp.messages.deleteFailure", {
            defaultValue: "Failed to delete selected items.",
          }),
          description: message,
        })
      }
    })()
  }, [connection, deleteDialog.entries, startDeleteBatch, t, tabId])

  const handleCommandDeleteConfirm = useCallback(() => {
    const entries = commandDeleteDialog.entries
    const command = commandDeleteDialog.command.trim()
    if (entries.length === 0 || !command) {
      return
    }

    void startDeleteBatch(entries, true, command)
  }, [commandDeleteDialog.command, commandDeleteDialog.entries, startDeleteBatch])

  const handleSftpDeleteConfirm = useCallback(() => {
    const entries = commandDeleteDialog.entries
    if (entries.length === 0) {
      return
    }

    void startDeleteBatch(entries, false)
  }, [commandDeleteDialog.entries, startDeleteBatch])

  const handleDownload = useCallback(async () => {
    const entry = contextMenuEntry ?? activeEntry
    if (!entry || entry.isDir) {
      return
    }

    await downloadEntry(entry)
  }, [activeEntry, contextMenuEntry, downloadEntry])

  const contextSelectionCount = useMemo(() => {
    if (!contextMenuEntry) {
      return selectedPaths.length
    }

    return selectedPaths.includes(contextMenuEntry.path) ? selectedPaths.length : 1
  }, [contextMenuEntry, selectedPaths])

  const isBusy = isLoading || isDeleting

  return (
    <div className={`sftp-drawer ${visible ? "is-open" : ""}`} aria-hidden={!visible}>
      <SftpDrawerHeader
        breadcrumbs={breadcrumbs}
        clearSelection={handleClearSelection}
        handleCreateDirectory={handleCreateDirectory}
        handleDeleteSelection={handleDeleteSelection}
        handleUploadDialog={handleUploadDialog}
        handleUploadFolderDialog={handleUploadFolderDialog}
        isLoading={isBusy}
        listingCurrentPath={listing?.currentPath}
        loadDirectory={loadDirectory}
        onClose={onClose}
        selectedCount={selectedPaths.length}
      />

      {error && (
        <Alert className="bg-destructive/10 text-destructive rounded-none border-x-0 border-t-0">
          <AlertCircle className="size-4" />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <SftpDrawerContent
        error={error}
        activePath={activePath}
        handleActivateEntry={handleActivateEntry}
        handleDragEnter={handleDragEnter}
        handleDragLeave={handleDragLeave}
        handleDragOver={handleDragOver}
        handleDrop={handleDrop}
        handleOpenEntry={handleOpenEntry}
        handleSelectRange={handleSelectRange}
        handleToggleEntrySelection={handleToggleEntrySelection}
        isDragActive={isDragActive}
        isLoading={isBusy}
        listing={listing}
        loadDirectory={loadDirectory}
        selectedPaths={selectedPaths}
        setContextMenu={setContextMenu}
      />

      <SftpEntryContextMenu
        contextMenu={contextMenu}
        contextMenuEntry={contextMenuEntry}
        handleDelete={handleDelete}
        handleDownload={handleDownload}
        handleRename={handleRename}
        onClose={() => setContextMenu(null)}
        selectionCount={contextSelectionCount}
      />

      <SftpDialogs
        commandDeleteDialog={commandDeleteDialog}
        createFolderDialog={createFolderDialog}
        deleteDialog={deleteDialog}
        handleCommandDeleteConfirm={handleCommandDeleteConfirm}
        handleCreateDirectoryConfirm={handleCreateDirectoryConfirm}
        handleDeleteConfirm={handleDeleteConfirm}
        handleRenameConfirm={handleRenameConfirm}
        handleSftpDeleteConfirm={handleSftpDeleteConfirm}
        isDeleting={isDeleting}
        renameDialog={renameDialog}
        setCommandDeleteDialog={setCommandDeleteDialog}
        setCreateFolderDialog={setCreateFolderDialog}
        setDeleteDialog={setDeleteDialog}
        setRenameDialog={setRenameDialog}
      />
    </div>
  )
}
