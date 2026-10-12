import type { TFunction } from "i18next"
import React, { useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"

import type {
  SftpDrawerProps,
  SftpDirectoryEntry,
  SftpDirectoryListing,
  SftpDialogAction,
  LoadSftpDirectory,
} from "@/components/SftpDrawer/types"
import { toast } from "@/hooks/use-toast"
import { joinRemotePath } from "@/components/SftpDrawer/sftpDrawerUtils"

interface UseSftpEntryActionsOptions {
  tabId: string
  connection: SftpDrawerProps["connection"]
  t: TFunction
  dispatchDialog: React.Dispatch<SftpDialogAction>
  createFolderName: string
  listingCurrentPath: string | null
  runAndRefresh: (action: () => Promise<void>) => Promise<void>
  contextMenuEntry: SftpDirectoryEntry | null
  activeEntry: SftpDirectoryEntry | null
  listing: SftpDirectoryListing | null
  renameEntry: SftpDirectoryEntry | null
  renameNewName: string
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
  createFolderName,
  listingCurrentPath,
  runAndRefresh,
  contextMenuEntry,
  activeEntry,
  listing,
  renameEntry,
  renameNewName,
  selectedEntries,
  selectedPaths,
  downloadEntries,
  loadDirectory,
  onOpenRemoteFile,
  setPermissionEntries,
  currentPath,
}: UseSftpEntryActionsOptions) {
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
    await downloadEntries(entries)
  }, [activeEntry, contextMenuEntry, downloadEntries, selectedEntries, selectedPaths])

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
    setPermissionEntries(entries)
  }, [activeEntry, contextMenuEntry, selectedEntries, selectedPaths, setPermissionEntries])

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
