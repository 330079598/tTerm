import React, { useCallback, useState } from "react"
import {
  Activity,
  ChevronDown,
  Cpu,
  Download,
  Database,
  HardDrive,
  MemoryStick,
  Network,
  Upload,
} from "lucide-react"
import type { TFunction } from "i18next"

import type {
  CpuHistorySample,
  DiskMetrics,
  NetworkHistorySample,
  NetworkRate,
  ServerMetricsSnapshot,
} from "@/components/TerminalTab/ServerMonitorBar"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"

const MIN_PANEL_HEIGHT = 160
const MAX_PANEL_HEIGHT = 420

function formatBytes(value?: number, perSecond = false) {
  if (value === undefined || !Number.isFinite(value)) return "--"
  const units = ["B", "KB", "MB", "GB", "TB"]
  let current = Math.max(0, value)
  let unitIndex = 0
  while (current >= 1024 && unitIndex < units.length - 1) {
    current /= 1024
    unitIndex += 1
  }
  const digits = current >= 100 || unitIndex === 0 ? 0 : current >= 10 ? 1 : 2
  return `${current.toFixed(digits)} ${units[unitIndex]}${perSecond ? "/s" : ""}`
}

function formatKib(value?: number) {
  return value === undefined ? "--" : formatBytes(value * 1024)
}

function linePath(values: number[], width = 100, height = 52, fixedMax?: number) {
  if (values.length < 2) return ""
  const maximum = fixedMax ?? Math.max(...values, 1)
  return values
    .map((value, index) => {
      const x = (index / (values.length - 1)) * width
      const y = height - (Math.min(Math.max(value, 0), maximum) / maximum) * (height - 4) - 2
      return `${index === 0 ? "M" : "L"}${x.toFixed(2)},${y.toFixed(2)}`
    })
    .join(" ")
}

function TrendChart({
  ariaLabel,
  emptyLabel,
  primary,
  secondary,
  fixedMax,
}: {
  ariaLabel: string
  emptyLabel: string
  primary: number[]
  secondary?: number[]
  fixedMax?: number
}) {
  const resolvedMax = fixedMax ?? Math.max(...primary, ...(secondary ?? []), 1)
  const primaryPath = linePath(primary, 100, 52, resolvedMax)
  const secondaryPath = secondary ? linePath(secondary, 100, 52, resolvedMax) : ""
  if (!primaryPath && !secondaryPath) {
    return <div className="server-monitor-panel-empty">{emptyLabel}</div>
  }

  return (
    <svg
      className="server-monitor-panel-chart"
      viewBox="0 0 100 52"
      preserveAspectRatio="none"
      role="img"
      aria-label={ariaLabel}
    >
      <path className="server-monitor-panel-grid" d="M0 13H100 M0 26H100 M0 39H100" />
      {secondaryPath && (
        <path className="server-monitor-panel-line is-secondary" d={secondaryPath} />
      )}
      {primaryPath && <path className="server-monitor-panel-line" d={primaryPath} />}
    </svg>
  )
}

