import { useEffect, useMemo, useRef, useState } from "react"

import type { LoadSftpDirectory, SftpDrawerProps } from "@/components/SftpDrawer/types"

type Connection = SftpDrawerProps["connection"]

/**
 * Whether the drawer works on the host as root through sudo, and the
 * connection its requests carry for that. The mode is this drawer's alone
 * and starts off whenever the tab opens, so root is never the quiet default.
 */
export function useSftpSudoMode(connection: Connection) {
  const [sudoMode, setSudoMode] = useState(false)
  const drawerConnection = useMemo<Connection>(
    () => (connection && sudoMode ? { ...connection, sftpSudo: true } : connection),
    [connection, sudoMode]
  )
  return { connection: drawerConnection, sudoMode, setSudoMode }
}

interface ReloadOnSudoSwitchOptions {
  sudoMode: boolean
  setSudoMode: (sudoMode: boolean) => void
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
  loadDirectory,
  currentPath,
}: ReloadOnSudoSwitchOptions) {
  const loadedAs = useRef(sudoMode)

  useEffect(() => {
    if (loadedAs.current === sudoMode) return
    loadedAs.current = sudoMode
    if (sudoMode) {
      loadDirectory(currentPath, { throwOnError: true }).catch(() => {
        loadedAs.current = false
        setSudoMode(false)
      })
    } else {
      loadDirectory(currentPath, { throwOnError: true }).catch(() => loadDirectory(null))
    }
  }, [currentPath, loadDirectory, setSudoMode, sudoMode])
}
