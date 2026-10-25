import { invoke } from "@tauri-apps/api/core"

import type { TerminalSessionContext } from "@/components/TerminalTab/terminalLifecycleTypes"
import {
  CommandMarks,
  COMMAND_MARK_OSC_CODES,
  parseCommandMark,
} from "@/components/TerminalTab/commandMarks"
import { watchAttentionReports } from "@/components/TerminalTab/attentionReports"
import { watchCwdReports } from "@/components/TerminalTab/cwdReports"
import { watchMouseReporting } from "@/components/TerminalTab/mouseReportingGuard"
import { watchOsReports } from "@/components/TerminalTab/osReports"
import { AgentTracker } from "@/lib/agentStatus"
import { setTabRemoteCwd } from "@/lib/remoteCwd"
import { setTabAgentStatus, setTabRunningCommand } from "@/lib/tabAttention"
import { CommandTimer } from "@/lib/terminalNotifications"
import { parseShellIntegrationCommand } from "@/lib/terminalCommandCapture"

/**
 * Follows the commands run in the session: shell integration marks, typed
 * commands, their timing, AI agents' reports, the working directory and,
 * under WSL, the distribution.
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
    onDistroChangeRef,
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

  const recordCommand = (command: string) => {
    const now = Date.now()
    if (lastEmittedCommand?.text === command && now - lastEmittedCommand.at < 1500) return
    // A new command means the last fill's sudo run is over.
    session.lastPasswordFill = null
    lastEmittedCommand = { text: command, at: now }
    const connection = connectionRef.current
    onCommandExecutedRef.current?.({
      commandText: command,
      profileId: connection?.profileId,
      profileName: connection?.profileName,
      executedAt: now,
    })
  }

  // Once the shell reports its command lines (OSC 633 E), lines rebuilt from
  // the keys, which miss history recall and completion, no longer count.
  let shellReportsCommands = false

  /** A line typed at the shell's prompt. */
  const emitTypedCommand = (commandText: string) => {
    const normalized = commandText.trim()
    if (!normalized || shellReportsCommands) return
    commandTimer.setTypedCommand(normalized)
    recordCommand(normalized)
  }

  const commandMarks = new CommandMarks(term)
  commandMarks.setEnabled(commandMarksEnabledRef.current)
  commandMarksRef.current = commandMarks

  const shellIntegrationDisposables = COMMAND_MARK_OSC_CODES.map((osc) =>
    term.parser.registerOscHandler(osc, (data) => {
      commandMarks.handleMark(data)
      timeCommand(data)
      const command = parseShellIntegrationCommand(data)
      if (command) {
        shellReportsCommands = true
        commandTimer.setCommand(command)
        recordCommand(command)
      }
      return false
    })
  )

  const attentionReports = watchAttentionReports(term, {
    agentTracker,
    onAttention: (event) => onAttentionRef.current?.(event),
  })

  // A local shell's directory, for the tab to restart in; a remote one's, for
  // the SFTP drawer to follow.
  const cwdReports = watchCwdReports(term, {
    tabId,
    sessionNonce,
    isRemote: () => connectionRef.current?.type === "ssh",
    onCwd: (cwd) => onCwdChangeRef.current?.(cwd),
    onRemoteCwd: (cwd) => setTabRemoteCwd(tabId, cwd),
  })

  // The distribution a local WSL shell runs, for the tab's logo.
  const osReports = watchOsReports(term, {
    isRemote: () => connectionRef.current?.type === "ssh",
    onDistro: (distroId) => onDistroChangeRef.current?.(distroId),
  })

  // Mouse reporting a killed program left on.
  const mouseReportingGuard = watchMouseReporting(term, {
    isShellInForeground: async () =>
      connectionRef.current?.type !== "ssh" &&
      (await invoke<boolean>("pty_shell_in_foreground", { tabId, sessionNonce })),
  })

  const lineFeedDisposable = term.onLineFeed(() => {
    session.lineFeedCount += 1
  })

  return {
    agentTracker,
    commandTimer,
    emitTypedCommand,
    commandMarks,
    shellIntegrationDisposables,
    attentionReports,
    cwdReports,
    osReports,
    mouseReportingGuard,
    lineFeedDisposable,
  }
}

export type CommandTracking = ReturnType<typeof createCommandTracking>
