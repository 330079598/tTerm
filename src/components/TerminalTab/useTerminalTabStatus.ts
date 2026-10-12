import { useCallback, useEffect, useState } from "react"

import type { ConnectionState, TerminalTabProps } from "@/components/TerminalTab/types"

interface ConnectionStateReportOptions {
  tabId: string
  sessionNonce: number
  connectionState: ConnectionState
  onConnectionStateChange: TerminalTabProps["onConnectionStateChange"]
}

/** Reports the session's connection state, and that it has none once the tab goes. */
export function useConnectionStateReport({
  tabId,
  sessionNonce,
  connectionState,
  onConnectionStateChange,
}: ConnectionStateReportOptions) {
  useEffect(() => {
    onConnectionStateChange?.(tabId, sessionNonce, connectionState)
  }, [connectionState, onConnectionStateChange, sessionNonce, tabId])

  useEffect(
    () => () => onConnectionStateChange?.(tabId, sessionNonce, null),
    [onConnectionStateChange, sessionNonce, tabId]
  )
}

/** Whether the server monitor bar shows; the host remembers it through the callback. */
export function useServerMonitorToggle(
  connection: TerminalTabProps["connection"],
  onServerMonitorVisibilityChange: TerminalTabProps["onServerMonitorVisibilityChange"]
) {
  const [showServerMonitor, setShowServerMonitor] = useState(
    () => connection?.serverMonitorVisible === true
  )

  const handleToggleServerMonitor = useCallback(() => {
    setShowServerMonitor((current) => {
      const nextVisible = !current
      onServerMonitorVisibilityChange?.(nextVisible)
      return nextVisible
    })
  }, [onServerMonitorVisibilityChange])

  return { showServerMonitor, handleToggleServerMonitor }
}
