import React, { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  Activity,
  ChevronUp,
  Clock3,
  Cpu,
  Download,
  Gauge,
  HardDrive,
  MemoryStick,
  Network,
  Server,
  Upload,
} from "lucide-react"
import { invoke } from "@tauri-apps/api/core"
import type { TFunction } from "i18next"

import type { TerminalTabProps } from "@/components/TerminalTab/types"
import { ServerMonitorPanel } from "@/components/TerminalTab/ServerMonitorPanel"
import { useConfig } from "@/contexts/ConfigContext"
import type { MonitorMetricId } from "@/contexts/ConfigContext"
import { useToast } from "@/hooks/use-toast"
import { toErrorMessage } from "@/lib/utils"

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

export type CpuHistorySample = {
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
  swapTotalKib: number
  swapFreeKib: number
  swapUsedKib: number
  swapUsedPercent: number
}

export type DiskMetrics = {
  filesystem: string
  filesystemType: string
  mount: string
  totalKib: number
  usedKib: number
  availableKib: number
  usedPercent: number
}

type NetworkMetrics = {
  interface: string
  receivedBytes: number
  transmittedBytes: number
  receiveErrors: number
  transmitErrors: number
  receiveDropped: number
  transmitDropped: number
}

type LoadAverageMetrics = {
  one: number
  five: number
  fifteen: number
}

export type NetworkRate = {
  receivedBytesPerSecond: number
  transmittedBytesPerSecond: number
}

export type NetworkHistorySample = NetworkRate & {
  capturedAt: number
}

export type ServerMetricsSnapshot = {
  supported: boolean
  unsupportedReason?: string
  distribution?: LinuxDistributionInfo
  kernel?: string
  cpuTimes?: CpuTimes
  cpuCoreTimes?: CpuCoreTimes[]
  memory?: MemoryMetrics
  primaryIp?: string
  disk?: DiskMetrics
  disks?: DiskMetrics[]
  network?: NetworkMetrics
  loadAverage?: LoadAverageMetrics
  uptimeSecs?: number
  networkLatencyMs?: number
}

