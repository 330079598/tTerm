import type { TFunction } from "i18next"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useCallback, useEffect } from "react"

import { toast } from "@/hooks/use-toast"
import { setRecordingTabIds, type TerminalLogStatus } from "@/lib/terminalLogRecording"
import { toErrorMessage } from "@/lib/utils"

/**
 * Follows terminal logging (failures, and which tabs are recorded) and
 * starts or stops it for a tab.
 */
export function useTerminalLogRecording(t: TFunction) {
  useEffect(() => {
    let disposed = false
    const cleanups: Array<() => void> = []
    void Promise.all([
      listen<{ message?: string; phase?: string }>("terminal-log-error", (event) => {
        toast({
          title:
            event.payload.phase === "start"
              ? t("terminalLogging.sessionLogFailed")
              : t("terminalLogging.writeFailed"),
          description: event.payload.message ?? t("terminalLogging.writeFailed"),
          variant: "destructive",
        })
      }),
      listen<TerminalLogStatus>("terminal-log-status", (event) => {
        setRecordingTabIds(event.payload.recordingTabIds)
      }),
    ]).then((unlisteners) => {
      if (disposed) unlisteners.forEach((cleanup) => cleanup())
      else cleanups.push(...unlisteners)
    })
    void invoke<TerminalLogStatus>("get_terminal_log_status")
      .then((status) => {
        if (!disposed) setRecordingTabIds(status.recordingTabIds)
      })
      .catch(console.error)
    return () => {
      disposed = true
      cleanups.forEach((cleanup) => cleanup())
    }
  }, [t])

  const handleSetTabLogRecording = useCallback(
    (tabId: string, recording: boolean) => {
      invoke<TerminalLogStatus>("set_terminal_log_recording", { tabId, recording })
        .then((status) => setRecordingTabIds(status.recordingTabIds))
        .catch((error) => {
          toast({
            title: recording ? t("terminalLogging.startFailed") : t("terminalLogging.stopFailed"),
            description: toErrorMessage(error),
            variant: "destructive",
          })
        })
    },
    [t]
  )

  return handleSetTabLogRecording
}
