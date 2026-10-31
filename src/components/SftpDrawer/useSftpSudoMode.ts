import type React from "react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import type { LoadSftpDirectory, SftpDrawerProps } from "@/components/SftpDrawer/types"

type Connection = SftpDrawerProps["connection"]

/**
 * Whether the drawer works on the host as root through sudo, and the
 * connection its requests carry for that. The mode is this drawer's alone
 * and starts off whenever the tab opens, so root is never the quiet default.
 */
export function useSftpSudoMode(connection: Connection) {
  const [sudoMode, setSudoMode] = useState(false)
  // A folder to open once the drawer works as root, instead of the one in view.
  const rootPathRef = useRef<string | null>(null)
  const drawerConnection = useMemo<Connection>(
    () => (connection && sudoMode ? { ...connection, sftpSudo: true } : connection),
    [connection, sudoMode]
  )
  /** Switches to root and opens `path` as root, such as a folder the user was refused. */
  const openAsRoot = useCallback((path: string | null) => {
    rootPathRef.current = path
    setSudoMode(true)
  }, [])
  return { connection: drawerConnection, sudoMode, setSudoMode, openAsRoot, rootPathRef }
}

interface ReloadOnSudoSwitchOptions {
  sudoMode: boolean
  setSudoMode: (sudoMode: boolean) => void
  /** Set by `openAsRoot`: the folder to open as root in place of the one in view. */
  rootPathRef: React.RefObject<string | null>
  loadDirectory: LoadSftpDirectory
  currentPath: string | null
}

/**
 * Loads the folder in view again once the drawer switched between working
 * as the user and as root, so the listing shows what the new identity sees.
 * A switch to root that fails, because sudo refused or the password prompt
 * was cancelled, goes back to the user without loading again: the error
 * stays in view, beside the listing the user already had. Back as the user,
 * a folder only root could read gives way to the drawer's first folder.
 */
export function useReloadOnSudoSwitch({
  sudoMode,
  setSudoMode,
  rootPathRef,
  loadDirectory,
  currentPath,
}: ReloadOnSudoSwitchOptions) {
  const loadedAs = useRef(sudoMode)

  useEffect(() => {
    if (loadedAs.current === sudoMode) return
    loadedAs.current = sudoMode
    if (sudoMode) {
      const path = rootPathRef.current ?? currentPath
      rootPathRef.current = null
      loadDirectory(path, { throwOnError: true }).catch(() => {
        loadedAs.current = false
        setSudoMode(false)
      })
    } else {
      loadDirectory(currentPath, { throwOnError: true }).catch(() => loadDirectory(null))
    }
  }, [currentPath, loadDirectory, rootPathRef, setSudoMode, sudoMode])
}
