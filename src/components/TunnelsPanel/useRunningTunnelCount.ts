import { useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"

import type { TunnelStatus } from "@/types/tunnel"

/**
 * Number of tunnels currently in the `running` state, kept live from the
 * `tunnel-status` event so the title bar can show it on every page.
 */
export function useRunningTunnelCount(): number {
  const [count, setCount] = useState(0)

  useEffect(() => {
    let disposed = false
    let off: (() => void) | undefined
    const running = new Set<string>()
    // Ids reported by an event; newer than anything the initial fetch returns.
    const seen = new Set<string>()

    const apply = (status: TunnelStatus) => {
      if (status.state === "running") running.add(status.id)
      else running.delete(status.id)
      setCount(running.size)
    }

    const register = async () => {
      // Subscribe before the first fetch so no status change is missed in between.
      const unlisten = await listen<TunnelStatus>("tunnel-status", (event) => {
        seen.add(event.payload.id)
        apply(event.payload)
      })
      if (disposed) {
        unlisten()
        return
      }
      off = unlisten
      try {
        const statuses = await invoke<TunnelStatus[]>("list_tunnel_statuses")
        if (disposed) return
        statuses.filter((status) => !seen.has(status.id)).forEach(apply)
      } catch (error) {
        console.error("Failed to load tunnel statuses:", error)
      }
    }
    void register()

    return () => {
      disposed = true
      off?.()
    }
  }, [])

  return count
}
