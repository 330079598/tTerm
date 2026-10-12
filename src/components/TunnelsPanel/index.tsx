import React, { useCallback, useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { openUrl } from "@tauri-apps/plugin-opener"
import { Plus, Waypoints } from "lucide-react"
import { useTranslation } from "react-i18next"

import { TunnelCard } from "@/components/TunnelsPanel/TunnelCard"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Button } from "@/components/ui/button"
import { useUserVerification } from "@/contexts/UserVerificationContext"
import { useToast } from "@/hooks/use-toast"
import { isVerificationCanceled } from "@/lib/userVerification"
import { onSyncApplied } from "@/lib/sync"
import { toErrorMessage } from "@/lib/utils"
import type { SavedProfile } from "@/types/tab"
import type {
  CredentialRequest,
  StartOutcome,
  TunnelCredentials,
  TunnelRule,
  TunnelStatus,
} from "@/types/tunnel"
import { CredentialsDialog } from "@/components/TunnelsPanel/CredentialsDialog"
import { TunnelDialog } from "@/components/TunnelsPanel/TunnelDialog"
import {
  isTunnelActive,
  mergeCredentials,
  profileLabel,
  stoppedStatus,
} from "@/components/TunnelsPanel/tunnelUtils"

interface TunnelsPanelProps {
  profilesRefreshKey?: number
}

