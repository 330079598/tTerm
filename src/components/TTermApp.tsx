import "@/components/TTermApp.css"
import { invoke } from "@tauri-apps/api/core"
import { platform } from "@tauri-apps/plugin-os"
import type { SerializedDockview } from "dockview-react"
import { BookMarked, Minus, Plus, Settings, Square, X } from "lucide-react"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { ConnectionDialog } from "@/components/ConnectionDialog"
import { BroadcastManager } from "@/components/BroadcastManager"
import { ContextMenu } from "@/components/ContextMenu"
import { ProfilesPanel, SavedProfile } from "@/components/ProfilesPanel"
import { RenameDialog } from "@/components/RenameDialog"
import { TabBar } from "@/components/TabBar"
import { TransferManager } from "@/components/TransferManager"
import { EmptyState } from "@/components/TTermApp/EmptyState"
import {
  TabPanels,
  type TabPanelsHandle,
  type WorkspaceSplitDirection,
} from "@/components/TTermApp/TabPanels"
import { buildTabFromConnection } from "@/components/TTermApp/ttermAppUtils"
import { VaultStartupUnlockDialog } from "@/components/VaultStartupUnlockDialog"
import { formatBytes, MAX_EDIT_FILE_BYTES } from "@/components/SftpDrawer/sftpDrawerUtils"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"
import { useConfig } from "@/contexts/ConfigContext"
import { useTransferManager } from "@/contexts/TransferContext"
import { useConnectionManager } from "@/hooks/useConnectionManager"
import { useSessionPersistence } from "@/hooks/useSessionPersistence"
import { useTabContextMenu } from "@/hooks/useTabContextMenu"
import { useTabs } from "@/hooks/useTabs"
import { toast } from "@/hooks/use-toast"
import { useWindowControls } from "@/hooks/useWindowControls"
import { markSessionReady } from "@/lib/startup"
import { getAdjacentTabId, getTabIdsForCloseAction } from "@/lib/tabClosing"
import { Tab } from "@/types/tab"
import type {
  BroadcastMode,
  LiveBroadcastState,
  PtyWriteResult,
  TerminalInputRequest,
  TerminalRuntimeState,
} from "@/types/broadcast"
import type { ConnectionState } from "@/components/TerminalTab/types"

const SETTINGS_TAB_TITLE = "Settings"

function formatRemoteFileConnectionLabel(connection?: Tab["connection"]): string | undefined {
  const profileName = connection?.profileName?.trim()
  if (profileName) {
    return profileName
  }

  const host = connection?.host?.trim()
  if (!host) {
    return undefined
  }

  const userHost = connection?.username ? `${connection.username}@${host}` : host
  return connection?.port && connection.port !== 22 ? `${userHost}:${connection.port}` : userHost
}

function getRemoteFileConnectionKey(connection?: Tab["connection"]): string {
  if (connection?.profileId) {
    return `profile:${connection.profileId}`
  }

  const host = connection?.host?.trim() ?? ""
  const username = connection?.username?.trim() ?? ""
  const port = connection?.port ?? 22
  return `target:${username}@${host}:${port}`
}

