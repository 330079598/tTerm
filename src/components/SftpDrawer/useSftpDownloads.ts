import { useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"
import { save as saveFileDialog } from "@tauri-apps/plugin-dialog"
import { useTranslation } from "react-i18next"

import type { TransferTask, Tab } from "@/types/tab"

import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

interface UseSftpDownloadsParams {
  addTransfer: (
    transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">,
    id?: string
  ) => string
  connection?: Tab["connection"]
  loadDirectory: (path?: string | null) => Promise<void>
  tabId: string
  updateTransfer: (id: string, updates: Partial<TransferTask>) => void
}

interface UseSftpDownloadsReturn {
  downloadEntry: (entry: SftpDirectoryEntry) => Promise<void>
  handleOpenEntry: (entry: SftpDirectoryEntry) => Promise<void>
}

export function useSftpDownloads({
  addTransfer,
  connection,
  loadDirectory,
  tabId,
  updateTransfer,
}: UseSftpDownloadsParams): UseSftpDownloadsReturn {
  const { t } = useTranslation()

  const downloadEntry = useCallback(
    async (entry: SftpDirectoryEntry) => {
      if (entry.isDir) {
        return
      }

      const targetPath = await saveFileDialog({
        title: t("sftp.actions.download", { defaultValue: "Download File" }),
        defaultPath: entry.name,
      })
      if (!targetPath) {
        return
      }

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
      } catch (invokeError) {
        updateTransfer(transferId, {
          status: "failed",
          error: String(invokeError),
          endTime: Date.now(),
        })
      }
    },
    [addTransfer, connection, t, tabId, updateTransfer]
  )

  const handleOpenEntry = useCallback(
    async (entry: SftpDirectoryEntry) => {
      if (entry.isDir) {
        void loadDirectory(entry.path)
      } else {
        await downloadEntry(entry)
      }
    },
    [downloadEntry, loadDirectory]
  )

  return {
    downloadEntry,
    handleOpenEntry,
  }
}
