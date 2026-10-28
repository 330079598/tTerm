import type { TFunction } from "i18next"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useCallback, useEffect, useRef } from "react"

import { toast } from "@/hooks/use-toast"
import { useLatestRef } from "@/hooks/useLatestRef"

/** A directory tTerm was asked to open a terminal in, as `take_launch_dirs` returns it. */
interface LaunchDir {
  path: string
  /** Whether the path is a directory a shell can start in. */
  exists: boolean
}

/**
 * Opens a local terminal in each directory tTerm is started with, or that a
 * later `tterm <dir>` hands to it. Returns the function that opens those
 * waiting; the session restore calls it first, so the new tabs come after the
 * restored ones, and it resolves to whether it opened any.
 */
export function useLaunchDirs(t: TFunction, openLocalTerminal: (cwd: string) => void) {
  const latest = useRef({ t, openLocalTerminal })
  useLatestRef(latest, { t, openLocalTerminal })
  // Directories announced before the restore are left for it to take.
  const startedRef = useRef(false)

  const openLaunchDirs = useCallback(async () => {
    startedRef.current = true
    let dirs: LaunchDir[]
    try {
      dirs = await invoke<LaunchDir[]>("take_launch_dirs")
    } catch (error) {
      console.error("Failed to take the directories to open:", error)
      return false
    }
    const { t, openLocalTerminal } = latest.current
    for (const dir of dirs) {
      if (dir.exists) {
        openLocalTerminal(dir.path)
      } else {
        toast({
          title: t("launchDir.notFoundTitle"),
          description: t("launchDir.notFoundDesc", { path: dir.path }),
          variant: "destructive",
        })
      }
    }
    return dirs.some((dir) => dir.exists)
  }, [])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void listen("launch-dirs", () => {
      if (startedRef.current) void openLaunchDirs()
    }).then((cleanup) => {
      if (disposed) {
        cleanup()
        return
      }
      unlisten = cleanup
      // One announced while the listener was being set up would be missed.
      if (startedRef.current) void openLaunchDirs()
    })
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [openLaunchDirs])

  return openLaunchDirs
}
