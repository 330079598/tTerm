import { invoke } from "@tauri-apps/api/core"
import { useCallback } from "react"

import type { TerminalTabRefs } from "@/components/TerminalTab/useTerminalTabRefs"
import { getTabRemoteAwayHost } from "@/lib/remoteCwd"
import { type CdRefusal, terminalCdCommand } from "@/lib/terminalCd"

type UseCdInTerminalOptions = Pick<TerminalTabRefs, "termRef" | "commandMarksRef"> & {
  tabId: string
  sessionNonce: number
  setShowSftpDrawer: (show: boolean) => void
}

/**
 * Takes the tab's shell to a directory for the SFTP drawer's "Open in
 * terminal", then shows the terminal. The line goes to this session only, not
 * to the tabs it broadcasts to. Refuses while the shell runs on another
 * host, and while a program has the terminal (an editor, a pager, a command
 * shell integration saw start, a line the shell has not come back from),
 * which would read the line instead.
 */
export function useCdInTerminal({
  tabId,
  sessionNonce,
  termRef,
  commandMarksRef,
  setShowSftpDrawer,
}: UseCdInTerminalOptions) {
  return useCallback(
    (path: string): CdRefusal | null => {
      const awayHost = getTabRemoteAwayHost(tabId)
      if (awayHost) return { reason: "elsewhere", host: awayHost }
      const term = termRef.current
      const commandMarks = commandMarksRef.current
      if (
        !term ||
        !commandMarks ||
        commandMarks.isRunningProgram() ||
        commandMarks.awaitsPrompt()
      ) {
        return { reason: "busy" }
      }
      void invoke("write_pty", { tabId, sessionNonce, data: terminalCdCommand(path) }).catch(
        console.error
      )
      setShowSftpDrawer(false)
      term.focus()
      return null
    },
    [commandMarksRef, sessionNonce, setShowSftpDrawer, tabId, termRef]
  )
}
