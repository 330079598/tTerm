import { useCallback, useEffect, useRef } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useTranslation } from "react-i18next"
import type { TFunction } from "i18next"

import { ToastAction } from "@/components/ui/toast"
import { useConfig } from "@/contexts/ConfigContext"
import { toast } from "@/hooks/use-toast"
import { markTabAttention } from "@/lib/tabAttention"
import {
  decideAttention,
  formatCommandDuration,
  type TerminalAttentionEvent,
} from "@/lib/terminalNotifications"
import type { Tab } from "@/types/tab"

/** What `show_notification` takes (src-tauri/src/notification). */
export interface SystemNotification {
  title: string
  subtitle?: string
  body: string
  tabId?: string
  sound: boolean
}

export function showSystemNotification(notification: SystemNotification) {
  return invoke("show_notification", { request: notification })
}

/** Announcements of one tab closer together than this are dropped. */
const MIN_INTERVAL_MS = 2000
/** A program ringing over and over announces it once in this long. */
const BELL_MIN_INTERVAL_MS = 30_000
const MAX_COMMAND_LENGTH = 80

function describeEvent(
  event: TerminalAttentionEvent,
  tabTitle: string,
  t: TFunction
): { title: string; subtitle?: string; body: string } {
  if (event.kind === "request") {
    // A program's own title leads; the tab says where it came from.
    return event.title
      ? { title: event.title, subtitle: tabTitle, body: event.body }
      : { title: tabTitle, body: event.body }
  }
  if (event.kind === "bell") {
    return { title: tabTitle, body: t("notifications.bell") }
  }

  const command =
    event.command && event.command.length > MAX_COMMAND_LENGTH
      ? `${event.command.slice(0, MAX_COMMAND_LENGTH - 1)}…`
      : event.command
  const duration = formatCommandDuration(event.durationMs)
  const options = { command: command ?? t("notifications.aCommand"), duration }
  const body =
    event.exitCode === undefined
      ? t("notifications.commandFinished", options)
      : event.exitCode === 0
        ? t("notifications.commandSucceeded", options)
        : t("notifications.commandFailed", { ...options, code: event.exitCode })
  return { title: tabTitle, body }
}

interface UseTerminalNotificationsOptions {
  tabs: Tab[]
  activateTab: (tabId: string) => void
  getVisibleTabIds: () => string[]
}

/**
 * Announces what terminals want attention for: marks tabs out of sight,
 * toasts while the window is in front, system notifications while it is
 * not. Clicking a notification shows its tab. Mounted once at the app level.
 */
export function useTerminalNotifications({
  tabs,
  activateTab,
  getVisibleTabIds,
}: UseTerminalNotificationsOptions) {
  const { t } = useTranslation()
  const { config } = useConfig()
  const windowFocusedRef = useRef(typeof document === "undefined" || document.hasFocus())
  const lastAnnouncedRef = useRef(new Map<string, number>())
  const tabsRef = useRef(tabs)
  const activateTabRef = useRef(activateTab)
  useEffect(() => {
    tabsRef.current = tabs
    activateTabRef.current = activateTab
  }, [tabs, activateTab])

  useEffect(() => {
    let disposed = false
    const unlisteners: Array<() => void> = []
    const keep = (unlisten: () => void) => {
      if (disposed) unlisten()
      else unlisteners.push(unlisten)
    }

    const appWindow = getCurrentWindow()
    appWindow
      .isFocused()
      .then((focused) => {
        if (!disposed) windowFocusedRef.current = focused
      })
      .catch(() => {})
    void appWindow
      .onFocusChanged(({ payload }) => {
        windowFocusedRef.current = payload
      })
      .then(keep)
      .catch(console.error)

    void listen<{ tabId?: string | null }>("notification-activated", (event) => {
      const tabId = event.payload.tabId
      if (tabId && tabsRef.current.some((tab) => tab.id === tabId)) {
        activateTabRef.current(tabId)
      }
    })
      .then(keep)
      .catch(console.error)

    return () => {
      disposed = true
      for (const unlisten of unlisteners) unlisten()
    }
  }, [])

  return useCallback(
    (tabId: string, event: TerminalAttentionEvent) => {
      const windowFocused = windowFocusedRef.current && document.visibilityState !== "hidden"
      const tabVisible = getVisibleTabIds().includes(tabId)
      const decision = decideAttention(event, config, { windowFocused, tabVisible })
      if (decision.mark) markTabAttention(tabId, decision.mark)
      if (!decision.system && !decision.toast) return

      const now = Date.now()
      const key = `${tabId}:${event.kind === "bell" ? "bell" : "event"}`
      const minInterval = event.kind === "bell" ? BELL_MIN_INTERVAL_MS : MIN_INTERVAL_MS
      if (now - (lastAnnouncedRef.current.get(key) ?? 0) < minInterval) return
      lastAnnouncedRef.current.set(key, now)

      const tabTitle = tabsRef.current.find((tab) => tab.id === tabId)?.title ?? "tTerm"
      const text = describeEvent(event, tabTitle, t)
      if (decision.system) {
        showSystemNotification({ ...text, tabId, sound: config.notify_sound }).catch(
          (error: unknown) => console.error("Failed to show a notification:", error)
        )
        return
      }

      toast({
        title: text.subtitle ? `${text.title} · ${text.subtitle}` : text.title,
        description: text.body,
        variant:
          event.kind === "command" && event.exitCode !== undefined && event.exitCode !== 0
            ? "destructive"
            : "default",
        action: (
          <ToastAction altText={t("notifications.showTab")} onClick={() => activateTab(tabId)}>
            {t("notifications.showTab")}
          </ToastAction>
        ),
      })
    },
    [activateTab, config, getVisibleTabIds, t]
  )
}
