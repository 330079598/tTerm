import React, { useCallback, useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { ArrowDown, ArrowUp, Pencil, Play, Plus, Square, Trash2, Waypoints } from "lucide-react"
import { useTranslation } from "react-i18next"

import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { useToast } from "@/hooks/use-toast"
import { cn, toErrorMessage } from "@/lib/utils"
import type { SavedProfile } from "@/types/tab"
import type {
  CredentialRequest,
  StartOutcome,
  TunnelCredentials,
  TunnelRule,
  TunnelState,
  TunnelStatus,
} from "@/types/tunnel"
import { CredentialsDialog } from "@/components/TunnelsPanel/CredentialsDialog"
import { TunnelDialog } from "@/components/TunnelsPanel/TunnelDialog"
import { TunnelRoute } from "@/components/TunnelsPanel/TunnelRoute"
import {
  formatBytes,
  formatUptime,
  isTunnelActive,
  mergeCredentials,
  profileLabel,
  stoppedStatus,
} from "@/components/TunnelsPanel/tunnelUtils"

const STATUS_DOT: Record<TunnelState, string> = {
  stopped: "bg-muted-foreground/40",
  starting: "bg-warning animate-pulse",
  running: "bg-success",
  reconnecting: "bg-warning animate-pulse",
  error: "bg-destructive",
  needsCredentials: "bg-warning",
}

interface TunnelsPanelProps {
  profilesRefreshKey?: number
}

interface TunnelCardProps {
  rule: TunnelRule
  status: TunnelStatus
  hostLabel: string
  hostMissing: boolean
  now: number
  onToggle: (rule: TunnelRule, status: TunnelStatus) => void
  onEdit: (rule: TunnelRule) => void
  onDelete: (rule: TunnelRule) => void
}

const TunnelCard = React.memo(function TunnelCard({
  rule,
  status,
  hostLabel,
  hostMissing,
  now,
  onToggle,
  onEdit,
  onDelete,
}: TunnelCardProps) {
  const { t } = useTranslation()
  const active = isTunnelActive(status.state)
  const shownBindPort = status.boundPort ?? rule.bindPort
  const stateLabel = t(`tunnels.state.${status.state}`, { defaultValue: status.state })

  return (
    <article
      className="bg-card flex flex-col gap-3 rounded-lg border p-4 shadow-sm"
      aria-label={rule.name}
    >
      <header className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h3 className="truncate text-sm font-semibold" title={rule.name}>
            {rule.name}
          </h3>
          <div className="text-muted-foreground mt-1 flex items-center gap-2 text-xs">
            <Badge variant="outline" className="font-mono text-[10px] uppercase">
              {t(`tunnels.kinds.${rule.kind}.badge`, { defaultValue: rule.kind })}
            </Badge>
            <span className="flex items-center gap-1.5">
              <span className={cn("size-2 rounded-full", STATUS_DOT[status.state])} aria-hidden />
              {stateLabel}
              {status.state === "running" && status.connectedAt != null && (
                <span className="tabular-nums">· {formatUptime(status.connectedAt, now)}</span>
              )}
            </span>
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-1">
          <Button
            type="button"
            size="icon-sm"
            variant="ghost"
            aria-label={t("tunnels.edit", { defaultValue: "Edit" })}
            title={t("tunnels.edit", { defaultValue: "Edit" })}
            onClick={() => onEdit(rule)}
          >
            <Pencil />
          </Button>
          <Button
            type="button"
            size="icon-sm"
            variant="ghost"
            className="hover:text-destructive"
            aria-label={t("tunnels.delete", { defaultValue: "Delete" })}
            title={t("tunnels.delete", { defaultValue: "Delete" })}
            onClick={() => onDelete(rule)}
          >
            <Trash2 />
          </Button>
          <Button
            type="button"
            size="sm"
            variant={active ? "outline" : "default"}
            disabled={hostMissing && !active}
            onClick={() => onToggle(rule, status)}
          >
            {active ? <Square className="size-3" /> : <Play className="size-3" />}
            {active
              ? t("tunnels.stop", { defaultValue: "Stop" })
              : t("tunnels.start", { defaultValue: "Start" })}
          </Button>
        </div>
      </header>

      <TunnelRoute
        kind={rule.kind}
        bindHost={rule.bindHost}
        bindPort={shownBindPort}
        destHost={rule.destHost}
        destPort={rule.destPort}
        hostLabel={hostLabel}
        state={status.state}
      />

      <footer className="text-muted-foreground min-h-4 text-xs">
        {hostMissing ? (
          <span className="text-destructive">
            {t("tunnels.hostMissing", {
              defaultValue: "The saved host for this tunnel was deleted.",
            })}
          </span>
        ) : status.state === "needsCredentials" ? (
          <span className="text-warning">
            {t("tunnels.needsCredentialsHint", {
              defaultValue:
                "Not started automatically — press Start to enter the missing password.",
            })}
          </span>
        ) : status.message && (status.state === "error" || status.state === "reconnecting") ? (
          <span
            className={status.state === "error" ? "text-destructive" : "text-warning"}
            role={status.state === "error" ? "alert" : undefined}
          >
            {status.message}
          </span>
        ) : status.state === "running" ? (
          <span className="flex flex-wrap items-center gap-x-3 gap-y-1 tabular-nums">
            <span>
              {t("tunnels.stats.connections", {
                count: status.activeConnections,
                defaultValue: "{{count}} active",
              })}
              {" · "}
              {t("tunnels.stats.total", {
                count: status.totalConnections,
                defaultValue: "{{count}} total",
              })}
            </span>
            <span className="inline-flex items-center gap-0.5">
              <ArrowUp
                className="size-3"
                aria-label={t("tunnels.stats.up", { defaultValue: "Sent" })}
              />
              {formatBytes(status.bytesUp)}
            </span>
            <span className="inline-flex items-center gap-0.5">
              <ArrowDown
                className="size-3"
                aria-label={t("tunnels.stats.down", { defaultValue: "Received" })}
              />
              {formatBytes(status.bytesDown)}
            </span>
          </span>
        ) : null}
      </footer>
    </article>
  )
})

export const TunnelsPanel: React.FC<TunnelsPanelProps> = ({ profilesRefreshKey }) => {
  const { t } = useTranslation()
  const { toast } = useToast()
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
        const outcome = await invoke<StartOutcome>("start_tunnel", {
          id: rule.id,
          credentials: entered,
        })
        setCredentialFlow(
          outcome.status === "needsCredentials"
            ? { rule, requests: outcome.requests, entered, busy: false }
            : null
        )
      } catch (error) {
        setCredentialFlow(null)
        reportError(t("tunnels.startFailed", { defaultValue: "Could not start the tunnel" }), error)
      }
    },
    [reportError, t]
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
    <section className="bg-background flex h-full min-h-0 flex-col">
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
                  onEdit={openEdit}
                  onDelete={handleDelete}
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
