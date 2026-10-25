import { useState, useEffect } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { useSessionPersistence } from "@/hooks/useSessionPersistence"
import { buildTabFromConnection } from "@/components/TTermApp/ttermAppUtils"
import { nameLocalTab, refreshSavedLocalTabs } from "@/lib/localTabTitle"
import { normalizeSavedSshTab } from "@/lib/tabIcons"
import { markSessionReady } from "@/lib/startup"
import {
  isTerminalContentRestoreEnabled,
  loadSavedTerminalContent,
  setTerminalContentRestore,
} from "@/components/TerminalTab/savedTerminalContent"

interface SessionRestoreOptions {
  isLoaded: boolean
  getPreloadedSession: ReturnType<typeof useSessionPersistence>["loadSession"]
  saveSession: ReturnType<typeof useSessionPersistence>["saveSession"]
  /** Whether a restart shows each terminal tab's content from before. */
  restoreTerminalContent: boolean
}

/**
 * Restores the tabs and layout of the last session once the config is
 * loaded, and saves them whenever they change after that.
 */
export function useSessionRestore(shell: AppShell, options: SessionRestoreOptions) {
  const { tabs, groups, activeTabId, addTab, restoreSession } = shell
  const { isLoaded, getPreloadedSession, saveSession, restoreTerminalContent } = options
  const [sessionRestored, setSessionRestored] = useState(false)

  // Ahead of the restore below, which reads the saved content only when on.
  useEffect(() => {
    if (isLoaded) setTerminalContentRestore(restoreTerminalContent)
  }, [isLoaded, restoreTerminalContent])

  useEffect(() => {
    if (!isLoaded) {
      return
    }

    let cancelled = false

    const loadAndRestoreSession = async () => {
      try {
        const savedSession = await getPreloadedSession()
        if (cancelled) {
          return
        }

        if (savedSession && savedSession.tabs.length > 0) {
          const restoredTabs = (await refreshSavedLocalTabs(savedSession.tabs)).map(
            normalizeSavedSshTab
          )
          if (isTerminalContentRestoreEnabled()) {
            // Before the tabs' terminals are made, which show it as they open.
            await loadSavedTerminalContent(restoredTabs.map((tab) => tab.id))
          }
          if (cancelled) {
            return
          }
          restoreSession(restoredTabs, savedSession.activeTabId, savedSession.layout)
        } else {
          const tab = await nameLocalTab({ title: "", type: "terminal", isModified: false })
          if (cancelled) {
            return
          }
          addTab(buildTabFromConnection(tab))
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
  }, [addTab, getPreloadedSession, isLoaded, restoreSession])

  useEffect(() => {
    if (!sessionRestored) {
      return
    }

    saveSession(tabs, activeTabId, groups)
  }, [tabs, groups, activeTabId, saveSession, sessionRestored])
}
