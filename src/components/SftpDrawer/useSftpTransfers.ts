import { useSftpDownloads } from "@/components/SftpDrawer/useSftpDownloads"
import { useSftpUploads } from "@/components/SftpDrawer/useSftpUploads"
import type { SftpDirectoryListing } from "@/components/SftpDrawer/types"
import { useTransferManager } from "@/contexts/TransferContext"
import type { Tab } from "@/types/tab"

interface UseSftpTransfersParams {
  connection?: Tab["connection"]
  listing: SftpDirectoryListing | null
  loadDirectory: (path?: string | null) => Promise<void>
  setError: React.Dispatch<React.SetStateAction<string | null>>
  tabId: string
}

interface UseSftpTransfersReturn {
  addTransfer: ReturnType<typeof useTransferManager>["addTransfer"]
  cancelTransfer: (id: string) => Promise<void>
  clearCompletedTransfers: () => void
  downloadEntry: (
    entry: import("@/components/SftpDrawer/types").SftpDirectoryEntry
  ) => Promise<void>
  handleOpenEntry: (
    entry: import("@/components/SftpDrawer/types").SftpDirectoryEntry
  ) => Promise<void>
  handleUploadDialog: () => Promise<void>
  handleUploadFolderDialog: () => Promise<void>
  removeTransfer: (id: string) => void
  transfers: import("@/types/tab").TransferTask[]
  transfersRef: React.MutableRefObject<import("@/types/tab").TransferTask[]>
  updateTransfer: ReturnType<typeof useTransferManager>["updateTransfer"]
  uploadPaths: (paths: string[]) => Promise<void>
}

export function useSftpTransfers({
  connection,
  listing,
  loadDirectory,
  setError,
  tabId,
}: UseSftpTransfersParams): UseSftpTransfersReturn {
  const {
    addTransfer,
    cancelTransfer: cancelTransferRaw,
    clearCompletedTransfers,
    lastProgressUpdateRef,
    removeTransfer: removeTransferRaw,
    transfers,
    transfersRef,
    updateTransfer,
  } = useTransferManager()

  const { downloadEntry, handleOpenEntry } = useSftpDownloads({
    addTransfer,
    connection,
    loadDirectory,
    tabId,
    updateTransfer,
  })

  const { handleUploadDialog, handleUploadFolderDialog, uploadPaths } = useSftpUploads({
    addTransfer,
    connection,
    lastProgressUpdateRef,
    listing,
    loadDirectory,
    setError,
    tabId,
    transfersRef,
    updateTransfer,
  })

  const cancelTransfer = async (id: string) => {
    const transfer = transfers.find((item) => item.id === id)
    const targetId = transfer?.batchId ?? id
    await cancelTransferRaw(targetId)
  }

  const removeTransfer = (id: string) => {
    removeTransferRaw(id)

    for (const transfer of transfers) {
      if (transfer.batchId === id) {
        removeTransferRaw(transfer.id)
      }
    }
  }

  return {
    addTransfer,
    cancelTransfer,
    clearCompletedTransfers,
    downloadEntry,
    handleOpenEntry,
    handleUploadDialog,
    handleUploadFolderDialog,
    removeTransfer,
    transfers,
    transfersRef,
    updateTransfer,
    uploadPaths,
  }
}
