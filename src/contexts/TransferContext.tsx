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

const TransferContext = createContext<TransferContextValue | null>(null)

export const TransferProvider: React.FC<React.PropsWithChildren> = ({ children }) => {
  const [transfers, setTransfers] = useState<TransferTask[]>([])
  const transfersRef = useRef<TransferTask[]>([])
  const lastProgressUpdateRef = useRef<Map<string, number>>(new Map())

  useEffect(() => {
    transfersRef.current = transfers
  }, [transfers])

  const addTransfer = useCallback(
    (transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">, id?: string) => {
      const nextId = id ?? `${Date.now()}-${Math.random().toString(36).substring(2, 11)}`
      const newTransfer: TransferTask = {
        ...transfer,
        id: nextId,
        startTime: Date.now(),
        status: "pending",
        transferred: 0,
      }

      setTransfers((prev) => {
        const existing = prev.find((item) => item.id === nextId)
        if (!existing) {
          return [newTransfer, ...prev]
        }

        return prev.map((item) =>
          item.id === nextId
            ? {
                ...item,
                ...transfer,
              }
            : item
        )
      })

      return nextId
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
