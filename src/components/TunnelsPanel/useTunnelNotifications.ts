import { useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useTranslation } from "react-i18next"

import { describeTransition } from "@/components/TunnelsPanel/tunnelUtils"
import { toast } from "@/hooks/use-toast"
import type { TunnelRule, TunnelState, TunnelStatus } from "@/types/tunnel"

/**
 * Tells the user when a tunnel drops, comes back or gives up, whichever page
 * of the app they are on. Mounted once at the app level.
 */
export function useTunnelNotifications(): void {
  const { t } = useTranslation()

  useEffect(() => {
    let disposed = false
    let off: (() => void) | undefined
    const previous = new Map<string, TunnelState>()

    // Transitions are rare, so look the name up fresh; it may have been renamed.
    const nameOf = async (id: string): Promise<string> => {
      try {
        const rules = await invoke<TunnelRule[]>("list_tunnels")
        return rules.find((rule) => rule.id === id)?.name ?? id
      } catch (error) {
        console.error("Failed to load tunnel names:", error)
        return id
      }
    }

    void listen<TunnelStatus>("tunnel-status", async (event) => {
      const status = event.payload
      const transition = describeTransition(previous.get(status.id), status.state)
      previous.set(status.id, status.state)
      if (!transition) return

      const name = await nameOf(status.id)
      const description = status.message ?? undefined
      if (transition === "lost") {
        toast({
          title: t("tunnels.notify.lost", { name, defaultValue: "“{{name}}” lost its connection" }),
          description,
        })
      } else if (transition === "restored") {
        toast({
          title: t("tunnels.notify.restored", { name, defaultValue: "“{{name}}” is back online" }),
        })
      } else {
        toast({
          title: t("tunnels.notify.failed", { name, defaultValue: "“{{name}}” stopped" }),
          description,
          variant: "destructive",
        })
      }
    }).then((unlisten) => {
      if (disposed) unlisten()
      else off = unlisten
    })

    return () => {
      disposed = true
      off?.()
    }
  }, [t])
}