export const TTermApp: React.FC = () => {
  const { t, i18n } = useTranslation()
  const [os] = useState<string>(() => platform())
  const isMacos = os === "macos"
  const isLinux = os === "linux"
  const isWindows = os === "windows"
  const [showConnectionDialog, setShowConnectionDialog] = useState(false)
  const [showProfilesPanel, setShowProfilesPanel] = useState(false)
  const [editingProfile, setEditingProfile] = useState<SavedProfile | null>(null)
  const [duplicatingProfile, setDuplicatingProfile] = useState<SavedProfile | null>(null)
  const [profilesRefreshKey, setProfilesRefreshKey] = useState(0)
  const [sessionRestored, setSessionRestored] = useState(false)
  const [workspaceLayout, setWorkspaceLayout] = useState<SerializedDockview | null>(null)
  const [startupVaultUnlockDismissed, setStartupVaultUnlockDismissed] = useState(false)
  const [broadcastMode, setBroadcastMode] = useState<BroadcastMode>("command")
  const [liveBroadcastState, setLiveBroadcastState] = useState<LiveBroadcastState>("idle")
  const [broadcastSource, setBroadcastSource] = useState<{
    tabId: string
    sessionNonce: number
  } | null>(null)
  const [broadcastTargetIds, setBroadcastTargetIds] = useState<string[]>([])
  const [terminalRuntimeStates, setTerminalRuntimeStates] = useState<
    Record<string, TerminalRuntimeState>
  >({})
  const [latestBroadcastResults, setLatestBroadcastResults] = useState<
    Record<string, PtyWriteResult>
  >({})
  const [unavailableBroadcastTargetIds, setUnavailableBroadcastTargetIds] = useState<string[]>([])
  const [isBroadcastPreparing, setIsBroadcastPreparing] = useState(false)
  const broadcastWriteQueueRef = useRef(Promise.resolve())
  const broadcastGenerationRef = useRef(0)
  const liveSourceRef = useRef<{ tabId: string; sessionNonce: number } | null>(null)
  const liveStateRef = useRef<LiveBroadcastState>(liveBroadcastState)
  const runtimeStatesRef = useRef(terminalRuntimeStates)
  const targetIdsRef = useRef(broadcastTargetIds)
  const unavailableTargetIdsRef = useRef(unavailableBroadcastTargetIds)
  const broadcastPreparingRef = useRef(false)
  const broadcastPreflightRef = useRef<{
    reject: (error: Error) => void
    resolve: (targets: Array<{ tabId: string; sessionNonce: number }>) => void
    targets: Map<string, number>
  } | null>(null)
  runtimeStatesRef.current = terminalRuntimeStates
  targetIdsRef.current = broadcastTargetIds
  unavailableTargetIdsRef.current = unavailableBroadcastTargetIds
  liveStateRef.current = liveBroadcastState
  const workspaceRef = useRef<TabPanelsHandle>(null)

  const {
    tabs,
    activeTabId,
    addTab,
    openSettingsTab,
    renameSettingsTab,
    removeTabs,
    setActiveTab,
    moveTab,
    duplicateTab,
    renameTab,
    restoreSession,
    updateTab,
  } = useTabs()
  const tabsRef = useRef(tabs)
  const activeTabIdRef = useRef(activeTabId)
  tabsRef.current = tabs
  activeTabIdRef.current = activeTabId

  const { saveSession, loadSession } = useSessionPersistence()
  const { cleanupConnection } = useConnectionManager()
  const { config, isLoaded, saveConfig, secretStatus } = useConfig()
  const { cancelTransfer, clearCompletedTransfers, removeTransfer, transfers } =
    useTransferManager()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const settingsTabTitle = t("settings.title", { defaultValue: SETTINGS_TAB_TITLE })
  const shouldPromptStartupVaultUnlock =
    isLoaded &&
    config.secret_vault_enabled &&
    (config.secret_storage_mode === "vault" ||
      config.secret_storage_mode === "auto" ||
      config.secret_storage_mode === "hybrid") &&
    (config.prompt_unlock_vault_on_startup || config.secret_storage_mode === "hybrid") &&
    !secretStatus.vaultUnlocked &&
    !startupVaultUnlockDismissed
  const startupConnectionsReady = !shouldPromptStartupVaultUnlock

  const stopLiveBroadcast = useCallback(() => {
    broadcastGenerationRef.current += 1
    liveSourceRef.current = null
    liveStateRef.current = "idle"
    setBroadcastSource(null)
    setLiveBroadcastState("idle")
    setBroadcastMode("command")
  }, [])

  const stopLiveSource = useCallback(
    (tabId: string, sessionNonce: number) => {
      const source = liveSourceRef.current
      if (source?.tabId !== tabId || source.sessionNonce !== sessionNonce) return false

      stopLiveBroadcast()
      return true
    },
    [stopLiveBroadcast]
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
      if (connectionState !== "connected") {
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
    [stopUnavailableLiveSource]
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
  }, [])

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
  }, [t])

  useEffect(() => {
    evaluateBroadcastPreflight()
  }, [evaluateBroadcastPreflight, tabs, terminalRuntimeStates])

  const ensureBroadcastTargetsConnected = useCallback(async () => {
    if (broadcastPreparingRef.current) return null

    const selected = new Set(targetIdsRef.current)
    const targets = tabsRef.current.filter(
      (tab) => selected.has(tab.id) && (tab.type === "terminal" || tab.type === "ssh")
    )
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
  }, [confirm, evaluateBroadcastPreflight, t, updateTab])

  const handleTerminalInput = useCallback(
    async ({ tabId, sessionNonce, data, kind }: TerminalInputRequest) => {
      const isLiveSource =
        liveSourceRef.current?.tabId === tabId &&
        liveSourceRef.current.sessionNonce === sessionNonce

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
    [confirm, resolveBroadcastTargets, stopLiveBroadcast, stopUnavailableLiveSource, t]
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
    [confirm, ensureBroadcastTargetsConnected, resolveBroadcastTargets, t, writeBroadcast]
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
      confirm,
      ensureBroadcastTargetsConnected,
      resolveBroadcastTargets,
      setActiveTab,
      t,
      tabs,
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
    [stopLiveBroadcast, t]
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
    [stopLiveSource, stopUnavailableLiveSource]
  )

  const toggleBroadcastTarget = useCallback((tabId: string) => {
    setBroadcastTargetIds((current) => {
      return current.includes(tabId) ? current.filter((id) => id !== tabId) : [...current, tabId]
    })
  }, [])

  const selectBroadcastTargets = useCallback(
    (tabIds: string[]) => {
      const requested = new Set(tabIds)
      const selected = tabs
        .filter((tab) => requested.has(tab.id) && (tab.type === "terminal" || tab.type === "ssh"))
        .map((tab) => tab.id)
      setBroadcastTargetIds(selected)
    },
    [tabs]
  )

  const selectVisibleBroadcastTargets = useCallback(
    (excludeTabId?: string) => {
      selectBroadcastTargets(
        (workspaceRef.current?.getVisibleTerminalTabIds() ?? []).filter((id) => id !== excludeTabId)
      )
    },
    [selectBroadcastTargets]
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
  }, [stopLiveBroadcast])

  useEffect(() => {
    const validIds = new Set(
      tabs.filter((tab) => tab.type === "terminal" || tab.type === "ssh").map((tab) => tab.id)
    )
    setBroadcastTargetIds((current) => {
      const next = current.filter((id) => validIds.has(id))
      return next.length === current.length ? current : next
    })

    if (!broadcastSource) return
    const sourceTab = tabs.find((tab) => tab.id === broadcastSource.tabId)
    const runtime = terminalRuntimeStates[broadcastSource.tabId]
    if (
      !sourceTab ||
      sourceTab.sessionNonce !== broadcastSource.sessionNonce ||
      runtime?.sessionNonce !== broadcastSource.sessionNonce ||
      runtime.connectionState !== "connected"
    ) {
      stopLiveBroadcast()
    }
  }, [broadcastSource, stopLiveBroadcast, tabs, terminalRuntimeStates])

  useEffect(() => {
    if (isLoaded) {
      i18n.changeLanguage(config.language)
    }
  }, [isLoaded, config.language, i18n])

  useEffect(() => {
    renameSettingsTab(settingsTabTitle)
  }, [renameSettingsTab, settingsTabTitle])

  useEffect(() => {
    if (!isLoaded) {
      return
    }

    let cancelled = false

    const loadAndRestoreSession = async () => {
      try {
        const savedSession = await loadSession()
        if (cancelled) {
          return
        }

        if (savedSession && savedSession.tabs.length > 0) {
          setWorkspaceLayout(savedSession.layout)
          restoreSession(savedSession.tabs, savedSession.activeTabId)
        } else {
          addTab(
            buildTabFromConnection({
              title: "Terminal",
              type: "terminal",
              isModified: false,
            })
          )
        }
      } finally {
        if (!cancelled) {
          setSessionRestored(true)
          markSessionReady()
        }
      }
    }

    void loadAndRestoreSession()

    return () => {
      cancelled = true
    }
  }, [addTab, isLoaded, loadSession, restoreSession])

  useEffect(() => {
    if (!sessionRestored) {
      return
    }

    saveSession(tabs, activeTabId, workspaceLayout)
  }, [tabs, activeTabId, saveSession, sessionRestored, workspaceLayout])

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      const target = e.target as HTMLElement
      if (!target.closest("[data-allow-context-menu]")) {
        e.preventDefault()
      }
    }
    document.addEventListener("contextmenu", handler)
    return () => document.removeEventListener("contextmenu", handler)
  }, [])

  const handleNewTab = useCallback(() => {
    setShowConnectionDialog(true)
  }, [])

  const handleConnect = useCallback(
    (connection: Omit<Tab, "id" | "isActive">) => {
      addTab(buildTabFromConnection(connection))
    },
    [addTab]
  )

  const handleEditProfile = useCallback((profile: SavedProfile) => {
    setDuplicatingProfile(null)
    setEditingProfile(profile)
    setShowConnectionDialog(true)
  }, [])

  const handleDuplicateProfile = useCallback((profile: SavedProfile) => {
    setEditingProfile(null)
    setDuplicatingProfile(profile)
    setShowConnectionDialog(true)
  }, [])

  const handleDeleteProfile = useCallback(
    (profile: SavedProfile) => {
      const profileTabs = tabs.filter(
        (tab) => tab.type === "ssh" && tab.connection?.profileId === profile.id
      )

      const source = liveSourceRef.current
      if (source && profileTabs.some((tab) => tab.id === source.tabId)) {
        stopLiveSource(source.tabId, source.sessionNonce)
      }

      for (const tab of profileTabs) {
        cleanupConnection(tab.id)
      }
      removeTabs(profileTabs.map((tab) => tab.id))
    },
    [cleanupConnection, removeTabs, stopLiveSource, tabs]
  )

  const handleEditTabProfile = useCallback(
    async (tab: Tab) => {
      const profileId = tab.connection?.profileId
      if (!profileId) {
        return
      }

      try {
        const profiles = await invoke<SavedProfile[]>("list_profiles")
        const profile = profiles.find((item) => item.id === profileId)
        if (!profile) {
          toast({
            title: t("common.error", { defaultValue: "Error" }),
            description: t("profiles.notFound", {
              defaultValue: "This connection configuration no longer exists.",
            }),
            variant: "destructive",
          })
          return
        }

        handleEditProfile(profile)
      } catch (error) {
        console.error("Failed to load connection profile:", error)
        toast({
          title: t("common.error", { defaultValue: "Error" }),
          description: t("profiles.loadFailed", {
            defaultValue: "Failed to load connection profiles.",
          }),
          variant: "destructive",
        })
      }
    },
    [handleEditProfile, t]
  )

  const handleOpenRemoteFile = useCallback(
    (entry: SftpDirectoryEntry, sourceTabId: string, connection?: Tab["connection"]) => {
      const fileSize = entry.size ?? 0
      if (fileSize > MAX_EDIT_FILE_BYTES) {
        const message = `Remote file is too large to edit in tTerm (${fileSize} bytes, limit ${MAX_EDIT_FILE_BYTES} bytes)`
        toast({
          variant: "destructive",
          title: t("remoteFileEditor.openFailed", { defaultValue: "Failed to open remote file" }),
          description: `${message} (${formatBytes(fileSize)}, limit ${formatBytes(MAX_EDIT_FILE_BYTES)})`,
        })
        return
      }

      const host = connection?.host
      const connectionLabel = formatRemoteFileConnectionLabel(connection)
      const connectionKey = getRemoteFileConnectionKey(connection)
      const existingTab = tabs.find(
        (tab) =>
          tab.type === "remote-file-editor" &&
          tab.remoteFile?.path === entry.path &&
          (tab.remoteFile.connectionKey ?? getRemoteFileConnectionKey(tab.connection)) ===
            connectionKey
      )

      if (existingTab) {
        setActiveTab(existingTab.id)
        return
      }

      addTab({
        title: entry.name,
        type: "remote-file-editor",
        isModified: false,
        connection,
        remoteFile: {
          sourceTabId,
          profileId: connection?.profileId,
          profileName: connection?.profileName,
          connectionLabel,
          connectionKey,
          host,
          path: entry.path,
          fileName: entry.name,
          size: entry.size ?? 0,
          modifiedAt: entry.modifiedAt,
        },
      })
    },
    [addTab, setActiveTab, t, tabs]
  )

  const getActiveTransfersForTabs = useCallback(
    (tabIds: string[]) => {
      const idSet = new Set(tabIds)
      return transfers.filter(
        (transfer) =>
          transfer.tabId &&
          idSet.has(transfer.tabId) &&
          (transfer.status === "pending" || transfer.status === "transferring")
      )
    },
    [transfers]
  )

  const confirmCloseTabsWithTransfers = useCallback(
    async (tabIds: string[]) => {
      const activeTransfers = getActiveTransfersForTabs(tabIds)
      if (activeTransfers.length === 0) {
        return true
      }

      return confirm({
        title: t("tabs.closeActiveTransferTitle", {
          count: activeTransfers.length,
          defaultValue: "Active transfer in progress",
        }),
        description: t("tabs.closeActiveTransferDescription", {
          count: activeTransfers.length,
          defaultValue:
            "Closing this tab will hide active SFTP transfer progress and may interrupt the transfer. Continue?",
        }),
        confirmText: t("tabs.closeActiveTransferConfirm", { defaultValue: "Close anyway" }),
        cancelText: t("common.cancel", { defaultValue: "Cancel" }),
        variant: "destructive",
      })
    },
    [confirm, getActiveTransfersForTabs, t]
  )

  const confirmCloseTabsWithUnsavedRemoteFiles = useCallback(
    async (tabIds: string[]) => {
      const idSet = new Set(tabIds)
      const unsavedTabs = tabsRef.current.filter(
        (tab) => idSet.has(tab.id) && tab.type === "remote-file-editor" && tab.isModified
      )
      if (unsavedTabs.length === 0) {
        return true
      }

      return confirm({
        title: t("remoteFileEditor.closeUnsavedTitle", {
          count: unsavedTabs.length,
          defaultValue: "Close unsaved remote file?",
        }),
        description: t("remoteFileEditor.closeUnsavedDescription", {
          count: unsavedTabs.length,
          defaultValue:
            "One or more remote file tabs have unsaved changes. Closing them will discard those changes.",
        }),
        confirmText: t("remoteFileEditor.closeUnsavedConfirm", { defaultValue: "Discard" }),
        cancelText: t("common.cancel", { defaultValue: "Cancel" }),
        variant: "destructive",
      })
    },
    [confirm, t]
  )

  const closeTabById = useCallback(
    (id: string, preferredActiveTabId?: string) => {
      const tab = tabsRef.current.find((currentTab) => currentTab.id === id)
      if (tab) {
        stopLiveSource(tab.id, tab.sessionNonce ?? 0)
      }
      if (tab?.type !== "settings" && tab?.type !== "remote-file-editor") {
        cleanupConnection(id)
      }
      if (activeTabIdRef.current === id && preferredActiveTabId) {
        workspaceRef.current?.activateTab(preferredActiveTabId)
      }
      removeTabs([id], { preferredActiveTabId })
    },
    [cleanupConnection, removeTabs, stopLiveSource]
  )

  const handleRemoveTab = useCallback(
    async (id: string) => {
      const tabIds =
        workspaceRef.current?.getGroupTabIds(id) ??
        tabsRef.current.map((currentTab) => currentTab.id)
      const preferredActiveTabId = getAdjacentTabId(tabIds, id)
      const unsavedConfirmed = await confirmCloseTabsWithUnsavedRemoteFiles([id])
      if (!unsavedConfirmed) {
        return
      }

      const confirmed = await confirmCloseTabsWithTransfers([id])
      if (!confirmed) {
        return
      }
      closeTabById(id, preferredActiveTabId)
    },
    [closeTabById, confirmCloseTabsWithTransfers, confirmCloseTabsWithUnsavedRemoteFiles]
  )

  const closeTabsWithConfirmation = useCallback(
    async (targetTabs: Tab[], closeAction: () => void) => {
      const targetTabIds = targetTabs.map((tab) => tab.id)
      const unsavedConfirmed = await confirmCloseTabsWithUnsavedRemoteFiles(targetTabIds)
      if (!unsavedConfirmed) {
        return
      }

      const confirmed = await confirmCloseTabsWithTransfers(targetTabIds)
      if (!confirmed) {
        return
      }

      for (const targetTab of targetTabs) {
        stopLiveSource(targetTab.id, targetTab.sessionNonce ?? 0)
        if (targetTab.type !== "settings" && targetTab.type !== "remote-file-editor") {
          cleanupConnection(targetTab.id)
        }
      }
      closeAction()
    },
    [
      cleanupConnection,
      confirmCloseTabsWithTransfers,
      confirmCloseTabsWithUnsavedRemoteFiles,
      stopLiveSource,
    ]
  )

  const handleCloseOtherTabs = useCallback(
    async (id: string) => {
      const tabIds = workspaceRef.current?.getGroupTabIds(id) ?? tabs.map((tab) => tab.id)
      const targetIds = getTabIdsForCloseAction(tabIds, id, "close-others")
      const targetIdSet = new Set(targetIds)
      const targetTabs = tabs.filter((tab) => targetIdSet.has(tab.id))
      await closeTabsWithConfirmation(targetTabs, () => {
        workspaceRef.current?.activateTab(id)
        removeTabs(targetIds, { preferredActiveTabId: id, activatePreferred: true })
      })
    },
    [closeTabsWithConfirmation, removeTabs, tabs]
  )

  const handleCloseTabsToLeft = useCallback(
    async (id: string) => {
      const tabIds = workspaceRef.current?.getGroupTabIds(id) ?? tabs.map((tab) => tab.id)
      const targetIds = getTabIdsForCloseAction(tabIds, id, "close-left")
      if (targetIds.length === 0) {
        return
      }

      const targetIdSet = new Set(targetIds)
      const targetTabs = tabs.filter((tab) => targetIdSet.has(tab.id))
      await closeTabsWithConfirmation(targetTabs, () => {
        if (activeTabId && targetIdSet.has(activeTabId)) {
          workspaceRef.current?.activateTab(id)
        }
        removeTabs(targetIds, { preferredActiveTabId: id })
      })
    },
    [activeTabId, closeTabsWithConfirmation, removeTabs, tabs]
  )

  const handleCloseTabsToRight = useCallback(
    async (id: string) => {
      const tabIds = workspaceRef.current?.getGroupTabIds(id) ?? tabs.map((tab) => tab.id)
      const targetIds = getTabIdsForCloseAction(tabIds, id, "close-right")
      if (targetIds.length === 0) {
        return
      }

      const targetIdSet = new Set(targetIds)
      const targetTabs = tabs.filter((tab) => targetIdSet.has(tab.id))
      await closeTabsWithConfirmation(targetTabs, () => {
        if (activeTabId && targetIdSet.has(activeTabId)) {
          workspaceRef.current?.activateTab(id)
        }
        removeTabs(targetIds, { preferredActiveTabId: id })
      })
    },
    [activeTabId, closeTabsWithConfirmation, removeTabs, tabs]
  )

  const handleSplitTab = useCallback((tabId: string, direction: WorkspaceSplitDirection) => {
    workspaceRef.current?.splitTab(tabId, direction)
  }, [])

  const handleTabDragMove = useCallback((tabId: string, clientX: number, clientY: number) => {
    workspaceRef.current?.previewTabDrop(tabId, clientX, clientY)
  }, [])

  const handleTabDrop = useCallback((tabId: string, clientX: number, clientY: number) => {
    return workspaceRef.current?.commitTabDrop(tabId, clientX, clientY) ?? false
  }, [])

  const handleTabDragCancel = useCallback(() => {
    workspaceRef.current?.clearTabDrop()
  }, [])

  const getTabContextIds = useCallback(
    (tabId: string) => workspaceRef.current?.getGroupTabIds(tabId) ?? tabs.map((tab) => tab.id),
    [tabs]
  )

  useEffect(() => {
    const handleWorkspaceShortcut = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey || event.key !== "\\") {
        return
      }
      if (!activeTabId) {
        return
      }

      event.preventDefault()
      handleSplitTab(activeTabId, event.shiftKey ? "below" : "right")
    }

    window.addEventListener("keydown", handleWorkspaceShortcut, true)
    return () => window.removeEventListener("keydown", handleWorkspaceShortcut, true)
  }, [activeTabId, handleSplitTab])

  const {
    nativeControlsReservePx,
    handleMinimizeWindow,
    handleToggleMaximizeWindow,
    handleCloseWindow,
  } = useWindowControls(isWindows)

  const {
    contextMenu,
    renameDialogState,
    handleTabContextMenu,
    handleContextMenuAction,
    handleCloseContextMenu,
    handleRenameConfirm,
    handleRenameClose,
  } = useTabContextMenu({
    handleNewTab,
    duplicateTab,
    splitTab: handleSplitTab,
    handleRemoveTab,
    handleCloseOtherTabs,
    handleCloseTabsToRight,
    handleCloseTabsToLeft,
    updateTab,
    renameTab,
    editTabProfile: handleEditTabProfile,
  })

  const handleReconnectTab = useCallback(
    (tabId: string) => {
      updateTab(tabId, (tab) => ({
        ...tab,
        sessionNonce: ((tab.sessionNonce ?? 0) + 1) >>> 0,
      }))
    },
    [updateTab]
  )

  const pauseLiveBroadcast = useCallback(() => {
    if (liveSourceRef.current) {
      broadcastGenerationRef.current += 1
      liveStateRef.current = "paused"
      setLiveBroadcastState("paused")
    }
  }, [])

  const resumeLiveBroadcast = useCallback(() => {
    if (liveSourceRef.current) {
      broadcastGenerationRef.current += 1
      liveStateRef.current = "active"
      setLiveBroadcastState("active")
    }
  }, [])

  const reconnectBroadcastTargets = useCallback(async () => {
    const selected = new Set(broadcastTargetIds)
    const openTerminalTabs = tabs.filter((tab) => tab.type === "terminal" || tab.type === "ssh")
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
  }, [broadcastTargetIds, confirm, handleReconnectTab, stopLiveBroadcast, t, tabs])

  const handlePinConnectionHeader = useCallback(
    (tabId: string) => {
      updateTab(tabId, (tab) => ({
        ...tab,
        connectionHeaderPinned: true,
      }))
    },
    [updateTab]
  )

  const handleUnpinConnectionHeader = useCallback(
    (tabId: string) => {
      updateTab(tabId, (tab) => ({
        ...tab,
        connectionHeaderPinned: false,
      }))
    },
    [updateTab]
  )

  const handleServerMonitorVisibilityChange = useCallback(
    (tabId: string, visible: boolean) => {
      const profileId = tabs.find((tab) => tab.id === tabId)?.connection?.profileId

      updateTab(tabId, (tab) => {
        if (!tab.connection || tab.connection.type !== "ssh") {
          return tab
        }

        return {
          ...tab,
          connection: {
            ...tab.connection,
            serverMonitorVisible: visible,
          },
        }
      })

      if (!profileId) {
        return
      }

      invoke("set_profile_server_monitor_visible", { id: profileId, visible }).catch((error) => {
        console.error("Failed to save server monitor preference:", error)
      })
    },
    [tabs, updateTab]
  )

  const handleSettingsClick = useCallback(() => {
    openSettingsTab(settingsTabTitle)
  }, [openSettingsTab, settingsTabTitle])

  const handleCollapsedProfileGroupKeysChange = useCallback(
    (groups: string[]) => {
      saveConfig({ collapsed_profile_group_keys: groups }).catch((error) => {
        console.error("Failed to save collapsed profile groups:", error)
      })
    },
    [saveConfig]
  )

  const renderTabContent = () => {
    if (tabs.length === 0) {
      return (
        <EmptyState
          collapsedProfileGroupKeys={config.collapsed_profile_group_keys}
          handleConnect={handleConnect}
          handleNewTab={handleNewTab}
          onCollapsedProfileGroupKeysChange={handleCollapsedProfileGroupKeysChange}
          onDuplicateProfile={handleDuplicateProfile}
          onDeleteProfile={handleDeleteProfile}
          onEditProfile={handleEditProfile}
          refreshKey={profilesRefreshKey}
        />
      )
    }

    return (
      <TabPanels
        ref={workspaceRef}
        activeTabId={activeTabId}
        broadcastSourceTabId={broadcastMode === "live" ? (broadcastSource?.tabId ?? null) : null}
        liveBroadcastState={liveBroadcastState}
        duplicateTab={duplicateTab}
        handlePinConnectionHeader={handlePinConnectionHeader}
        handleReconnectTab={handleReconnectTab}
        handleServerMonitorVisibilityChange={handleServerMonitorVisibilityChange}
        handleUnpinConnectionHeader={handleUnpinConnectionHeader}
        initialLayout={workspaceLayout}
        onActiveTabChange={setActiveTab}
        onConnectProfile={handleConnect}
        onEditProfile={handleEditProfile}
        onLayoutChange={setWorkspaceLayout}
        onOpenRemoteFile={handleOpenRemoteFile}
        onPauseBroadcast={pauseLiveBroadcast}
        onResumeBroadcast={resumeLiveBroadcast}
        onStopBroadcast={stopLiveBroadcast}
        onTabClose={handleRemoveTab}
        onTabContextMenu={handleTabContextMenu}
        onTerminalConnectionStateChange={handleTerminalConnectionStateChange}
        onTerminalInput={handleTerminalInput}
        onTerminalSessionUnavailable={handleTerminalSessionUnavailable}
        onTerminalSensitivePrompt={handleTerminalSensitivePrompt}
        profilesRefreshKey={profilesRefreshKey}
        startupConnectionsReady={startupConnectionsReady}
        startupSessionRestoreMode={config.startup_session_restore_mode}
        tabs={tabs}
        updateTab={updateTab}
      />
    )
  }

  return (
    <div
      className={`app ${isMacos ? "macos" : ""} ${isLinux ? "linux" : ""}`}
      data-tab-width-mode={config.tab_width_mode}
      style={
        {
          "--tab-standard-width": `${config.tab_standard_width}px`,
        } as React.CSSProperties
      }
    >
      <div className="title-bar">
        <div className="title-bar-left">
          <div className="tab-list-container">
            <TabBar
              tabs={tabs}
              activeTabId={activeTabId}
              onTabClick={setActiveTab}
              onTabClose={handleRemoveTab}
              onNewTab={handleNewTab}
              onTabMove={moveTab}
              onTabDragMove={handleTabDragMove}
              onTabDrop={handleTabDrop}
              onTabDragCancel={handleTabDragCancel}
              getTabContextIds={getTabContextIds}
              onContextMenu={handleTabContextMenu}
            />
            <div className="tab-add-button">
              <button
                className="tab-action"
                onClick={handleNewTab}
                aria-label={t("tabs.newTab", { defaultValue: "New tab" })}
              >
                <Plus size={16} />
              </button>
              <button
                className="tab-action"
                onClick={() => setShowProfilesPanel(true)}
                aria-label={t("profiles.title", { defaultValue: "Profiles" })}
              >
                <BookMarked size={16} />
              </button>
              <TransferManager
                transfers={transfers}
                onCancel={cancelTransfer}
                onRemove={removeTransfer}
                onClearCompleted={clearCompletedTransfers}
              />
              <BroadcastManager
                activeTabId={activeTabId}
                mode={broadcastMode}
                liveState={liveBroadcastState}
                preparing={isBroadcastPreparing}
                latestResults={latestBroadcastResults}
                unavailableTabIds={unavailableBroadcastTargetIds}
                runtimeStates={terminalRuntimeStates}
                selectedTabIds={broadcastTargetIds}
                sourceTabId={broadcastSource?.tabId ?? null}
                tabs={tabs}
                onClear={clearBroadcastTargets}
                onModeChange={setBroadcastMode}
                onReconnectTargets={reconnectBroadcastTargets}
                onSelectAll={selectAllBroadcastTargets}
                onSelectVisible={selectVisibleBroadcastTargets}
                onSendCommand={handleSendBroadcastCommand}
                onStartLive={handleStartLiveBroadcast}
                onStopLive={stopLiveBroadcast}
                onToggleTarget={toggleBroadcastTarget}
              />
            </div>
          </div>
        </div>

        <div className="drag-space" data-tauri-drag-region></div>

        <div className="title-bar-right" style={{ paddingRight: `${nativeControlsReservePx}px` }}>
          <button
            className="tab-action settings-button"
            onClick={handleSettingsClick}
            aria-label={settingsTabTitle}
          >
            <Settings size={16} />
          </button>
          {isLinux && (
            <div className="window-controls" aria-label="Window controls">
              <button
                className="window-control-button"
                onClick={handleMinimizeWindow}
                aria-label={t("window.minimize", { defaultValue: "Minimize" })}
              >
                <Minus size={16} />
              </button>
              <button
                className="window-control-button"
                onClick={handleToggleMaximizeWindow}
                aria-label={t("window.maximize", { defaultValue: "Maximize" })}
              >
                <Square size={13} />
              </button>
              <button
                className="window-control-button close"
                onClick={handleCloseWindow}
                aria-label={t("window.close", { defaultValue: "Close" })}
              >
                <X size={16} />
              </button>
            </div>
          )}
        </div>
      </div>

      <div className="content-area">{renderTabContent()}</div>

      {showConnectionDialog && (
        <ConnectionDialog
          isOpen={showConnectionDialog}
          onClose={() => {
            setShowConnectionDialog(false)
            setEditingProfile(null)
            setDuplicatingProfile(null)
            setProfilesRefreshKey((key) => key + 1)
          }}
          onConnect={handleConnect}
          editProfile={editingProfile}
          duplicateProfile={duplicatingProfile}
        />
      )}

      <Dialog open={showProfilesPanel} onOpenChange={setShowProfilesPanel}>
        <DialogContent
          showCloseButton={false}
          className="flex h-[min(720px,85vh)] flex-col overflow-hidden border-0 p-0 shadow-none sm:max-w-3xl"
        >
          <ProfilesPanel
            refreshKey={profilesRefreshKey}
            collapsedGroupKeys={config.collapsed_profile_group_keys}
            surface="panel"
            onCollapsedGroupKeysChange={handleCollapsedProfileGroupKeysChange}
            onClose={() => setShowProfilesPanel(false)}
            onCreate={() => {
              setEditingProfile(null)
              setDuplicatingProfile(null)
              setShowConnectionDialog(true)
              setShowProfilesPanel(false)
            }}
            onConnect={(connection) => {
              handleConnect(connection)
              setShowProfilesPanel(false)
            }}
            onEdit={(profile) => {
              handleEditProfile(profile)
              setShowProfilesPanel(false)
            }}
            onDuplicate={(profile) => {
              handleDuplicateProfile(profile)
              setShowProfilesPanel(false)
            }}
            onDeleteProfile={handleDeleteProfile}
          />
        </DialogContent>
      </Dialog>

      {contextMenu.visible && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          actions={contextMenu.actions}
          onAction={handleContextMenuAction}
          onClose={handleCloseContextMenu}
        />
      )}

      <ConfirmDialog />

      <RenameDialog
        isOpen={renameDialogState.isOpen}
        currentName={renameDialogState.currentName}
        onConfirm={handleRenameConfirm}
        onClose={handleRenameClose}
      />

      <VaultStartupUnlockDialog
        open={shouldPromptStartupVaultUnlock}
        onClose={() => setStartupVaultUnlockDismissed(true)}
      />
    </div>
  )
}
