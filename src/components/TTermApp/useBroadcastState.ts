import { useRef, useState } from "react"

import { useLatestRef } from "@/hooks/useLatestRef"
import type {
  BroadcastMode,
  LiveBroadcastState,
  PtyWriteResult,
  TerminalRuntimeState,
} from "@/types/broadcast"

/**
 * The broadcast's state, with refs that let its callbacks read the latest of
 * it without being recreated.
 */
export function useBroadcastState() {
  const [broadcastMode, setBroadcastMode] = useState<BroadcastMode>("command")
  const [liveBroadcastState, setLiveBroadcastState] = useState<LiveBroadcastState>("idle")
  const [broadcastSource, setBroadcastSource] = useState<{
    tabId: string
    sessionNonce: number
  } | null>(null)
  const [broadcastTargetIds, setBroadcastTargetIds] = useState<string[]>([])
  const [terminalRuntimeStates, setTerminalRuntimeStates] = useState<
    Record<string, TerminalRuntimeState>
  >({})
  const [latestBroadcastResults, setLatestBroadcastResults] = useState<
    Record<string, PtyWriteResult>
  >({})
  const [unavailableBroadcastTargetIds, setUnavailableBroadcastTargetIds] = useState<string[]>([])
  const [isBroadcastPreparing, setIsBroadcastPreparing] = useState(false)
  const broadcastWriteQueueRef = useRef(Promise.resolve())
  const broadcastGenerationRef = useRef(0)
  const savedPasswordPromptTargetsRef = useRef<
    Map<string, { sessionNonce: number; prompt: string }>
  >(new Map())
  const liveSourceRef = useRef<{ tabId: string; sessionNonce: number } | null>(null)
  const liveStateRef = useRef<LiveBroadcastState>(liveBroadcastState)
  const runtimeStatesRef = useRef(terminalRuntimeStates)
  const targetIdsRef = useRef(broadcastTargetIds)
  const unavailableTargetIdsRef = useRef(unavailableBroadcastTargetIds)
  const broadcastPreparingRef = useRef(false)
  const broadcastPreflightRef = useRef<{
    reject: (error: Error) => void
    resolve: (targets: Array<{ tabId: string; sessionNonce: number }>) => void
    targets: Map<string, number>
  } | null>(null)
  useLatestRef(runtimeStatesRef, terminalRuntimeStates)
  useLatestRef(targetIdsRef, broadcastTargetIds)
  useLatestRef(unavailableTargetIdsRef, unavailableBroadcastTargetIds)
  useLatestRef(liveStateRef, liveBroadcastState)

  return {
    broadcastMode,
    setBroadcastMode,
    liveBroadcastState,
    setLiveBroadcastState,
    broadcastSource,
    setBroadcastSource,
    broadcastTargetIds,
    setBroadcastTargetIds,
    terminalRuntimeStates,
    setTerminalRuntimeStates,
    latestBroadcastResults,
    setLatestBroadcastResults,
    unavailableBroadcastTargetIds,
    setUnavailableBroadcastTargetIds,
    isBroadcastPreparing,
    setIsBroadcastPreparing,
    broadcastWriteQueueRef,
    broadcastGenerationRef,
    savedPasswordPromptTargetsRef,
    liveSourceRef,
    liveStateRef,
    runtimeStatesRef,
    targetIdsRef,
    unavailableTargetIdsRef,
    broadcastPreparingRef,
    broadcastPreflightRef,
  }
}

export type BroadcastState = ReturnType<typeof useBroadcastState>
