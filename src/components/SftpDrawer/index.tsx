import "@/components/SftpDrawer.css"
import React, { useCallback, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
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
  SftpContextMenuState,
  SftpCreateFolderDialogState,
  SftpDeleteDialogState,
  SftpDirectoryEntry,
  SftpDirectoryListing,
  SftpDrawerProps,
  SftpRenameDialogState,
} from "@/components/SftpDrawer/types"
import { useSftpTransfers } from "@/components/SftpDrawer/useSftpTransfers"

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
    cancelTransfer,
    clearCompletedTransfers,
    downloadEntry,
    handleOpenEntry,
    handleUploadDialog,
    removeTransfer,
    transfers,
    uploadFiles,
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
      uploadFiles,
      visible,
    })

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

  const handleDeleteConfirm = useCallback(() => {
    const entries = deleteDialog.entries
    if (entries.length === 0) {
      return
    }

    void (async () => {
      setIsDeleting(true)
      setError(null)

      let successCount = 0
      const failures: string[] = []

      for (const entry of entries) {
        try {
          await invoke("sftp_delete_entry", {
            tabId,
            connection,
            path: entry.path,
            isDir: entry.isDir,
          })
          successCount += 1
        } catch (invokeError) {
          failures.push(`${entry.name}: ${String(invokeError)}`)
        }
      }

      await loadDirectory(listing?.currentPath ?? null)
      setDeleteDialog({ open: false, entries: [] })
      setContextMenu(null)

      if (failures.length === 0) {
        toast({
          title: t("sftp.messages.deleteSuccess", {
            count: successCount,
            defaultValue: `Deleted ${successCount} item(s).`,
          }),
        })
      } else if (successCount > 0) {
        toast({
          variant: "destructive",
          title: t("sftp.messages.deletePartialFailure", {
            successCount,
            failureCount: failures.length,
            defaultValue: `Deleted ${successCount} item(s). ${failures.length} failed.`,
          }),
          description: failures[0],
        })
      } else {
        setError(failures.join("\n"))
        toast({
          variant: "destructive",
          title: t("sftp.messages.deleteFailure", {
            defaultValue: "Failed to delete selected items.",
          }),
          description: failures[0],
        })
      }

      setIsDeleting(false)
    })()
  }, [connection, deleteDialog.entries, listing?.currentPath, loadDirectory, t, tabId])

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
        clearCompletedTransfers={clearCompletedTransfers}
        clearSelection={handleClearSelection}
        cancelTransfer={cancelTransfer}
        handleCreateDirectory={handleCreateDirectory}
        handleDeleteSelection={handleDeleteSelection}
        handleUploadDialog={handleUploadDialog}
        isLoading={isBusy}
        listingCurrentPath={listing?.currentPath}
        loadDirectory={loadDirectory}
        onClose={onClose}
        removeTransfer={removeTransfer}
        selectedCount={selectedPaths.length}
        transfers={transfers}
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
        createFolderDialog={createFolderDialog}
        deleteDialog={deleteDialog}
        handleCreateDirectoryConfirm={handleCreateDirectoryConfirm}
        handleDeleteConfirm={handleDeleteConfirm}
        handleRenameConfirm={handleRenameConfirm}
        renameDialog={renameDialog}
        setCreateFolderDialog={setCreateFolderDialog}
        setDeleteDialog={setDeleteDialog}
        setRenameDialog={setRenameDialog}
      />
    </div>
  )
}
