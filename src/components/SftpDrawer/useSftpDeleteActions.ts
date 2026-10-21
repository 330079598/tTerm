import type { TFunction } from "i18next"
import React, { useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"

import type {
  SftpDrawerProps,
  SftpDirectoryEntry,
  SftpDirectoryListing,
  SftpContextMenuState,
  SftpDialogAction,
  SftpDialogState,
  DeleteBatchStartResult,
  DeletePreviewResult,
} from "@/components/SftpDrawer/types"
import { toast } from "@/hooks/use-toast"

const NO_ENTRIES: SftpDirectoryEntry[] = []

function isDeletedEntryPath(path: string, deletedPaths: Set<string>) {
  for (const deletedPath of deletedPaths) {
    if (path === deletedPath || path.startsWith(`${deletedPath}/`)) {
      return true
    }
  }

  return false
}

interface UseSftpDeleteActionsOptions {
  tabId: string
  connection: SftpDrawerProps["connection"]
  t: TFunction
  isDeleting: boolean
  setIsDeleting: React.Dispatch<React.SetStateAction<boolean>>
  selectedEntries: SftpDirectoryEntry[]
  selectedPaths: string[]
  contextMenuEntry: SftpDirectoryEntry | null
  activeEntry: SftpDirectoryEntry | null
  dispatchDialog: React.Dispatch<SftpDialogAction>
  setError: React.Dispatch<React.SetStateAction<string | null>>
  setContextMenu: React.Dispatch<React.SetStateAction<SftpContextMenuState | null>>
  setListing: React.Dispatch<React.SetStateAction<SftpDirectoryListing | null>>
  setSelectedPaths: React.Dispatch<React.SetStateAction<string[]>>
  setActivePath: React.Dispatch<React.SetStateAction<string | null>>
  dialog: SftpDialogState
}

/**
 * Deleting entries: asking first, previewing what a folder holds, offering
 * a remote command for large trees, and dropping what was deleted from the
 * listing.
 */
export function useSftpDeleteActions({
  tabId,
  connection,
  t,
  isDeleting,
  setIsDeleting,
  selectedEntries,
  selectedPaths,
  contextMenuEntry,
  activeEntry,
  dispatchDialog,
  setError,
  setContextMenu,
  setListing,
  setSelectedPaths,
  setActivePath,
  dialog,
}: UseSftpDeleteActionsOptions) {
  const deleteDialogEntries = dialog.type === "delete" ? dialog.entries : NO_ENTRIES
  const commandDeleteEntries = dialog.type === "commandDelete" ? dialog.entries : NO_ENTRIES
  const commandDeleteCommand = dialog.type === "commandDelete" ? dialog.command : ""

  const handleDeleteSelection = useCallback(() => {
    if (isDeleting) {
      return
    }

    if (selectedEntries.length === 0) {
      return
    }

    dispatchDialog({ action: "openDelete", entries: selectedEntries })
  }, [isDeleting, selectedEntries, dispatchDialog])

  const handleDelete = useCallback(() => {
    if (isDeleting) {
      return
    }

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

    dispatchDialog({ action: "openDelete", entries })
  }, [activeEntry, contextMenuEntry, isDeleting, selectedEntries, selectedPaths, dispatchDialog])

  const startDeleteBatch = useCallback(
    async (entries: SftpDirectoryEntry[], useCommandDelete: boolean, command?: string) => {
      setIsDeleting(true)
      setError(null)
      dispatchDialog({ action: "close" })
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
        const deletedPaths = new Set(entries.map((entry) => entry.path))
        setListing((current) =>
          current
            ? {
                ...current,
                entries: current.entries.filter(
                  (entry) => !isDeletedEntryPath(entry.path, deletedPaths)
                ),
              }
            : current
        )
        setSelectedPaths((current) =>
          current.filter((path) => !isDeletedEntryPath(path, deletedPaths))
        )
        setActivePath((current) =>
          current && isDeletedEntryPath(current, deletedPaths) ? null : current
        )
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
    [
      connection,
      t,
      tabId,
      dispatchDialog,
      setActivePath,
      setContextMenu,
      setError,
      setIsDeleting,
      setListing,
      setSelectedPaths,
    ]
  )

  const handleDeleteConfirm = useCallback(() => {
    if (isDeleting) {
      return
    }

    if (deleteDialogEntries.length === 0) {
      return
    }

    void (async () => {
      setIsDeleting(true)
      setError(null)
      try {
        const preview = await invoke<DeletePreviewResult>("sftp_preview_delete_entries", {
          tabId,
          connection,
          entries: deleteDialogEntries.map((entry) => ({
            path: entry.path,
            name: entry.name,
            isDir: entry.isDir,
          })),
        })

        setIsDeleting(false)
        if (preview.shouldPromptForCommand) {
          dispatchDialog({
            action: "openCommandDelete",
            entries: deleteDialogEntries,
            command: preview.command,
            totalDirectories: preview.totalDirectories,
            totalEntries: preview.totalEntries,
            totalFiles: preview.totalFiles,
            totalTruncated: preview.totalTruncated,
          })
          return
        }

        await startDeleteBatch(deleteDialogEntries, false)
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
  }, [
    connection,
    deleteDialogEntries,
    isDeleting,
    startDeleteBatch,
    t,
    tabId,
    dispatchDialog,
    setError,
    setIsDeleting,
  ])

  const handleCommandDeleteConfirm = useCallback(() => {
    if (isDeleting) {
      return
    }

    const command = commandDeleteCommand.trim()
    if (commandDeleteEntries.length === 0 || !command) {
      return
    }

    void startDeleteBatch(commandDeleteEntries, true, command)
  }, [commandDeleteCommand, commandDeleteEntries, isDeleting, startDeleteBatch])

  const handleSftpDeleteConfirm = useCallback(() => {
    if (isDeleting) {
      return
    }

    if (commandDeleteEntries.length === 0) {
      return
    }

    void startDeleteBatch(commandDeleteEntries, false)
  }, [commandDeleteEntries, isDeleting, startDeleteBatch])

  return {
    handleDeleteSelection,
    handleDelete,
    handleDeleteConfirm,
    handleCommandDeleteConfirm,
    handleSftpDeleteConfirm,
  }
}
