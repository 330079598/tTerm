import { listen } from "@tauri-apps/api/event"
import { useCallback, useEffect, useRef } from "react"

import type { AppShell } from "@/components/TTermApp/appShell"
import { useLatestRef } from "@/hooks/useLatestRef"

interface TitleBarPagesOptions {
  settingsTabTitle: string
  tunnelsTabTitle: string
  runningTunnelCount: number
}

/**
 * The title bar's page buttons: settings and port forwarding, which the
 * tray menu can also open.
 */
export function useTitleBarPages(shell: AppShell, options: TitleBarPagesOptions) {
  const { t, openSettingsTab, openTunnelsTab } = shell
  const { settingsTabTitle, tunnelsTabTitle, runningTunnelCount } = options

  const handleSettingsClick = useCallback(() => {
    openSettingsTab(settingsTabTitle)
  }, [openSettingsTab, settingsTabTitle])

  const tunnelsButtonLabel =
    runningTunnelCount > 0
      ? t("tunnels.runningCountLabel", {
          title: tunnelsTabTitle,
          count: runningTunnelCount,
          defaultValue: "{{title}} ({{count}} running)",
        })
      : tunnelsTabTitle

  const handleTunnelsClick = useCallback(() => {
    openTunnelsTab(tunnelsTabTitle)
  }, [openTunnelsTab, tunnelsTabTitle])

  // The tray menu opens the page itself, and after a tunnel it could not
  // start so the user sees why.
  const handleTunnelsClickRef = useRef(handleTunnelsClick)
  useLatestRef(handleTunnelsClickRef, handleTunnelsClick)
  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void listen("tray-open-tunnels", () => handleTunnelsClickRef.current()).then((cleanup) => {
      if (disposed) cleanup()
      else unlisten = cleanup
    })
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  return {
    handleSettingsClick,
    tunnelsButtonLabel,
    handleTunnelsClick,
  }
}
