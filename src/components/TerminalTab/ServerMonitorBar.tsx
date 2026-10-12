import React, { useCallback, useMemo, useState } from "react"
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

import { selectMostUsedDisk } from "@/components/TerminalTab/serverMetrics"
import {
  formatKib,
  formatKibAsG,
  severityClass,
  latencySeverityClass,
  formatLatency,
  formatNetworkRate,
  formatUptime,
  normalizeDistroId,
  getDistroLabel,
  getDistroIconSrc,
} from "@/components/TerminalTab/serverMonitorFormat"
import {
  PercentValue,
  CpuSparkline,
  MetricItem,
} from "@/components/TerminalTab/ServerMonitorMetrics"
import type { TerminalTabProps } from "@/components/TerminalTab/types"
import { ServerMonitorPanel } from "@/components/TerminalTab/ServerMonitorPanel"
import { useServerMetricsPolling } from "@/components/TerminalTab/useServerMetricsPolling"
import { useConfig } from "@/contexts/ConfigContext"
import type { MonitorMetricId } from "@/lib/monitorMetrics"
import { useToast } from "@/hooks/use-toast"
interface ServerMonitorBarProps {
  /** Whether the tab is in view; polling pauses while it is not. */
  active: boolean
  connection?: TerminalTabProps["connection"]
  connectionState: string
  sessionNonce: number
  tabId: string
  t: TFunction
  visible: boolean
}

function hasClipboardPluginError(error: unknown) {
  return String(error).toLowerCase().includes("plugin:clipboard-manager")
}

export const ServerMonitorBar: React.FC<ServerMonitorBarProps> = ({
  active,
  connection,
  connectionState,
  sessionNonce,
  tabId,
  t,
  visible,
}) => {
  const { toast } = useToast()
  const { config } = useConfig()
  const { state, cpuHistory, networkHistory, stale } = useServerMetricsPolling({
    active,
    connection,
    connectionState,
    sessionNonce,
    tabId,
    t,
    visible,
    config,
  })
  const [panelExpanded, setPanelExpanded] = useState(false)
  const [panelHeight, setPanelHeight] = useState(228)

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

    const { snapshot, cpuPercent, networkRate } = state
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
  }, [config.monitor_visible_metrics, connectionState, copyIpAddress, cpuHistory, stale, state, t])

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
