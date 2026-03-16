import { useCallback, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"
import { Tab } from "@/types/tab"

interface SessionData {
  tabs: Tab[]
  activeTabId: string | null
  lastSaved: number
}

const SAVE_DEBOUNCE_MS = 1000 // 1 second debounce

export function useSessionPersistence() {
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>()

  // Save session data to file system
  const saveSession = useCallback((tabs: Tab[], activeTabId: string | null) => {
    try {
      const sessionData = {
        tabs: tabs.map((tab) => ({
          ...tab,
          isActive: tab.id === activeTabId,
        })),
        active_tab_id: activeTabId,
        last_saved: Date.now(),
      }

      invoke("save_session", { session: sessionData }).catch((error) => {
        console.error("Failed to save session:", error)
      })
    } catch (error) {
      console.error("Failed to prepare session data:", error)
    }
  }, [])

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
      return {
        tabs: session.tabs || [],
        activeTabId: session.active_tab_id,
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
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current)
      }
      timeoutRef.current = setTimeout(() => {
        saveSession(tabs, activeTabId)
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
