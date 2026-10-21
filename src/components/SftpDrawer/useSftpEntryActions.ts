import type { TFunction } from "i18next"
import React, { useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"

import type {
  SftpDrawerProps,
  SftpDirectoryEntry,
  SftpDirectoryListing,
  SftpDialogAction,
  SftpDialogState,
  LoadSftpDirectory,
} from "@/components/SftpDrawer/types"
import { toast } from "@/hooks/use-toast"
import { joinRemotePath } from "@/components/SftpDrawer/sftpDrawerUtils"

interface UseSftpEntryActionsOptions {
  tabId: string
  connection: SftpDrawerProps["connection"]
  t: TFunction
  dispatchDialog: React.Dispatch<SftpDialogAction>
  dialog: SftpDialogState
  listingCurrentPath: string | null
  setError: (error: string | null) => void
  contextMenuEntry: SftpDirectoryEntry | null
  activeEntry: SftpDirectoryEntry | null
  listing: SftpDirectoryListing | null
  selectedEntries: SftpDirectoryEntry[]
  selectedPaths: string[]
  downloadEntries: (entries: SftpDirectoryEntry[]) => Promise<void>
  loadDirectory: LoadSftpDirectory
  onOpenRemoteFile: SftpDrawerProps["onOpenRemoteFile"]
  setPermissionEntries: React.Dispatch<React.SetStateAction<SftpDirectoryEntry[]>>
  currentPath: string | null
}

/**
 * What can be done to entries besides deleting them: creating a folder,
 * renaming, downloading, opening or editing, permissions and copying a path.
 */
export function useSftpEntryActions({
  tabId,
  connection,
  t,
  dispatchDialog,
  dialog,
  listingCurrentPath,
  setError,
  contextMenuEntry,
  activeEntry,
  listing,
  selectedEntries,
  selectedPaths,
  downloadEntries,
  loadDirectory,
  onOpenRemoteFile,
  setPermissionEntries,
  currentPath,
}: UseSftpEntryActionsOptions) {
  const createFolderName = dialog.type === "createFolder" ? dialog.folderName : ""
  const renameEntry = dialog.type === "rename" ? dialog.entry : null
  const renameNewName = dialog.type === "rename" ? dialog.newName : ""
  // What a menu action applies to: the whole selection when the menu opened
  // on a selected row, else the row it opened on.
  const targetEntries = useCallback(
    () =>
      contextMenuEntry &&
      selectedPaths.includes(contextMenuEntry.path) &&
      selectedEntries.length > 0
        ? selectedEntries
        : contextMenuEntry
          ? [contextMenuEntry]
          : activeEntry
            ? [activeEntry]
            : [],
    [activeEntry, contextMenuEntry, selectedEntries, selectedPaths]
  )

  const runAndRefresh = useCallback(
    async (action: () => Promise<void>) => {
      setError(null)
      try {
        await action()
        await loadDirectory(currentPath)
      } catch (invokeError) {
        setError(String(invokeError))
      }
    },
    [currentPath, loadDirectory, setError]
  )

  const handleCreateDirectory = useCallback(() => {
    dispatchDialog({ action: "openCreateFolder" })
  }, [dispatchDialog])

  const handleCreateDirectoryConfirm = useCallback(() => {
    const trimmedName = createFolderName.trim()
    if (!trimmedName || !listingCurrentPath) {
      return
    }

    void runAndRefresh(() =>
      invoke("sftp_create_directory", {
        tabId,
        connection,
        path: joinRemotePath(listingCurrentPath, trimmedName),
      })
    )
    dispatchDialog({ action: "close" })
  }, [connection, createFolderName, listingCurrentPath, runAndRefresh, tabId, dispatchDialog])

  const handleRename = useCallback(() => {
    const entry = contextMenuEntry ?? activeEntry
    if (!entry || !listing) {
      return
    }

    dispatchDialog({ action: "openRename", entry, newName: entry.name })
  }, [activeEntry, contextMenuEntry, listing, dispatchDialog])

  const handleRenameConfirm = useCallback(() => {
    if (!renameEntry || !listingCurrentPath) {
      return
    }

    const trimmedName = renameNewName.trim()
    if (!trimmedName || trimmedName === renameEntry.name) {
      dispatchDialog({ action: "close" })
      return
    }

    void runAndRefresh(() =>
      invoke("sftp_rename_entry", {
        tabId,
        connection,
        oldPath: renameEntry.path,
        newPath: joinRemotePath(listingCurrentPath, trimmedName),
      })
    )
    dispatchDialog({ action: "close" })
  }, [
    connection,
    listingCurrentPath,
    renameEntry,
    renameNewName,
    runAndRefresh,
    tabId,
    dispatchDialog,
  ])

  const handleDownload = useCallback(async () => {
    await downloadEntries(targetEntries())
  }, [downloadEntries, targetEntries])

  const handleDownloadSelection = useCallback(async () => {
    await downloadEntries(selectedEntries)
  }, [downloadEntries, selectedEntries])

  const handleOpenEntry = useCallback(
    async (entry: SftpDirectoryEntry) => {
      if (entry.isDir) {
        await loadDirectory(entry.path)
        return
      }

      onOpenRemoteFile?.(entry, tabId, connection)
    },
    [connection, loadDirectory, onOpenRemoteFile, tabId]
  )

  const handleEdit = useCallback(() => {
    const entry = contextMenuEntry ?? activeEntry
    if (!entry || entry.isDir) {
      return
    }

    onOpenRemoteFile?.(entry, tabId, connection)
  }, [activeEntry, connection, contextMenuEntry, onOpenRemoteFile, tabId])

  const handlePermissions = useCallback(() => {
    setPermissionEntries(targetEntries())
  }, [setPermissionEntries, targetEntries])

  const handleApplyPermissions = useCallback(
    async (entries: SftpDirectoryEntry[], mode: number) => {
      await invoke("sftp_set_permissions", {
        tabId,
        connection,
        paths: entries.map((entry) => entry.path),
        mode,
      })
      await loadDirectory(currentPath)
    },
    [connection, currentPath, loadDirectory, tabId]
  )

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

  return {
    handleCreateDirectory,
    handleCreateDirectoryConfirm,
    handleRename,
    handleRenameConfirm,
    handleDownload,
    handleDownloadSelection,
    handleOpenEntry,
    handleEdit,
    handlePermissions,
    handleApplyPermissions,
    handleCopyPath,
  }
}