type MonitorState =
  | { status: "loading" }
  | {
      status: "ready"
      snapshot: ServerMetricsSnapshot
      cpuPercent?: number
      networkRate?: NetworkRate
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

export function calculateNetworkRate(
  previous: { metrics: NetworkMetrics; capturedAt: number } | undefined,
  next: NetworkMetrics | undefined,
  capturedAt: number
): NetworkRate | undefined {
  if (!previous || !next || previous.metrics.interface !== next.interface) return undefined
  const elapsedSeconds = (capturedAt - previous.capturedAt) / 1000
  if (
    elapsedSeconds <= 0 ||
    next.receivedBytes < previous.metrics.receivedBytes ||
    next.transmittedBytes < previous.metrics.transmittedBytes
  ) {
    return undefined
  }

  return {
    receivedBytesPerSecond: (next.receivedBytes - previous.metrics.receivedBytes) / elapsedSeconds,
    transmittedBytesPerSecond:
      (next.transmittedBytes - previous.metrics.transmittedBytes) / elapsedSeconds,
  }
}

export function selectMostUsedDisk(disks: DiskMetrics[] | undefined): DiskMetrics | undefined {
  if (!disks?.length) return undefined
  return disks.reduce((mostUsed, disk) =>
    disk.usedPercent > mostUsed.usedPercent ? disk : mostUsed
  )
}

function appendNetworkHistorySample(
  history: NetworkHistorySample[],
  rate: NetworkRate | undefined,
  capturedAt: number
) {
  if (!rate) return history
  return [...history, { ...rate, capturedAt }].slice(-CPU_HISTORY_SAMPLE_LIMIT)
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

function latencySeverityClass(latencyMs?: number) {
  if (latencyMs === undefined) return "is-muted"
  if (latencyMs >= 300) return "is-critical"
  if (latencyMs >= 150) return "is-warning"
  return "is-normal"
}

function formatLatency(latencyMs?: number) {
  if (latencyMs === undefined) return "--"
  return latencyMs === 0 ? "<1 ms" : `${latencyMs} ms`
}

export function formatNetworkRate(value?: number) {
  if (value === undefined || !Number.isFinite(value)) return "--"
  const units = ["B", "K", "M", "G"]
  let current = Math.max(0, value)
  let index = 0
  while (current >= 1024 && index < units.length - 1) {
    current /= 1024
    index += 1
  }
  const digits = current >= 100 || index === 0 ? 0 : current >= 10 ? 1 : 2
  return `${current.toFixed(digits)}${units[index]}/s`
}

function formatUptime(seconds?: number) {
  if (seconds === undefined) return "--"
  const days = Math.floor(seconds / 86400)
  const hours = Math.floor((seconds % 86400) / 3600)
  return days > 0 ? `${days}d ${hours}h` : `${hours}h`
}

const DISTRO_RULES: Array<{ patterns: string[]; id: string; exact?: boolean }> = [
  { patterns: ["archlinux"], id: "arch" },
  { patterns: ["ubuntu"], id: "ubuntu" },
  { patterns: ["linuxmint", "linux mint"], id: "linuxmint" },
  { patterns: ["mint"], id: "linuxmint" },
  { patterns: ["debian"], id: "debian" },
  { patterns: ["gentoo"], id: "gentoo" },
  { patterns: ["manjaro"], id: "manjaro" },
  { patterns: ["kali"], id: "kali" },
  { patterns: ["pop!_os", "pop_os"], id: "popos" },
  { patterns: ["pop os", "pop!"], id: "popos" },
  { patterns: ["zorin"], id: "zorin" },
  { patterns: ["elementary"], id: "elementary" },
  { patterns: ["mx linux"], id: "mx" },
  { patterns: ["mx"], id: "mx", exact: true },
  { patterns: ["endeavouros"], id: "endeavouros" },
  { patterns: ["endeavour"], id: "endeavouros" },
  { patterns: ["cachyos"], id: "cachyos" },
  { patterns: ["nixos", "nix os"], id: "nixos" },
  { patterns: ["nobara"], id: "nobara" },
  { patterns: ["bazzite"], id: "bazzite" },
  { patterns: ["antix"], id: "antix" },
  { patterns: ["biglinux", "big linux"], id: "biglinux" },
  { patterns: ["deepin"], id: "deepin" },
  { patterns: ["garuda"], id: "garuda" },
  { patterns: ["slackware"], id: "slackware" },
  { patterns: ["void"], id: "void" },
  { patterns: ["parrot"], id: "parrot" },
  { patterns: ["rocky"], id: "rocky" },
  { patterns: ["alma"], id: "alma" },
  { patterns: ["centos"], id: "centos" },
  { patterns: ["rhel", "redhat", "red hat"], id: "rhel" },
  { patterns: ["fedora"], id: "fedora" },
  { patterns: ["suse", "opensuse"], id: "suse" },
  { patterns: ["arch"], id: "arch" },
  { patterns: ["alpine"], id: "alpine" },
  { patterns: ["amazon"], id: "amazon" },
  { patterns: ["amzn"], id: "amazon", exact: true },
  { patterns: ["oracle"], id: "oracle" },
]

function normalizeDistroId(snapshot?: ServerMetricsSnapshot) {
  const values = [
    snapshot?.distribution?.id,
    ...(snapshot?.distribution?.idLike ?? []),
    snapshot?.distribution?.name,
    snapshot?.distribution?.prettyName,
  ]
    .filter(Boolean)
    .map((value) => value!.toLowerCase())

  for (const rule of DISTRO_RULES) {
    const matches = rule.exact
      ? values.some((value) => rule.patterns.some((pattern) => value === pattern))
      : values.some((value) => rule.patterns.some((pattern) => value.includes(pattern)))
    if (matches) {
      return rule.id
    }
  }

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
  const valuesByCore = new Map<string, number[]>()
  samples.forEach((sample, sampleIndex) => {
    for (const core of sample.cores) {
      let values = valuesByCore.get(core.id)
      if (!values) {
        values = new Array<number>(samples.length).fill(0)
        valuesByCore.set(core.id, values)
      }
      values[sampleIndex] = core.percent
    }
  })

  return Array.from(valuesByCore, ([id, values]) => ({
    id,
    values,
    latest: values[values.length - 1] ?? 0,
  }))
    .sort((first, second) => second.latest - first.latest)
    .slice(0, CPU_SPARKLINE_MAX_CORE_LINES)
}

const CpuSparkline = React.memo(function CpuSparkline({ samples }: { samples: CpuHistorySample[] }) {
  const { hasLine, linePath, areaPath, coreSeries, latestPoint } = useMemo(() => {
    const values = samples.map((sample) => sample.total)
    const points = buildCpuSparklinePoints(values)
    const hasLine = points.length > 1
    const linePath = hasLine ? buildSmoothPath(points) : ""
    const areaPath = hasLine
      ? `${linePath} L${points[points.length - 1].x},${CPU_SPARKLINE_HEIGHT - 1} L${
          points[0].x
        },${CPU_SPARKLINE_HEIGHT - 1} Z`
      : ""
    const coreSeries = getCoreSeries(samples).map((series) => ({
      id: series.id,
      path: buildSmoothPath(buildCpuSparklinePoints(series.values)),
    }))
    const latestPoint = points[points.length - 1]
    return { hasLine, linePath, areaPath, coreSeries, latestPoint }
  }, [samples])

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
          coreSeries.map((series, index) =>
            series.path ? (
              <path
                key={series.id}
                className="server-monitor-cpu-core-line"
                data-core-index={index}
                d={series.path}
              />
            ) : null
          )}
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
})

const MetricItem = React.memo(function MetricItem({
  ariaLabel,
  className,
  icon,
  label,
  labelTitle,
  onClick,
  value,
}: {
  ariaLabel?: string
  className?: string
  icon: React.ReactNode
  label: string
  labelTitle?: string
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
        <span className="server-monitor-metric-label" title={labelTitle}>
          {label}
        </span>
        <span className="server-monitor-metric-value">{value}</span>
      </button>
    )
  }

  return (
    <span className={`server-monitor-metric ${className ?? ""}`.trim()}>
      {icon}
      <span className="server-monitor-metric-label" title={labelTitle}>
        {label}
      </span>
      <span className="server-monitor-metric-value">{value}</span>
    </span>
  )
})

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
  const [networkHistory, setNetworkHistory] = useState<NetworkHistorySample[]>([])
  const [panelExpanded, setPanelExpanded] = useState(false)
  const [panelHeight, setPanelHeight] = useState(228)
  const previousCpuTimesRef = useRef<CpuTimes | undefined>()
  const previousCpuCoreTimesRef = useRef<CpuCoreTimes[] | undefined>()
  const previousNetworkRef = useRef<{ metrics: NetworkMetrics; capturedAt: number } | undefined>()
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
    previousNetworkRef.current = undefined
    setCpuHistory([])
    setNetworkHistory([])

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

        const capturedAt = Date.now()
        const hadCpuBaseline = previousCpuTimesRef.current !== undefined
        const cpuPercent = calculateCpuPercent(previousCpuTimesRef.current, snapshot.cpuTimes)
        const cpuCorePercents = calculateCpuCorePercents(
          previousCpuCoreTimesRef.current,
          snapshot.cpuCoreTimes
        )
        const networkRate = calculateNetworkRate(
          previousNetworkRef.current,
          snapshot.network,
          capturedAt
        )
        if (!usedInitialCpuSampleDelay && !hadCpuBaseline && snapshot.cpuTimes) {
          usedInitialCpuSampleDelay = true
          nextRefreshDelayMs = Math.min(INITIAL_CPU_SAMPLE_DELAY_MS, refreshIntervalMs)
        }
        previousCpuTimesRef.current = snapshot.cpuTimes
        previousCpuCoreTimesRef.current = snapshot.cpuCoreTimes
        previousNetworkRef.current = snapshot.network
          ? { metrics: snapshot.network, capturedAt }
          : undefined
        setCpuHistory((history) => appendCpuHistorySample(history, cpuPercent, cpuCorePercents))
        setNetworkHistory((history) => appendNetworkHistorySample(history, networkRate, capturedAt))
        setState({ status: "ready", snapshot, cpuPercent, networkRate, collectedAt: capturedAt })
      } catch (error) {
        if (cancelled || requestId !== requestIdRef.current) return
        setState({
          status: "error",
          message: toErrorMessage(error),
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

    const { snapshot, cpuPercent, networkRate, collectedAt } = state
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
    const primaryDisk = selectMostUsedDisk(snapshot.disks) ?? snapshot.disk
    const memoryValue = snapshot.memory
      ? `${formatKib(snapshot.memory.usedKib)}/${formatKib(snapshot.memory.totalKib)}`
      : "--"
    const diskValue = primaryDisk ? (
      <span className="server-monitor-disk-value">
        <span>
          {formatKibAsG(primaryDisk.usedKib)}
          <span className="server-monitor-disk-join"> / </span>
          {formatKibAsG(primaryDisk.totalKib)}
        </span>
        <PercentValue value={primaryDisk.usedPercent} />
      </span>
    ) : (
      "--"
    )
    const ipValue = snapshot.primaryIp?.trim() || "--"
    const copyIp = snapshot.primaryIp ? () => void copyIpAddress(snapshot.primaryIp!) : undefined
    const renderMetric = (metric: MonitorMetricId) => {
      switch (metric) {
        case "cpu":
          return (
            <MetricItem
              key={metric}
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
          )
        case "memory":
          return (
            <MetricItem
              key={metric}
              className={severityClass(snapshot.memory?.usedPercent)}
              icon={<MemoryStick size={13} />}
              label="MEM"
              value={memoryValue}
            />
          )
        case "network":
          return (
            <MetricItem
              key={metric}
              className="server-monitor-network-metric"
              icon={<Network size={13} />}
              label="NET"
              value={
                <span className="server-monitor-network-value">
                  <span>
                    <Download size={11} />
                    {formatNetworkRate(networkRate?.receivedBytesPerSecond)}
                  </span>
                  <span>
                    <Upload size={11} />
                    {formatNetworkRate(networkRate?.transmittedBytesPerSecond)}
                  </span>
                </span>
              }
            />
          )
        case "ip":
          return (
            <MetricItem
              key={metric}
              ariaLabel={t("serverMonitor.copyIp", {
                defaultValue: "Copy server IP address",
              })}
              icon={<Network size={13} />}
              label="IP"
              onClick={copyIp}
              value={ipValue}
            />
          )
        case "latency":
          return (
            <MetricItem
              key={metric}
              ariaLabel={t("serverMonitor.networkLatency", {
                defaultValue: "SSH network round-trip latency",
              })}
              className={latencySeverityClass(snapshot.networkLatencyMs)}
              icon={<Gauge size={13} />}
              label="RTT"
              value={formatLatency(snapshot.networkLatencyMs)}
            />
          )
        case "disk":
          return (
            <MetricItem
              key={metric}
              className={`server-monitor-disk-metric ${severityClass(primaryDisk?.usedPercent)}`}
              icon={<HardDrive size={13} />}
              label={primaryDisk ? `DISK ${primaryDisk.mount}` : "DISK"}
              labelTitle={primaryDisk?.mount}
              value={diskValue}
            />
          )
        case "load":
          return (
            <MetricItem
              key={metric}
              icon={<Activity size={13} />}
              label="LOAD"
              value={snapshot.loadAverage ? snapshot.loadAverage.one.toFixed(2) : "--"}
            />
          )
        case "uptime":
          return (
            <MetricItem
              key={metric}
              icon={<Clock3 size={13} />}
              label="UP"
              value={formatUptime(snapshot.uptimeSecs)}
            />
          )
      }
    }

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
        {config.monitor_visible_metrics.map(renderMetric)}
        {stale && (
          <span className="server-monitor-stale">
            {t("serverMonitor.stale", { defaultValue: "stale" })}
          </span>
        )}
      </>
    )
  }, [
    config.monitor_visible_metrics,
    connectionState,
    copyIpAddress,
    cpuHistory,
    staleAfterMs,
    state,
    t,
  ])

  if (!visible || connection?.type !== "ssh") {
    return null
  }

  const readyState = state.status === "ready" && state.snapshot.supported ? state : undefined

  return (
    <>
      {panelExpanded && readyState && (
        <ServerMonitorPanel
          cpuHistory={cpuHistory}
          cpuPercent={readyState.cpuPercent}
          height={panelHeight}
          networkHistory={networkHistory}
          networkRate={readyState.networkRate}
          onClose={() => setPanelExpanded(false)}
          onHeightChange={setPanelHeight}
          primaryDisk={selectMostUsedDisk(readyState.snapshot.disks) ?? readyState.snapshot.disk}
          snapshot={readyState.snapshot}
          t={t}
        />
      )}
      <div className="server-monitor-bar">
        <div className="server-monitor-status" role="status" aria-live="polite">
          <Server size={13} className="server-monitor-leading-icon" />
          {content}
        </div>
        <button
          type="button"
          className="server-monitor-expand"
          onClick={() => setPanelExpanded((expanded) => !expanded)}
          disabled={!readyState}
          aria-expanded={panelExpanded}
          aria-label={
            panelExpanded
              ? t("serverMonitor.collapse", { defaultValue: "Collapse monitor details" })
              : t("serverMonitor.expand", { defaultValue: "Expand monitor details" })
          }
        >
          <ChevronUp size={14} aria-hidden="true" />
        </button>
      </div>
    </>
  )
}
