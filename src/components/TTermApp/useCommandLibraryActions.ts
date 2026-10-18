import { invoke } from "@tauri-apps/api/core"
import { useCallback, useEffect, useMemo, useState } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { BroadcastState } from "@/components/TTermApp/useBroadcastState"
import { useKeymap } from "@/contexts/KeymapContext"
import { toast } from "@/hooks/use-toast"
import {
  addRecentCommand,
  createCommandDraft,
  loadRecentCommands,
  saveRecentCommands,
} from "@/lib/recentCommands"
import type { CommandDraft, ExecutedCommand, RecentCommand, SavedCommand } from "@/types/command"
import { isSessionTab } from "@/types/tab"

/**
 * The command library: opening it, inserting a command into the active
 * terminal, and the recent and quickly saved commands.
 */
export function useCommandLibraryActions(shell: AppShell, state: BroadcastState) {
  const { t, tabsRef, activeTabIdRef, tabs, activeTabId } = shell
  const { terminalRuntimeStates, runtimeStatesRef } = state
  const [showCommandLibrary, setShowCommandLibrary] = useState(false)
  const [commandLibrarySearchQuery, setCommandLibrarySearchQuery] = useState("")
  const [quickSaveRequest, setQuickSaveRequest] = useState<{
    draft: CommandDraft
    profile?: { id: string; name: string }
  } | null>(null)
  const [recentCommands, setRecentCommands] = useState<RecentCommand[]>(() =>
    typeof window === "undefined" ? [] : loadRecentCommands(window.localStorage)
  )
  const { registerHandler } = useKeymap()

  const activeTerminalTab = useMemo(() => {
    const tab = tabs.find((candidate) => candidate.id === activeTabId)
    return isSessionTab(tab) ? tab : null
  }, [activeTabId, tabs])
  const activeTerminalRuntime = activeTerminalTab
    ? terminalRuntimeStates[activeTerminalTab.id]
    : undefined
  const canInsertSavedCommand = Boolean(
    activeTerminalTab &&
    activeTerminalRuntime?.connectionState === "connected" &&
    activeTerminalRuntime.sessionNonce === (activeTerminalTab.sessionNonce ?? 0)
  )
  const activeCommandProfile =
    activeTerminalTab?.connection?.profileId && activeTerminalTab.connection.profileName
      ? {
          id: activeTerminalTab.connection.profileId,
          name: activeTerminalTab.connection.profileName,
        }
      : undefined

  const handleInsertCommandText = useCallback(
    async (commandText: string) => {
      const tabId = activeTabIdRef.current
      const tab = tabsRef.current.find((candidate) => candidate.id === tabId)
      if (!tab || !isSessionTab(tab)) {
        toast({
          title: t("commandLibrary.messages.insertFailed"),
          description: t("commandLibrary.selectTerminalHint"),
          variant: "destructive",
        })
        return false
      }

      const sessionNonce = tab.sessionNonce ?? 0
      const runtime = runtimeStatesRef.current[tab.id]
      if (runtime?.connectionState !== "connected" || runtime.sessionNonce !== sessionNonce) {
        toast({
          title: t("commandLibrary.messages.insertFailed"),
          description: t("commandLibrary.messages.terminalUnavailable"),
          variant: "destructive",
        })
        return false
      }

      try {
        await invoke("write_pty", { tabId: tab.id, sessionNonce, data: commandText })
        toast({ title: t("commandLibrary.messages.inserted") })
        return true
      } catch (error) {
        toast({
          title: t("commandLibrary.messages.insertFailed"),
          description: String(error),
          variant: "destructive",
        })
        return false
      }
    },
    [activeTabIdRef, runtimeStatesRef, t, tabsRef]
  )

  const handleInsertSavedCommand = useCallback(
    (command: SavedCommand) => handleInsertCommandText(command.commandText),
    [handleInsertCommandText]
  )

  const handleTerminalCommandExecuted = useCallback((command: ExecutedCommand) => {
    setRecentCommands((current) => addRecentCommand(current, command))
  }, [])

  // Saved once per change rather than per command: a pasted script reports
  // every line as a command at once.
  useEffect(() => {
    try {
      saveRecentCommands(window.localStorage, recentCommands)
    } catch (error) {
      console.error("Failed to persist recent commands:", error)
    }
  }, [recentCommands])

  const handleSaveCommand = useCallback(
    (commandText: string, profile?: { id: string; name: string }) => {
      const draft = createCommandDraft(commandText, profile)
      if (!draft.commandText) return
      setQuickSaveRequest({ draft, profile })
    },
    []
  )

  const handleOpenCommandLibrary = useCallback((query?: string) => {
    setCommandLibrarySearchQuery(query ?? "")
    setShowCommandLibrary(true)
  }, [])

  useEffect(() => {
    const unregister = registerHandler("commandLibrary.open", () => {
      handleOpenCommandLibrary()
    })
    return unregister
  }, [registerHandler, handleOpenCommandLibrary])

  return {
    showCommandLibrary,
    setShowCommandLibrary,
    commandLibrarySearchQuery,
    quickSaveRequest,
    setQuickSaveRequest,
    recentCommands,
    canInsertSavedCommand,
    activeCommandProfile,
    handleInsertCommandText,
    handleInsertSavedCommand,
    handleTerminalCommandExecuted,
    handleSaveCommand,
    handleOpenCommandLibrary,
  }
}
