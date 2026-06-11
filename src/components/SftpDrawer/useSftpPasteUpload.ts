import { useCallback, useEffect, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import type { SftpDirectoryListing } from "@/components/SftpDrawer/types"

interface UseSftpPasteUploadParams {
  enabled: boolean
  listing: SftpDirectoryListing | null
  setError: React.Dispatch<React.SetStateAction<string | null>>
  uploadPaths: (paths: string[]) => Promise<void>
  visible: boolean
}

const PASTE_DEDUP_WINDOW_MS = 750

function isEditablePasteTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false
  }

  return Boolean(
    target.isContentEditable ||
    target.closest("input, textarea, select, [contenteditable='true'], [role='textbox']")
  )
}

export function useSftpPasteUpload({
  enabled,
  listing,
  setError,
  uploadPaths,
  visible,
}: UseSftpPasteUploadParams) {
  const { t } = useTranslation()
  const isPasteUploadRunningRef = useRef(false)
  const lastPasteRef = useRef<{ signature: string; timestamp: number } | null>(null)

  const uploadClipboardPaths = useCallback(
    async (event: KeyboardEvent) => {
      if (isPasteUploadRunningRef.current) {
        return
      }

      isPasteUploadRunningRef.current = true
      let validPaths: string[]
      try {
        const paths = await invoke<string[]>("read_clipboard_file_paths")
        validPaths = paths.filter((path) => typeof path === "string" && path.length > 0)
      } catch (invokeError) {
        setError(String(invokeError))
        return
      } finally {
        isPasteUploadRunningRef.current = false
      }

      if (validPaths.length === 0) {
        return
      }

      event.preventDefault()
      event.stopPropagation()

      if (!listing) {
        setError(t("sftp.errors.notReady", { defaultValue: "SFTP not ready" }))
        return
      }

      const signature = [...validPaths].sort().join("\0")
      const now = Date.now()
      const lastPaste = lastPasteRef.current
      if (
        lastPaste &&
        lastPaste.signature === signature &&
        now - lastPaste.timestamp < PASTE_DEDUP_WINDOW_MS
      ) {
        return
      }
      lastPasteRef.current = { signature, timestamp: now }

      await uploadPaths(validPaths)
    },
    [listing, setError, t, uploadPaths]
  )

  useEffect(() => {
    if (!enabled || !visible) {
      return
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      const isPasteShortcut =
        event.key.toLowerCase() === "v" &&
        (event.ctrlKey || event.metaKey) &&
        !event.altKey &&
        !event.shiftKey

      if (!isPasteShortcut || isEditablePasteTarget(event.target)) {
        return
      }

      if (event.repeat) {
        event.preventDefault()
        event.stopPropagation()
        return
      }

      void uploadClipboardPaths(event)
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  }, [enabled, uploadClipboardPaths, visible])
}
