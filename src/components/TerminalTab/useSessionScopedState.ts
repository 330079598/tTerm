import { useCallback, useState } from "react"

import type {
  ConnectionState,
  HostKeyPromptState,
  SshConnectionProgress,
} from "@/components/TerminalTab/types"

type UseSessionScopedStateOptions = { sessionResetKey: string }

/**
 * State that belongs to one session: a host key prompt, the connection
 * state and its progress, and why a local shell failed to start. A new
 * session starts over without resetting it.
 */
export function useSessionScopedState({ sessionResetKey }: UseSessionScopedStateOptions) {
  const defaultConnectionState: ConnectionState = "connecting"
  const [hostKeyPromptState, setHostKeyPromptState] = useState<{
    sessionKey: string
    value: HostKeyPromptState
  } | null>(null)
  const [connectionStateState, setConnectionStateState] = useState<{
    sessionKey: string
    value: ConnectionState
  } | null>(null)
  const [connectionProgressState, setConnectionProgressState] = useState<{
    sessionKey: string
    value: SshConnectionProgress
  } | null>(null)
  const [startErrorState, setStartErrorState] = useState<{
    sessionKey: string
    value: string
  } | null>(null)

  const hostKeyPrompt =
    hostKeyPromptState?.sessionKey === sessionResetKey ? hostKeyPromptState.value : null
  const connectionState =
    connectionStateState?.sessionKey === sessionResetKey
      ? connectionStateState.value
      : defaultConnectionState
  const connectionProgress =
    connectionProgressState?.sessionKey === sessionResetKey ? connectionProgressState.value : null
  const startError = startErrorState?.sessionKey === sessionResetKey ? startErrorState.value : null

  const setHostKeyPrompt = useCallback(
    (value: HostKeyPromptState | null) => {
      setHostKeyPromptState(value ? { sessionKey: sessionResetKey, value } : null)
    },
    [sessionResetKey]
  )

  const setConnectionState = useCallback(
    (value: ConnectionState) => {
      setConnectionStateState({ sessionKey: sessionResetKey, value })
    },
    [sessionResetKey]
  )

  const setConnectionProgress = useCallback(
    (value: SshConnectionProgress | null) => {
      setConnectionProgressState(value ? { sessionKey: sessionResetKey, value } : null)
    },
    [sessionResetKey]
  )

  const setStartError = useCallback(
    (value: string | null) => {
      setStartErrorState(value ? { sessionKey: sessionResetKey, value } : null)
    },
    [sessionResetKey]
  )

  return {
    hostKeyPrompt,
    connectionState,
    connectionProgress,
    startError,
    setHostKeyPrompt,
    setConnectionState,
    setConnectionProgress,
    setStartError,
  }
}
