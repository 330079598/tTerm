import { useState, useEffect } from "react"
import type { SerializedDockview } from "dockview-react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { useSessionPersistence } from "@/hooks/useSessionPersistence"
import { buildTabFromConnection } from "@/components/TTermApp/ttermAppUtils"
import { nameLocalTab, refreshSavedLocalTabs } from "@/lib/localTabTitle"
import { normalizeSavedSshTab } from "@/lib/tabIcons"
import { markSessionReady } from "@/lib/startup"

interface SessionRestoreOptions {
  isLoaded: boolean
  getPreloadedSession: ReturnType<typeof useSessionPersistence>["loadSession"]
  saveSession: ReturnType<typeof useSessionPersistence>["saveSession"]
}

/**
 * Restores the tabs and layout of the last session once the config is
 * loaded, and saves them whenever they change after that.
 */
export function useSessionRestore(shell: AppShell, options: SessionRestoreOptions) {
  const { tabs, activeTabId, addTab, restoreSession } = shell
  const { isLoaded, getPreloadedSession, saveSession } = options
  const [sessionRestored, setSessionRestored] = useState(false)
  const [workspaceLayout, setWorkspaceLayout] = useState<SerializedDockview | null>(null)

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
          if (cancelled) {
            return
          }
          setWorkspaceLayout(savedSession.layout)
          restoreSession(restoredTabs, savedSession.activeTabId)
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

    saveSession(tabs, activeTabId, workspaceLayout)
  }, [tabs, activeTabId, saveSession, sessionRestored, workspaceLayout])

  return {
    workspaceLayout,
    setWorkspaceLayout,
  }
}
