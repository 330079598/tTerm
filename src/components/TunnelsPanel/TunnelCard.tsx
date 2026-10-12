import React from "react"
import {
  AlertTriangle,
  ArrowDown,
  ArrowUp,
  Copy,
  ExternalLink,
  Pencil,
  Play,
  RotateCcw,
  Square,
  Trash2,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import type { TunnelRule, TunnelState, TunnelStatus, TunnelTraffic } from "@/types/tunnel"
import { TunnelRoute } from "@/components/TunnelsPanel/TunnelRoute"
import {
  formatBytes,
  formatUptime,
  isTunnelActive,
  tunnelBrowserUrl,
  tunnelClientAddress,
} from "@/components/TunnelsPanel/tunnelUtils"

const STATUS_DOT: Record<TunnelState, string> = {
  stopped: "bg-muted-foreground/40",
  starting: "bg-warning animate-pulse",
  running: "bg-success",
  reconnecting: "bg-warning animate-pulse",
  error: "bg-destructive",
  needsCredentials: "bg-warning",
}

function TrafficAmounts({ up, down }: { up: number; down: number }) {
  const { t } = useTranslation()
  return (
    <>
      <span className="inline-flex items-center gap-0.5">
        <ArrowUp className="size-3" aria-label={t("tunnels.stats.up", { defaultValue: "Sent" })} />
        {formatBytes(up)}
      </span>
      <span className="inline-flex items-center gap-0.5">
        <ArrowDown
          className="size-3"
          aria-label={t("tunnels.stats.down", { defaultValue: "Received" })}
        />
        {formatBytes(down)}
      </span>
    </>
  )
}

function LifetimeTraffic({ traffic, onReset }: { traffic: TunnelTraffic; onReset: () => void }) {
  const { t, i18n } = useTranslation()
  return (
    <span className="flex flex-wrap items-center gap-x-3 gap-y-1 tabular-nums">
      <span>{t("tunnels.stats.lifetime", { defaultValue: "All time" })}</span>
      <TrafficAmounts up={traffic.bytesUp} down={traffic.bytesDown} />
      <span>
        {t("tunnels.stats.lifetimeConnections", {
          count: traffic.connections,
          defaultValue: "{{count}} connections",
        })}
        {traffic.since != null && (
          <>
            {" · "}
            {t("tunnels.stats.since", {
              date: new Date(traffic.since).toLocaleDateString(i18n.language),
              defaultValue: "since {{date}}",
            })}
          </>
        )}
      </span>
      <Button
        type="button"
        size="icon-xs"
        variant="ghost"
        className="-my-1"
        aria-label={t("tunnels.stats.reset", { defaultValue: "Reset totals" })}
        title={t("tunnels.stats.reset", { defaultValue: "Reset totals" })}
        onClick={onReset}
      >
        <RotateCcw />
      </Button>
    </span>
  )
}

interface TunnelCardProps {
  rule: TunnelRule
  status: TunnelStatus
  hostLabel: string
  hostMissing: boolean
  now: number
  onToggle: (rule: TunnelRule, status: TunnelStatus) => void
  onCopy: (address: string) => void
  onOpen: (url: string) => void
  onEdit: (rule: TunnelRule) => void
  onDelete: (rule: TunnelRule) => void
  onResetTraffic: (rule: TunnelRule) => void
}

export const TunnelCard = React.memo(function TunnelCard({
  rule,
  status,
  hostLabel,
  hostMissing,
  now,
  onToggle,
  onCopy,
  onOpen,
  onEdit,
  onDelete,
  onResetTraffic,
}: TunnelCardProps) {
  const { t } = useTranslation()
  const active = isTunnelActive(status.state)
  const shownBindPort = status.boundPort ?? rule.bindPort
  const stateLabel = t(`tunnels.state.${status.state}`, { defaultValue: status.state })
  const running = status.state === "running"
  const clientAddress = running ? tunnelClientAddress(rule, status.boundPort) : null
  const browserUrl = running ? tunnelBrowserUrl(rule, status.boundPort) : null
  const lifetime = status.lifetime
  const hasLifetime =
    lifetime.since != null ||
    lifetime.bytesUp > 0 ||
    lifetime.bytesDown > 0 ||
    lifetime.connections > 0
  const lifetimeTraffic = hasLifetime ? (
    <LifetimeTraffic traffic={lifetime} onReset={() => onResetTraffic(rule)} />
  ) : null

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
          {browserUrl && (
            <Button
              type="button"
              size="icon-sm"
              variant="ghost"
              aria-label={t("tunnels.openInBrowser", { defaultValue: "Open in browser" })}
              title={`${t("tunnels.openInBrowser", { defaultValue: "Open in browser" })} · ${browserUrl}`}
              onClick={() => onOpen(browserUrl)}
            >
              <ExternalLink />
            </Button>
          )}
          {clientAddress && (
            <Button
              type="button"
              size="icon-sm"
              variant="ghost"
              aria-label={t("tunnels.copyAddress", { defaultValue: "Copy address" })}
              title={`${t("tunnels.copyAddress", { defaultValue: "Copy address" })} · ${clientAddress}`}
              onClick={() => onCopy(clientAddress)}
            >
              <Copy />
            </Button>
          )}
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
            variant={active ? "destructive" : "ghost"}
            className={
              active
                ? "dark:bg-destructive dark:hover:bg-destructive/90"
                : "border-primary/30 bg-primary/10 text-primary hover:bg-primary/20 hover:text-primary dark:hover:bg-primary/20 border"
            }
            disabled={hostMissing && !active}
            onClick={() => onToggle(rule, status)}
          >
            {active ? (
              <Square className="size-3" fill="currentColor" />
            ) : (
              <Play className="size-3" fill="currentColor" />
            )}
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
              defaultValue: "A password is missing — press Start to enter it.",
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
          <div className="flex flex-col gap-1">
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
              <TrafficAmounts up={status.bytesUp} down={status.bytesDown} />
            </span>
            {lifetimeTraffic}
            {status.lastFailure && (
              <span
                className="text-warning flex items-start gap-1 break-all"
                title={status.lastFailure.message}
              >
                <AlertTriangle className="mt-0.5 size-3 shrink-0" aria-hidden />
                <span className="min-w-0">
                  {t("tunnels.stats.failed", {
                    count: status.failedConnections,
                    defaultValue: "{{count}} failed",
                  })}
                  {" · "}
                  {status.lastFailure.message}
                  {" · "}
                  {t("tunnels.stats.ago", {
                    time: formatUptime(status.lastFailure.at, now),
                    defaultValue: "{{time}} ago",
                  })}
                </span>
              </span>
            )}
          </div>
        ) : (
          lifetimeTraffic
        )}
      </footer>
    </article>
  )
})
