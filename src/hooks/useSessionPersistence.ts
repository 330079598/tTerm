import { useCallback, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"
import { invokeSafe, reportError } from "@/lib/errors"
import { Tab } from "@/types/tab"

interface SessionData {
  tabs: Tab[]
  activeTabId: string | null
  lastSaved: number
}

const SAVE_DEBOUNCE_MS = 1000 // 1 second debounce
const SAVE_ERROR_TOAST_COOLDOWN_MS = 30_000

function getPersistableTabs(tabs: Tab[]): Tab[] {
  return tabs.filter((tab) => tab.type !== "settings" && tab.type !== "remote-file-editor")
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
          jumpHosts: connection.jumpHosts?.map((jump) => ({ ...jump, password: undefined })),
        }
      : undefined,
  }
}

export function useSessionPersistence() {
  const { t } = useTranslation()
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>()
  const lastActiveContentTabIdRef = useRef<string | null>(null)
  const lastSaveErrorToastAtRef = useRef(0)

  const reportSessionSaveError = useCallback(
    (error: unknown, context: string) => {
      const now = Date.now()
      const showToast = now - lastSaveErrorToastAtRef.current >= SAVE_ERROR_TOAST_COOLDOWN_MS
      if (showToast) {
        lastSaveErrorToastAtRef.current = now
      }

      reportError(error, {
        context,
        title: t("errors.sessionSaveFailed"),
        userMessage: t("errors.sessionSaveFailedDesc"),
        silent: !showToast,
      })
    },
    [t]
  )

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

        void invokeSafe<void>(
          "save_session",
          { session: sessionData },
          {
            context: "save_session",
            title: t("errors.sessionSaveFailed"),
            userMessage: t("errors.sessionSaveFailedDesc"),
            silent: () => {
              const now = Date.now()
              const showToast =
                now - lastSaveErrorToastAtRef.current >= SAVE_ERROR_TOAST_COOLDOWN_MS
              if (showToast) {
                lastSaveErrorToastAtRef.current = now
              }
              return !showToast
            },
          }
        )
      } catch (error) {
        reportSessionSaveError(error, "prepare_session")
      }
    },
    [reportSessionSaveError, t]
  )

  // Clear session data
  const clearSession = useCallback(async () => {
    try {
      await invoke("clear_session")
    } catch (error) {
      reportError(error, {
        context: "clear_session",
        title: t("errors.operationFailed"),
      })
    }
  }, [t])

  // Load session data from file system
  const loadSession = useCallback(async (): Promise<SessionData | null> => {
    const result = await invokeSafe<{
      tabs: Tab[]
      active_tab_id: string | null
      last_saved: number
    }>("load_session", undefined, {
      context: "load_session",
      title: t("errors.sessionLoadFailed"),
      userMessage: t("errors.sessionLoadFailedDesc"),
    })

    if (!result.ok) {
      return null
    }

    const session = result.value

    // Convert snake_case to camelCase
    const persistableTabs = getPersistableTabs(session.tabs || [])
    const persistedActiveTabId = getPersistedActiveTabId(persistableTabs, session.active_tab_id)

    return {
      tabs: persistableTabs.map((tab) => sanitizeTabForPersistence(tab, persistedActiveTabId)),
      activeTabId: persistedActiveTabId,
      lastSaved: session.last_saved,
    }
  }, [t])

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
