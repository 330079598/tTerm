import { useCallback, useEffect, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { open as openDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog"
import { useTranslation } from "react-i18next"

import type { TransferTask, Tab } from "@/types/tab"

import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

interface UseSftpDownloadsParams {
  addTransfer: (
    transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">,
    id?: string
  ) => string
  connection?: Tab["connection"]
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
  resumedFrom?: number
  parallelism?: number
  completedChunks?: number
  chunkCount?: number
}

interface DownloadItemStartEvent {
  batchId: string
  fileName: string
  fileSize: number
  localPath: string
  remotePath: string
  transferId: string
}

interface DownloadItemCompleteEvent {
  cancelled: boolean
  error?: string
  localPath: string
  remotePath: string
  success: boolean
  transferId: string
  skipped?: boolean
}

interface DownloadBatchCompleteEvent {
  batchId: string
  cancelled: boolean
  error?: string
  total: number
  transferred: number
  /** Bytes of files the run skipped because the local copy was current. */
  skipped?: number
  /** Bytes of files the run resumed from a previous partial attempt. */
  resumed?: number
}

interface UseSftpDownloadsReturn {
  downloadEntry: (entry: SftpDirectoryEntry) => Promise<void>
}

export function useSftpDownloads({
  addTransfer,
  connection,
  tabId,
  transfersRef,
  updateTransfer,
}: UseSftpDownloadsParams): UseSftpDownloadsReturn {
  const { t } = useTranslation()
  const lastProgressUpdateRef = useRef<Map<string, number>>(new Map())
  const batchTransferIdsRef = useRef(new Map<string, Set<string>>())
  const transferStartTimesRef = useRef<Map<string, number>>(new Map())

  const runDownloadFile = useCallback(
    async (transferId: string, remotePath: string, localPath: string) => {
      transferStartTimesRef.current.set(transferId, Date.now())
      updateTransfer(transferId, {
        status: "transferring",
        error: undefined,
        endTime: undefined,
        speed: 0,
      })

      try {
        const startTime = Date.now()
        await invoke("sftp_download_file", {
          tabId,
          connection,
          transferId,
          remotePath,
          localPath,
        })

        const duration = Date.now() - startTime
        const currentTransfer = transfersRef.current.find((item) => item.id === transferId)
        const completedSize = currentTransfer?.fileSize || currentTransfer?.transferred || 0
        // Only the bytes actually fetched in this attempt count towards the
        // speed: a resumed transfer would otherwise report the whole file
        // size over the final attempt's duration.
        const resumedFrom = currentTransfer?.resumedFrom ?? 0
        const transferredActual = Math.max(0, completedSize - resumedFrom)
        const speed = duration > 0 ? (transferredActual / duration) * 1000 : 0
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
    [connection, tabId, transfersRef, updateTransfer]
  )

  const runDownloadDirectory = useCallback(
    async (
      transferId: string,
      remotePath: string,
      localParentPath: string,
      skipExisting = false
    ) => {
      transferStartTimesRef.current.set(transferId, Date.now())
      updateTransfer(transferId, {
        status: "transferring",
        error: undefined,
        endTime: undefined,
        speed: 0,
      })

      try {
        await invoke("sftp_download_directory", {
          tabId,
          connection,
          transferId,
          remotePath,
          localParentPath,
          skipExisting,
        })
        lastProgressUpdateRef.current.delete(transferId)
        transferStartTimesRef.current.delete(transferId)
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
    [connection, tabId, updateTransfer]
  )

  useEffect(() => {
    const appWindow = getCurrentWindow()
    let disposed = false
    const unlisteners: Array<() => void> = []

    const setupListeners = async () => {
      const nextUnlisteners = await Promise.all([
        appWindow.listen<DownloadItemStartEvent>(`sftp-download-item-start-${tabId}`, (event) => {
          const { batchId, fileName, fileSize, localPath, remotePath, transferId } = event.payload
          const nextIds = new Set(batchTransferIdsRef.current.get(batchId) ?? [])
          nextIds.add(transferId)
          batchTransferIdsRef.current.set(batchId, nextIds)

          addTransfer(
            {
              batchId,
              tabId,
              direction: "download",
              fileName,
              fileSize,
              localPath,
              remotePath,
              speed: 0,
            },
            transferId
          )
          transferStartTimesRef.current.set(transferId, Date.now())
          updateTransfer(transferId, {
            batchId,
            error: undefined,
            fileSize,
            speed: 0,
            status: "transferring",
            retry: () => {
              void runDownloadFile(transferId, remotePath, localPath)
            },
          })
        }),
        appWindow.listen<DownloadProgressEvent>(`sftp-download-progress-${tabId}`, (event) => {
          const { progress, resumedFrom, parallelism, total, transferId, transferred } =
            event.payload
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
          const measured = Math.max(0, transferred - (resumedFrom ?? 0))
          const speed = duration > 0 ? (measured / duration) * 1000 : 0

          updateTransfer(transferId, {
            transferred,
            fileSize: total,
            speed,
            status: "transferring",
            resumedFrom: resumedFrom && resumedFrom > 0 ? resumedFrom : undefined,
            parallelism: parallelism && parallelism > 0 ? parallelism : undefined,
          })
        }),
        appWindow.listen<DownloadItemCompleteEvent>(
          `sftp-download-item-complete-${tabId}`,
          (event) => {
            const { cancelled, error, skipped, success, transferId } = event.payload
            const transfer = transfersRef.current.find((item) => item.id === transferId)
            const now = Date.now()
            const duration = now - (transferStartTimesRef.current.get(transferId) || now)
            const completedFileSize = transfer?.fileSize || transfer?.transferred || 0
            // Exclude bytes restored by resume from the final speed, matching
            // the in-progress calculation. A skipped item moved nothing at all
            // in this run.
            const transferredActual = skipped
              ? 0
              : Math.max(0, completedFileSize - (transfer?.resumedFrom ?? 0))
            const speed = duration > 0 ? (transferredActual / duration) * 1000 : 0

            lastProgressUpdateRef.current.delete(transferId)
            transferStartTimesRef.current.delete(transferId)

            if (success) {
              updateTransfer(transferId, {
                endTime: now,
                fileSize: completedFileSize,
                speed,
                status: "completed",
                transferred: completedFileSize,
              })
              return
            }

            if (cancelled) {
              updateTransfer(transferId, {
                endTime: now,
                error: undefined,
                status: "cancelled",
              })
              return
            }

            updateTransfer(transferId, {
              endTime: now,
              error: error || "Download failed",
              status: "failed",
            })
          }
        ),
        appWindow.listen<DownloadBatchCompleteEvent>(
          `sftp-download-batch-complete-${tabId}`,
          (event) => {
            const { batchId, cancelled, error, resumed, skipped, total, transferred } =
              event.payload
            const childTransferIds = batchTransferIdsRef.current.get(batchId) ?? new Set<string>()
            const now = Date.now()
            const startTime = transferStartTimesRef.current.get(batchId) || now
            const duration = now - startTime
            // Skipped files and resumed bytes are part of `transferred` so the
            // bar reaches 100%, but no bytes moved for them in this run:
            // counting them would report more than what was fetched.
            const measured = Math.max(0, transferred - (skipped ?? 0) - (resumed ?? 0))
            const speed = duration > 0 ? (measured / duration) * 1000 : 0

            for (const transferId of childTransferIds) {
              lastProgressUpdateRef.current.delete(transferId)
              transferStartTimesRef.current.delete(transferId)

              const transfer = transfersRef.current.find((item) => item.id === transferId)
              if (cancelled && transfer?.status === "transferring") {
                updateTransfer(transferId, {
                  endTime: now,
                  error: undefined,
                  status: "cancelled",
                })
              }
            }

            batchTransferIdsRef.current.delete(batchId)
            lastProgressUpdateRef.current.delete(batchId)
            transferStartTimesRef.current.delete(batchId)

            if (cancelled) {
              updateTransfer(batchId, {
                endTime: now,
                error: undefined,
                status: "cancelled",
              })
              return
            }

            if (error) {
              updateTransfer(batchId, {
                endTime: now,
                error,
                status: "failed",
              })
              return
            }

            updateTransfer(batchId, {
              endTime: now,
              error: undefined,
              fileSize: total,
              speed,
              status: "completed",
              transferred,
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

    void setupListeners().catch((error) => {
      console.warn("Failed to subscribe to SFTP download progress:", error)
    })

    return () => {
      disposed = true
      unlisteners.forEach((unlisten) => unlisten())
    }
  }, [addTransfer, runDownloadFile, tabId, transfersRef, updateTransfer])

  const downloadEntry = useCallback(
    async (entry: SftpDirectoryEntry) => {
      if (entry.isDir) {
        const targetPath = await openDialog({
          title: t("sftp.actions.downloadFolder", { defaultValue: "Download Folder" }),
          directory: true,
          multiple: false,
        })
        if (!targetPath || Array.isArray(targetPath)) {
          return
        }

        const transferId = addTransfer({
          tabId,
          direction: "download",
          localPath: targetPath,
          remotePath: entry.path,
          fileName: entry.name,
          fileSize: entry.size || 0,
          speed: 0,
        })

        updateTransfer(transferId, {
          retry: () => {
            // A retry re-runs the whole folder: files the previous attempt
            // already finished are skipped instead of fetched again.
            void runDownloadDirectory(transferId, entry.path, targetPath, true)
          },
        })

        await runDownloadDirectory(transferId, entry.path, targetPath)
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
        tabId,
        direction: "download",
        localPath: targetPath,
        remotePath: entry.path,
        fileName: entry.name,
        fileSize: entry.size || 0,
        speed: 0,
      })

      updateTransfer(transferId, {
        retry: () => {
          void runDownloadFile(transferId, entry.path, targetPath)
        },
      })

      await runDownloadFile(transferId, entry.path, targetPath)
    },
    [addTransfer, runDownloadDirectory, runDownloadFile, t, tabId, updateTransfer]
  )

  return {
    downloadEntry,
  }
}
