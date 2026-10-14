import { useCallback, useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { startDragOut, supportsDragOut } from "@/components/SftpDrawer/dragOut"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

/** How far a press must move before it becomes a drag, as in the browser. */
const DRAG_THRESHOLD_PX = 4

interface UseSftpDragOutParams {
  downloadEntriesToFolder: (entries: SftpDirectoryEntry[], folder: string) => Promise<void>
  setError: React.Dispatch<React.SetStateAction<string | null>>
}

export interface SftpDragOut {
  /**
   * Watches a press on a highlighted row; moving it far enough drags
   * `entries` out to the file manager, which downloads them where dropped.
   */
  beginPress: (event: React.MouseEvent, entries: SftpDirectoryEntry[]) => void
  /** Set once the current press became a drag, so its click is ignored. */
  startedRef: React.RefObject<boolean>
}

/** Drags rows out of the SFTP list onto a folder to download them; null where unsupported. */
export function useSftpDragOut({
  downloadEntriesToFolder,
  setError,
}: UseSftpDragOutParams): SftpDragOut | null {
  const { t } = useTranslation()
  const startedRef = useRef(false)
  const stopWatchingRef = useRef<(() => void) | null>(null)
  const [supported] = useState(supportsDragOut)

  useEffect(() => () => stopWatchingRef.current?.(), [])

  const dragOut = useCallback(
    async (entries: SftpDirectoryEntry[]) => {
      try {
        const result = await startDragOut(entries).finally(() => {
          // The click that ends the press, if the drag lets one through,
          // has fired by now.
          window.setTimeout(() => {
            startedRef.current = false
          }, 0)
        })
        if (result.kind === "dropped") {
          await downloadEntriesToFolder(entries, result.directory)
        } else if (result.kind === "unknownTarget") {
          setError(
            t("sftp.errors.dragOutUnknownTarget", {
              defaultValue: "Drop the files on a folder or on the desktop to download them there.",
            })
          )
        }
      } catch (error) {
        setError(String(error))
      }
    },
    [downloadEntriesToFolder, setError, t]
  )

  const beginPress = useCallback(
    (event: React.MouseEvent, entries: SftpDirectoryEntry[]) => {
      stopWatchingRef.current?.()
      startedRef.current = false
      const startX = event.clientX
      const startY = event.clientY

      const handleMove = (move: MouseEvent) => {
        if (Math.hypot(move.clientX - startX, move.clientY - startY) < DRAG_THRESHOLD_PX) {
          return
        }
        stopWatching()
        startedRef.current = true
        void dragOut(entries)
      }
      const stopWatching = () => {
        document.body.classList.remove("sftp-no-select")
        window.removeEventListener("mousemove", handleMove)
        window.removeEventListener("mouseup", stopWatching)
        stopWatchingRef.current = null
      }

      document.body.classList.add("sftp-no-select")
      window.addEventListener("mousemove", handleMove)
      window.addEventListener("mouseup", stopWatching)
      stopWatchingRef.current = stopWatching
    },
    [dragOut]
  )

  return supported ? { beginPress, startedRef } : null
}
