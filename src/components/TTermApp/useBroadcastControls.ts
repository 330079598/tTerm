import { useCallback, useEffect, useState } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { BroadcastState } from "@/components/TTermApp/useBroadcastState"
import type { BroadcastRuntime } from "@/components/TTermApp/useBroadcastRuntime"
import { isTerminalConnectionUnavailable } from "@/components/TTermApp/ttermAppUtils"
import { isSessionTab } from "@/types/tab"

/**
 * Choosing the broadcast targets, pausing and resuming a live broadcast,
 * and reconnecting targets.
 */
export function useBroadcastControls(
  shell: AppShell,
  state: BroadcastState,
  broadcastRuntime: BroadcastRuntime,
  handleReconnectTab: (tabId: string) => void
) {
  const { t, workspaceRef, confirm, tabs } = shell
  const {
    setLiveBroadcastState,
    broadcastSource,
    broadcastTargetIds,
    setBroadcastTargetIds,
    terminalRuntimeStates,
    setLatestBroadcastResults,
    setUnavailableBroadcastTargetIds,
    broadcastGenerationRef,
    savedPasswordPromptTargetsRef,
    liveSourceRef,
    liveStateRef,
  } = state
  const { stopLiveBroadcast, stopLiveSource, stopUnavailableLiveSource } = broadcastRuntime

  const handleTerminalSavedPasswordPromptChange = useCallback(
    (tabId: string, sessionNonce: number, prompt: string | null) => {
      if (prompt !== null) {
        savedPasswordPromptTargetsRef.current.set(tabId, { sessionNonce, prompt })
      } else if (savedPasswordPromptTargetsRef.current.get(tabId)?.sessionNonce === sessionNonce) {
        savedPasswordPromptTargetsRef.current.delete(tabId)
      }
    },
    [savedPasswordPromptTargetsRef]
  )

  const handleTerminalSessionUnavailable = useCallback(
    (tabId: string, sessionNonce: number, unexpected: boolean) => {
      setUnavailableBroadcastTargetIds((current) =>
        current.includes(tabId) ? current : [...current, tabId]
      )
      if (unexpected) {
        stopUnavailableLiveSource(tabId, sessionNonce)
      } else {
        stopLiveSource(tabId, sessionNonce)
      }
    },
    [setUnavailableBroadcastTargetIds, stopLiveSource, stopUnavailableLiveSource]
  )

  const toggleBroadcastTarget = useCallback(
    (tabId: string) => {
      setBroadcastTargetIds((current) => {
        return current.includes(tabId) ? current.filter((id) => id !== tabId) : [...current, tabId]
      })
    },
    [setBroadcastTargetIds]
  )

  const selectBroadcastTargets = useCallback(
    (tabIds: string[]) => {
      const requested = new Set(tabIds)
      const selected = tabs
        .filter((tab) => requested.has(tab.id) && isSessionTab(tab))
        .map((tab) => tab.id)
      setBroadcastTargetIds(selected)
    },
    [setBroadcastTargetIds, tabs]
  )

  const selectVisibleBroadcastTargets = useCallback(
    (excludeTabId?: string) => {
      selectBroadcastTargets(
        (workspaceRef.current?.getVisibleTerminalTabIds() ?? []).filter((id) => id !== excludeTabId)
      )
    },
    [selectBroadcastTargets, workspaceRef]
  )

  const selectAllBroadcastTargets = useCallback(
    (excludeTabId?: string) => {
      selectBroadcastTargets(tabs.filter((tab) => tab.id !== excludeTabId).map((tab) => tab.id))
    },
    [selectBroadcastTargets, tabs]
  )

  const clearBroadcastTargets = useCallback(() => {
    setBroadcastTargetIds([])
    if (liveSourceRef.current) {
      stopLiveBroadcast()
    }
  }, [liveSourceRef, setBroadcastTargetIds, stopLiveBroadcast])

  // Drop broadcast targets whose tabs were closed. Done during render so no
  // commit ever sees the stale ids.
  const [broadcastTargetsTabs, setBroadcastTargetsTabs] = useState(tabs)
  if (broadcastTargetsTabs !== tabs) {
    setBroadcastTargetsTabs(tabs)
    const validIds = new Set(tabs.filter((tab) => isSessionTab(tab)).map((tab) => tab.id))
    setBroadcastTargetIds((current) => {
      const next = current.filter((id) => validIds.has(id))
      return next.length === current.length ? current : next
    })
  }

  useEffect(() => {
    if (!broadcastSource) return
    const sourceTab = tabs.find((tab) => tab.id === broadcastSource.tabId)
    const runtime = terminalRuntimeStates[broadcastSource.tabId]
    if (
      !sourceTab ||
      sourceTab.sessionNonce !== broadcastSource.sessionNonce ||
      (runtime !== undefined &&
        (runtime.sessionNonce !== broadcastSource.sessionNonce ||
          isTerminalConnectionUnavailable(runtime.connectionState)))
    ) {
      // Also resets refs the live input path reads (generation, source), so it
      // can't move into render; closed tabs / dropped sessions are external.
      stopLiveBroadcast()
    }
  }, [broadcastSource, stopLiveBroadcast, tabs, terminalRuntimeStates])

  const pauseLiveBroadcast = useCallback(() => {
    if (liveSourceRef.current) {
      broadcastGenerationRef.current += 1
      liveStateRef.current = "paused"
      setLiveBroadcastState("paused")
    }
  }, [broadcastGenerationRef, liveSourceRef, liveStateRef, setLiveBroadcastState])

  const resumeLiveBroadcast = useCallback(() => {
    if (liveSourceRef.current) {
      broadcastGenerationRef.current += 1
      liveStateRef.current = "active"
      setLiveBroadcastState("active")
    }
  }, [broadcastGenerationRef, liveSourceRef, liveStateRef, setLiveBroadcastState])

  const reconnectBroadcastTargets = useCallback(async () => {
    const selected = new Set(broadcastTargetIds)
    const openTerminalTabs = tabs.filter((tab) => isSessionTab(tab))
    const targets =
      selected.size > 0 ? openTerminalTabs.filter((tab) => selected.has(tab.id)) : openTerminalTabs
    if (targets.length === 0) return

    const confirmed = await confirm({
      title: t("broadcast.reconnectConfirmTitle", { count: targets.length }),
      description: t("broadcast.reconnectConfirmDescription", { count: targets.length }),
      confirmText: t("broadcast.reconnectConfirmAction"),
      variant: "destructive",
    })
    if (!confirmed) return

    stopLiveBroadcast()
    const targetIds = new Set(targets.map((tab) => tab.id))
    setLatestBroadcastResults((current) =>
      Object.fromEntries(Object.entries(current).filter(([tabId]) => !targetIds.has(tabId)))
    )
    setUnavailableBroadcastTargetIds((current) => current.filter((id) => !targetIds.has(id)))
    targets.forEach((tab) => handleReconnectTab(tab.id))
  }, [
    broadcastTargetIds,
    confirm,
    handleReconnectTab,
    setLatestBroadcastResults,
    setUnavailableBroadcastTargetIds,
    stopLiveBroadcast,
    t,
    tabs,
  ])

  return {
    handleTerminalSavedPasswordPromptChange,
    handleTerminalSessionUnavailable,
    toggleBroadcastTarget,
    selectVisibleBroadcastTargets,
    selectAllBroadcastTargets,
    clearBroadcastTargets,
    pauseLiveBroadcast,
    resumeLiveBroadcast,
    reconnectBroadcastTargets,
  }
}
