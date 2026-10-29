import { useState, useEffect, useRef } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import type { useSessionPersistence } from "@/hooks/useSessionPersistence"
import { buildTabFromConnection } from "@/components/TTermApp/ttermAppUtils"
import { useLatestRef } from "@/hooks/useLatestRef"
import {
  defaultShellChoice,
  localShellConnection,
  withCurrentLocalShell,
  type ShellConfig,
} from "@/lib/localShells"
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
  /** The shells local tabs start, read when the session is restored. */
  shellConfig: ShellConfig
  /** Opens the directories tTerm was asked to open; resolves to whether it opened any. */
  openLaunchDirs: () => Promise<boolean>
}

/**
 * Restores the tabs and layout of the last session once the config is
 * loaded, and saves them whenever they change after that.
 */
export function useSessionRestore(shell: AppShell, options: SessionRestoreOptions) {
  const { tabs, groups, activeTabId, addTab, restoreSession } = shell
  const { isLoaded, getPreloadedSession, saveSession, restoreTerminalContent, openLaunchDirs } =
    options
  const [sessionRestored, setSessionRestored] = useState(false)
  // Read once, when the restore runs; a change to the shells must not restore again.
  const shellConfigRef = useRef(options.shellConfig)
  useLatestRef(shellConfigRef, options.shellConfig)

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

        const shellConfig = shellConfigRef.current
        if (savedSession && savedSession.tabs.length > 0) {
          // A tab started by one of the user's shells follows that shell's edits.
          const savedTabs = savedSession.tabs.map((tab) =>
            tab.type === "terminal"
              ? {
                  ...tab,
                  connection: withCurrentLocalShell(tab.connection, shellConfig.local_shells),
                }
              : tab
          )
          const restoredTabs = (await refreshSavedLocalTabs(savedTabs)).map(normalizeSavedSshTab)
          if (isTerminalContentRestoreEnabled()) {
            // Before the tabs' terminals are made, which show it as they open.
            await loadSavedTerminalContent(restoredTabs.map((tab) => tab.id))
          }
          if (cancelled) {
            return
          }
          restoreSession(restoredTabs, savedSession.activeTabId, savedSession.layout)
          await openLaunchDirs()
        } else if (!(await openLaunchDirs())) {
          // A first start in a directory needs no second tab in the home directory.
          const tab = await nameLocalTab({
            title: "",
            type: "terminal",
            isModified: false,
            connection: localShellConnection(defaultShellChoice(shellConfig)),
          })
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
  }, [addTab, getPreloadedSession, isLoaded, openLaunchDirs, restoreSession])

  useEffect(() => {
    if (!sessionRestored) {
      return
    }

    saveSession(tabs, activeTabId, groups)
  }, [tabs, groups, activeTabId, saveSession, sessionRestored])
}
