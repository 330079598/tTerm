import { useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"

import { configHostToLookUp, type SshConfigHost } from "@/lib/quickConnect"

/** How long typing pauses before the typed host is looked up. */
const LOOKUP_DELAY_MS = 150

interface LookedUpHost {
  name: string
  host: SshConfigHost | null
}

/**
 * What `~/.ssh/config` sets for the host being typed when the config does not
 * list it by that name: its wildcard and `Match` blocks still reach it, as
 * they do for `ssh name`. `null` until the answer for the current name comes.
 */
export function useTypedConfigHost(
  open: boolean,
  input: string,
  configHosts: readonly SshConfigHost[]
): SshConfigHost | null {
  const name = open ? configHostToLookUp(input, configHosts) : null
  const [found, setFound] = useState<LookedUpHost | null>(null)

  useEffect(() => {
    if (!name) return
    let cancelled = false
    const timer = window.setTimeout(() => {
      invoke<SshConfigHost | null>("resolve_ssh_config_host", { name })
        .then((host) => {
          if (!cancelled) setFound({ name, host })
        })
        // Without a config the typed host is used as it is.
        .catch(() => {})
    }, LOOKUP_DELAY_MS)
    return () => {
      cancelled = true
      window.clearTimeout(timer)
    }
  }, [name])

  return found && found.name === name ? found.host : null
}
