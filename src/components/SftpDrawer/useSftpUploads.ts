import { useCallback, useEffect, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { useTranslation } from "react-i18next"

import { useUploadEvents } from "@/components/SftpDrawer/useUploadEvents"
import type { TransferTask, Tab } from "@/types/tab"
import type {
  ConflictPolicy,
  ConflictReport,
  LoadSftpDirectory,
  PromptConflictPolicy,
  SftpDirectoryListing,
} from "@/components/SftpDrawer/types"

interface UploadBatchResult {
  cancelled: boolean
  failed: number
  succeeded: number
}

export interface RunUploadOptions {
  /** Set by a batch retry: skip files an earlier attempt already finished. */
  skipExisting?: boolean
  /**
   * Policy for destination files that already exist. When unset, the remote
   * is checked first and the user is asked if anything collides.
   */
  conflictPolicy?: ConflictPolicy
}

export interface UseSftpUploadsParams {
  addTransfer: (
    transfer: Omit<TransferTask, "id" | "startTime" | "status" | "transferred">,
    id?: string
  ) => string
  connection?: Tab["connection"]
  lastProgressUpdateRef: React.RefObject<Map<string, number>>
  listing: SftpDirectoryListing | null
  loadDirectory: LoadSftpDirectory
  promptConflictPolicy: PromptConflictPolicy
  setError: React.Dispatch<React.SetStateAction<string | null>>
  tabId: string
  transfersRef: React.RefObject<TransferTask[]>
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
  promptConflictPolicy,
  setError,
  tabId,
  transfersRef,
  updateTransfer,
}: UseSftpUploadsParams): UseSftpUploadsReturn {
  const { t } = useTranslation()
  const uploadPathsRunnerRef = useRef<
    ((paths: string[], remoteBasePath: string, options: RunUploadOptions) => void) | null
  >(null)
  useUploadEvents({
    addTransfer,
    connection,
    lastProgressUpdateRef,
    tabId,
    transfersRef,
    updateTransfer,
    uploadPathsRunnerRef,
  })

  const runUploadPaths = useCallback(
    async (paths: string[], remoteBasePath: string, options: RunUploadOptions = {}) => {
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

      let conflictPolicy = options.conflictPolicy
      if (!conflictPolicy) {
        try {
          const report = await invoke<ConflictReport>("sftp_check_upload_conflicts", {
            connection,
            localPaths: validPaths,
            remoteBasePath,
            tabId,
          })
          if (report.total > 0) {
            const chosen = await promptConflictPolicy(report, "upload")
            if (!chosen) return
            conflictPolicy = chosen
          }
        } catch (invokeError) {
          setError(String(invokeError))
          return
        }
      }

      try {
        const result = await invoke<UploadBatchResult>("sftp_upload_paths", {
          connection,
          conflictPolicy: conflictPolicy ?? "overwrite",
          localPaths: validPaths,
          remoteBasePath,
          skipExisting: options.skipExisting ?? false,
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
        await loadDirectory(remoteBasePath)
      }
    },
    [connection, loadDirectory, promptConflictPolicy, setError, t, tabId]
  )

  // Assign in an effect (not during render) so a discarded concurrent render
  // can never leave an uncommitted closure in the ref for retry to pick up.
  useEffect(() => {
    uploadPathsRunnerRef.current = (paths, remoteBasePath, options) => {
      void runUploadPaths(paths, remoteBasePath, options)
    }
  }, [runUploadPaths])

  const uploadPaths = useCallback(
    async (paths: string[]) => {
      if (!listing) {
        setError(t("sftp.errors.notReady", { defaultValue: "SFTP not ready" }))
        return
      }

      await runUploadPaths(paths, listing.currentPath)
    },
    [listing, runUploadPaths, setError, t]
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
