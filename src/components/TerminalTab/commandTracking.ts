import type { TerminalSessionContext } from "@/components/TerminalTab/terminalLifecycleTypes"
import {
  CommandMarks,
  COMMAND_MARK_OSC_CODES,
  parseCommandMark,
} from "@/components/TerminalTab/commandMarks"
import { watchAttentionReports } from "@/components/TerminalTab/attentionReports"
import { watchCwdReports } from "@/components/TerminalTab/cwdReports"
import { AgentTracker } from "@/lib/agentStatus"
import { setTabAgentStatus, setTabRunningCommand } from "@/lib/tabAttention"
import { CommandTimer } from "@/lib/terminalNotifications"
import { parseShellIntegrationCommand } from "@/lib/terminalCommandCapture"

/**
 * Follows the commands run in the session: shell integration marks, typed
 * commands, their timing, AI agents' reports and the working directory.
 */
export function createCommandTracking(ctx: TerminalSessionContext) {
  const {
    term,
    session,
    tabId,
    sessionNonce,
    onAttentionRef,
    connectionRef,
    onCommandExecutedRef,
    commandMarksEnabledRef,
    commandMarksRef,
    onCwdChangeRef,
  } = ctx
  let lastEmittedCommand: { text: string; at: number } | null = null

  // An AI agent reporting through tTerm's hooks; its marks on the tab go
  // with this session.
  const agentTracker = new AgentTracker({
    onStatus: (status) => setTabAgentStatus(tabId, status),
    onAnnounce: (announcement) => onAttentionRef.current?.({ kind: "agent", ...announcement }),
  })

  const commandTimer = new CommandTimer((command) => setTabRunningCommand(tabId, command))
  const timeCommand = (data: string) => {
    const mark = parseCommandMark(data)
    if (mark?.kind === "prompt") agentTracker.reset()
    const finished = mark && commandTimer.mark(mark, Date.now())
    if (finished) onAttentionRef.current?.({ kind: "command", ...finished })
  }

  const emitExecutedCommand = (commandText: string) => {
    const normalized = commandText.trim()
    if (!normalized) return
    commandTimer.setTypedCommand(normalized)
    const now = Date.now()
    if (lastEmittedCommand?.text === normalized && now - lastEmittedCommand.at < 1500) return
    // A new command means the last fill's sudo run is over.
    session.lastPasswordFill = null
    lastEmittedCommand = { text: normalized, at: now }
    const connection = connectionRef.current
    onCommandExecutedRef.current?.({
      commandText: normalized,
      profileId: connection?.profileId,
      profileName: connection?.profileName,
      executedAt: now,
    })
  }

  const commandMarks = new CommandMarks(term)
  commandMarks.setEnabled(commandMarksEnabledRef.current)
  commandMarksRef.current = commandMarks

  const shellIntegrationDisposables = COMMAND_MARK_OSC_CODES.map((osc) =>
    term.parser.registerOscHandler(osc, (data) => {
      commandMarks.handleMark(data)
      timeCommand(data)
      const command = parseShellIntegrationCommand(data)
      if (command) emitExecutedCommand(command)
      return false
    })
  )

  const attentionReports = watchAttentionReports(term, {
    agentTracker,
    onAttention: (event) => onAttentionRef.current?.(event),
  })

  // A local shell's directory, for the tab to restart in.
  const cwdReports = watchCwdReports(term, {
    tabId,
    sessionNonce,
    isRemote: () => connectionRef.current?.type === "ssh",
    onCwd: (cwd) => onCwdChangeRef.current?.(cwd),
  })

  const lineFeedDisposable = term.onLineFeed(() => {
    session.lineFeedCount += 1
  })

  return {
    agentTracker,
    commandTimer,
    emitExecutedCommand,
    commandMarks,
    shellIntegrationDisposables,
    attentionReports,
    cwdReports,
    lineFeedDisposable,
  }
}

export type CommandTracking = ReturnType<typeof createCommandTracking>
