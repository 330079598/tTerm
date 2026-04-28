import React, { useEffect } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import type { TFunction } from "i18next"

import { toast } from "@/hooks/use-toast"
import type {
  DeleteBatchCompleteEvent,
  DeleteBatchStartEvent,
  SftpDeleteProgressState,
} from "@/components/SftpDrawer/types"
import type { TransferTask } from "@/types/tab"

type SftpDeleteTransferEventsProps = {
  addTransfer: (
    transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">,
    id?: string
  ) => void
  listingCurrentPath?: string
  loadDirectory: (path?: string | null) => Promise<void>
  setError: (error: string | null) => void
  setIsDeleting: (isDeleting: boolean) => void
  t: TFunction
  tabId: string
  transfersRef: React.MutableRefObject<TransferTask[]>
  updateTransfer: (id: string, updates: Partial<TransferTask>) => void
}

export const SftpDeleteTransferEvents: React.FC<SftpDeleteTransferEventsProps> = ({
  addTransfer,
  listingCurrentPath,
  loadDirectory,
  setError,
  setIsDeleting,
  t,
  tabId,
  transfersRef,
  updateTransfer,
}) => {
  useEffect(() => {
    const appWindow = getCurrentWindow()
    let disposed = false
    const unlisteners: Array<() => void> = []

    const setupListeners = async () => {
      const nextUnlisteners = await Promise.all([
        appWindow.listen<DeleteBatchStartEvent>(`sftp-delete-batch-start-${tabId}`, (event) => {
          const { payload } = event
          addTransfer(
            {
              direction: "delete",
              fileName:
                payload.entries.join(", ") ||
                t("sftp.deleteProgress.sftp", { defaultValue: "Deleting files" }),
              fileSize: payload.totalEntries,
              localPath: "",
              remotePath: payload.entries.join(", "),
              speed: 0,
            },
            payload.batchId
          )
          updateTransfer(payload.batchId, {
            status: "transferring",
            transferred: 0,
          })
        }),
        appWindow.listen<SftpDeleteProgressState>(`sftp-delete-progress-${tabId}`, (event) => {
          const payload = event.payload
          const transferred = payload.deletedDirectories + payload.deletedFiles
          const transfer = transfersRef.current.find((item) => item.id === payload.batchId)
          const now = Date.now()
          const duration = now - (transfer?.startTime || now)
          const speed = duration > 0 ? (transferred / duration) * 1000 : 0

          updateTransfer(payload.batchId, {
            fileSize: payload.totalEntries,
            remotePath: payload.currentPath || transfer?.remotePath || "",
            speed,
            status: "transferring",
            transferred,
          })
        }),
        appWindow.listen<DeleteBatchCompleteEvent>(
          `sftp-delete-batch-complete-${tabId}`,
          (event) => {
            const { payload } = event
            setIsDeleting(false)
            void loadDirectory(listingCurrentPath ?? null)

            if (payload.error || payload.failed > 0) {
              const message = payload.error ?? `Failed to delete ${payload.failed} item(s).`
              updateTransfer(payload.batchId, {
                endTime: Date.now(),
                error: message,
                fileSize: payload.totalEntries,
                status: "failed",
                transferred: payload.deletedDirectories + payload.deletedFiles,
              })
              setError(message)
              toast({
                variant: "destructive",
                title: t("sftp.messages.deleteFailure", {
                  defaultValue: "Failed to delete selected items.",
                }),
                description: message,
              })
              return
            }

            updateTransfer(payload.batchId, {
              endTime: Date.now(),
              fileSize: payload.totalEntries,
              speed: 0,
              status: "completed",
              transferred: payload.totalEntries,
            })

            toast({
              title: t("sftp.messages.deleteSuccess", {
                count: payload.deletedDirectories + payload.deletedFiles,
                defaultValue: `Deleted ${payload.deletedDirectories + payload.deletedFiles} item(s).`,
              }),
              description:
                payload.method === "command"
                  ? t("sftp.messages.deleteUsedCommand", {
                      defaultValue: "Used remote command delete for a large folder.",
                    })
                  : undefined,
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

    void setupListeners()

    return () => {
      disposed = true
      unlisteners.forEach((unlisten) => unlisten())
    }
  }, [
    addTransfer,
    listingCurrentPath,
    loadDirectory,
    setError,
    setIsDeleting,
    t,
    tabId,
    transfersRef,
    updateTransfer,
  ])

  return null
}
