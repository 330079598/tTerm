import { useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"

import type { SshConfigHost } from "@/lib/quickConnect"
import type { TerminalShellProfile } from "@/lib/terminalShells"
import type { SavedProfile } from "@/types/tab"

/** What the launcher lists: the saved connections, `~/.ssh/config` hosts and local shells. */
export interface LauncherData {
  profiles: SavedProfile[]
  configHosts: SshConfigHost[]
  shells: TerminalShellProfile[]
}

/** The saved connections the launcher can open. */
const LAUNCHABLE_TYPES = new Set(["ssh", "serial", "terminal"])

/** Loads what the launcher lists each time it opens. */
export function useLauncherData(open: boolean): LauncherData {
  const [profiles, setProfiles] = useState<SavedProfile[]>([])
  const [configHosts, setConfigHosts] = useState<SshConfigHost[]>([])
  const [shells, setShells] = useState<TerminalShellProfile[]>([])

  useEffect(() => {
    if (!open) return
    let cancelled = false
    invoke<SavedProfile[]>("list_profiles")
      .then((loaded) => {
        if (!cancelled) {
          setProfiles(loaded.filter((profile) => LAUNCHABLE_TYPES.has(profile.connection_type)))
        }
      })
      .catch(() => {})
    // No ~/.ssh/config is the common case, not an error.
    invoke<{ hosts: SshConfigHost[] }>("preview_ssh_config_import", { sourcePath: null })
      .then((preview) => {
        if (!cancelled) setConfigHosts(preview.hosts.filter((host) => !host.skipped))
      })
      .catch(() => {})
    invoke<TerminalShellProfile[]>("list_available_terminal_shells")
      .then((detected) => {
        if (!cancelled) setShells(detected)
      })
      .catch(() => {})
    return () => {
      cancelled = true
    }
  }, [open])

  return { profiles, configHosts, shells }
}
