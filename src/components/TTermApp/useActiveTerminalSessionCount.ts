import { useEffect } from "react"

import { useAppActivity } from "@/contexts/AppActivityContext"
import type { TerminalRuntimeState } from "@/types/broadcast"
import { isSessionTab, type Tab } from "@/types/tab"

/**
 * Tells the app how many terminal tabs hold a connection that is up or on
 * its way up, and that none are left once the app goes away.
 */
export function useActiveTerminalSessionCount(
  tabs: Tab[],
  runtimeStates: Record<string, TerminalRuntimeState>
) {
  const { setActiveTerminalSessionCount } = useAppActivity()

  const activeTerminalSessionCount = tabs.reduce((count, tab) => {
    if (!isSessionTab(tab)) {
      return count
    }

    const connectionState = runtimeStates[tab.id]?.connectionState
    return connectionState === "connected" ||
      connectionState === "connecting" ||
      connectionState === "reconnecting"
      ? count + 1
      : count
  }, 0)

  useEffect(() => {
    setActiveTerminalSessionCount(activeTerminalSessionCount)
  }, [activeTerminalSessionCount, setActiveTerminalSessionCount])

  useEffect(
    () => () => {
      setActiveTerminalSessionCount(0)
    },
    [setActiveTerminalSessionCount]
  )
}
