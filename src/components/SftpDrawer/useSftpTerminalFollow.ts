import { useCallback, useEffect, useRef, useState } from "react"

import type {
  LoadSftpDirectory,
  SftpDirectoryListing,
  SftpDrawerProps,
} from "@/components/SftpDrawer/types"
import { useConfig } from "@/contexts/ConfigContext"
import { useStableRef } from "@/hooks/useStableRef"
import { useTabRemoteCwd } from "@/lib/remoteCwd"
import { sftpFallbackPath, sftpStartPath } from "@/lib/sftpStartDirectory"

interface UseSftpTerminalFollowOptions {
  tabId: string
  connection: SftpDrawerProps["connection"]
  visible: boolean
  listing: SftpDirectoryListing | null
  isLoading: boolean
  loadDirectory: LoadSftpDirectory
}

/**
 * Opens the drawer's first listing where the connection or the SFTP setting
 * says, and while following is on, moves the drawer to each directory the
 * terminal's shell reports. Following starts on for drawers that open at the
 * terminal's directory, and each tab can turn it on or off.
 */
export function useSftpTerminalFollow({
  tabId,
  connection,
  visible,
  listing,
  isLoading,
  loadDirectory,
}: UseSftpTerminalFollowOptions) {
  const { config } = useConfig()
  const terminalCwd = useTabRemoteCwd(tabId) ?? null
  const startChoice = connection?.sftpStartDirectory ?? config.sftp_start_directory
  const [followChoice, setFollowChoice] = useState<boolean | null>(null)
  const follows = followChoice ?? startChoice === "terminal"
  // The terminal directory the drawer last went to: one it could not list is
  // not tried again until the shell reports another.
  const followedCwdRef = useRef<string | null>(null)
  const hasListing = listing !== null
  const startRef = useStableRef({ startChoice, follows, terminalCwd })

  useEffect(() => {
    if (!visible || hasListing) return
    const { startChoice, follows, terminalCwd } = startRef.current
    const start = sftpStartPath(startChoice, follows, terminalCwd)
    const fallback = sftpFallbackPath(startChoice)
    followedCwdRef.current = terminalCwd
    if (start === fallback) {
      void loadDirectory(start)
      return
    }
    // A terminal directory SFTP cannot list (another user's after `su`, a
    // path that is gone) opens the drawer where it would open otherwise.
    loadDirectory(start, { throwOnError: true }).catch(() => loadDirectory(fallback))
  }, [hasListing, loadDirectory, startRef, visible])

  useEffect(() => {
    if (!follows || !visible || !hasListing || isLoading || !terminalCwd) return
    if (terminalCwd === followedCwdRef.current) return
    followedCwdRef.current = terminalCwd
    void loadDirectory(terminalCwd)
  }, [follows, hasListing, isLoading, loadDirectory, terminalCwd, visible])

  const toggleFollow = useCallback(() => {
    // Turning following on goes to the terminal's directory right away.
    if (!follows) followedCwdRef.current = null
    setFollowChoice(!follows)
  }, [follows])

  return { follows, toggleFollow, terminalCwd }
}
