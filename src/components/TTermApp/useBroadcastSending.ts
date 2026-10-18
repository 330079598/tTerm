import { invoke } from "@tauri-apps/api/core"
import { useCallback } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { BroadcastState } from "@/components/TTermApp/useBroadcastState"
import type { BroadcastRuntime } from "@/components/TTermApp/useBroadcastRuntime"
import { resolveSavedPasswordInjectionTargets } from "@/components/TTermApp/ttermAppUtils"
import { toast } from "@/hooks/use-toast"
import type { PtyWriteResult, TerminalInputRequest } from "@/types/broadcast"
import { notifySavedPasswordNotSent } from "@/components/TerminalTab/savedPasswordNotice"
import { isSessionTab } from "@/types/tab"

/**
 * Sends to the broadcast targets: commands, live input from the source
 * terminal, and saved passwords for sudo prompts.
 */
export function useBroadcastSending(
  shell: AppShell,
  state: BroadcastState,
  broadcastRuntime: BroadcastRuntime
) {
  const { t, tabsRef, confirm, tabs, setActiveTab, updateTab } = shell
  const {
    setBroadcastMode,
    setLiveBroadcastState,
    setBroadcastSource,
    setBroadcastTargetIds,
    terminalRuntimeStates,
    setLatestBroadcastResults,
    setUnavailableBroadcastTargetIds,
    setIsBroadcastPreparing,
    broadcastWriteQueueRef,
    broadcastGenerationRef,
    savedPasswordPromptTargetsRef,
    liveSourceRef,
    liveStateRef,
    runtimeStatesRef,
    targetIdsRef,
    broadcastPreparingRef,
    broadcastPreflightRef,
  } = state
  const {
    stopLiveBroadcast,
    stopUnavailableLiveSource,
    resolveBroadcastTargets,
    writeBroadcast,
    evaluateBroadcastPreflight,
  } = broadcastRuntime

  const ensureBroadcastTargetsConnected = useCallback(async () => {
    if (broadcastPreparingRef.current) return null

    const selected = new Set(targetIdsRef.current)
    const targets = tabsRef.current.filter((tab) => selected.has(tab.id) && isSessionTab(tab))
    if (targets.length !== selected.size || targets.length === 0) return null

    const reconnectTargets = targets.filter((tab) => {
      const runtime = runtimeStatesRef.current[tab.id]
      return !(
        runtime?.connectionState === "connected" && runtime.sessionNonce === (tab.sessionNonce ?? 0)
      )
    })
    if (reconnectTargets.length === 0) {
      return targets.map((tab) => ({ tabId: tab.id, sessionNonce: tab.sessionNonce ?? 0 }))
    }

    broadcastPreparingRef.current = true
    setIsBroadcastPreparing(true)
    try {
      const confirmed = await confirm({
        title: t("broadcast.autoReconnectConfirmTitle", { count: reconnectTargets.length }),
        description: t("broadcast.autoReconnectConfirmDescription", {
          count: reconnectTargets.length,
        }),
        confirmText: t("broadcast.reconnectConfirmAction"),
        variant: "destructive",
      })
      if (!confirmed) return null

      const expected = new Map<string, number>()
      const reconnectIds = new Set(reconnectTargets.map((tab) => tab.id))
      for (const tab of targets) {
        const runtime = runtimeStatesRef.current[tab.id]
        const nonce = tab.sessionNonce ?? 0
        const keepConnecting =
          reconnectIds.has(tab.id) &&
          runtime?.connectionState === "connecting" &&
          runtime.sessionNonce === nonce
        expected.set(
          tab.id,
          reconnectIds.has(tab.id) && !keepConnecting ? (nonce + 1) >>> 0 : nonce
        )
      }

      const connectedTargets = await new Promise<Array<{ tabId: string; sessionNonce: number }>>(
        (resolve, reject) => {
          broadcastPreflightRef.current = { reject, resolve, targets: expected }
          for (const tab of reconnectTargets) {
            const runtime = runtimeStatesRef.current[tab.id]
            const nonce = tab.sessionNonce ?? 0
            if (runtime?.connectionState !== "connecting" || runtime.sessionNonce !== nonce) {
              updateTab(tab.id, (current) => ({
                ...current,
                sessionNonce: ((current.sessionNonce ?? 0) + 1) >>> 0,
              }))
            }
          }
          evaluateBroadcastPreflight()
        }
      )
      return connectedTargets
    } catch (error) {
      toast({
        title: t("broadcast.reconnectFailedTitle"),
        description: String(error instanceof Error ? error.message : error),
        variant: "destructive",
      })
      return null
    } finally {
      broadcastPreflightRef.current = null
      broadcastPreparingRef.current = false
      setIsBroadcastPreparing(false)
    }
  }, [
    broadcastPreflightRef,
    broadcastPreparingRef,
    confirm,
    evaluateBroadcastPreflight,
    runtimeStatesRef,
    setIsBroadcastPreparing,
    t,
    tabsRef,
    targetIdsRef,
    updateTab,
  ])

  const handleTerminalInput = useCallback(
    async ({ tabId, sessionNonce, data, kind }: TerminalInputRequest) => {
      const isLiveSource =
        liveSourceRef.current?.tabId === tabId &&
        liveSourceRef.current.sessionNonce === sessionNonce

      if (kind === "saved-password") {
        const source = { tabId, sessionNonce, prompt: data }
        const targets = resolveSavedPasswordInjectionTargets(
          source,
          isLiveSource && liveStateRef.current === "active",
          resolveBroadcastTargets(),
          savedPasswordPromptTargetsRef.current
        )

        const written = await Promise.all(
          targets.map(async (target) => {
            const tab = tabsRef.current.find((candidate) => candidate.id === target.tabId)
            if (!tab) return false

            try {
              return await invoke<boolean>("write_saved_password_for_sudo", {
                tabId: target.tabId,
                sessionNonce: target.sessionNonce,
                profileId: tab.connection?.profileId,
                profileName: tab.connection?.profileName,
                prompt: target.prompt,
              })
            } catch (error) {
              console.error(`Failed to inject saved password into terminal ${target.tabId}`, error)
              return false
            }
          })
        )
        // targets[0] is always the source terminal.
        if (!written[0]) notifySavedPasswordNotSent(t)
        return written[0]
      }

      if (!isLiveSource) {
        await invoke("write_pty", { tabId, sessionNonce, data })
        return
      }

      if (liveStateRef.current === "paused") {
        await invoke("write_pty", { tabId, sessionNonce, data })
        return
      }

      if (kind === "paste" && /[\r\n]/.test(data)) {
        const confirmed = await confirm({
          title: t("broadcast.multilinePasteTitle"),
          description: t("broadcast.multilinePasteDescription", {
            count: resolveBroadcastTargets().length,
          }),
          confirmText: t("broadcast.sendAnyway"),
          defaultAction: "confirm",
          variant: "destructive",
        })
        if (!confirmed) return
      }

      let results: PtyWriteResult[] = []
      let liveTargetCount = 0
      const generation = broadcastGenerationRef.current
      const queuedWrite = broadcastWriteQueueRef.current.then(async () => {
        if (generation !== broadcastGenerationRef.current || !liveSourceRef.current) return
        const targets = resolveBroadcastTargets()
        const source = liveSourceRef.current
        if (!source) return
        liveTargetCount = targets.filter((target) => target.tabId !== source.tabId).length
        results = await invoke<PtyWriteResult[]>("write_pty_batch", {
          targets: targets.filter((target) => target.tabId !== source.tabId),
          guardTarget: source,
          data,
        })
      })
      broadcastWriteQueueRef.current = queuedWrite.catch(() => undefined)
      try {
        await queuedWrite
      } catch (error) {
        stopLiveBroadcast()
        toast({
          title: t("broadcast.sendFailedTitle"),
          description: String(error),
          variant: "destructive",
        })
        return
      }
      if (generation !== broadcastGenerationRef.current || !liveSourceRef.current) return
      setLatestBroadcastResults(Object.fromEntries(results.map((result) => [result.tabId, result])))
      const sourceResult = results.find((result) => result.tabId === tabId)
      if (!sourceResult || sourceResult.status !== "written") {
        stopUnavailableLiveSource(tabId, sessionNonce)
        return
      }

      const failedIds = results
        .filter((result) => result.tabId !== tabId && result.status !== "written")
        .map((result) => result.tabId)

      if (failedIds.length > 0) {
        setUnavailableBroadcastTargetIds((current) => [...new Set([...current, ...failedIds])])
        setBroadcastTargetIds((current) => current.filter((id) => !failedIds.includes(id)))
        toast({
          title: t("broadcast.partialFailureTitle"),
          description: t("broadcast.partialFailureDescription", {
            failed: failedIds.length,
            total: liveTargetCount,
          }),
          variant: "destructive",
        })
      }
    },
    [
      broadcastGenerationRef,
      broadcastWriteQueueRef,
      confirm,
      liveSourceRef,
      liveStateRef,
      resolveBroadcastTargets,
      savedPasswordPromptTargetsRef,
      setBroadcastTargetIds,
      setLatestBroadcastResults,
      setUnavailableBroadcastTargetIds,
      stopLiveBroadcast,
      stopUnavailableLiveSource,
      t,
      tabsRef,
    ]
  )

  const handleSendBroadcastCommand = useCallback(
    async (command: string) => {
      const targets = await ensureBroadcastTargetsConnected()
      if (!targets) return false

      if (/\r|\n/.test(command)) {
        const confirmed = await confirm({
          title: t("broadcast.multilineCommandTitle"),
          description: t("broadcast.multilineCommandDescription", {
            count: targets.length,
          }),
          confirmText: t("broadcast.sendAnyway"),
          defaultAction: "confirm",
          variant: "destructive",
        })
        if (!confirmed) return false
      }

      try {
        let results: PtyWriteResult[] = []
        const queuedWrite = broadcastWriteQueueRef.current.then(async () => {
          const validTargets = resolveBroadcastTargets()
          if (
            validTargets.length !== targets.length ||
            targets.some(
              (target) =>
                !validTargets.some(
                  (valid) =>
                    valid.tabId === target.tabId && valid.sessionNonce === target.sessionNonce
                )
            )
          ) {
            throw new Error(t("broadcast.targetsChanged"))
          }
          results = await writeBroadcast(`${command}\r`, targets)
        })
        broadcastWriteQueueRef.current = queuedWrite.catch(() => undefined)
        await queuedWrite
        setLatestBroadcastResults(
          Object.fromEntries(results.map((result) => [result.tabId, result]))
        )
        const written = results.filter((result) => result.status === "written").length
        toast({
          title: written > 0 ? t("broadcast.sentTitle") : t("broadcast.sendFailedTitle"),
          description: t("broadcast.sentDescription", { written, total: results.length }),
          variant: written > 0 ? "default" : "destructive",
        })
        return written > 0
      } catch (error) {
        console.error("Failed to broadcast command:", error)
        toast({
          title: t("broadcast.sendFailedTitle"),
          description: String(error),
          variant: "destructive",
        })
        return false
      }
    },
    [
      broadcastWriteQueueRef,
      confirm,
      ensureBroadcastTargetsConnected,
      resolveBroadcastTargets,
      setLatestBroadcastResults,
      t,
      writeBroadcast,
    ]
  )

  const handleStartLiveBroadcast = useCallback(
    async (sourceTabId: string) => {
      const sourceTab = tabs.find((tab) => tab.id === sourceTabId)
      const runtime = terminalRuntimeStates[sourceTabId]
      if (!sourceTab || runtime?.connectionState !== "connected") return false

      const targets = await ensureBroadcastTargetsConnected()
      if (!targets) return false

      const confirmed = await confirm({
        title: t("broadcast.liveConfirmTitle"),
        description: t("broadcast.liveConfirmDescription"),
        confirmText: t("broadcast.startLive"),
        variant: "destructive",
      })
      if (!confirmed) return false

      const currentSourceTab = tabsRef.current.find((tab) => tab.id === sourceTabId)
      const currentRuntime = runtimeStatesRef.current[sourceTabId]
      if (
        !currentSourceTab ||
        currentRuntime?.connectionState !== "connected" ||
        currentRuntime.sessionNonce !== (currentSourceTab.sessionNonce ?? 0)
      ) {
        return false
      }

      const liveTargets = resolveBroadcastTargets().filter((target) => target.tabId !== sourceTabId)
      if (
        liveTargets.length === 0 ||
        liveTargets.length !== targets.filter((target) => target.tabId !== sourceTabId).length ||
        liveTargets.some(
          (target) =>
            !targets.some(
              (expected) =>
                expected.tabId === target.tabId && expected.sessionNonce === target.sessionNonce
            )
        )
      ) {
        toast({
          title: t("broadcast.sendFailedTitle"),
          description: t("broadcast.targetsChanged"),
          variant: "destructive",
        })
        return false
      }
      setBroadcastTargetIds((current) => current.filter((tabId) => tabId !== sourceTabId))

      const source = { tabId: sourceTabId, sessionNonce: currentSourceTab.sessionNonce ?? 0 }
      broadcastGenerationRef.current += 1
      liveSourceRef.current = source
      liveStateRef.current = "active"
      setBroadcastSource(source)
      setLiveBroadcastState("active")
      setBroadcastMode("live")
      setActiveTab(sourceTabId)
      return true
    },
    [
      broadcastGenerationRef,
      confirm,
      ensureBroadcastTargetsConnected,
      liveSourceRef,
      liveStateRef,
      resolveBroadcastTargets,
      runtimeStatesRef,
      setActiveTab,
      setBroadcastMode,
      setBroadcastSource,
      setBroadcastTargetIds,
      setLiveBroadcastState,
      t,
      tabs,
      tabsRef,
      terminalRuntimeStates,
    ]
  )

  const handleTerminalSensitivePrompt = useCallback(
    (tabId: string) => {
      if (liveSourceRef.current?.tabId === tabId) {
        stopLiveBroadcast()
        toast({
          title: t("broadcast.stoppedForPromptTitle"),
          description: t("broadcast.stoppedForPromptDescription"),
        })
      }
    },
    [liveSourceRef, stopLiveBroadcast, t]
  )

  return {
    handleTerminalInput,
    handleSendBroadcastCommand,
    handleStartLiveBroadcast,
    handleTerminalSensitivePrompt,
  }
}
