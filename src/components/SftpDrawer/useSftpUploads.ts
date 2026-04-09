import { useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { useTranslation } from "react-i18next"

import type { TransferTask, Tab } from "@/types/tab"

import { joinRemotePath } from "@/components/SftpDrawer/sftpDrawerUtils"
import type { SftpDirectoryListing } from "@/components/SftpDrawer/types"

interface UseSftpUploadsParams {
  addTransfer: (
    transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">
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
  uploadFiles: (files: File[]) => Promise<void>
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

  const runUpload = useCallback(
    async ({
      fileName,
      fileSize,
      localPath,
      remotePath,
    }: {
      fileName: string
      fileSize: number
      localPath: string
      remotePath: string
    }) => {
      const transferId = addTransfer({
        direction: "upload",
        localPath,
        remotePath,
        fileName,
        fileSize,
        speed: 0,
      })

      updateTransfer(transferId, { status: "transferring" })

      const appWindow = getCurrentWindow()
      const unlisten = await appWindow.listen<{
        localPath: string
        transferred: number
        total: number
        progress: number
      }>(`sftp-upload-progress-${tabId}`, (event) => {
        if (event.payload.localPath === localPath) {
          const now = Date.now()
          const lastUpdate = lastProgressUpdateRef.current.get(transferId) || 0
          if (now - lastUpdate < 100 && event.payload.progress < 100) {
            return
          }
          lastProgressUpdateRef.current.set(transferId, now)

          const transfer = transfersRef.current.find((item) => item.id === transferId)
          const duration = now - (transfer?.startTime || now)
          const speed = duration > 0 ? (event.payload.transferred / duration) * 1000 : 0

          updateTransfer(transferId, {
            transferred: event.payload.transferred,
            speed,
          })
        }
      })

      try {
        const startTime = Date.now()
        await invoke("sftp_upload_file", {
          tabId,
          connection,
          localPath,
          remotePath,
          transferId,
        })

        unlisten()

        const duration = Date.now() - startTime
        const completedFileSize = fileSize || 1024
        const speed = duration > 0 ? (completedFileSize / duration) * 1000 : 0

        updateTransfer(transferId, {
          status: "completed",
          transferred: completedFileSize,
          fileSize: completedFileSize,
          endTime: Date.now(),
          speed,
        })

        lastProgressUpdateRef.current.delete(transferId)

        return { success: true, fileName }
      } catch (invokeError) {
        unlisten()
        lastProgressUpdateRef.current.delete(transferId)

        const errorMessage = String(invokeError)

        if (errorMessage.includes("cancelled")) {
          updateTransfer(transferId, {
            status: "cancelled",
            endTime: Date.now(),
          })
          return { success: false, fileName, cancelled: true }
        }

        updateTransfer(transferId, {
          status: "failed",
          error: errorMessage,
          endTime: Date.now(),
        })
        return { success: false, fileName, error: errorMessage }
      }
    },
    [addTransfer, connection, lastProgressUpdateRef, tabId, transfersRef, updateTransfer]
  )

  const uploadFiles = useCallback(
    async (files: File[]) => {
      console.log("uploadFiles called with:", files)
      console.log("listing:", listing)

      if (!listing || files.length === 0) {
        console.log("Early return: listing or files empty")
        return
      }

      const validFiles = files.filter((file) => {
        console.log("Checking file:", file.name, "path:", file.path)
        if (!file.path) {
          console.warn("File does not have path property:", file.name)
          setError(
            t("sftp.errors.invalidFile", {
              defaultValue: `Cannot upload ${file.name}: file path not available`,
              name: file.name,
            })
          )
          return false
        }
        return true
      })

      console.log("Valid files:", validFiles)

      if (validFiles.length === 0) {
        setError(
          t("sftp.errors.noValidFiles", {
            defaultValue: "No valid files to upload",
          })
        )
        return
      }

      setError(null)

      const uploadPromises = validFiles.map(async (file) => {
        const fileName = file.name
        const localPath = file.path!

        console.log("Uploading file:", fileName, "from:", localPath)

        return runUpload({
          fileName,
          fileSize: file.size || 0,
          localPath,
          remotePath: joinRemotePath(listing.currentPath, fileName),
        })
      })

      const results = await Promise.allSettled(uploadPromises)

      const succeeded = results.filter(
        (result) => result.status === "fulfilled" && result.value.success
      ).length
      const failed = results.filter(
        (result) =>
          result.status === "rejected" || (result.status === "fulfilled" && !result.value.success)
      ).length

      console.log(`Upload summary: ${succeeded} succeeded, ${failed} failed`)
      console.log("Refreshing directory listing")
      await loadDirectory(listing.currentPath)
    },
    [listing, loadDirectory, runUpload, setError, t]
  )

  const handleUploadDialog = useCallback(async () => {
    const selection = await openFileDialog({
      multiple: true,
      directory: false,
      title: t("sftp.actions.upload", { defaultValue: "Upload Files" }),
    })

    if (!selection || !listing) {
      return
    }

    const paths = Array.isArray(selection) ? selection : [selection]

    const uploadPromises = paths.map(async (path) => {
      const fileName = path.split(/[\\/]/).pop() ?? "file"

      let fileSize = 0
      try {
        fileSize = await invoke<number>("get_file_size", { localPath: path })
      } catch (invokeError) {
        console.warn("Failed to get file size:", invokeError)
      }

      return runUpload({
        fileName,
        fileSize,
        localPath: path,
        remotePath: joinRemotePath(listing.currentPath, fileName),
      })
    })

    await Promise.allSettled(uploadPromises)
    await loadDirectory(listing.currentPath)
  }, [listing, loadDirectory, runUpload, t])

  return {
    handleUploadDialog,
    uploadFiles,
  }
}
