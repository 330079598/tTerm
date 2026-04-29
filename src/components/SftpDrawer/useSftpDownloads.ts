import { useCallback, useEffect, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
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
  transfersRef: React.MutableRefObject<TransferTask[]>
  updateTransfer: (id: string, updates: Partial<TransferTask>) => void
}

interface DownloadProgressEvent {
  localPath: string
  progress: number
  remotePath: string
  total: number
  transferred: number
  transferId: string
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
  transfersRef,
  updateTransfer,
}: UseSftpDownloadsParams): UseSftpDownloadsReturn {
  const { t } = useTranslation()
  const lastProgressUpdateRef = useRef<Map<string, number>>(new Map())
  const transferStartTimesRef = useRef<Map<string, number>>(new Map())

  useEffect(() => {
    const appWindow = getCurrentWindow()
    let unlisten: (() => void) | undefined

    appWindow
      .listen<DownloadProgressEvent>(`sftp-download-progress-${tabId}`, (event) => {
        const { progress, total, transferId, transferred } = event.payload
        const now = Date.now()
        const lastUpdate = lastProgressUpdateRef.current.get(transferId) || 0
        if (now - lastUpdate < 100 && progress < 100) {
          return
        }

        lastProgressUpdateRef.current.set(transferId, now)
        const currentTransfer = transfersRef.current.find((item) => item.id === transferId)
        if (currentTransfer?.status === "cancelled") {
          return
        }

        const startTime = transferStartTimesRef.current.get(transferId) || now
        const duration = now - startTime
        const speed = duration > 0 ? (transferred / duration) * 1000 : 0

        updateTransfer(transferId, {
          transferred,
          fileSize: total,
          speed,
          status: "transferring",
        })
      })
      .then((cleanup) => {
        unlisten = cleanup
      })
      .catch((error) => {
        console.warn("Failed to subscribe to SFTP download progress:", error)
      })

    return () => {
      unlisten?.()
    }
  }, [tabId, transfersRef, updateTransfer])

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

      transferStartTimesRef.current.set(transferId, Date.now())
      updateTransfer(transferId, { status: "transferring" })

      try {
        const startTime = Date.now()
        await invoke("sftp_download_file", {
          tabId,
          connection,
          transferId,
          remotePath: entry.path,
          localPath: targetPath,
        })

        const duration = Date.now() - startTime
        const currentTransfer = transfersRef.current.find((item) => item.id === transferId)
        const completedSize =
          entry.size || currentTransfer?.fileSize || currentTransfer?.transferred || 0
        const speed = duration > 0 ? (completedSize / duration) * 1000 : 0
        lastProgressUpdateRef.current.delete(transferId)
        transferStartTimesRef.current.delete(transferId)

        if (currentTransfer?.status === "cancelled") {
          return
        }

        updateTransfer(transferId, {
          status: "completed",
          transferred: completedSize,
          fileSize: completedSize,
          endTime: Date.now(),
          speed,
        })
      } catch (invokeError) {
        const error = String(invokeError)
        const cancelled = error.toLowerCase().includes("cancelled")
        lastProgressUpdateRef.current.delete(transferId)
        transferStartTimesRef.current.delete(transferId)

        updateTransfer(transferId, {
          status: cancelled ? "cancelled" : "failed",
          error: cancelled ? undefined : error,
          endTime: Date.now(),
        })
      }
    },
    [addTransfer, connection, t, tabId, transfersRef, updateTransfer]
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
