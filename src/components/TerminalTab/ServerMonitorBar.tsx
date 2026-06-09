import React, { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { Cpu, HardDrive, MemoryStick, Network, Server } from "lucide-react"
import { invoke } from "@tauri-apps/api/core"
import type { TFunction } from "i18next"

import type { TerminalTabProps } from "@/components/TerminalTab/types"
import { useConfig } from "@/contexts/ConfigContext"
import { useToast } from "@/hooks/use-toast"

const MIN_REFRESH_INTERVAL_MS = 1000
const MAX_REFRESH_INTERVAL_MS = 60000
const INITIAL_CPU_SAMPLE_DELAY_MS = 300
const CPU_HISTORY_SAMPLE_LIMIT = 48
const CPU_SPARKLINE_WIDTH = 78
const CPU_SPARKLINE_HEIGHT = 16
const CPU_SPARKLINE_MAX_CORE_LINES = 6
const CPU_SPARKLINE_SMOOTHING = 0.45
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

type CpuCoreTimes = {
  id: string
  times: CpuTimes
}

type CpuHistorySample = {
  total: number
  cores: Array<{ id: string; percent: number }>
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
  cpuCoreTimes?: CpuCoreTimes[]
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

function normalizePercent(value: number) {
  return Math.max(0, Math.min(100, value))
}

function smoothPercent(previous: number | undefined, next: number) {
  if (previous === undefined) {
    return normalizePercent(next)
  }

  return normalizePercent(previous * CPU_SPARKLINE_SMOOTHING + next * (1 - CPU_SPARKLINE_SMOOTHING))
}

function calculateCpuCorePercents(previous?: CpuCoreTimes[], next?: CpuCoreTimes[]) {
  if (!previous?.length || !next?.length) {
    return []
  }

  const previousById = new Map(previous.map((core) => [core.id, core.times]))
  return next
    .map((core) => ({
      id: core.id,
      percent: calculateCpuPercent(previousById.get(core.id), core.times),
    }))
    .filter((core): core is { id: string; percent: number } => core.percent !== undefined)
}

function appendCpuHistorySample(
  history: CpuHistorySample[],
  total?: number,
  cores: Array<{ id: string; percent: number }> = []
) {
  if (total === undefined || Number.isNaN(total)) {
    return history
  }

  const previousSample = history[history.length - 1]
  const previousCoresById = new Map(
    previousSample?.cores.map((core) => [core.id, core.percent]) ?? []
  )

  return [
    ...history,
    {
      total: smoothPercent(previousSample?.total, total),
      cores: cores.map((core) => ({
        id: core.id,
        percent: smoothPercent(previousCoresById.get(core.id), core.percent),
      })),
    },
  ].slice(-CPU_HISTORY_SAMPLE_LIMIT)
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

function formatKibAsG(value?: number) {
  if (value === undefined) return "--"
  const gib = value / 1024 / 1024
  const digits = gib >= 10 ? 0 : 1
  return `${gib.toFixed(digits)}G`
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

function normalizeSessionNonce(sessionNonce: number) {
  return sessionNonce >>> 0
}

function PercentValue({ value }: { value?: number }) {
  const percent =
    value === undefined || Number.isNaN(value)
      ? "--"
      : String(Math.max(0, Math.min(100, Math.round(value))))

  return (
    <span className="server-monitor-percent-value">
      <span className="server-monitor-percent-number">{percent}</span>
      {percent !== "--" && <span>%</span>}
    </span>
  )
}

function buildCpuSparklinePoints(values: number[]) {
  const paddingX = 2
  const paddingY = 2
  const plotWidth = CPU_SPARKLINE_WIDTH - paddingX * 2
  const plotHeight = CPU_SPARKLINE_HEIGHT - paddingY * 2

  if (values.length === 0) {
    return []
  }

  return values.map((value, index) => {
    const x =
      values.length === 1
        ? CPU_SPARKLINE_WIDTH / 2
        : paddingX + (index / (values.length - 1)) * plotWidth
    const y = paddingY + (1 - normalizePercent(value) / 100) * plotHeight
    return { x, y }
  })
}

function buildSmoothPath(points: Array<{ x: number; y: number }>) {
  if (points.length < 2) {
    return ""
  }

  return points.reduce((path, point, index) => {
    if (index === 0) {
      return `M${point.x},${point.y}`
    }

    const previous = points[index - 1]
    const controlX = previous.x + (point.x - previous.x) * 0.5
    return `${path} C${controlX},${previous.y} ${controlX},${point.y} ${point.x},${point.y}`
  }, "")
}

function getCoreSeries(samples: CpuHistorySample[]) {
  const coreIds = Array.from(
    new Set(samples.flatMap((sample) => sample.cores.map((core) => core.id)))
  )

  return coreIds
    .map((id) => {
      const values = samples.map(
        (sample) => sample.cores.find((core) => core.id === id)?.percent ?? 0
      )
      return { id, values, latest: values[values.length - 1] ?? 0 }
    })
    .sort((first, second) => second.latest - first.latest)
    .slice(0, CPU_SPARKLINE_MAX_CORE_LINES)
}

function CpuSparkline({ samples }: { samples: CpuHistorySample[] }) {
  const values = samples.map((sample) => sample.total)
  const points = buildCpuSparklinePoints(values)
  const hasLine = points.length > 1
  const linePath = hasLine ? buildSmoothPath(points) : ""
  const areaPath = hasLine
    ? `${linePath} L${points[points.length - 1].x},${CPU_SPARKLINE_HEIGHT - 1} L${
        points[0].x
      },${CPU_SPARKLINE_HEIGHT - 1} Z`
    : ""
  const coreSeries = getCoreSeries(samples)
  const latestPoint = points[points.length - 1]

  return (
    <span className="server-monitor-cpu-chart" aria-hidden="true">
      <svg
        viewBox={`0 0 ${CPU_SPARKLINE_WIDTH} ${CPU_SPARKLINE_HEIGHT}`}
        preserveAspectRatio="none"
        focusable="false"
      >
        <path
          className="server-monitor-cpu-threshold"
          d={`M2,${2 + (1 - 0.7) * (CPU_SPARKLINE_HEIGHT - 4)} L${
            CPU_SPARKLINE_WIDTH - 2
          },${2 + (1 - 0.7) * (CPU_SPARKLINE_HEIGHT - 4)}`}
        />
        {hasLine &&
          coreSeries.map((series, index) => {
            const corePath = buildSmoothPath(buildCpuSparklinePoints(series.values))
            if (!corePath) return null

            return (
              <path
                key={series.id}
                className="server-monitor-cpu-core-line"
                data-core-index={index}
                d={corePath}
              />
            )
          })}
        {hasLine && <path className="server-monitor-cpu-area" d={areaPath} />}
        {hasLine && <path className="server-monitor-cpu-line" d={linePath} />}
        {!hasLine && <path className="server-monitor-cpu-placeholder" d="M4,10 L64,10" />}
        {latestPoint && (
          <circle
            className="server-monitor-cpu-dot"
            cx={latestPoint.x}
            cy={latestPoint.y}
            r="1.8"
          />
        )}
      </svg>
    </span>
  )
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
  value: React.ReactNode
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
  const [cpuHistory, setCpuHistory] = useState<CpuHistorySample[]>([])
  const previousCpuTimesRef = useRef<CpuTimes | undefined>()
  const previousCpuCoreTimesRef = useRef<CpuCoreTimes[] | undefined>()
  const requestIdRef = useRef(0)
  const monitorSessionNonce = normalizeSessionNonce(sessionNonce)
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
    let usedInitialCpuSampleDelay = false
    previousCpuTimesRef.current = undefined
    previousCpuCoreTimesRef.current = undefined
    setCpuHistory([])

    const refresh = async () => {
      const requestId = requestIdRef.current + 1
      requestIdRef.current = requestId
      let nextRefreshDelayMs = refreshIntervalMs

      try {
        const snapshot = await invoke<ServerMetricsSnapshot>("get_server_metrics_snapshot", {
          tabId,
          sessionNonce: monitorSessionNonce,
          connection,
        })
        if (cancelled || requestId !== requestIdRef.current) return

        const hadCpuBaseline = previousCpuTimesRef.current !== undefined
        const cpuPercent = calculateCpuPercent(previousCpuTimesRef.current, snapshot.cpuTimes)
        const cpuCorePercents = calculateCpuCorePercents(
          previousCpuCoreTimesRef.current,
          snapshot.cpuCoreTimes
        )
        if (!usedInitialCpuSampleDelay && !hadCpuBaseline && snapshot.cpuTimes) {
          usedInitialCpuSampleDelay = true
          nextRefreshDelayMs = Math.min(INITIAL_CPU_SAMPLE_DELAY_MS, refreshIntervalMs)
        }
        previousCpuTimesRef.current = snapshot.cpuTimes
        previousCpuCoreTimesRef.current = snapshot.cpuCoreTimes
        setCpuHistory((history) => appendCpuHistorySample(history, cpuPercent, cpuCorePercents))
        setState({ status: "ready", snapshot, cpuPercent, collectedAt: Date.now() })
      } catch (error) {
        if (cancelled || requestId !== requestIdRef.current) return
        setState({
          status: "error",
          message: error instanceof Error ? error.message : String(error),
        })
      } finally {
        if (!cancelled) {
          timeoutId = window.setTimeout(refresh, nextRefreshDelayMs)
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
      void invoke("release_server_monitor_session", { tabId, sessionNonce: monitorSessionNonce })
    }
  }, [connection, connectionState, monitorSessionNonce, refreshIntervalMs, t, tabId, visible])

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
    const diskValue = snapshot.disk ? (
      <span className="server-monitor-disk-value">
        <span>
          {formatKibAsG(snapshot.disk.usedKib)}
          <span className="server-monitor-disk-join"> / </span>
          {formatKibAsG(snapshot.disk.totalKib)}
        </span>
        <PercentValue value={snapshot.disk.usedPercent} />
      </span>
    ) : (
      "--"
    )
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
          className={`server-monitor-cpu-metric ${severityClass(cpuPercent)}`}
          icon={<Cpu size={13} />}
          label="CPU"
          value={
            <span className="server-monitor-cpu-value">
              <CpuSparkline samples={cpuHistory} />
              <PercentValue value={cpuPercent} />
            </span>
          }
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
          label="DISK"
          value={diskValue}
        />
        {stale && (
          <span className="server-monitor-stale">
            {t("serverMonitor.stale", { defaultValue: "stale" })}
          </span>
        )}
      </>
    )
  }, [connectionState, copyIpAddress, cpuHistory, staleAfterMs, state, t])

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
