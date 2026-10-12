import { invoke } from "@tauri-apps/api/core"
import { useCallback, useEffect } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { BroadcastState } from "@/components/TTermApp/useBroadcastState"
import { isTerminalConnectionUnavailable } from "@/components/TTermApp/ttermAppUtils"
import { toast } from "@/hooks/use-toast"
import type { PtyWriteResult } from "@/types/broadcast"
import type { ConnectionState } from "@/components/TerminalTab/types"

/**
 * The core of broadcasting: stopping a live broadcast, following each
 * terminal's connection, and finding and writing to the targets.
 */
export function useBroadcastRuntime(shell: AppShell, state: BroadcastState) {
  const { t, tabsRef, tabs } = shell
  const {
    setBroadcastMode,
    setLiveBroadcastState,
    setBroadcastSource,
    terminalRuntimeStates,
    setTerminalRuntimeStates,
    setLatestBroadcastResults,
    setUnavailableBroadcastTargetIds,
    broadcastGenerationRef,
    liveSourceRef,
    liveStateRef,
    runtimeStatesRef,
    targetIdsRef,
    unavailableTargetIdsRef,
    broadcastPreflightRef,
  } = state

  const stopLiveBroadcast = useCallback(() => {
    broadcastGenerationRef.current += 1
    liveSourceRef.current = null
    liveStateRef.current = "idle"
    setBroadcastSource(null)
    setLiveBroadcastState("idle")
    setBroadcastMode("command")
  }, [
    broadcastGenerationRef,
    liveSourceRef,
    liveStateRef,
    setBroadcastMode,
    setBroadcastSource,
    setLiveBroadcastState,
  ])

  const stopLiveSource = useCallback(
    (tabId: string, sessionNonce: number) => {
      const source = liveSourceRef.current
      if (source?.tabId !== tabId || source.sessionNonce !== sessionNonce) return false

      stopLiveBroadcast()
      return true
    },
    [liveSourceRef, stopLiveBroadcast]
  )

  const stopUnavailableLiveSource = useCallback(
    (tabId: string, sessionNonce: number) => {
      if (!stopLiveSource(tabId, sessionNonce)) return

      toast({
        title: t("broadcast.sourceUnavailableTitle"),
        description: t("broadcast.sourceUnavailableDescription"),
      })
    },
    [stopLiveSource, t]
  )

  const handleTerminalConnectionStateChange = useCallback(
    (tabId: string, sessionNonce: number, connectionState: ConnectionState | null) => {
      if (isTerminalConnectionUnavailable(connectionState)) {
        stopUnavailableLiveSource(tabId, sessionNonce)
      }
      setTerminalRuntimeStates((current) => {
        if (connectionState === null) {
          if (current[tabId]?.sessionNonce !== sessionNonce) return current
          const next = { ...current }
          delete next[tabId]
          return next
        }

        const previous = current[tabId]
        if (
          previous?.sessionNonce === sessionNonce &&
          previous.connectionState === connectionState
        ) {
          return current
        }
        return { ...current, [tabId]: { connectionState, sessionNonce } }
      })
      if (connectionState === "connected") {
        setUnavailableBroadcastTargetIds((current) => current.filter((id) => id !== tabId))
        setLatestBroadcastResults((current) => {
          if (!current[tabId]) return current
          const next = { ...current }
          delete next[tabId]
          return next
        })
      }
    },
    [
      setLatestBroadcastResults,
      setTerminalRuntimeStates,
      setUnavailableBroadcastTargetIds,
      stopUnavailableLiveSource,
    ]
  )

  const resolveBroadcastTargets = useCallback(() => {
    const selected = new Set(targetIdsRef.current)
    const unavailable = new Set(unavailableTargetIdsRef.current)
    return tabsRef.current
      .filter(
        (tab) =>
          selected.has(tab.id) &&
          !unavailable.has(tab.id) &&
          (tab.type === "terminal" || tab.type === "ssh") &&
          runtimeStatesRef.current[tab.id]?.connectionState === "connected" &&
          runtimeStatesRef.current[tab.id]?.sessionNonce === (tab.sessionNonce ?? 0)
      )
      .map((tab) => ({ tabId: tab.id, sessionNonce: tab.sessionNonce ?? 0 }))
  }, [runtimeStatesRef, tabsRef, targetIdsRef, unavailableTargetIdsRef])

  const writeBroadcast = useCallback(
    async (data: string, targets = resolveBroadcastTargets()) => {
      if (targets.length === 0 || data.length === 0) return []
      return invoke<PtyWriteResult[]>("write_pty_batch", { targets, guardTarget: null, data })
    },
    [resolveBroadcastTargets]
  )

  const evaluateBroadcastPreflight = useCallback(() => {
    const preflight = broadcastPreflightRef.current
    if (!preflight) return

    const targets: Array<{ tabId: string; sessionNonce: number }> = []
    for (const [tabId, sessionNonce] of preflight.targets) {
      const tab = tabsRef.current.find((candidate) => candidate.id === tabId)
      if (!tab || (tab.type !== "terminal" && tab.type !== "ssh")) {
        preflight.reject(new Error(t("broadcast.reconnectTargetMissing")))
        broadcastPreflightRef.current = null
        return
      }
      const tabNonce = tab.sessionNonce ?? 0
      if (tabNonce > sessionNonce) {
        preflight.reject(new Error(t("broadcast.reconnectTargetChanged")))
        broadcastPreflightRef.current = null
        return
      }
      if (tabNonce < sessionNonce) return

      const runtime = runtimeStatesRef.current[tabId]
      if (!runtime || runtime.sessionNonce < sessionNonce) return
      if (runtime.sessionNonce > sessionNonce) {
        preflight.reject(new Error(t("broadcast.reconnectTargetChanged")))
        broadcastPreflightRef.current = null
        return
      }
      if (runtime.connectionState === "error" || runtime.connectionState === "disconnected") {
        preflight.reject(new Error(t("broadcast.reconnectTargetFailed", { name: tab.title })))
        broadcastPreflightRef.current = null
        return
      }
      if (runtime.connectionState !== "connected") return
      targets.push({ tabId, sessionNonce })
    }

    broadcastPreflightRef.current = null
    preflight.resolve(targets)
  }, [broadcastPreflightRef, runtimeStatesRef, t, tabsRef])

  useEffect(() => {
    evaluateBroadcastPreflight()
  }, [evaluateBroadcastPreflight, tabs, terminalRuntimeStates])

  return {
    stopLiveBroadcast,
    stopLiveSource,
    stopUnavailableLiveSource,
    handleTerminalConnectionStateChange,
    resolveBroadcastTargets,
    writeBroadcast,
    evaluateBroadcastPreflight,
  }
}

export type BroadcastRuntime = ReturnType<typeof useBroadcastRuntime>
