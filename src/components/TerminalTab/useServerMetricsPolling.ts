import { useEffect, useMemo, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import type { TFunction } from "i18next"

import type { AppConfig } from "@/lib/appConfig"
import {
  type CpuTimes,
  type CpuCoreTimes,
  type CpuHistorySample,
  type NetworkMetrics,
  type NetworkRate,
  type NetworkHistorySample,
  type ServerMetricsSnapshot,
  calculateCpuPercent,
  calculateCpuCorePercents,
  appendCpuHistorySample,
  calculateNetworkRate,
  appendNetworkHistorySample,
} from "@/components/TerminalTab/serverMetrics"
import type { TerminalTabProps } from "@/components/TerminalTab/types"
import { toErrorMessage } from "@/lib/utils"

const MIN_REFRESH_INTERVAL_MS = 1000
const MAX_REFRESH_INTERVAL_MS = 60000
const INITIAL_CPU_SAMPLE_DELAY_MS = 300

export type MonitorState =
  | { status: "loading" }
  | {
      status: "ready"
      snapshot: ServerMetricsSnapshot
      cpuPercent?: number
      networkRate?: NetworkRate
      collectedAt: number
    }
  | { status: "error"; message: string }

function normalizeSessionNonce(sessionNonce: number) {
  return sessionNonce >>> 0
}

interface ServerMetricsPollingOptions {
  active: boolean
  connection?: TerminalTabProps["connection"]
  connectionState: string
  sessionNonce: number
  tabId: string
  t: TFunction
  visible: boolean
  config: AppConfig
}

/**
 * Polls the server's metrics while the monitor is shown and the tab is in
 * view, keeping the CPU and network history and whether the last sample is
 * stale.
 */
export function useServerMetricsPolling({
  active,
  connection,
  connectionState,
  sessionNonce,
  tabId,
  t,
  visible,
  config,
}: ServerMetricsPollingOptions) {
  const [state, setState] = useState<MonitorState>({ status: "loading" })
  const [cpuHistory, setCpuHistory] = useState<CpuHistorySample[]>([])
  const [networkHistory, setNetworkHistory] = useState<NetworkHistorySample[]>([])
  const previousCpuTimesRef = useRef<CpuTimes | undefined>(undefined)
  const previousCpuCoreTimesRef = useRef<CpuCoreTimes[] | undefined>(undefined)
  const previousNetworkRef = useRef<{ metrics: NetworkMetrics; capturedAt: number } | undefined>(
    undefined
  )
  const requestIdRef = useRef(0)
  const activeRef = useRef(active)
  const resumeRef = useRef<(() => void) | null>(null)
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
  // Flag the sample as stale once no newer one arrives within staleAfterMs.
  // A timer is needed: nothing re-renders while a snapshot request hangs.
  const readyCollectedAt = state.status === "ready" ? state.collectedAt : null
  const [staleCollectedAt, setStaleCollectedAt] = useState<number | null>(null)
  useEffect(() => {
    if (readyCollectedAt === null) return
    const timeoutId = window.setTimeout(
      () => setStaleCollectedAt(readyCollectedAt),
      Math.max(0, readyCollectedAt + staleAfterMs - Date.now())
    )
    return () => window.clearTimeout(timeoutId)
  }, [readyCollectedAt, staleAfterMs])
  const stale = readyCollectedAt !== null && staleCollectedAt === readyCollectedAt

  useEffect(() => {
    activeRef.current = active
    if (active) resumeRef.current?.()
  }, [active])

  useEffect(() => {
    if (!visible || connection?.type !== "ssh" || connectionState !== "connected") {
      return
    }

    let cancelled = false
    let timeoutId: number | undefined
    let usedInitialCpuSampleDelay = false
    // Set while the tab is out of view: polling stops, but the monitor
    // connection stays open (the backend drops it after a few idle minutes),
    // so switching back does not log in again.
    let paused = false
    // The first sample after a pause only sets new baselines, so the bar
    // keeps its last values instead of showing an average over the pause.
    let resuming = false
    previousCpuTimesRef.current = undefined
    previousCpuCoreTimesRef.current = undefined
    previousNetworkRef.current = undefined

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
        if (resuming) {
          resuming = false
          previousCpuTimesRef.current = snapshot.cpuTimes
          previousCpuCoreTimesRef.current = snapshot.cpuCoreTimes
          previousNetworkRef.current = snapshot.network
            ? { metrics: snapshot.network, capturedAt }
            : undefined
          nextRefreshDelayMs = Math.min(INITIAL_CPU_SAMPLE_DELAY_MS, refreshIntervalMs)
          return
        }
        const hadCpuBaseline = previousCpuTimesRef.current !== undefined
        const cpuPercent =
          snapshot.cpuPercent ?? calculateCpuPercent(previousCpuTimesRef.current, snapshot.cpuTimes)
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
          if (activeRef.current) {
            timeoutId = window.setTimeout(refresh, nextRefreshDelayMs)
          } else {
            paused = true
          }
        }
      }
    }

    resumeRef.current = () => {
      if (!paused) return
      paused = false
      // A tab never seen yet has no baselines to keep.
      resuming = previousCpuTimesRef.current !== undefined
      void refresh()
    }

    // A tab restored in the background connects only once it is shown.
    if (activeRef.current) {
      void refresh()
    } else {
      paused = true
    }

    return () => {
      cancelled = true
      resumeRef.current = null
      if (timeoutId !== undefined) {
        window.clearTimeout(timeoutId)
      }
      void invoke("release_server_monitor_session", { tabId, sessionNonce: monitorSessionNonce })
      // Reset on stop rather than on (re)start, so the next run begins clean
      // without a synchronous setState in the effect body.
      setCpuHistory([])
      setNetworkHistory([])
      setState((current) => (current.status === "ready" ? current : { status: "loading" }))
    }
  }, [connection, connectionState, monitorSessionNonce, refreshIntervalMs, t, tabId, visible])

  return { state, cpuHistory, networkHistory, stale }
}
