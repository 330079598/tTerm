import React, { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { Cpu, HardDrive, MemoryStick, Network, Server } from "lucide-react"
import { invoke } from "@tauri-apps/api/core"
import type { TFunction } from "i18next"

import type { TerminalTabProps } from "@/components/TerminalTab/types"
import { useConfig } from "@/contexts/ConfigContext"
import { useToast } from "@/hooks/use-toast"

const MIN_REFRESH_INTERVAL_MS = 2000
const MAX_REFRESH_INTERVAL_MS = 60000
const DISTRO_ICON_BASE = "/assets/distro-icons"

type CpuTimes = {
  user: number
  nice: number
  system: number
  idle: number
  iowait: number
  irq: number
  softirq: number
  steal: number
  guest: number
  guestNice: number
}

type LinuxDistributionInfo = {
  id: string
  idLike: string[]
  name: string
  prettyName: string
  versionId?: string
}

type MemoryMetrics = {
  totalKib: number
  availableKib: number
  usedKib: number
  usedPercent: number
}

type DiskMetrics = {
  mount: string
  totalKib: number
  usedKib: number
  availableKib: number
  usedPercent: number
}

type ServerMetricsSnapshot = {
  supported: boolean
  unsupportedReason?: string
  distribution?: LinuxDistributionInfo
  kernel?: string
  cpuTimes?: CpuTimes
  memory?: MemoryMetrics
  primaryIp?: string
  disk?: DiskMetrics
}

type MonitorState =
  | { status: "loading" }
  | {
      status: "ready"
      snapshot: ServerMetricsSnapshot
      cpuPercent?: number
      collectedAt: number
    }
  | { status: "error"; message: string }

interface ServerMonitorBarProps {
  connection?: TerminalTabProps["connection"]
  connectionState: string
  sessionNonce: number
  tabId: string
  t: TFunction
  visible: boolean
}

function cpuTotal(times: CpuTimes) {
  return (
    times.user +
    times.nice +
    times.system +
    times.idle +
    times.iowait +
    times.irq +
    times.softirq +
    times.steal
  )
}

function cpuIdle(times: CpuTimes) {
  return times.idle + times.iowait
}

function calculateCpuPercent(previous?: CpuTimes, next?: CpuTimes) {
  if (!previous || !next) return undefined

  const totalDelta = cpuTotal(next) - cpuTotal(previous)
  const idleDelta = cpuIdle(next) - cpuIdle(previous)
  if (totalDelta <= 0) return undefined

  return Math.max(0, Math.min(100, ((totalDelta - idleDelta) / totalDelta) * 100))
}

function formatPercent(value?: number) {
  if (value === undefined || Number.isNaN(value)) return "--"
  return `${Math.round(value)}%`
}

function formatKib(value?: number) {
  if (value === undefined) return "--"
  const bytes = value * 1024
  const units = ["B", "KB", "MB", "GB", "TB"]
  let current = bytes
  let unitIndex = 0
  while (current >= 1024 && unitIndex < units.length - 1) {
    current /= 1024
    unitIndex += 1
  }
  const digits = current >= 10 || unitIndex < 2 ? 0 : 1
  return `${current.toFixed(digits)}${units[unitIndex]}`
}

function severityClass(percent?: number) {
  if (percent === undefined) return "is-muted"
  if (percent >= 90) return "is-critical"
  if (percent >= 70) return "is-warning"
  return "is-normal"
}

function normalizeDistroId(snapshot?: ServerMetricsSnapshot) {
  const values = [
    snapshot?.distribution?.id,
    ...(snapshot?.distribution?.idLike ?? []),
    snapshot?.distribution?.name,
    snapshot?.distribution?.prettyName,
  ]
    .filter(Boolean)
    .map((value) => value!.toLowerCase())

  if (values.some((value) => value.includes("archlinux"))) return "arch"
  if (values.some((value) => value.includes("ubuntu"))) return "ubuntu"
  if (values.some((value) => value.includes("linuxmint") || value.includes("linux mint"))) {
    return "linuxmint"
  }
  if (values.some((value) => value.includes("mint"))) return "linuxmint"
  if (values.some((value) => value.includes("debian"))) return "debian"
  if (values.some((value) => value.includes("gentoo"))) return "gentoo"
  if (values.some((value) => value.includes("manjaro"))) return "manjaro"
  if (values.some((value) => value.includes("kali"))) return "kali"
  if (values.some((value) => value.includes("pop!_os") || value.includes("pop_os"))) return "popos"
  if (values.some((value) => value.includes("pop os") || value.includes("pop!"))) return "popos"
  if (values.some((value) => value.includes("zorin"))) return "zorin"
  if (values.some((value) => value.includes("elementary"))) return "elementary"
  if (values.some((value) => value === "mx" || value.includes("mx linux"))) return "mx"
  if (values.some((value) => value.includes("endeavouros"))) return "endeavouros"
  if (values.some((value) => value.includes("endeavour"))) return "endeavouros"
  if (values.some((value) => value.includes("cachyos"))) return "cachyos"
  if (values.some((value) => value.includes("nixos") || value.includes("nix os"))) return "nixos"
  if (values.some((value) => value.includes("nobara"))) return "nobara"
  if (values.some((value) => value.includes("bazzite"))) return "bazzite"
  if (values.some((value) => value.includes("antix"))) return "antix"
  if (values.some((value) => value.includes("biglinux") || value.includes("big linux")))
    return "biglinux"
  if (values.some((value) => value.includes("deepin"))) return "deepin"
  if (values.some((value) => value.includes("garuda"))) return "garuda"
  if (values.some((value) => value.includes("slackware"))) return "slackware"
  if (values.some((value) => value.includes("void"))) return "void"
  if (values.some((value) => value.includes("parrot"))) return "parrot"
  if (values.some((value) => value.includes("rocky"))) return "rocky"
  if (values.some((value) => value.includes("alma"))) return "alma"
  if (values.some((value) => value.includes("centos"))) return "centos"
  if (
    values.some(
      (value) => value.includes("rhel") || value.includes("redhat") || value.includes("red hat")
    )
  )
    return "rhel"
  if (values.some((value) => value.includes("fedora"))) return "fedora"
  if (values.some((value) => value.includes("suse") || value.includes("opensuse"))) return "suse"
  if (values.some((value) => value.includes("arch"))) return "arch"
  if (values.some((value) => value.includes("alpine"))) return "alpine"
  if (values.some((value) => value.includes("amazon") || value === "amzn")) return "amazon"
  if (values.some((value) => value.includes("oracle"))) return "oracle"
  return "linux"
}

function getDistroLabel(snapshot?: ServerMetricsSnapshot) {
  if (!snapshot?.distribution) return "Linux"
  const pretty = snapshot.distribution.prettyName || snapshot.distribution.name
  return pretty || "Linux"
}

function getDistroIconSrc(distroId: string) {
  return `${DISTRO_ICON_BASE}/${distroId}.svg`
}

function hasClipboardPluginError(error: unknown) {
  return String(error).toLowerCase().includes("plugin:clipboard-manager")
}

function MetricItem({
  ariaLabel,
  className,
  icon,
  label,
  onClick,
  value,
}: {
  ariaLabel?: string
  className?: string
  icon: React.ReactNode
  label: string
  onClick?: () => void
  value: string
}) {
  if (onClick) {
    return (
      <button
        type="button"
        className={`server-monitor-metric is-clickable ${className ?? ""}`.trim()}
        onClick={onClick}
        aria-label={ariaLabel}
      >
        {icon}
        <span className="server-monitor-metric-label">{label}</span>
        <span className="server-monitor-metric-value">{value}</span>
      </button>
    )
  }

  return (
    <span className={`server-monitor-metric ${className ?? ""}`.trim()}>
      {icon}
      <span className="server-monitor-metric-label">{label}</span>
      <span className="server-monitor-metric-value">{value}</span>
    </span>
  )
}

export const ServerMonitorBar: React.FC<ServerMonitorBarProps> = ({
  connection,
  connectionState,
  sessionNonce,
  tabId,
  t,
  visible,
}) => {
  const { toast } = useToast()
  const { config } = useConfig()
  const [state, setState] = useState<MonitorState>({ status: "loading" })
  const previousCpuTimesRef = useRef<CpuTimes | undefined>()
  const requestIdRef = useRef(0)
  const refreshIntervalMs = useMemo(() => {
    const configuredMs = config.monitor_refresh_interval_secs * 1000
    if (!Number.isFinite(configuredMs)) {
      return 5000
    }

    return Math.min(
      Math.max(Math.round(configuredMs), MIN_REFRESH_INTERVAL_MS),
      MAX_REFRESH_INTERVAL_MS
    )
  }, [config.monitor_refresh_interval_secs])
  const staleAfterMs = useMemo(
    () => Math.max(12000, refreshIntervalMs * 2 + 2000),
    [refreshIntervalMs]
  )

  const copyIpAddress = useCallback(
    async (ipAddress: string) => {
      try {
        await invoke("plugin:clipboard-manager|write_text", { text: ipAddress })
        toast({
          title: t("serverMonitor.copyIpSuccess", { defaultValue: "IP copied" }),
          description: ipAddress,
        })
      } catch (error) {
        if (!hasClipboardPluginError(error) && navigator.clipboard?.writeText) {
          try {
            await navigator.clipboard.writeText(ipAddress)
            toast({
              title: t("serverMonitor.copyIpSuccess", { defaultValue: "IP copied" }),
              description: ipAddress,
            })
            return
          } catch (clipboardError) {
            console.error("Failed to copy server IP address:", clipboardError)
          }
        } else {
          console.error("Failed to copy server IP address:", error)
        }

        toast({
          title: t("serverMonitor.copyIpFailed", { defaultValue: "Copy failed" }),
          description: t("serverMonitor.copyIpFailedDescription", {
            defaultValue: "Unable to copy the server IP address.",
          }),
        })
      }
    },
    [t, toast]
  )

  useEffect(() => {
    if (!visible || connection?.type !== "ssh" || connectionState !== "connected") {
      return
    }

    let cancelled = false
    let timeoutId: number | undefined
    previousCpuTimesRef.current = undefined

    const refresh = async () => {
      const requestId = requestIdRef.current + 1
      requestIdRef.current = requestId

      try {
        const snapshot = await invoke<ServerMetricsSnapshot>("get_server_metrics_snapshot", {
          tabId,
          connection,
        })
        if (cancelled || requestId !== requestIdRef.current) return

        const cpuPercent = calculateCpuPercent(previousCpuTimesRef.current, snapshot.cpuTimes)
        previousCpuTimesRef.current = snapshot.cpuTimes
        setState({ status: "ready", snapshot, cpuPercent, collectedAt: Date.now() })
      } catch (error) {
        if (cancelled || requestId !== requestIdRef.current) return
        setState({
          status: "error",
          message: error instanceof Error ? error.message : String(error),
        })
      } finally {
        if (!cancelled) {
          timeoutId = window.setTimeout(refresh, refreshIntervalMs)
        }
      }
    }

    setState((current) => (current.status === "ready" ? current : { status: "loading" }))
    refresh()

    return () => {
      cancelled = true
      if (timeoutId !== undefined) {
        window.clearTimeout(timeoutId)
      }
    }
  }, [connection, connectionState, refreshIntervalMs, sessionNonce, t, tabId, visible])

  const content = useMemo(() => {
    if (connectionState !== "connected") {
      return (
        <span className="server-monitor-message">
          {t("serverMonitor.disconnected", { defaultValue: "Monitor paused until SSH reconnects" })}
        </span>
      )
    }

    if (state.status === "loading") {
      return (
        <span className="server-monitor-message">
          {t("serverMonitor.loading", { defaultValue: "Loading server metrics..." })}
        </span>
      )
    }

    if (state.status === "error") {
      return <span className="server-monitor-message is-error">{state.message}</span>
    }

    const { snapshot, cpuPercent, collectedAt } = state
    if (!snapshot.supported) {
      return (
        <span className="server-monitor-message">
          {snapshot.unsupportedReason ??
            t("serverMonitor.unsupported", { defaultValue: "Only Linux hosts are supported" })}
        </span>
      )
    }

    const distroId = normalizeDistroId(snapshot)
    const distroLabel = getDistroLabel(snapshot)
    const stale = Date.now() - collectedAt > staleAfterMs
    const memoryValue = snapshot.memory
      ? `${formatKib(snapshot.memory.usedKib)}/${formatKib(snapshot.memory.totalKib)}`
      : "--"
    const ipValue = snapshot.primaryIp?.trim() || "--"
    const copyIp = snapshot.primaryIp ? () => void copyIpAddress(snapshot.primaryIp!) : undefined

    return (
      <>
        <span className="server-monitor-distro">
          <img
            className="server-monitor-distro-icon"
            src={getDistroIconSrc(distroId)}
            alt=""
            aria-hidden="true"
            onError={(event) => {
              const image = event.currentTarget
              if (!image.src.endsWith("/linux.svg")) {
                image.src = getDistroIconSrc("linux")
              }
            }}
          />
          <span className="server-monitor-distro-name">{distroLabel}</span>
        </span>
        <MetricItem
          className={severityClass(cpuPercent)}
          icon={<Cpu size={13} />}
          label="CPU"
          value={formatPercent(cpuPercent)}
        />
        <MetricItem
          className={severityClass(snapshot.memory?.usedPercent)}
          icon={<MemoryStick size={13} />}
          label="MEM"
          value={memoryValue}
        />
        <MetricItem
          ariaLabel={t("serverMonitor.copyIp", {
            defaultValue: "Copy server IP address",
          })}
          icon={<Network size={13} />}
          label="IP"
          onClick={copyIp}
          value={ipValue}
        />
        <MetricItem
          className={severityClass(snapshot.disk?.usedPercent)}
          icon={<HardDrive size={13} />}
          label={`DISK ${snapshot.disk?.mount ?? "/"}`}
          value={formatPercent(snapshot.disk?.usedPercent)}
        />
        {stale && (
          <span className="server-monitor-stale">
            {t("serverMonitor.stale", { defaultValue: "stale" })}
          </span>
        )}
      </>
    )
  }, [connectionState, copyIpAddress, staleAfterMs, state, t])

  if (!visible || connection?.type !== "ssh") {
    return null
  }

  return (
    <div className="server-monitor-bar" role="status" aria-live="polite">
      <Server size={13} className="server-monitor-leading-icon" />
      {content}
    </div>
  )
}
