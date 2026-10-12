import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useCallback, useEffect, useMemo, useState } from "react"
import { useTranslation } from "react-i18next"

import { useToast } from "@/hooks/use-toast"
import { onSyncApplied } from "@/lib/sync"
import { toErrorMessage } from "@/lib/utils"
import type { SavedProfile } from "@/types/tab"
import type { TunnelRule, TunnelStatus } from "@/types/tunnel"

/**
 * The tunnels, their live status and the hosts they go through, kept up to
 * date by the backend's status events and by syncs. `now` ticks every second
 * while one is running, for its uptime.
 */
export function useTunnelList(profilesRefreshKey: number | undefined) {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [rules, setRules] = useState<TunnelRule[]>([])
  const [profiles, setProfiles] = useState<SavedProfile[]>([])
  const [statuses, setStatuses] = useState<Record<string, TunnelStatus>>({})
  const [loading, setLoading] = useState(true)
  const [now, setNow] = useState(() => Date.now())

  const sshProfiles = useMemo(
    () => profiles.filter((profile) => profile.connection_type === "ssh"),
    [profiles]
  )
  const profilesById = useMemo(
    () => new Map(profiles.map((profile) => [profile.id, profile])),
    [profiles]
  )

  const applyStatus = useCallback((status: TunnelStatus) => {
    setStatuses((current) => ({ ...current, [status.id]: status }))
  }, [])

  const reload = useCallback(async () => {
    const [nextRules, nextStatuses] = await Promise.all([
      invoke<TunnelRule[]>("list_tunnels"),
      invoke<TunnelStatus[]>("list_tunnel_statuses"),
    ])
    setRules(nextRules)
    setStatuses(Object.fromEntries(nextStatuses.map((status) => [status.id, status])))
  }, [])

  useEffect(() => {
    let disposed = false
    const unlisteners: Array<() => void> = []
    const register = async () => {
      // Subscribe before the first fetch so no status change is missed in between.
      const offStatus = await listen<TunnelStatus>("tunnel-status", (event) =>
        applyStatus(event.payload)
      )
      if (disposed) {
        offStatus()
        return
      }
      unlisteners.push(offStatus)
      try {
        await reload()
      } catch (error) {
        toast({
          title: t("tunnels.loadFailed", { defaultValue: "Failed to load tunnels" }),
          description: toErrorMessage(error),
          variant: "destructive",
        })
      } finally {
        if (!disposed) setLoading(false)
      }
    }
    void register()
    return () => {
      disposed = true
      unlisteners.forEach((off) => off())
    }
  }, [applyStatus, reload, t, toast])

  useEffect(
    () =>
      onSyncApplied(["tunnels"], () => {
        reload().catch((error) => console.error("Failed to reload tunnels:", error))
      }),
    [reload]
  )

  useEffect(() => {
    invoke<SavedProfile[]>("list_profiles")
      .then(setProfiles)
      .catch((error) => console.error("Failed to load profiles:", error))
  }, [profilesRefreshKey])

  const hasRunning = rules.some((rule) => statuses[rule.id]?.state === "running")
  useEffect(() => {
    if (!hasRunning) return
    const timer = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [hasRunning])

  return { rules, profiles, statuses, loading, now, sshProfiles, profilesById, reload }
}
