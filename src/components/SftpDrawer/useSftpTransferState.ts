import { useCallback, useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"

import type { TransferTask } from "@/types/tab"

interface UseSftpTransferStateReturn {
  addTransfer: (
    transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">,
    id?: string
  ) => string
  cancelTransfer: (id: string) => Promise<void>
  clearCompletedTransfers: () => void
  lastProgressUpdateRef: React.MutableRefObject<Map<string, number>>
  removeTransfer: (id: string) => void
  transfers: TransferTask[]
  transfersRef: React.MutableRefObject<TransferTask[]>
  updateTransfer: (id: string, updates: Partial<TransferTask>) => void
}

export function useSftpTransferState(): UseSftpTransferStateReturn {
  const [transfers, setTransfers] = useState<TransferTask[]>([])
  const transfersRef = useRef<TransferTask[]>([])
  const lastProgressUpdateRef = useRef<Map<string, number>>(new Map())

  useEffect(() => {
    transfersRef.current = transfers
  }, [transfers])

  const addTransfer = useCallback(
    (transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">, id?: string) => {
      const newTransfer: TransferTask = {
        ...transfer,
        id: id ?? `${Date.now()}-${Math.random().toString(36).substring(2, 11)}`,
        startTime: Date.now(),
        status: "pending",
        transferred: 0,
      }
      setTransfers((prev) => [newTransfer, ...prev])
      return newTransfer.id
    },
    []
  )

  const updateTransfer = useCallback((id: string, updates: Partial<TransferTask>) => {
    setTransfers((prev) =>
      prev.map((transfer) => (transfer.id === id ? { ...transfer, ...updates } : transfer))
    )
  }, [])

  const cancelTransfer = useCallback(
    async (id: string) => {
      updateTransfer(id, { status: "cancelled", endTime: Date.now() })

      try {
        await invoke("sftp_cancel_upload", { transferId: id })
      } catch (invokeError) {
        console.warn("Failed to cancel transfer on backend:", invokeError)
      }
    },
    [updateTransfer]
  )

  const removeTransfer = useCallback((id: string) => {
    setTransfers((prev) => prev.filter((transfer) => transfer.id !== id))
  }, [])

  const clearCompletedTransfers = useCallback(() => {
    setTransfers((prev) =>
      prev.filter((transfer) => transfer.status === "pending" || transfer.status === "transferring")
    )
  }, [])

  return {
    addTransfer,
    cancelTransfer,
    clearCompletedTransfers,
    lastProgressUpdateRef,
    removeTransfer,
    transfers,
    transfersRef,
    updateTransfer,
  }
}
