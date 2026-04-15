import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react"
import { invoke } from "@tauri-apps/api/core"

import type { TransferTask } from "@/types/tab"

interface TransferContextValue {
  addTransfer: (
    transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">
  ) => string
  cancelTransfer: (id: string) => Promise<void>
  clearCompletedTransfers: () => void
  lastProgressUpdateRef: React.MutableRefObject<Map<string, number>>
  removeTransfer: (id: string) => void
  transfers: TransferTask[]
  transfersRef: React.MutableRefObject<TransferTask[]>
  updateTransfer: (id: string, updates: Partial<TransferTask>) => void
}

const TransferContext = createContext<TransferContextValue | null>(null)

export const TransferProvider: React.FC<React.PropsWithChildren> = ({ children }) => {
  const [transfers, setTransfers] = useState<TransferTask[]>([])
  const transfersRef = useRef<TransferTask[]>([])
  const lastProgressUpdateRef = useRef<Map<string, number>>(new Map())

  useEffect(() => {
    transfersRef.current = transfers
  }, [transfers])

  const addTransfer = useCallback(
    (transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">) => {
      const newTransfer: TransferTask = {
        ...transfer,
        id: `${Date.now()}-${Math.random().toString(36).substring(2, 11)}`,
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

  const cancelTransfer = useCallback(async (id: string) => {
    try {
      await invoke("sftp_cancel_upload", { transferId: id })
    } catch (invokeError) {
      console.warn("Failed to cancel transfer on backend:", invokeError)
    }
  }, [])

  const removeTransfer = useCallback((id: string) => {
    setTransfers((prev) => prev.filter((transfer) => transfer.id !== id))
  }, [])

  const clearCompletedTransfers = useCallback(() => {
    setTransfers((prev) =>
      prev.filter((transfer) => transfer.status === "pending" || transfer.status === "transferring")
    )
  }, [])

  const value = useMemo(
    () => ({
      addTransfer,
      cancelTransfer,
      clearCompletedTransfers,
      lastProgressUpdateRef,
      removeTransfer,
      transfers,
      transfersRef,
      updateTransfer,
    }),
    [
      addTransfer,
      cancelTransfer,
      clearCompletedTransfers,
      removeTransfer,
      transfers,
      updateTransfer,
    ]
  )

  return <TransferContext.Provider value={value}>{children}</TransferContext.Provider>
}

export function useTransferManager(): TransferContextValue {
  const context = useContext(TransferContext)

  if (!context) {
    throw new Error("useTransferManager must be used within a TransferProvider")
  }

  return context
}
