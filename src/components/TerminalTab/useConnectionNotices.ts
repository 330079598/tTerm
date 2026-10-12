import type { TFunction } from "i18next"
import { useCallback, useEffect, useRef, useState } from "react"

import type { TerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import type { AppConfig } from "@/lib/appConfig"
import type { useConfig } from "@/contexts/ConfigContext"
import type { useSessionScopedState } from "@/components/TerminalTab/useSessionScopedState"
import {
  getConnectionDisplay,
  getShellIntegrationFallbackReason,
} from "@/components/TerminalTab/terminalTabUtils"
import type { TerminalTabProps } from "@/components/TerminalTab/types"
import { toast } from "@/hooks/use-toast"
import { toErrorMessage } from "@/lib/utils"

type SaveConfig = ReturnType<typeof useConfig>["saveConfig"]

type UseConnectionNoticesOptions = Pick<TerminalTabProps, "connection"> &
  Pick<TerminalTabRefs, "connectionRef"> & {
    config: AppConfig
    saveConfig: SaveConfig
    sessionResetKey: string
    t: TFunction
  } & Pick<ReturnType<typeof useSessionScopedState>, "connectionProgress">

/**
 * Tells the user when shell integration is unavailable, and when the
 * connection went through jump hosts.
 */
export function useConnectionNotices({
  connection,
  connectionRef,
  config,
  saveConfig,
  sessionResetKey,
  t,
  connectionProgress,
}: UseConnectionNoticesOptions) {
  const lastJumpHostReadyKeyRef = useRef<string | null>(null)
  const [jumpHostInfoOpen, setJumpHostInfoOpen] = useState(false)
  const [dontShowJumpHostInfoAgain, setDontShowJumpHostInfoAgain] = useState(false)
  // Once per tab: reconnects that fall back again for the same reason stay quiet.
  const shellIntegrationNoticeShownRef = useRef(false)
  const shellIntegration = connectionProgress?.shellIntegration
  useEffect(() => {
    if (shellIntegration?.status !== "unavailable" || shellIntegrationNoticeShownRef.current) {
      return
    }
    shellIntegrationNoticeShownRef.current = true
    toast({
      title: t("sessionHeader.shellIntegrationFallback", {
        connection: getConnectionDisplay(connectionRef.current, t),
      }),
      description: getShellIntegrationFallbackReason(shellIntegration.reason, t),
    })
  }, [connectionRef, shellIntegration, t])

  const jumpHostCount = connection?.jumpHosts?.length ?? 0

  useEffect(() => {
    if (
      connectionProgress?.phase !== "ready" ||
      connection?.type !== "ssh" ||
      jumpHostCount === 0
    ) {
      return
    }

    const readyKey = `${sessionResetKey}:${jumpHostCount}`
    if (lastJumpHostReadyKeyRef.current === readyKey) {
      return
    }
    lastJumpHostReadyKeyRef.current = readyKey

    if (config.show_jump_host_connection_info) {
      const openDialogTimer = window.setTimeout(() => {
        setDontShowJumpHostInfoAgain(false)
        setJumpHostInfoOpen(true)
      }, 0)
      return () => window.clearTimeout(openDialogTimer)
    }

    toast({
      title: t("jumpHostInfo.toastTitle", { defaultValue: "Jump host route ready" }),
      description: t("jumpHostInfo.toastDescription", {
        count: jumpHostCount,
        defaultValue: "Connected through {{count}} jump host(s).",
      }),
    })
  }, [
    config.show_jump_host_connection_info,
    connection,
    connectionProgress?.phase,
    jumpHostCount,
    sessionResetKey,
    t,
  ])

  const handleJumpHostInfoOpenChange = useCallback(
    (open: boolean) => {
      setJumpHostInfoOpen(open)
      if (open || !dontShowJumpHostInfoAgain) {
        return
      }

      saveConfig({ show_jump_host_connection_info: false }).catch((error) => {
        console.error("Failed to save jump host info preference:", error)
        toast({
          title: t("settings.saveFailed", { defaultValue: "Failed to save settings" }),
          description: toErrorMessage(error),
          variant: "destructive",
        })
      })
    },
    [dontShowJumpHostInfoAgain, saveConfig, t]
  )

  return {
    jumpHostInfoOpen,
    dontShowJumpHostInfoAgain,
    setDontShowJumpHostInfoAgain,
    handleJumpHostInfoOpenChange,
  }
}
