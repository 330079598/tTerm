import { useCallback, useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useTranslation } from "react-i18next"

import type { TerminalLogStatus } from "@/lib/terminalLogRecording"
import { toErrorMessage } from "@/lib/utils"

/**
 * Terminal logging's status: loaded when the settings open, then kept
 * current from the backend's events and a periodic refresh.
 */
export function useTerminalLogStatus() {
  const { t } = useTranslation()
  const [status, setStatus] = useState<TerminalLogStatus | null>(null)
  const [statusError, setStatusError] = useState<string | null>(null)

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await invoke<TerminalLogStatus>("get_terminal_log_status"))
      setStatusError(null)
    } catch (error) {
      setStatusError(toErrorMessage(error))
    }
  }, [])

  useEffect(() => {
    let disposed = false
    const cleanups: Array<() => void> = []
    void invoke<TerminalLogStatus>("get_terminal_log_status")
      .then((nextStatus) => {
        if (!disposed) setStatus(nextStatus)
      })
      .catch((error) => {
        if (!disposed) setStatusError(toErrorMessage(error))
      })
    void Promise.all([
      listen<TerminalLogStatus>("terminal-log-status", (event) => {
        if (!disposed) setStatus(event.payload)
      }),
      listen<{ message?: string }>("terminal-log-error", (event) => {
        if (!disposed) {
          setStatusError(event.payload.message ?? t("terminalLogging.writeFailed"))
          void refreshStatus()
        }
      }),
    ]).then((unlisteners) => {
      if (disposed) {
        unlisteners.forEach((unlisten) => unlisten())
      } else {
        cleanups.push(...unlisteners)
      }
    })
    const statusInterval = window.setInterval(() => void refreshStatus(), 5_000)
    return () => {
      disposed = true
      window.clearInterval(statusInterval)
      cleanups.forEach((cleanup) => cleanup())
    }
  }, [refreshStatus, t])

  return { status, setStatus, statusError, setStatusError, refreshStatus }
}
