import "@/components/SftpDrawer.css"
import React, { useCallback, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { AlertCircle } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Alert, AlertDescription } from "@/components/ui/alert"

import { SftpDrawerContent } from "@/components/SftpDrawer/SftpDrawerContent"
import { SftpDrawerHeader } from "@/components/SftpDrawer/SftpDrawerHeader"
import { SftpDialogs } from "@/components/SftpDrawer/SftpDialogs"
import { SftpEntryContextMenu } from "@/components/SftpDrawer/SftpEntryContextMenu"
import { useSftpDragDrop } from "@/components/SftpDrawer/useSftpDragDrop"
import { useSftpTransfers } from "@/components/SftpDrawer/useSftpTransfers"
import { joinRemotePath } from "@/components/SftpDrawer/sftpDrawerUtils"
import type {
  SftpContextMenuState,
  SftpCreateFolderDialogState,
  SftpDeleteDialogState,
  SftpDirectoryListing,
  SftpDrawerProps,
  SftpRenameDialogState,
} from "@/components/SftpDrawer/types"

export const SftpDrawer: React.FC<SftpDrawerProps> = ({ tabId, visible, connection, onClose }) => {
  const { t } = useTranslation()
  const [listing, setListing] = useState<SftpDirectoryListing | null>(null)
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [contextMenu, setContextMenu] = useState<SftpContextMenuState | null>(null)
  const [deleteDialog, setDeleteDialog] = useState<SftpDeleteDialogState>({
    open: false,
    entry: null,
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
        setSelectedPath(null)
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
    const entry = contextMenuEntry || selectedEntry
    if (!entry || !listing) {
      return
    }

    setRenameDialog({ open: true, entry, newName: entry.name })
  }, [contextMenuEntry, listing, selectedEntry])

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

  const handleDelete = useCallback(() => {
    const entry = contextMenuEntry || selectedEntry
    if (!entry) {
      return
    }

    setDeleteDialog({ open: true, entry })
  }, [contextMenuEntry, selectedEntry])

  const handleDeleteConfirm = useCallback(() => {
    const { entry } = deleteDialog
    if (!entry) {
      return
    }

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
    if (!entry || entry.isDir) {
      return
    }

    await downloadEntry(entry)
  }, [contextMenuEntry, downloadEntry, selectedEntry])

  return (
    <div className={`sftp-drawer ${visible ? "is-open" : ""}`} aria-hidden={!visible}>
      <SftpDrawerHeader
        breadcrumbs={breadcrumbs}
        clearCompletedTransfers={clearCompletedTransfers}
        cancelTransfer={cancelTransfer}
        handleCreateDirectory={handleCreateDirectory}
        handleUploadDialog={handleUploadDialog}
        isLoading={isLoading}
        listingCurrentPath={listing?.currentPath}
        loadDirectory={loadDirectory}
        onClose={onClose}
        removeTransfer={removeTransfer}
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
        handleDragEnter={handleDragEnter}
        handleDragLeave={handleDragLeave}
        handleDragOver={handleDragOver}
        handleDrop={handleDrop}
        handleOpenEntry={handleOpenEntry}
        isDragActive={isDragActive}
        isLoading={isLoading}
        listing={listing}
        loadDirectory={loadDirectory}
        selectedPath={selectedPath}
        setContextMenu={setContextMenu}
        setSelectedPath={setSelectedPath}
      />

      <SftpEntryContextMenu
        contextMenu={contextMenu}
        contextMenuEntry={contextMenuEntry}
        handleDelete={handleDelete}
        handleDownload={handleDownload}
        handleRename={handleRename}
        onClose={() => setContextMenu(null)}
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
