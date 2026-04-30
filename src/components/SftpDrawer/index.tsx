import "@/components/SftpDrawer.css"
import React, { useCallback, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { AlertCircle } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { toast } from "@/hooks/use-toast"

import { SftpDeleteTransferEvents } from "@/components/SftpDrawer/SftpDeleteTransferEvents"
import { SftpDrawerContent } from "@/components/SftpDrawer/SftpDrawerContent"
import { SftpDrawerHeader } from "@/components/SftpDrawer/SftpDrawerHeader"
import { SftpDialogs } from "@/components/SftpDrawer/SftpDialogs"
import { SftpEntryContextMenu } from "@/components/SftpDrawer/SftpEntryContextMenu"
import { useSftpDragDrop } from "@/components/SftpDrawer/useSftpDragDrop"
import { useSftpSelection } from "@/components/SftpDrawer/useSftpSelection"
import { joinRemotePath } from "@/components/SftpDrawer/sftpDrawerUtils"
import {
  createSftpSearchMatcher,
  DEFAULT_SFTP_SEARCH_OPTIONS,
  type SftpSearchOptions,
} from "@/components/SftpDrawer/sftpSearch"
import type {
  DeleteBatchStartResult,
  DeletePreviewResult,
  SftpCommandDeleteDialogState,
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
  const [searchQuery, setSearchQuery] = useState("")
  const [searchOptions, setSearchOptions] = useState<SftpSearchOptions>(DEFAULT_SFTP_SEARCH_OPTIONS)
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
        setSearchQuery("")
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

  const searchMatcher = useMemo(
    () => createSftpSearchMatcher(searchQuery, searchOptions),
    [searchOptions, searchQuery]
  )

  const filteredListing = useMemo(() => {
    if (!listing || !searchMatcher.hasQuery) {
      return listing
    }

    return {
      ...listing,
      entries: listing.entries.filter(searchMatcher.matches),
    }
  }, [listing, searchMatcher])

  const toggleSearchOption = useCallback((option: keyof SftpSearchOptions) => {
    setSearchOptions((current) => ({
      ...current,
      [option]: !current[option],
    }))
  }, [])

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
  const {
    activeEntry,
    breadcrumbs,
    contextMenuEntry,
    handleActivateEntry,
    handleClearSelection,
    handleSelectRange,
    handleToggleEntrySelection,
    selectedEntries,
  } = useSftpSelection({
    activePath,
    contextMenu,
    listing,
    rangeEntries: filteredListing?.entries,
    selectedPaths,
    setActivePath,
    setSelectedPaths,
  })

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

  const handleCopyPath = useCallback(async () => {
    const entry = contextMenuEntry ?? activeEntry
    if (!entry) {
      return
    }

    try {
      await invoke("plugin:clipboard-manager|write_text", { text: entry.path })
      toast({
        title: t("sftp.messages.copyPathSuccess", {
          defaultValue: "Full path copied.",
        }),
        description: entry.path,
      })
    } catch (invokeError) {
      console.error("Failed to copy SFTP path:", invokeError)
      toast({
        variant: "destructive",
        title: t("sftp.messages.copyPathFailure", {
          defaultValue: "Failed to copy full path.",
        }),
        description: String(invokeError),
      })
    }
  }, [activeEntry, contextMenuEntry, t])

  const contextSelectionCount = useMemo(() => {
    if (!contextMenuEntry) {
      return selectedPaths.length
    }

    return selectedPaths.includes(contextMenuEntry.path) ? selectedPaths.length : 1
  }, [contextMenuEntry, selectedPaths])

  const isBusy = isLoading || isDeleting

  return (
    <div className={`sftp-drawer ${visible ? "is-open" : ""}`} aria-hidden={!visible}>
      <SftpDeleteTransferEvents
        addTransfer={addTransfer}
        listingCurrentPath={listing?.currentPath}
        loadDirectory={loadDirectory}
        setError={setError}
        setIsDeleting={setIsDeleting}
        t={t}
        tabId={tabId}
        transfersRef={transfersRef}
        updateTransfer={updateTransfer}
      />
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
        searchError={searchMatcher.error?.message ?? null}
        searchOptions={searchOptions}
        searchQuery={searchQuery}
        selectedCount={selectedPaths.length}
        setSearchQuery={setSearchQuery}
        toggleSearchOption={toggleSearchOption}
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
        searchMatcher={searchMatcher}
        selectedPaths={selectedPaths}
        setContextMenu={setContextMenu}
      />

      <SftpEntryContextMenu
        contextMenu={contextMenu}
        contextMenuEntry={contextMenuEntry}
        handleCopyPath={handleCopyPath}
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
