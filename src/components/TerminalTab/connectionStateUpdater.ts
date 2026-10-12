import type { ConnectionState } from "@/components/TerminalTab/types"

/**
 * Sets the tab's connection state, remembering the last one so that the
 * output-driven updates can skip a state that has not changed.
 */
export function createConnectionStateUpdater(setConnectionState: (next: ConnectionState) => void) {
  let lastConnectionState: ConnectionState = "connecting"

  // Every state change in this effect must go through these two helpers so
  // lastConnectionState stays in sync; otherwise the output-driven
  // "connected" update is skipped as a no-op and the tab stays stuck.
  const applyConnectionState = (next: ConnectionState) => {
    lastConnectionState = next
    setConnectionState(next)
  }

  const setConnectionStateIfChanged = (next: ConnectionState) => {
    if (next === lastConnectionState) return
    applyConnectionState(next)
  }

  return { applyConnectionState, setConnectionStateIfChanged }
}

export type ConnectionStateUpdater = ReturnType<typeof createConnectionStateUpdater>
