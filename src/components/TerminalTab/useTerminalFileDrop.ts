import { useEffect } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"

import type { TerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import type { TerminalTabProps } from "@/components/TerminalTab/types"
import { useStableRef } from "@/hooks/useStableRef"
import { dropQuotingFor, droppedPathsText } from "@/lib/droppedPaths"
import { getDetectedPlatform } from "@/lib/uiPlatform"

type UseTerminalFileDropOptions = Pick<TerminalTabProps, "connection" | "isActive" | "shellName"> &
  Pick<TerminalTabRefs, "termRef" | "surfaceRef">

/**
 * Files dropped on a local terminal type their paths at the prompt, quoted
 * for the tab's shell, as the system terminals do. SSH tabs leave drops to
 * the SFTP drawer, which uploads them.
 */
export function useTerminalFileDrop({
  connection,
  isActive,
  shellName,
  termRef,
  surfaceRef,
}: UseTerminalFileDropOptions) {
  const isLocal = connection?.type !== "ssh"
  const quotingRef = useStableRef(dropQuotingFor(getDetectedPlatform(), shellName))

  useEffect(() => {
    if (!isLocal || !isActive) return

    let disposed = false
    let unlisten: (() => void) | undefined
    getCurrentWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type !== "drop") return
        const surface = surfaceRef.current
        const term = termRef.current
        if (!surface || !term) return
        // The OS drops on the window, which may show several panes side by
        // side; only the pane under the cursor takes the paths. A dialog
        // over the terminal is the target instead, so it takes none.
        const { x, y } = event.payload.position
        const ratio = window.devicePixelRatio || 1
        const target = document.elementFromPoint(x / ratio, y / ratio)
        if (!target || !surface.contains(target)) return
        const text = droppedPathsText(event.payload.paths, quotingRef.current)
        if (!text) return
        term.focus()
        term.paste(text)
      })
      .then((stop) => {
        if (disposed) stop()
        else unlisten = stop
      })
      .catch(console.error)

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [isActive, isLocal, quotingRef, surfaceRef, termRef])
}
