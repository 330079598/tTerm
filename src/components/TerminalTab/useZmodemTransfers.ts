import { useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useTranslation } from "react-i18next"

import { useTransferManager } from "@/contexts/TransferContext"

interface UseZmodemTransfersParams {
  tabId: string
  sessionNonce: number
}

interface ZmodemTransferStartEvent {
  transferId: string
  direction: "upload" | "download"
  fileName: string
  fileSize: number
  localPath: string
}

interface ZmodemTransferProgressEvent {
  transferId: string
  transferred: number
  speed: number
}

interface ZmodemTransferCompleteEvent {
  transferId: string
  success: boolean
  cancelled: boolean
  error?: string
}

/**
 * Listens for backend-driven ZMODEM transfer events on this tab's channel
 * and feeds them into the shared transfer UI (`TransferContext`), the same
 * way `SftpDrawer/useSftpTransfers.ts` wires SFTP progress. Auto-detected
 * ZMODEM transfers start entirely on the backend (no frontend round trip),
 * so this hook is purely reactive.
 */
export function useZmodemTransfers({ tabId, sessionNonce }: UseZmodemTransfersParams): void {
  const { addTransfer, updateTransfer } = useTransferManager()
  const { t } = useTranslation()

  useEffect(() => {
    const appWindow = getCurrentWindow()
    let disposed = false
    let unlisteners: Array<() => void> = []

    const cancel = async (): Promise<void> => {
      try {
        await invoke("zmodem_cancel", { tabId, sessionNonce })
      } catch {
        // Best-effort: the tab may already be gone.
      }
    }

    const startSend = async (localPaths: string[]): Promise<void> => {
      try {
        await invoke("zmodem_start_send", { tabId, sessionNonce, localPaths })
      } catch {
        // Best-effort: the peer's rz may have already given up waiting.
      }
    }

    Promise.all([
      appWindow.listen(`zmodem-send-requested-${tabId}`, () => {
        // The peer just ran `rz` and is waiting; prompt for local files to
        // send. No frontend round trip happens for the receive direction
        // (auto-detected downloads start entirely on the backend), only
        // here, since tTerm can't guess which files the user means.
        void (async () => {
          const selection = await openFileDialog({
            directory: false,
            multiple: true,
            title: t("zmodem.actions.sendFilesDialogTitle", { defaultValue: "Send Files via ZMODEM" }),
          }).catch(() => null)
          const paths = selection ? (Array.isArray(selection) ? selection : [selection]) : []
          await startSend(paths)
        })()
      }),
      appWindow.listen<ZmodemTransferStartEvent>(`zmodem-transfer-start-${tabId}`, (event) => {
        const { transferId, direction, fileName, fileSize, localPath } = event.payload
        addTransfer(
          {
            tabId,
            direction,
            localPath,
            remotePath: fileName,
            fileName,
            fileSize,
            cancel,
          },
          transferId
        )
      }),
      appWindow.listen<ZmodemTransferProgressEvent>(`zmodem-transfer-progress-${tabId}`, (event) => {
        const { transferId, transferred, speed } = event.payload
        updateTransfer(transferId, { transferred, speed, status: "transferring" })
      }),
      appWindow.listen<ZmodemTransferCompleteEvent>(`zmodem-transfer-complete-${tabId}`, (event) => {
        const { transferId, success, cancelled, error } = event.payload
        updateTransfer(transferId, {
          status: cancelled ? "cancelled" : success ? "completed" : "failed",
          endTime: Date.now(),
          error,
        })
      }),
    ]).then((created) => {
      if (disposed) {
        created.forEach((unlisten) => unlisten())
        return
      }
      unlisteners = created
    })

    return () => {
      disposed = true
      unlisteners.forEach((unlisten) => unlisten())
    }
  }, [tabId, sessionNonce, addTransfer, updateTransfer, t])
}