function Stat({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  return (
    <div className="server-monitor-panel-stat">
      <span className="server-monitor-panel-stat-icon">{icon}</span>
      <span className="server-monitor-panel-stat-label">{label}</span>
      <strong>{value}</strong>
    </div>
  )
}

function UsageBar({ value }: { value?: number }) {
  const percent = Math.min(Math.max(value ?? 0, 0), 100)
  const severity = percent >= 90 ? "is-critical" : percent >= 80 ? "is-warning" : "is-normal"
  return (
    <div className={`server-monitor-usage-track ${severity}`} aria-hidden="true">
      <span style={{ width: `${percent}%` }} />
    </div>
  )
}

interface ServerMonitorPanelProps {
  cpuHistory: CpuHistorySample[]
  cpuPercent?: number
  height: number
  networkHistory: NetworkHistorySample[]
  networkRate?: NetworkRate
  onClose: () => void
  onHeightChange: (height: number) => void
  primaryDisk?: DiskMetrics
  snapshot: ServerMetricsSnapshot
  t: TFunction
}

export function ServerMonitorPanel({
  cpuHistory,
  cpuPercent,
  height,
  networkHistory,
  networkRate,
  onClose,
  onHeightChange,
  primaryDisk,
  snapshot,
  t,
}: ServerMonitorPanelProps) {
  const [activeTab, setActiveTab] = useState("overview")

  const resizeFromPointer = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      event.preventDefault()
      const startY = event.clientY
      const startHeight = height
      const onPointerMove = (moveEvent: PointerEvent) => {
        onHeightChange(
          Math.min(
            Math.max(startHeight + startY - moveEvent.clientY, MIN_PANEL_HEIGHT),
            MAX_PANEL_HEIGHT
          )
        )
      }
      const stopResize = () => {
        window.removeEventListener("pointermove", onPointerMove)
        window.removeEventListener("pointerup", stopResize)
      }
      window.addEventListener("pointermove", onPointerMove)
      window.addEventListener("pointerup", stopResize, { once: true })
    },
    [height, onHeightChange]
  )

  const memoryPercent = snapshot.memory?.usedPercent
  const swapConfigured = (snapshot.memory?.swapTotalKib ?? 0) > 0
  const swapPercent = swapConfigured ? snapshot.memory?.swapUsedPercent : undefined
  const diskPercent = primaryDisk?.usedPercent
  const disks = snapshot.disks?.length ? snapshot.disks : snapshot.disk ? [snapshot.disk] : []
  const receiveValues = networkHistory.map((sample) => sample.receivedBytesPerSecond)
  const transmitValues = networkHistory.map((sample) => sample.transmittedBytesPerSecond)
  const network = snapshot.network
  const networkErrors = (network?.receiveErrors ?? 0) + (network?.transmitErrors ?? 0)
  const networkDropped = (network?.receiveDropped ?? 0) + (network?.transmitDropped ?? 0)

  return (
    <section
      className="server-monitor-panel"
      style={{ "--server-monitor-panel-height": `${height}px` } as React.CSSProperties}
      aria-label={t("settings.monitor", { defaultValue: "Monitor" })}
    >
      <div
        className="server-monitor-panel-resize"
        role="separator"
        tabIndex={0}
        aria-label={t("serverMonitor.resize", { defaultValue: "Resize monitor details" })}
        aria-orientation="horizontal"
        aria-valuemin={MIN_PANEL_HEIGHT}
        aria-valuemax={MAX_PANEL_HEIGHT}
        aria-valuenow={height}
        onPointerDown={resizeFromPointer}
        onKeyDown={(event) => {
          if (event.key === "ArrowUp" || event.key === "ArrowDown") {
            event.preventDefault()
            const delta = event.key === "ArrowUp" ? 20 : -20
            onHeightChange(Math.min(Math.max(height + delta, MIN_PANEL_HEIGHT), MAX_PANEL_HEIGHT))
          }
        }}
      />
      <Tabs value={activeTab} onValueChange={setActiveTab} className="server-monitor-panel-tabs">
        <header className="server-monitor-panel-header">
          <TabsList className="server-monitor-panel-tab-list">
            {(["overview", "cpu", "memory", "network", "disk"] as const).map((tab) => (
              <TabsTrigger key={tab} value={tab} className="server-monitor-panel-tab">
                {t(`serverMonitor.tabs.${tab}`)}
              </TabsTrigger>
            ))}
          </TabsList>
          <button
            type="button"
            className="server-monitor-panel-close"
            onClick={onClose}
            aria-label={t("serverMonitor.collapse", { defaultValue: "Collapse monitor details" })}
          >
            <ChevronDown size={15} aria-hidden="true" />
          </button>
        </header>

        <TabsContent value="overview" className="server-monitor-panel-content">
          <div className="server-monitor-panel-overview">
            <Stat
              icon={<Cpu size={15} />}
              label="CPU"
              value={cpuPercent === undefined ? "--" : `${Math.round(cpuPercent)}%`}
            />
            <Stat
              icon={<MemoryStick size={15} />}
              label={t("serverMonitor.tabs.memory")}
              value={memoryPercent === undefined ? "--" : `${Math.round(memoryPercent)}%`}
            />
            <Stat
              icon={<Download size={15} />}
              label={t("serverMonitor.download")}
              value={formatBytes(networkRate?.receivedBytesPerSecond, true)}
            />
            <Stat
              icon={<Upload size={15} />}
              label={t("serverMonitor.upload")}
              value={formatBytes(networkRate?.transmittedBytesPerSecond, true)}
            />
            <Stat
              icon={<Activity size={15} />}
              label={t("serverMonitor.loadAverage")}
              value={snapshot.loadAverage ? snapshot.loadAverage.one.toFixed(2) : "--"}
            />
            <Stat
              icon={<HardDrive size={15} />}
              label={t("serverMonitor.tabs.disk")}
              value={diskPercent === undefined ? "--" : `${Math.round(diskPercent)}%`}
            />
          </div>
        </TabsContent>

        <TabsContent value="cpu" className="server-monitor-panel-content is-chart-view">
          <div className="server-monitor-panel-kpi">
            <span>CPU</span>
            <strong>{cpuPercent === undefined ? "--" : `${Math.round(cpuPercent)}%`}</strong>
          </div>
          <TrendChart
            ariaLabel={t("serverMonitor.history")}
            emptyLabel={t("serverMonitor.noTrend")}
            primary={cpuHistory.map((sample) => sample.total)}
            fixedMax={100}
          />
        </TabsContent>

        <TabsContent value="memory" className="server-monitor-panel-content">
          <div className="server-monitor-memory-resources">
            <div className="server-monitor-panel-resource is-memory-section">
              <div className="server-monitor-panel-resource-head">
                <span>
                  <MemoryStick size={16} /> {t("serverMonitor.physicalMemory")}
                </span>
                <strong>
                  {memoryPercent === undefined ? "--" : `${Math.round(memoryPercent)}%`}
                </strong>
              </div>
              <UsageBar value={memoryPercent} />
              <div className="server-monitor-panel-resource-values">
                <span>
                  {t("serverMonitor.used")}: {formatKib(snapshot.memory?.usedKib)}
                </span>
                <span>
                  {t("serverMonitor.available")}: {formatKib(snapshot.memory?.availableKib)}
                </span>
                <span>
                  {t("serverMonitor.total")}: {formatKib(snapshot.memory?.totalKib)}
                </span>
              </div>
            </div>

            <div className="server-monitor-panel-resource is-memory-section">
              <div className="server-monitor-panel-resource-head">
                <span>
                  <Database size={16} /> {t("serverMonitor.swap")}
                </span>
                <strong>
                  {swapConfigured
                    ? `${Math.round(swapPercent ?? 0)}%`
                    : t("serverMonitor.notConfigured")}
                </strong>
              </div>
              <UsageBar value={swapPercent} />
              <div className="server-monitor-panel-resource-values">
                <span>
                  {t("serverMonitor.used")}: {formatKib(snapshot.memory?.swapUsedKib)}
                </span>
                <span>
                  {t("serverMonitor.available")}: {formatKib(snapshot.memory?.swapFreeKib)}
                </span>
                <span>
                  {t("serverMonitor.total")}: {formatKib(snapshot.memory?.swapTotalKib)}
                </span>
              </div>
            </div>
          </div>
        </TabsContent>

        <TabsContent value="network" className="server-monitor-panel-content is-network-view">
          <div className="server-monitor-network-summary">
            <Stat
              icon={<Download size={15} />}
              label={t("serverMonitor.download")}
              value={formatBytes(networkRate?.receivedBytesPerSecond, true)}
            />
            <Stat
              icon={<Upload size={15} />}
              label={t("serverMonitor.upload")}
              value={formatBytes(networkRate?.transmittedBytesPerSecond, true)}
            />
            <Stat
              icon={<Network size={15} />}
              label={t("serverMonitor.interface")}
              value={network?.interface ?? "--"}
            />
          </div>
          <div className="server-monitor-network-chart-wrap">
            <TrendChart
              ariaLabel={t("serverMonitor.history")}
              emptyLabel={t("serverMonitor.noTrend")}
              primary={receiveValues}
              secondary={transmitValues}
            />
            <div className="server-monitor-chart-legend">
              <span>
                <i className="is-primary" />
                {t("serverMonitor.download")}
              </span>
              <span>
                <i className="is-secondary" />
                {t("serverMonitor.upload")}
              </span>
            </div>
          </div>
          <div className="server-monitor-network-totals">
            <span>
              {t("serverMonitor.received")}: <strong>{formatBytes(network?.receivedBytes)}</strong>
            </span>
            <span>
              {t("serverMonitor.transmitted")}:{" "}
              <strong>{formatBytes(network?.transmittedBytes)}</strong>
            </span>
            <span>
              {t("serverMonitor.errors")}: <strong>{networkErrors}</strong>
            </span>
            <span>
              {t("serverMonitor.dropped")}: <strong>{networkDropped}</strong>
            </span>
          </div>
        </TabsContent>

        <TabsContent value="disk" className="server-monitor-panel-content">
          {disks.length ? (
            <div className="server-monitor-disk-list">
              {disks.map((disk) => {
                const isPrimary = disk.mount === primaryDisk?.mount
                return (
                  <div
                    key={`${disk.filesystem}:${disk.mount}`}
                    className="server-monitor-panel-resource is-disk-section"
                  >
                    <div className="server-monitor-panel-resource-head">
                      <span className="min-w-0">
                        <HardDrive size={16} className="shrink-0" />
                        <code className="truncate">{disk.mount}</code>
                        {isPrimary && (
                          <small>
                            {t("serverMonitor.primaryDisk", { defaultValue: "Status bar" })}
                          </small>
                        )}
                      </span>
                      <strong>{Math.round(disk.usedPercent)}%</strong>
                    </div>
                    <UsageBar value={disk.usedPercent} />
                    <div className="server-monitor-panel-resource-values">
                      <span>
                        {t("serverMonitor.used")}: {formatKib(disk.usedKib)}
                      </span>
                      <span>
                        {t("serverMonitor.available")}: {formatKib(disk.availableKib)}
                      </span>
                      <span>
                        {t("serverMonitor.total")}: {formatKib(disk.totalKib)}
                      </span>
                      {disk.filesystem && (
                        <span title={disk.filesystem}>
                          {t("serverMonitor.filesystem", { defaultValue: "Filesystem" })}:{" "}
                          {disk.filesystem}
                          {disk.filesystemType ? ` (${disk.filesystemType})` : ""}
                        </span>
                      )}
                    </div>
                  </div>
                )
              })}
            </div>
          ) : (
            <div className="server-monitor-panel-empty">
              {t("serverMonitor.noDisks", { defaultValue: "No physical filesystems found" })}
            </div>
          )}
        </TabsContent>
      </Tabs>
    </section>
  )
}
