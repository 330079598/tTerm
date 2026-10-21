import { invoke } from "@tauri-apps/api/core"
import { useCallback, useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import {
  archiveFormatOf,
  commonParent,
  defaultArchiveName,
  defaultExtractDestination,
  type ArchiveFormat,
  type CompressFormat,
} from "@/components/SftpDrawer/sftpArchive"
import { joinRemotePath, parentRemotePath } from "@/components/SftpDrawer/sftpDrawerUtils"
import type {
  LoadSftpDirectory,
  SftpDirectoryEntry,
  SftpDrawerProps,
} from "@/components/SftpDrawer/types"
import type { useTransferManager } from "@/contexts/TransferContext"
import { toast } from "@/hooks/use-toast"

export type SftpArchiveDialogState =
  | { type: "none" }
  | {
      type: "compress"
      entries: SftpDirectoryEntry[]
      directory: string
      name: string
      format: CompressFormat
    }
  | {
      type: "extract"
      entry: SftpDirectoryEntry
      format: ArchiveFormat
      destination: string
      overwrite: boolean
    }

interface RemoteOperationResult {
  cancelled: boolean
}

interface UseSftpArchiveOptions {
  tabId: string
  connection: SftpDrawerProps["connection"]
  addTransfer: ReturnType<typeof useTransferManager>["addTransfer"]
  updateTransfer: ReturnType<typeof useTransferManager>["updateTransfer"]
  listingCurrentPath: string | null
  loadDirectory: LoadSftpDirectory
}

interface RemoteOperation {
  direction: "compress" | "extract"
  command: "sftp_compress_entries" | "sftp_extract_archive"
  request: Record<string, unknown>
  fileName: string
  remotePath: string
  /** Folders whose listing changes; one being shown is reloaded. */
  touchedFolders: string[]
  successTitle: string
  failureTitle: string
}

/** `path` as typed, made absolute against `base` and without a trailing slash. */
function resolveFolder(base: string, path: string) {
  const absolute = path.startsWith("/") ? path : joinRemotePath(base, path)
  return absolute.replace(/\/+$/, "") || "/"
}

/**
 * Compressing entries and extracting archives on the server: the dialogs
 * that ask how, and the commands, which show in the transfers panel.
 */
export function useSftpArchive({
  tabId,
  connection,
  addTransfer,
  updateTransfer,
  listingCurrentPath,
  loadDirectory,
}: UseSftpArchiveOptions) {
  const { t } = useTranslation()
  const [dialog, setDialog] = useState<SftpArchiveDialogState>({ type: "none" })
  // An operation can outlast a visit to its folder; the listing is reloaded
  // only when the user is still looking at a folder it changed.
  const currentPathRef = useRef(listingCurrentPath)
  useEffect(() => {
    currentPathRef.current = listingCurrentPath
  }, [listingCurrentPath])

  const openCompress = useCallback(
    (entries: SftpDirectoryEntry[]) => {
      if (entries.length === 0) return
      const directory = commonParent(entries)
      if (!directory) {
        toast({ variant: "destructive", title: t("sftp.archive.differentFolders") })
        return
      }
      setDialog({
        type: "compress",
        entries,
        directory,
        name: defaultArchiveName(entries, directory, "tarGz"),
        format: "tarGz",
      })
    },
    [t]
  )

  const openExtract = useCallback((entry: SftpDirectoryEntry) => {
    const format = archiveFormatOf(entry)
    if (!format) return
    setDialog({
      type: "extract",
      entry,
      format,
      destination: defaultExtractDestination(entry.path, format),
      overwrite: false,
    })
  }, [])

  const run = useCallback(
    async (operation: RemoteOperation) => {
      const operationId = crypto.randomUUID()
      addTransfer(
        {
          tabId,
          direction: operation.direction,
          fileName: operation.fileName,
          fileSize: 0,
          localPath: "",
          remotePath: operation.remotePath,
          speed: 0,
          cancel: () => invoke("sftp_cancel_remote_operation", { operationId }),
        },
        operationId
      )
      updateTransfer(operationId, { status: "transferring" })
      try {
        const result = await invoke<RemoteOperationResult>(operation.command, {
          tabId,
          connection,
          operationId,
          request: operation.request,
        })
        if (result.cancelled) {
          updateTransfer(operationId, { status: "cancelled", endTime: Date.now() })
          return
        }
        updateTransfer(operationId, { status: "completed", endTime: Date.now() })
        toast({ title: operation.successTitle, description: operation.remotePath })
        const current = currentPathRef.current
        if (current && operation.touchedFolders.includes(current)) {
          await loadDirectory(current)
        }
      } catch (invokeError) {
        const message = String(invokeError)
        updateTransfer(operationId, { status: "failed", error: message, endTime: Date.now() })
        toast({ variant: "destructive", title: operation.failureTitle, description: message })
      }
    },
    [addTransfer, connection, loadDirectory, tabId, updateTransfer]
  )

  const confirm = useCallback(() => {
    if (dialog.type === "compress") {
      const name = dialog.name.trim()
      if (!name) return
      const archivePath = joinRemotePath(dialog.directory, name)
      void run({
        direction: "compress",
        command: "sftp_compress_entries",
        request: {
          directory: dialog.directory,
          names: dialog.entries.map((entry) => entry.name),
          archiveName: name,
          format: dialog.format,
        },
        fileName: name,
        remotePath: archivePath,
        touchedFolders: [dialog.directory],
        successTitle: t("sftp.archive.compressSuccess", { name }),
        failureTitle: t("sftp.archive.compressFailure"),
      })
    } else if (dialog.type === "extract") {
      if (!dialog.destination.trim()) return
      const destination = resolveFolder(
        parentRemotePath(dialog.entry.path),
        dialog.destination.trim()
      )
      void run({
        direction: "extract",
        command: "sftp_extract_archive",
        request: {
          archivePath: dialog.entry.path,
          destination,
          format: dialog.format,
          overwrite: dialog.overwrite,
        },
        fileName: dialog.entry.name,
        remotePath: destination,
        touchedFolders: [destination, parentRemotePath(destination)],
        successTitle: t("sftp.archive.extractSuccess", { name: dialog.entry.name }),
        failureTitle: t("sftp.archive.extractFailure"),
      })
    }
    setDialog({ type: "none" })
  }, [dialog, run, t])

  return {
    openCompress,
    openExtract,
    dialogProps: { dialog, setDialog, onConfirm: confirm },
  }
}
