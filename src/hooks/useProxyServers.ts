import { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"

import { onSyncApplied } from "@/lib/sync"
import { toErrorMessage } from "@/lib/utils"
import type { ProxyServer, ProxyServerEntry } from "@/types/proxy"

/** What saving a server changes about its password. */
export interface ProxyPasswordChange {
  /** A new password to save; the saved one is kept when missing. */
  password?: string
  /** Remove the saved password. */
  clearPassword?: boolean
}

/**
 * The saved proxy servers, loaded when mounted and again after a sync
 * changed them, with saving and deleting that update the list.
 */
export function useProxyServers() {
  const [servers, setServers] = useState<ProxyServerEntry[]>([])
  const [loaded, setLoaded] = useState(false)
  const [loadError, setLoadError] = useState<string | null>(null)

  // The state is set in the promise callbacks rather than after an `await`,
  // which the effect lint can't yet tell apart from setting it synchronously.
  const reload = useCallback(
    () =>
      invoke<ProxyServerEntry[]>("list_proxies").then(
        (entries) => {
          setServers(entries)
          setLoadError(null)
          setLoaded(true)
        },
        (error: unknown) => {
          setLoadError(toErrorMessage(error))
          setLoaded(true)
        }
      ),
    []
  )

  useEffect(() => {
    void reload()
    return onSyncApplied(["proxies", "secrets"], () => void reload())
  }, [reload])

  const save = useCallback(async (server: ProxyServer, change: ProxyPasswordChange = {}) => {
    const entries = await invoke<ProxyServerEntry[]>("save_proxy", {
      server,
      password: change.password || null,
      clearPassword: change.clearPassword === true,
    })
    setServers(entries)
  }, [])

  const remove = useCallback(async (id: string) => {
    const entries = await invoke<ProxyServerEntry[]>("delete_proxy", { id })
    setServers(entries)
  }, [])

  return { servers, loaded, loadError, reload, save, remove }
}
