import { useSftpDownloads } from "@/components/SftpDrawer/useSftpDownloads"
import { useSftpTransferState } from "@/components/SftpDrawer/useSftpTransferState"
import { useSftpUploads } from "@/components/SftpDrawer/useSftpUploads"
import type { SftpDirectoryListing } from "@/components/SftpDrawer/types"
import type { Tab } from "@/types/tab"

interface UseSftpTransfersParams {
  connection?: Tab["connection"]
  listing: SftpDirectoryListing | null
  loadDirectory: (path?: string | null) => Promise<void>
  setError: React.Dispatch<React.SetStateAction<string | null>>
  tabId: string
}

interface UseSftpTransfersReturn {
  cancelTransfer: (id: string) => Promise<void>
  clearCompletedTransfers: () => void
  downloadEntry: (
    entry: import("@/components/SftpDrawer/types").SftpDirectoryEntry
  ) => Promise<void>
  handleOpenEntry: (
    entry: import("@/components/SftpDrawer/types").SftpDirectoryEntry
  ) => Promise<void>
  handleUploadDialog: () => Promise<void>
  removeTransfer: (id: string) => void
  transfers: import("@/types/tab").TransferTask[]
  uploadFiles: (files: File[]) => Promise<void>
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
    cancelTransfer,
    clearCompletedTransfers,
    lastProgressUpdateRef,
    removeTransfer,
    transfers,
    transfersRef,
    updateTransfer,
  } = useSftpTransferState()

  const { downloadEntry, handleOpenEntry } = useSftpDownloads({
    addTransfer,
    connection,
    loadDirectory,
    tabId,
    updateTransfer,
  })

  const { handleUploadDialog, uploadFiles } = useSftpUploads({
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

  return {
    cancelTransfer,
    clearCompletedTransfers,
    downloadEntry,
    handleOpenEntry,
    handleUploadDialog,
    removeTransfer,
    transfers,
    uploadFiles,
  }
}