export const TunnelsPanel: React.FC<TunnelsPanelProps> = ({ profilesRefreshKey }) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const { withVaultUnlock } = useUserVerification()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const [rules, setRules] = useState<TunnelRule[]>([])
  const [profiles, setProfiles] = useState<SavedProfile[]>([])
  const [statuses, setStatuses] = useState<Record<string, TunnelStatus>>({})
  const [loading, setLoading] = useState(true)
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editing, setEditing] = useState<TunnelRule | null>(null)
  const [credentialFlow, setCredentialFlow] = useState<{
    rule: TunnelRule
    requests: CredentialRequest[]
    entered: TunnelCredentials
    busy: boolean
  } | null>(null)
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

  const reportError = useCallback(
    (title: string, error: unknown) =>
      toast({ title, description: toErrorMessage(error), variant: "destructive" }),
    [toast]
  )

  /**
   * Starts a tunnel, asking for any password or key passphrase it is missing.
   * `entered` carries answers from earlier rounds so a wrong passphrase only
   * re-asks for that one secret.
   */
  const startTunnel = useCallback(
    async (rule: TunnelRule, entered: TunnelCredentials = {}) => {
      try {
        const outcome = await withVaultUnlock(() =>
          invoke<StartOutcome>("start_tunnel", { id: rule.id, credentials: entered })
        )
        setCredentialFlow(
          outcome.status === "needsCredentials"
            ? { rule, requests: outcome.requests, entered, busy: false }
            : null
        )
      } catch (error) {
        setCredentialFlow(null)
        if (isVerificationCanceled(error)) return
        reportError(t("tunnels.startFailed", { defaultValue: "Could not start the tunnel" }), error)
      }
    },
    [reportError, t, withVaultUnlock]
  )

  const handleCredentialsSubmit = useCallback(
    async (values: Record<string, string>, remember: boolean) => {
      if (!credentialFlow) return
      const entered = mergeCredentials(
        credentialFlow.entered,
        credentialFlow.requests,
        values,
        remember
      )
      setCredentialFlow({ ...credentialFlow, busy: true })
      await startTunnel(credentialFlow.rule, entered)
    },
    [credentialFlow, startTunnel]
  )

  const handleToggle = useCallback(
    async (rule: TunnelRule, status: TunnelStatus) => {
      if (!isTunnelActive(status.state)) {
        await startTunnel(rule)
        return
      }
      try {
        await invoke("stop_tunnel", { id: rule.id })
      } catch (error) {
        reportError(t("tunnels.stopFailed", { defaultValue: "Could not stop the tunnel" }), error)
      }
    },
    [reportError, startTunnel, t]
  )

  const handleSave = useCallback(
    async (rule: TunnelRule) => {
      const wasActive = isTunnelActive(statuses[rule.id]?.state ?? "stopped")
      await invoke("save_tunnel", { tunnel: rule })
      await reload()
      if (wasActive) {
        // A running tunnel keeps its old settings, so restart it to apply the edit.
        try {
          await invoke("stop_tunnel", { id: rule.id })
        } catch (error) {
          reportError(t("tunnels.stopFailed", { defaultValue: "Could not stop the tunnel" }), error)
          return
        }
        await startTunnel(rule)
      }
    },
    [reload, reportError, startTunnel, statuses, t]
  )

  const handleDelete = useCallback(
    async (rule: TunnelRule) => {
      const confirmed = await confirm({
        title: t("tunnels.deleteConfirmTitle", { defaultValue: "Delete this tunnel?" }),
        description: t("tunnels.deleteConfirmDescription", {
          name: rule.name,
          defaultValue: "“{{name}}” will be stopped and removed.",
        }),
        confirmText: t("tunnels.delete", { defaultValue: "Delete" }),
        variant: "destructive",
      })
      if (!confirmed) return
      try {
        await invoke("delete_tunnel", { id: rule.id })
        await reload()
      } catch (error) {
        reportError(t("tunnels.deleteFailed", { defaultValue: "Failed to delete tunnel" }), error)
      }
    },
    [confirm, reload, reportError, t]
  )

  const handleResetTraffic = useCallback(
    async (rule: TunnelRule) => {
      const confirmed = await confirm({
        title: t("tunnels.stats.resetConfirmTitle", { defaultValue: "Reset traffic totals?" }),
        description: t("tunnels.stats.resetConfirmDescription", {
          name: rule.name,
          defaultValue: "The all-time traffic of “{{name}}” will be set to zero.",
        }),
        confirmText: t("tunnels.stats.reset", { defaultValue: "Reset totals" }),
        variant: "destructive",
      })
      if (!confirmed) return
      try {
        await invoke("reset_tunnel_traffic_totals", { id: rule.id })
        // A running tunnel reports its new totals itself; a stopped one does not.
        await reload()
      } catch (error) {
        reportError(
          t("tunnels.stats.resetFailed", { defaultValue: "Could not reset the traffic totals" }),
          error
        )
      }
    },
    [confirm, reload, reportError, t]
  )

  const handleCopy = useCallback(
    async (address: string) => {
      try {
        await invoke("plugin:clipboard-manager|write_text", { text: address })
      } catch {
        try {
          await navigator.clipboard.writeText(address)
        } catch (error) {
          reportError(t("tunnels.copyFailed", { defaultValue: "Could not copy" }), error)
          return
        }
      }
      toast({ title: t("tunnels.copied", { address, defaultValue: "Copied {{address}}" }) })
    },
    [reportError, t, toast]
  )

  const handleOpen = useCallback(
    (url: string) => {
      openUrl(url).catch((error) =>
        reportError(t("tunnels.openFailed", { defaultValue: "Could not open the browser" }), error)
      )
    },
    [reportError, t]
  )

  const openCreate = () => {
    setEditing(null)
    setDialogOpen(true)
  }
  const openEdit = useCallback((rule: TunnelRule) => {
    setEditing(rule)
    setDialogOpen(true)
  }, [])

  const runningCount = rules.filter((rule) => statuses[rule.id]?.state === "running").length

  return (
    <section className="flex h-full min-h-0 flex-col">
      <header className="border-border/80 flex items-start justify-between gap-4 border-b px-6 py-4">
        <div className="min-w-0">
          <h2 className="text-base font-semibold tracking-[-0.02em]">
            {t("tunnels.title", { defaultValue: "Port Forwarding" })}
          </h2>
          <p className="text-muted-foreground mt-1 text-sm">
            {t("tunnels.description", {
              defaultValue: "Reach private services through SSH, or expose local ones to a server.",
            })}
            {runningCount > 0 && (
              <span className="text-success ml-2 font-medium">
                {t("tunnels.runningCount", {
                  count: runningCount,
                  defaultValue: "{{count}} running",
                })}
              </span>
            )}
          </p>
        </div>
        <Button type="button" onClick={openCreate}>
          <Plus />
          {t("tunnels.new", { defaultValue: "New tunnel" })}
        </Button>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto p-6">
        {loading ? null : rules.length === 0 ? (
          <div className="mx-auto flex max-w-md flex-col items-center gap-3 py-16 text-center">
            <div className="bg-muted text-muted-foreground flex size-12 items-center justify-center rounded-xl border">
              <Waypoints className="size-6" />
            </div>
            <h3 className="text-base font-semibold">
              {t("tunnels.emptyTitle", { defaultValue: "No tunnels yet" })}
            </h3>
            <p className="text-muted-foreground text-sm">
              {t("tunnels.emptyDescription", {
                defaultValue:
                  "Create a local, remote or dynamic (SOCKS5) forward and switch it on whenever you need it.",
              })}
            </p>
            <Button type="button" onClick={openCreate}>
              <Plus />
              {t("tunnels.new", { defaultValue: "New tunnel" })}
            </Button>
          </div>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,380px),1fr))] gap-4">
            {rules.map((rule) => {
              const profile = profilesById.get(rule.profileId)
              return (
                <TunnelCard
                  key={rule.id}
                  rule={rule}
                  status={statuses[rule.id] ?? stoppedStatus(rule.id)}
                  hostLabel={
                    profileLabel(profile) || t("tunnels.route.server", { defaultValue: "SSH host" })
                  }
                  hostMissing={profiles.length > 0 && !profile}
                  now={now}
                  onToggle={handleToggle}
                  onCopy={handleCopy}
                  onOpen={handleOpen}
                  onEdit={openEdit}
                  onDelete={handleDelete}
                  onResetTraffic={handleResetTraffic}
                />
              )
            })}
          </div>
        )}
      </div>

      <TunnelDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        tunnel={editing}
        profiles={sshProfiles}
        onSave={handleSave}
      />
      {credentialFlow && (
        <CredentialsDialog
          key={credentialFlow.requests
            .map((request) => `${request.hop}:${request.kind}:${request.incorrect}`)
            .join("|")}
          tunnelName={credentialFlow.rule.name}
          requests={credentialFlow.requests}
          busy={credentialFlow.busy}
          onSubmit={handleCredentialsSubmit}
          onCancel={() => setCredentialFlow(null)}
        />
      )}
      <ConfirmDialog />
    </section>
  )
}
