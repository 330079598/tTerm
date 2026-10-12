import { invoke } from "@tauri-apps/api/core"
import { useCallback, useState } from "react"
import { useTranslation } from "react-i18next"

import type {
  LoadSftpDirectory,
  SftpDirectoryListing,
  SftpDrawerProps,
} from "@/components/SftpDrawer/types"
import { useUserVerification } from "@/contexts/UserVerificationContext"

interface SftpDirectoryLoaderOptions {
  tabId: string
  connection: SftpDrawerProps["connection"]
  recordLoaded: (path: string, historyIndex?: number) => void
  recordFailed: (historyIndex?: number) => void
  setActivePath: (path: string | null) => void
  setSelectedPaths: (paths: string[]) => void
  setSearchQuery: (query: string) => void
}

/**
 * The directory listing shown in the drawer and `loadDirectory`, which
 * fetches one and resets the selection and search for it.
 */
export function useSftpDirectoryLoader({
  tabId,
  connection,
  recordLoaded,
  recordFailed,
  setActivePath,
  setSelectedPaths,
  setSearchQuery,
}: SftpDirectoryLoaderOptions) {
  const { t } = useTranslation()
  const { withVaultUnlock } = useUserVerification()
  const [listing, setListing] = useState<SftpDirectoryListing | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const loadDirectory = useCallback<LoadSftpDirectory>(
    async (path, options) => {
      if (!connection) {
        const message = t("sftp.errors.missingConnection", {
          defaultValue: "SSH connection is missing",
        })
        setError(message)
        if (options?.throwOnError) {
          throw new Error(message)
        }
        return
      }

      setIsLoading(true)
      setError(null)
      try {
        const nextListing = await withVaultUnlock(() =>
          invoke<SftpDirectoryListing>("sftp_list_directory", {
            tabId,
            connection,
            path: path ?? undefined,
          })
        )
        setListing(nextListing)
        recordLoaded(nextListing.currentPath, options?.historyIndex)
        setActivePath(null)
        setSelectedPaths([])
        setSearchQuery("")
      } catch (invokeError) {
        const message = String(invokeError)
        setError(message)
        recordFailed(options?.historyIndex)
        if (options?.throwOnError) {
          throw new Error(message, { cause: invokeError })
        }
      } finally {
        setIsLoading(false)
      }
    },
    [
      connection,
      recordFailed,
      recordLoaded,
      setActivePath,
      setSearchQuery,
      setSelectedPaths,
      t,
      tabId,
      withVaultUnlock,
    ]
  )

  return { listing, setListing, isLoading, error, setError, loadDirectory }
}
