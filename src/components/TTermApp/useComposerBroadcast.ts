import type { ComposerBroadcast } from "@/components/CommandComposer/useComposerSubmit"
import type { BroadcastRuntime } from "@/components/TTermApp/useBroadcastRuntime"
import type { BroadcastState } from "@/components/TTermApp/useBroadcastState"
import type { useBroadcastSending } from "@/components/TTermApp/useBroadcastSending"
import type { ExecutedCommand } from "@/types/command"

/**
 * The broadcast panel's targets as the compose bar sends to them. While a
 * live broadcast runs, what the bar sends to its source goes to them anyway.
 */
export function useComposerBroadcast(
  state: BroadcastState,
  runtime: BroadcastRuntime,
  sending: ReturnType<typeof useBroadcastSending>,
  recordCommand: (command: ExecutedCommand) => void
): ComposerBroadcast {
  return {
    targetCount: state.liveBroadcastState === "idle" ? state.broadcastTargetIds.length : 0,
    send: sending.handleSendBroadcastCommand,
    connectedTargetIds: () => runtime.resolveBroadcastTargets().map((target) => target.tabId),
    record: (commandText) => recordCommand({ commandText, executedAt: Date.now() }),
  }
}
