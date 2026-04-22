import { useCallback, useEffect, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { useTranslation } from "react-i18next"

import type { TransferTask, TransferStatus, Tab } from "@/types/tab"

import type { SftpDirectoryListing } from "@/components/SftpDrawer/types"

interface UploadItemStartEvent {
  batchId?: string
  fileName: string
  fileSize: number
  localPath: string
  remotePath: string
  transferId: string
}

interface UploadItemProgressEvent {
  localPath: string
  progress: number
  total: number
  transferred: number
  transferId: string
}

interface UploadItemCompleteEvent {
  cancelled: boolean
  error?: string
  localPath: string
  remotePath: string
  success: boolean
  transferId: string
}

interface UploadBatchStartEvent {
  batchId: string
  displayName: string
  localPath: string
  remoteBasePath: string
}

interface UploadBatchCompleteEvent {
  batchId: string
  cancelled: boolean
  error?: string
  failed: number
  succeeded: number
}

interface UploadBatchResult {
  cancelled: boolean
  failed: number
  succeeded: number
}

interface BatchChildProgress {
  fileSize: number
  speed: number
  status: TransferStatus
  transferred: number
}

interface UseSftpUploadsParams {
  addTransfer: (
    transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">,
    id?: string
  ) => string
  connection?: Tab["connection"]
  lastProgressUpdateRef: React.MutableRefObject<Map<string, number>>
  listing: SftpDirectoryListing | null
  loadDirectory: (path?: string | null) => Promise<void>
  setError: React.Dispatch<React.SetStateAction<string | null>>
  tabId: string
  transfersRef: React.MutableRefObject<TransferTask[]>
  updateTransfer: (id: string, updates: Partial<TransferTask>) => void
}

interface UseSftpUploadsReturn {
  handleUploadDialog: () => Promise<void>
  handleUploadFolderDialog: () => Promise<void>
  uploadPaths: (paths: string[]) => Promise<void>
}

export function useSftpUploads({
  addTransfer,
  connection,
  lastProgressUpdateRef,
  listing,
  loadDirectory,
  setError,
  tabId,
  transfersRef,
  updateTransfer,
}: UseSftpUploadsParams): UseSftpUploadsReturn {
  const { t } = useTranslation()
  const batchTransferIdsRef = useRef(new Map<string, Set<string>>())
  const batchProgressRef = useRef(new Map<string, Map<string, BatchChildProgress>>())

  const syncBatchTransfer = useCallback(
    (batchId: string, fallbackStatus?: TransferStatus) => {
      const children = batchProgressRef.current.get(batchId)
      const childEntries = children ? Array.from(children.values()) : []
      const fileSize = childEntries.reduce((sum, child) => sum + child.fileSize, 0)
      const transferred = childEntries.reduce((sum, child) => sum + child.transferred, 0)
      const speed = childEntries.reduce((sum, child) => sum + child.speed, 0)
      const hasFailed = childEntries.some((child) => child.status === "failed")
      const hasActive = childEntries.some(
        (child) => child.status === "pending" || child.status === "transferring"
      )

      updateTransfer(batchId, {
        fileSize,
        speed,
        status: fallbackStatus ?? (hasFailed ? "failed" : hasActive ? "transferring" : "pending"),
        transferred,
      })
    },
    [updateTransfer]
  )

  useEffect(() => {
    const appWindow = getCurrentWindow()
    let disposed = false
    const unlisteners: Array<() => void> = []

    const setupListeners = async () => {
      const nextUnlisteners = await Promise.all([
        appWindow.listen<UploadBatchStartEvent>(`sftp-upload-batch-start-${tabId}`, (event) => {
          const { batchId, displayName, localPath, remoteBasePath } = event.payload
          addTransfer(
            {
              direction: "upload",
              fileName: displayName,
              fileSize: 0,
              localPath,
              remotePath: remoteBasePath,
              speed: 0,
            },
            batchId
          )

          batchTransferIdsRef.current.set(batchId, new Set())
          batchProgressRef.current.set(batchId, new Map())

          updateTransfer(batchId, {
            error: undefined,
            fileSize: 0,
            speed: 0,
            status: "pending",
            transferred: 0,
          })
        }),
        appWindow.listen<UploadBatchCompleteEvent>(
          `sftp-upload-batch-complete-${tabId}`,
          (event) => {
            const { batchId, cancelled, error, failed, succeeded } = event.payload
            const childTransferIds = batchTransferIdsRef.current.get(batchId) ?? new Set<string>()
            const transfer = transfersRef.current.find((item) => item.id === batchId)
            const now = Date.now()

            for (const transferId of childTransferIds) {
              lastProgressUpdateRef.current.delete(transferId)
            }
            batchTransferIdsRef.current.delete(batchId)
            batchProgressRef.current.delete(batchId)

            if (cancelled) {
              for (const transferId of childTransferIds) {
                updateTransfer(transferId, {
                  endTime: now,
                  error: undefined,
                  status: "cancelled",
                })
              }

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

            const hasFailedChildren = Array.from(childTransferIds).some((transferId) => {
              const childTransfer = transfersRef.current.find((item) => item.id === transferId)
              return childTransfer?.status === "failed"
            })

            updateTransfer(batchId, {
              endTime: now,
              error: hasFailedChildren
                ? t("sftp.errors.uploadPartialFailed", {
                    failed,
                    succeeded,
                    defaultValue: `${succeeded} upload(s) succeeded, ${failed} failed`,
                  })
                : undefined,
              fileSize: transfer?.fileSize ?? 0,
              speed: 0,
              status: hasFailedChildren ? "failed" : "completed",
              transferred: transfer?.fileSize ?? transfer?.transferred ?? 0,
            })
          }
        ),
        appWindow.listen<UploadItemStartEvent>(`sftp-upload-item-start-${tabId}`, (event) => {
          const { batchId, fileName, fileSize, localPath, remotePath, transferId } = event.payload

          if (batchId) {
            const nextIds = new Set(batchTransferIdsRef.current.get(batchId) ?? [])
            nextIds.add(transferId)
            batchTransferIdsRef.current.set(batchId, nextIds)

            const nextBatchProgress = new Map(batchProgressRef.current.get(batchId) ?? [])
            nextBatchProgress.set(transferId, {
              fileSize,
              speed: 0,
              status: "transferring",
              transferred: 0,
            })
            batchProgressRef.current.set(batchId, nextBatchProgress)
            syncBatchTransfer(batchId, "transferring")
          }

          addTransfer(
            {
              batchId,
              direction: "upload",
              fileName,
              fileSize,
              localPath,
              remotePath,
              speed: 0,
            },
            transferId
          )

          updateTransfer(transferId, {
            batchId,
            error: undefined,
            fileSize,
            speed: 0,
            status: "transferring",
          })
        }),
        appWindow.listen<UploadItemProgressEvent>(`sftp-upload-progress-${tabId}`, (event) => {
          const { progress, transferId, transferred } = event.payload
          const now = Date.now()
          const lastUpdate = lastProgressUpdateRef.current.get(transferId) || 0
          if (now - lastUpdate < 100 && progress < 100) {
            return
          }
          lastProgressUpdateRef.current.set(transferId, now)

          const transfer = transfersRef.current.find((item) => item.id === transferId)
          const duration = now - (transfer?.startTime || now)
          const speed = duration > 0 ? (transferred / duration) * 1000 : 0

          updateTransfer(transferId, {
            speed,
            transferred,
          })

          if (transfer?.batchId) {
            const nextBatchProgress = new Map(batchProgressRef.current.get(transfer.batchId) ?? [])
            const previous = nextBatchProgress.get(transferId)
            if (previous) {
              nextBatchProgress.set(transferId, {
                ...previous,
                speed,
                transferred,
              })
              batchProgressRef.current.set(transfer.batchId, nextBatchProgress)
              syncBatchTransfer(transfer.batchId, "transferring")
            }
          }
        }),
        appWindow.listen<UploadItemCompleteEvent>(`sftp-upload-item-complete-${tabId}`, (event) => {
          const { cancelled, error, success, transferId } = event.payload
          const transfer = transfersRef.current.find((item) => item.id === transferId)
          const now = Date.now()
          const duration = now - (transfer?.startTime || now)
          const completedFileSize = transfer?.fileSize || transfer?.transferred || 0
          const speed = duration > 0 ? (completedFileSize / duration) * 1000 : 0

          lastProgressUpdateRef.current.delete(transferId)

          if (transfer?.batchId) {
            const nextBatchProgress = new Map(batchProgressRef.current.get(transfer.batchId) ?? [])
            const previous = nextBatchProgress.get(transferId)
            if (previous) {
              nextBatchProgress.set(transferId, {
                ...previous,
                speed: 0,
                status: success ? "completed" : cancelled ? "cancelled" : "failed",
                transferred: success ? completedFileSize : previous.transferred,
              })
              batchProgressRef.current.set(transfer.batchId, nextBatchProgress)
              syncBatchTransfer(transfer.batchId)
            }
          }

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
            error: error || "Upload failed",
            status: "failed",
          })
        }),
      ])

      if (disposed) {
        nextUnlisteners.forEach((unlisten) => unlisten())
        return
      }

      unlisteners.push(...nextUnlisteners)
    }

    void setupListeners()

    return () => {
      disposed = true
      unlisteners.forEach((unlisten) => unlisten())
    }
  }, [
    addTransfer,
    lastProgressUpdateRef,
    syncBatchTransfer,
    t,
    tabId,
    transfersRef,
    updateTransfer,
  ])

  const uploadPaths = useCallback(
    async (paths: string[]) => {
      if (!listing) {
        setError(t("sftp.errors.notReady", { defaultValue: "SFTP not ready" }))
        return
      }

      const validPaths = paths.filter((path) => typeof path === "string" && path.length > 0)
      if (validPaths.length === 0) {
        setError(
          t("sftp.errors.noValidFiles", {
            defaultValue: "No valid files to upload",
          })
        )
        return
      }

      setError(null)

      try {
        const result = await invoke<UploadBatchResult>("sftp_upload_paths", {
          connection,
          localPaths: validPaths,
          remoteBasePath: listing.currentPath,
          tabId,
        })

        if (result.cancelled) {
          return
        }

        if (result.failed > 0 && result.succeeded === 0) {
          setError(
            t("sftp.errors.uploadAllFailed", {
              count: result.failed,
              defaultValue: `${result.failed} upload(s) failed`,
            })
          )
        } else if (result.failed > 0) {
          setError(
            t("sftp.errors.uploadPartialFailed", {
              failed: result.failed,
              succeeded: result.succeeded,
              defaultValue: `${result.succeeded} upload(s) succeeded, ${result.failed} failed`,
            })
          )
        }
      } catch (invokeError) {
        setError(String(invokeError))
      } finally {
        await loadDirectory(listing.currentPath)
      }
    },
    [connection, listing, loadDirectory, setError, t, tabId]
  )

  const handleUploadDialog = useCallback(async () => {
    const selection = await openFileDialog({
      directory: false,
      multiple: true,
      title: t("sftp.actions.uploadFiles", { defaultValue: "Upload Files" }),
    })

    if (!selection) {
      return
    }

    const paths = Array.isArray(selection) ? selection : [selection]
    await uploadPaths(paths)
  }, [t, uploadPaths])

  const handleUploadFolderDialog = useCallback(async () => {
    const selection = await openFileDialog({
      directory: true,
      multiple: true,
      title: t("sftp.actions.uploadFolder", { defaultValue: "Upload Folder" }),
    })

    if (!selection) {
      return
    }

    const paths = Array.isArray(selection) ? selection : [selection]
    await uploadPaths(paths)
  }, [t, uploadPaths])

  return {
    handleUploadDialog,
    handleUploadFolderDialog,
    uploadPaths,
  }
}
