import { useCallback, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"
import { Tab } from "@/types/tab"

interface SessionData {
  tabs: Tab[]
  activeTabId: string | null
  lastSaved: number
}

const SAVE_DEBOUNCE_MS = 1000 // 1 second debounce

function getPersistableTabs(tabs: Tab[]): Tab[] {
  return tabs.filter((tab) => tab.type !== "settings")
}

function getPersistedActiveTabId(
  tabs: Tab[],
  activeTabId: string | null,
  fallbackActiveTabId?: string | null
): string | null {
  if (activeTabId && tabs.some((tab) => tab.id === activeTabId)) {
    return activeTabId
  }

  if (fallbackActiveTabId && tabs.some((tab) => tab.id === fallbackActiveTabId)) {
    return fallbackActiveTabId
  }

  return tabs[0]?.id ?? null
}

// Strip transient secrets before session data is written or restored.
function sanitizeTabForPersistence(tab: Tab, activeTabId: string | null): Tab {
  const connection = tab.connection

  return {
    ...tab,
    isActive: tab.id === activeTabId,
    hasConnected: tab.id === activeTabId,
    sessionNonce: 0,
    connectionHeaderPinned: tab.connectionHeaderPinned ?? true,
    connection: connection
      ? {
          ...connection,
          password: undefined,
          privateKeyPassphrase: undefined,
          jumpHost: connection.jumpHost
            ? { ...connection.jumpHost, password: undefined }
            : undefined,
          jumpHosts: connection.jumpHosts?.map((jump) => ({ ...jump, password: undefined })),
        }
      : undefined,
  }
}

export function useSessionPersistence() {
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>()
  const lastActiveContentTabIdRef = useRef<string | null>(null)

  // Save session data to file system
  const saveSession = useCallback(
    (tabs: Tab[], activeTabId: string | null, fallbackActiveTabId?: string | null) => {
      try {
        const persistableTabs = getPersistableTabs(tabs)
        const persistedActiveTabId = getPersistedActiveTabId(
          persistableTabs,
          activeTabId,
          fallbackActiveTabId
        )
        const sessionData = {
          // Persist the reconnect metadata, but never the raw credentials.
          tabs: persistableTabs.map((tab) => sanitizeTabForPersistence(tab, persistedActiveTabId)),
          active_tab_id: persistedActiveTabId,
          last_saved: Date.now(),
        }

        invoke("save_session", { session: sessionData }).catch((error) => {
          console.error("Failed to save session:", error)
        })
      } catch (error) {
        console.error("Failed to prepare session data:", error)
      }
    },
    []
  )

  // Clear session data
  const clearSession = useCallback(async () => {
    try {
      await invoke("clear_session")
    } catch (error) {
      console.error("Failed to clear session:", error)
    }
  }, [])

  // Load session data from file system
  const loadSession = useCallback(async (): Promise<SessionData | null> => {
    try {
      const session = await invoke<{
        tabs: Tab[]
        active_tab_id: string | null
        last_saved: number
      }>("load_session")

      // Convert snake_case to camelCase
      const persistableTabs = getPersistableTabs(session.tabs || [])
      const persistedActiveTabId = getPersistedActiveTabId(persistableTabs, session.active_tab_id)

      return {
        tabs: persistableTabs.map((tab) => sanitizeTabForPersistence(tab, persistedActiveTabId)),
        activeTabId: persistedActiveTabId,
        lastSaved: session.last_saved,
      }
    } catch (error) {
      console.error("Failed to load session:", error)
      return null
    }
  }, [])

  // Debounced save
  const debouncedSave = useCallback(
    (tabs: Tab[], activeTabId: string | null) => {
      const activeContentTab = tabs.find((tab) => tab.id === activeTabId && tab.type !== "settings")
      if (activeContentTab) {
        lastActiveContentTabIdRef.current = activeContentTab.id
      }

      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current)
      }
      timeoutRef.current = setTimeout(() => {
        saveSession(tabs, activeTabId, lastActiveContentTabIdRef.current)
      }, SAVE_DEBOUNCE_MS)
    },
    [saveSession]
  )

  return {
    saveSession: debouncedSave,
    loadSession,
    clearSession,
  }
}
